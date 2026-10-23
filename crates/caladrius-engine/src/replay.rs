//! `project.replay`: the history of a saved project, run again and checked against its seals.
//!
//! A project file holds the project as it was saved and the history of the commands that made it,
//! each with the parameters it received and the seal of its answer ([`crate::seal`]). Replaying
//! starts from an empty project and runs the commands of the history again, in order, with the
//! parameters the history kept. For every step it says whether the seal it computes now is the
//! one stored, and at the end whether the project the steps produce is the project stored in the
//! file. A flipped bit anywhere shows: in a parameter of the history (the step does not match),
//! in a result stored in the project (the project does not match), or in the engine (the first
//! step it computes differently).
//!
//! What a replay cannot do is told, never hidden:
//!
//! * A command whose parameters were too large to be kept (the history keeps up to
//!   [`crate::MAX_RECORDED_PARAMS_BYTES`]) cannot be run again. If it changed the project, the
//!   steps after it cannot be run either, and the verdict is `incomplete`.
//! * A project saved before seals existed replays too: every step says `no_seal`, and the
//!   comparison with the stored project is the only check.
//! * The replay reads no clock and no file; the engine that replays is this one, and the report
//!   says which engine sealed the steps and which one replays them.

use std::collections::{BTreeMap, BTreeSet};

use caladrius_project::Project;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Engine;
use crate::commands::read_saved;
use crate::history::{HistoryEntry, omitted_bytes};
use crate::seal::{self, ENGINE_VERSION, FORMAT, GIT_COMMIT, canonical_json, sha256_hex};

/// How the replay ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Every step that carries a seal reproduces it, nothing was left out, and the project the
    /// steps produce is the stored one.
    Verified,
    /// A step does not reproduce its seal, or the stored project is not the one the steps produce.
    Diverged,
    /// Nothing differs, but some steps could not be run again: the check does not cover them.
    Incomplete,
    /// Nothing differs, but no step carries a seal (a project saved before seals existed).
    Unsealed,
    /// The project has no history: there is nothing to replay.
    Empty,
}

/// What happened to one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The seal computed now is the stored one.
    Match,
    /// The seal computed now is not the stored one.
    Diverged,
    /// No seal was recorded for the step, so there is nothing to compare: a project saved before
    /// seals, or a command that never carries one (`project.load`).
    NoSeal,
    /// The step was not run again: its parameters were too large to be kept in the history.
    NotReplayable,
    /// The step was not run: it comes after a step that could not be run again and changed the
    /// project, so it would run on another project.
    Blocked,
}

/// One step of the history, after the replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// Position in the history, from 1.
    pub seq: u64,
    /// The command.
    pub command: String,
    /// What happened.
    pub status: Status,
    /// The stored seal: 64 hexadecimal digits. Absent when none was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stored: Option<String>,
    /// The seal computed now. Absent when the step was not run, or carries no seal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recomputed: Option<String>,
    /// Why, in words, when the status is not obvious.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// How many steps ended in each status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    /// Steps whose seal was reproduced.
    pub matched: u64,
    /// Steps whose seal was not reproduced.
    pub diverged: u64,
    /// Steps with no seal to compare.
    pub no_seal: u64,
    /// Steps that could not be run again.
    pub not_replayable: u64,
    /// Steps that were not run, for want of a step before them.
    pub blocked: u64,
}

/// The comparison of the stored project with the one the steps produce.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateCheck {
    /// True when they are the same, false when they differ, absent when they were not compared
    /// (`note` says why).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matches: Option<bool>,
    /// SHA-256 of the canonical text of the stored project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stored: Option<String>,
    /// SHA-256 of the canonical text of the project the steps produce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replayed: Option<String>,
    /// What differs, when they differ: `worksheet 1`, `analysis 2`, `project name`...
    pub differing: Vec<String>,
    /// Why they were not compared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// An engine: its version and the commit it was built from, when known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineId {
    /// The version of the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The git commit it was built from, when it was known at build time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// The report of a replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// How it ended.
    pub verdict: Verdict,
    /// The verdict in a sentence for a person.
    pub summary: String,
    /// How many steps ended in each status.
    pub counts: Counts,
    /// The first step whose seal was not reproduced, if any.
    pub first_divergence: Option<Step>,
    /// The stored project against the replayed one.
    pub state: StateCheck,
    /// The engine that replayed. Absent when the engine that runs the command leaves its
    /// version out of its seals (the digest workload does).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineId>,
    /// The engines that sealed the steps, in order of appearance.
    pub sealed_by: Vec<EngineId>,
    /// Every step, in order.
    pub steps: Vec<Step>,
}

/// What a replay carries from one step to the next.
struct Run {
    /// The engine the steps run on: an empty project, and the stored history as far as it goes.
    scratch: Engine,
    /// The first step that could not be run again and changed the project: from there on the
    /// replayed project is no longer the one the next steps ran on.
    lost: Option<(u64, String)>,
    /// The first step that opened a project whose content the history does not hold.
    assumed: Option<u64>,
}

/// Runs the history of a saved project again. `project` is the project as it was stored.
/// `provenance` is whether the engine that asks says which engine it is in its report.
pub(crate) fn replay(project: &Project, entries: &[HistoryEntry], provenance: bool) -> Report {
    let mut run = Run {
        scratch: Engine::new(),
        lost: None,
        assumed: None,
    };
    run.scratch.without_provenance = !provenance;
    let mut steps = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let (step, executed) = match &run.lost {
            Some((seq, command)) => (blocked(entry, *seq, command), false),
            None => run_step(&mut run, entry, &entries[..index]),
        };
        run.scratch.history.follow(entries, index, executed);
        steps.push(step);
    }

    let counts = count(&steps);
    let first_divergence = steps
        .iter()
        .find(|step| step.status == Status::Diverged)
        .cloned();
    let state = match (&run.lost, entries.is_empty()) {
        (_, true) => StateCheck::not_compared("there is no history to replay"),
        (Some((seq, command)), _) => StateCheck::not_compared(format!(
            "step {seq} ({command}) could not be run again, so the project the steps produce is not known"
        )),
        (None, false) => compare_states(project, &run.scratch.project),
    };
    let verdict = if entries.is_empty() {
        Verdict::Empty
    } else if counts.diverged > 0 || state.matches == Some(false) {
        Verdict::Diverged
    } else if counts.not_replayable > 0 || counts.blocked > 0 {
        Verdict::Incomplete
    } else if counts.matched == 0 {
        Verdict::Unsealed
    } else {
        Verdict::Verified
    };
    Report {
        summary: summary(
            verdict,
            &counts,
            first_divergence.as_ref(),
            &state,
            steps.len(),
            run.assumed,
        ),
        verdict,
        counts,
        first_divergence,
        state,
        engine: provenance.then(|| EngineId {
            version: Some(ENGINE_VERSION.to_owned()),
            commit: GIT_COMMIT.map(str::to_owned),
        }),
        sealed_by: sealed_by(entries),
        steps,
    }
}

fn step_of(entry: &HistoryEntry, status: Status) -> Step {
    Step {
        seq: entry.seq,
        command: entry.command.clone(),
        status,
        stored: entry.seal.as_ref().map(|seal| seal.sha256.clone()),
        recomputed: None,
        note: None,
    }
}

fn blocked(entry: &HistoryEntry, seq: u64, command: &str) -> Step {
    let mut step = step_of(entry, Status::Blocked);
    step.note = Some(format!(
        "not run: it comes after step {seq} ({command}), which could not be run again"
    ));
    step
}

/// The project that a `project.load` brought in, when the document it was given carries as its
/// history exactly the steps that come before the load: the project was opened together with its
/// own history, which is what the replay has just run.
fn carried_project(params: &Value, before: &[HistoryEntry]) -> Option<Project> {
    let document = match (
        params.get("project"),
        params.get("text").and_then(Value::as_str),
    ) {
        (Some(document), None) => document.clone(),
        (None, Some(text)) => serde_json::from_str::<Value>(text).ok()?,
        _ => return None,
    };
    let saved = read_saved(document).ok()?;
    match saved.history {
        Ok(carried) if !carried.is_empty() && carried == before => Some(saved.project),
        _ => None,
    }
}

/// A `project.load` that changed the project, when it is not simply run again: the step and
/// whether a command was run, or `None` to run it like any other step.
fn open_project(
    run: &mut Run,
    entry: &HistoryEntry,
    before: &[HistoryEntry],
) -> Option<(Step, bool)> {
    let mut step = step_of(entry, Status::NoSeal);
    match omitted_bytes(&entry.params) {
        // The file is not in the history. After steps that were replayed it is taken as the
        // project those steps made, and the comparison at the end checks the steps after it.
        Some(_) if run.scratch.revision() > 0 => {
            run.assumed.get_or_insert(entry.seq);
            step.note = Some(
                "a project was opened here and its content was not kept in the history; the steps after it are checked as if it were the project the steps before it produce"
                    .to_owned(),
            );
        }
        Some(_) => return None,
        // The file is in the history, with the history it carried: the project it holds is
        // compared with the one the steps before it produced, and the steps go on from the file.
        None => {
            let carried = carried_project(&entry.params, before)?;
            let check = compare_states(&carried, &run.scratch.project);
            if check.matches == Some(false) {
                step.status = Status::Diverged;
                step.stored = check.stored;
                step.recomputed = check.replayed;
                step.note = Some(format!(
                    "the project opened here is not the one its own history produces: {}",
                    check.differing.join(", ")
                ));
            } else {
                step.note = Some(
                    "a project was opened here with its own history; it is the project the steps before it produce"
                        .to_owned(),
                );
            }
            run.scratch.project = carried;
        }
    }
    Some((step, false))
}

/// Runs one step on the scratch engine. `before` are the steps that come before it. Returns the
/// step and whether a command was run.
fn run_step(run: &mut Run, entry: &HistoryEntry, before: &[HistoryEntry]) -> (Step, bool) {
    // A replay is a check, not a change: it is not run again, and it cannot call itself.
    if entry.command == "project.replay" {
        let mut step = step_of(entry, Status::NoSeal);
        step.note = Some("a replay: it changes nothing and is not run again".to_owned());
        return (step, false);
    }
    if entry.command == "project.load" && entry.changed_project {
        if let Some(done) = open_project(run, entry, before) {
            return done;
        }
    }
    if let Some(bytes) = omitted_bytes(&entry.params) {
        let mut step = step_of(entry, Status::NotReplayable);
        step.note = Some(format!(
            "its parameters were too large to be kept in the history ({bytes} bytes), so it was not run again"
        ));
        if entry.changed_project {
            run.lost = Some((entry.seq, entry.command.clone()));
        }
        return (step, false);
    }

    let outcome = run.scratch.execute(&entry.command, entry.params.clone());
    let recomputed = run
        .scratch
        .history
        .entries()
        .last()
        .and_then(|recorded| recorded.seal.clone());
    let mut step = step_of(entry, Status::NoSeal);
    step.recomputed = recomputed.as_ref().map(|seal| seal.sha256.clone());
    match (&entry.seal, &recomputed) {
        (Some(stored), Some(_)) if stored.format != FORMAT => {
            step.note = Some(format!(
                "sealed in format {}, which this engine (format {FORMAT}) does not recompute",
                stored.format
            ));
        }
        (Some(stored), Some(now)) => {
            if stored.sha256 == now.sha256 {
                step.status = Status::Match;
            } else {
                step.status = Status::Diverged;
                step.note = Some(difference(entry, &outcome));
            }
        }
        _ if !seal::is_sealed(&entry.command) => {
            step.note = Some("this command carries no seal".to_owned());
        }
        _ => {}
    }
    (step, true)
}

/// What changed, in words, for a step whose seal is not reproduced.
fn difference(entry: &HistoryEntry, outcome: &Result<Value, crate::CommandError>) -> String {
    match (entry.ok, outcome) {
        (true, Err(error)) => format!(
            "it ran when it was sealed; now it fails with `{}`: {}",
            error.code, error.message
        ),
        (false, Ok(_)) => "it failed when it was sealed; now it runs".to_owned(),
        _ => "the parameters or the answer differ from the sealed ones".to_owned(),
    }
}

fn count(steps: &[Step]) -> Counts {
    let mut counts = Counts::default();
    for step in steps {
        match step.status {
            Status::Match => counts.matched += 1,
            Status::Diverged => counts.diverged += 1,
            Status::NoSeal => counts.no_seal += 1,
            Status::NotReplayable => counts.not_replayable += 1,
            Status::Blocked => counts.blocked += 1,
        }
    }
    counts
}

/// The engines that sealed the steps, each once, in order of appearance.
fn sealed_by(entries: &[HistoryEntry]) -> Vec<EngineId> {
    let mut seen: Vec<EngineId> = Vec::new();
    for seal in entries.iter().filter_map(|entry| entry.seal.as_ref()) {
        if seal.engine.is_none() && seal.commit.is_none() {
            continue;
        }
        let id = EngineId {
            version: seal.engine.clone(),
            commit: seal.commit.clone(),
        };
        if !seen.contains(&id) {
            seen.push(id);
        }
    }
    seen
}

// ---- the stored project against the replayed one -------------------------------------------

impl StateCheck {
    fn not_compared(note: impl Into<String>) -> Self {
        Self {
            matches: None,
            stored: None,
            replayed: None,
            differing: Vec::new(),
            note: Some(note.into()),
        }
    }
}

fn compare_states(stored: &Project, replayed: &Project) -> StateCheck {
    let (Ok(a), Ok(b)) = (serde_json::to_value(stored), serde_json::to_value(replayed)) else {
        return StateCheck::not_compared("a project could not be written as text to compare them");
    };
    let hash = |value: &Value| sha256_hex(canonical_json(value).0.as_bytes());
    let (stored_hash, replayed_hash) = (hash(&a), hash(&b));
    let same = stored_hash == replayed_hash;
    StateCheck {
        matches: Some(same),
        differing: if same {
            Vec::new()
        } else {
            differences(&a, &b)
        },
        stored: Some(stored_hash),
        replayed: Some(replayed_hash),
        note: None,
    }
}

/// Which parts of two projects differ: each worksheet and each analysis by identifier, then the
/// other members of the project.
fn differences(stored: &Value, replayed: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for (key, label) in [("worksheets", "worksheet"), ("analyses", "analysis")] {
        let by_id = |project: &'_ Value| -> BTreeMap<u64, Value> {
            project
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| Some((item.get("id")?.as_u64()?, item.clone())))
                .collect()
        };
        let (a, b) = (by_id(stored), by_id(replayed));
        let ids: BTreeSet<u64> = a.keys().chain(b.keys()).copied().collect();
        for id in ids {
            if a.get(&id) != b.get(&id) {
                let what = match (a.contains_key(&id), b.contains_key(&id)) {
                    (true, false) => " (the steps do not make it)",
                    (false, true) => " (the file does not hold it)",
                    _ => "",
                };
                out.push(format!("{label} {id}{what}"));
            }
        }
    }
    for (key, label) in [
        ("name", "project name"),
        ("next_id", "identifier counter"),
        ("format_version", "format version"),
    ] {
        if stored.get(key) != replayed.get(key) {
            out.push(label.to_owned());
        }
    }
    if out.is_empty() {
        out.push("the project".to_owned());
    }
    out
}

// ---- the sentence ---------------------------------------------------------------------------

fn summary(
    verdict: Verdict,
    counts: &Counts,
    first: Option<&Step>,
    state: &StateCheck,
    n: usize,
    assumed: Option<u64>,
) -> String {
    let plural = |k: u64, one: &str, many: &str| {
        if k == 1 {
            one.to_owned()
        } else {
            many.to_owned()
        }
    };
    // A project opened from a file that the history does not hold: say that the steps after it
    // were checked on trust, as far as the comparison at the end goes.
    let assumption = match assumed {
        Some(seq) => format!(
            " The project opened at step {seq} is not in the history: the steps after it were checked as if it were the project the steps before it produce."
        ),
        None => String::new(),
    };
    match verdict {
        Verdict::Empty => "This project has no history, so there is nothing to replay.".to_owned(),
        Verdict::Verified => {
            let unsealed = if counts.no_seal > 0 {
                format!(
                    " {} {} no seal to compare.",
                    counts.no_seal,
                    plural(counts.no_seal, "step has", "steps have")
                )
            } else {
                String::new()
            };
            format!(
                "All {n} steps were replayed: {} {} their seal bit for bit, and the replayed project is the stored one.{unsealed}{assumption}",
                counts.matched,
                plural(counts.matched, "reproduces", "reproduce"),
            )
        }
        Verdict::Diverged => {
            let mut text = match first {
                Some(step) => format!(
                    "Step {} ({}) does not reproduce its seal.",
                    step.seq, step.command
                ),
                None if counts.matched == 0 => {
                    "The steps carry no seal to compare, and the replay does not reproduce the stored project."
                        .to_owned()
                }
                None => "Every sealed step reproduces its seal, but the stored project is not the one these steps produce."
                    .to_owned(),
            };
            if counts.diverged > 1 {
                text.push_str(&format!(" {} steps differ in all.", counts.diverged));
            }
            if state.matches == Some(false) && first.is_some() {
                text.push_str(" The stored project is not the replayed one either.");
            }
            if state.matches == Some(false) {
                text.push_str(&format!(" Differs: {}.", state.differing.join(", ")));
            }
            text.push_str(&assumption);
            text
        }
        Verdict::Incomplete => format!(
            "{} of {n} steps could not be run again, so the check does not cover them; the other steps do not differ ({} reproduce their seal).{assumption}",
            counts.not_replayable + counts.blocked,
            counts.matched,
        ),
        Verdict::Unsealed => format!(
            "All {n} steps were replayed and the replayed project is the stored one, but none of them carries a seal (the project was saved before seals existed), so there is nothing to compare bit for bit.{assumption}"
        ),
    }
}
