//! Verify a project: a saved project file, run again from an empty project and checked, step by
//! step, against the seals it carries (`project.replay`). The page says in words how it ended,
//! then lists every step as matching or differing, with the first twelve digits of the seal that
//! was stored and of the one computed now. The replay runs on an engine of its own, on the bytes
//! the program around hands over: the project in the window, its history and its saved marker
//! are not touched, and nothing here computes anything.

use caladrius_engine::Engine;
use caladrius_engine::replay::{Counts, EngineId, StateCheck, Step};
use caladrius_engine::seal::short;
use egui::{RichText, Ui};

use crate::app::{Action, Notice, NoticeKind, Selection, UiApp};
use crate::fmt;
use crate::theme::{Rgba, Tokens};
use crate::widgets::section;

/// What the replay reports, as the engine words it: the page shows it and the program around can
/// read it ([`UiApp::verification`]).
pub use caladrius_engine::replay::{Report, Status, Verdict};

/// How many digits of a commit the page shows: the usual short form.
const COMMIT_DIGITS: usize = 7;

/// A project file after its replay.
#[derive(Debug, Clone)]
pub(crate) struct Verification {
    /// The name of the file.
    pub name: String,
    /// What the replay found, as the engine reported it.
    pub report: Report,
    /// What was on screen before the page: where "Close" goes back to.
    pub back: Selection,
}

impl UiApp {
    /// The program read the project file `name` for verification: its history is run again and
    /// checked against its seals, and the page with the report opens. True when the file could be
    /// replayed, whatever the replay found; false when it is not a project, and a sentence says
    /// why. The project in the window is not read or changed.
    pub fn verify_project(&mut self, name: &str, bytes: &[u8]) -> bool {
        let mut scratch = Engine::new();
        let report = match scratch.replay_bytes(bytes) {
            Ok(answer) => serde_json::from_value::<Report>(answer)
                .map_err(|e| format!("the report cannot be read: {e}")),
            Err(e) => Err(fmt::plain(&e.message)),
        };
        match report {
            Ok(report) => {
                let back = match (&self.verification, &self.state.selection) {
                    (Some(earlier), Selection::Verification) => earlier.back.clone(),
                    (_, shown) => shown.clone(),
                };
                let kind = match report.verdict {
                    Verdict::Diverged => NoticeKind::Error,
                    _ => NoticeKind::Info,
                };
                self.notice = Some(Notice {
                    kind,
                    text: format!("{name}: {}.", sentence(report.verdict)),
                });
                self.verification = Some(Verification {
                    name: name.to_owned(),
                    report,
                    back,
                });
                self.state.selection = Selection::Verification;
                true
            }
            Err(message) => {
                self.failure(format!("{name} cannot be verified: {message}"));
                false
            }
        }
    }

    /// The file verified last and what its replay found, for the program around and for tests.
    pub fn verification(&self) -> Option<(&str, &Report)> {
        let done = self.verification.as_ref()?;
        Some((done.name.as_str(), &done.report))
    }

    /// Leaves the verification page for what was on screen before it.
    pub(crate) fn close_verification(&mut self) {
        if let Some(done) = self.verification.take() {
            if self.state.selection == Selection::Verification {
                self.select(done.back);
            }
        }
    }
}

/// The verdict as a short phrase, for the status bar.
fn sentence(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Verified => "verified",
        Verdict::Diverged => "does not reproduce",
        Verdict::Incomplete => "incomplete, some steps could not be run again",
        Verdict::Unsealed => "no seal to compare",
        Verdict::Empty => "nothing to replay",
    }
}

/// The verdict as the heading of the page.
fn headline(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Verified => "Verified",
        Verdict::Diverged => "Does not reproduce",
        Verdict::Incomplete => "Incomplete",
        Verdict::Unsealed => "No seal to compare",
        Verdict::Empty => "Nothing to replay",
    }
}

fn verdict_color(tokens: &Tokens, verdict: Verdict) -> Rgba {
    let c = &tokens.colors;
    match verdict {
        Verdict::Verified => c.ok,
        Verdict::Diverged => c.error,
        Verdict::Incomplete | Verdict::Unsealed => c.stale,
        Verdict::Empty => c.text_muted,
    }
}

/// What a step ended in, in words.
fn status_text(status: Status) -> &'static str {
    match status {
        Status::Match => "matches",
        Status::Diverged => "differs",
        Status::NoSeal => "no seal recorded",
        Status::NotReplayable => "not replayed",
        Status::Blocked => "not run",
    }
}

fn status_color(tokens: &Tokens, status: Status) -> Rgba {
    let c = &tokens.colors;
    match status {
        Status::Match => c.ok,
        Status::Diverged => c.error,
        Status::NoSeal => c.text_muted,
        Status::NotReplayable | Status::Blocked => c.stale,
    }
}

/// How many steps ended in each status: the ones that matter always, the others when there are.
fn counts_text(counts: &Counts) -> String {
    let mut parts = vec![
        format!("{} matching", counts.matched),
        format!("{} differing", counts.diverged),
    ];
    for (n, what) in [
        (counts.no_seal, "without a seal"),
        (counts.not_replayable, "not replayed"),
        (counts.blocked, "not run"),
    ] {
        if n > 0 {
            parts.push(format!("{n} {what}"));
        }
    }
    parts.join(", ")
}

/// The stored project against the replayed one, in words.
fn state_text(state: &StateCheck) -> String {
    let digest = |hex: &Option<String>| hex.as_deref().map(short).unwrap_or_default().to_owned();
    match state.matches {
        Some(true) => format!(
            "the project stored in the file is the one the steps produce ({})",
            digest(&state.stored)
        ),
        Some(false) => {
            // What differs, as the engine names it (`worksheet 1`, `analysis 3`).
            let what = match state.differing.as_slice() {
                [] => String::new(),
                [one] => format!(": {one} differs"),
                several => format!(": {} differ", several.join(", ")),
            };
            format!(
                "the project stored in the file is not the one the steps produce{what} (stored {}, replayed {})",
                digest(&state.stored),
                digest(&state.replayed)
            )
        }
        None => format!(
            "not compared: {}",
            fmt::plain(
                state
                    .note
                    .as_deref()
                    .unwrap_or("the replay could not finish")
            )
        ),
    }
}

/// An engine as a person reads it: `Caladrius 0.3.2 (commit 1a2b3c4)`.
fn engine_text(id: &EngineId) -> String {
    let version = id.version.as_deref().unwrap_or("of unknown version");
    match id.commit.as_deref() {
        Some(commit) => format!(
            "Caladrius {version} (commit {})",
            commit.get(..COMMIT_DIGITS).unwrap_or(commit)
        ),
        None => format!("Caladrius {version}"),
    }
}

/// True when the engine that replayed is not the one (or ones) that sealed the steps.
fn another_engine(report: &Report) -> bool {
    let Some(now) = &report.engine else {
        return false;
    };
    report.sealed_by.iter().any(|then| {
        then.version != now.version
            || matches!((&then.commit, &now.commit), (Some(a), Some(b)) if a != b)
    })
}

/// The page: the verdict, what it rests on, and every step.
pub(crate) fn screen(
    ui: &mut Ui,
    tokens: &Tokens,
    verification: &Verification,
    actions: &mut Vec<Action>,
) {
    let c = &tokens.colors;
    let report = &verification.report;
    ui.label(
        RichText::new("Verify a project")
            .size(tokens.font.title)
            .strong(),
    );
    ui.label(RichText::new(&verification.name).color(c.text_muted.color()));
    ui.add_space(tokens.spacing.small);
    ui.label(
        RichText::new(
            "The history in this file is run again from an empty project, with this engine, and every step is checked against the seal stored with it. The project you have open is not read or changed.",
        )
        .color(c.text_muted.color()),
    );
    ui.add_space(tokens.spacing.small);
    ui.horizontal(|ui| {
        if ui
            .add(tokens.primary_button("Verify another project…"))
            .clicked()
        {
            actions.push(Action::VerifyProject);
        }
        if ui.button("Close").clicked() {
            actions.push(Action::CloseVerification);
        }
    });
    ui.add_space(tokens.spacing.medium);

    let color = verdict_color(tokens, report.verdict).color();
    tokens
        .card_frame()
        .stroke(egui::Stroke::new(tokens.stroke.medium, color))
        .show(ui, |ui| {
            ui.label(
                RichText::new(headline(report.verdict))
                    .size(tokens.font.heading)
                    .strong()
                    .color(color),
            );
            ui.label(fmt::plain(&report.summary));
        });
    ui.add_space(tokens.spacing.medium);

    let sealed: Vec<String> = report.sealed_by.iter().map(engine_text).collect();
    let mut facts = vec![
        ("Steps", counts_text(&report.counts)),
        ("Project", state_text(&report.state)),
        (
            "Sealed by",
            if sealed.is_empty() {
                "no step carries a seal".to_owned()
            } else {
                sealed.join("; ")
            },
        ),
    ];
    if let Some(now) = &report.engine {
        facts.push(("Replayed by", engine_text(now)));
    }
    egui::Grid::new("verify-facts")
        .num_columns(2)
        .spacing([tokens.spacing.large, tokens.spacing.small])
        .show(ui, |ui| {
            for (label, text) in &facts {
                ui.label(RichText::new(*label).color(c.text_muted.color()));
                ui.add(egui::Label::new(text.as_str()).wrap());
                ui.end_row();
            }
        });
    if report.verdict == Verdict::Diverged && another_engine(report) {
        ui.label(
            RichText::new(
                "The engine that replayed is not the one that sealed the steps: a step that differs may come from the engine and not from the file.",
            )
            .color(c.stale.color()),
        );
    }

    section(ui, tokens, "Steps", |ui| {
        ui.label(
            RichText::new(
                "A seal is the SHA-256 of a command, its parameters and its answer. The first twelve hexadecimal digits are shown; hover one to see all of it.",
            )
            .small()
            .color(c.text_muted.color()),
        );
        ui.add_space(tokens.spacing.small);
        let first = report.first_divergence.as_ref().map(|step| step.seq);
        // A table wider than the window scrolls sideways instead of squeezing its columns.
        egui::ScrollArea::horizontal()
            .id_salt("verify-steps-scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| steps(ui, tokens, &report.steps, first));
        notes(ui, tokens, &report.steps);
    });
}

fn steps(ui: &mut Ui, tokens: &Tokens, steps: &[Step], first: Option<u64>) {
    egui::Grid::new("verify-steps")
        .num_columns(5)
        .spacing([tokens.spacing.large, tokens.spacing.small])
        .striped(true)
        .show(ui, |ui| {
            for title in [
                "Step",
                "Command",
                "Result",
                "Stored seal",
                "Recomputed seal",
            ] {
                ui.label(RichText::new(title).strong());
            }
            ui.end_row();
            for step in steps {
                row(ui, tokens, step, first == Some(step.seq));
                ui.end_row();
            }
        });
}

/// One step. `first`: it is the first step whose seal is not reproduced; what comes after it may
/// differ only because of it.
fn row(ui: &mut Ui, tokens: &Tokens, step: &Step, first: bool) {
    ui.label(step.seq.to_string());
    ui.label(&step.command);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(status_text(step.status))
                .strong()
                .color(status_color(tokens, step.status).color()),
        );
        if first {
            ui.label(
                RichText::new("first difference")
                    .small()
                    .color(tokens.colors.error.color()),
            );
        }
    });
    for seal in [&step.stored, &step.recomputed] {
        match seal {
            Some(hex) => {
                ui.label(RichText::new(short(hex)).monospace())
                    .on_hover_text(hex);
            }
            None => {
                ui.label(RichText::new("-").color(tokens.colors.text_muted.color()));
            }
        }
    }
}

/// What the engine says about steps, under the table: a sentence that is the same for several
/// steps is written once, with the steps it concerns.
fn notes(ui: &mut Ui, tokens: &Tokens, steps: &[Step]) {
    let mut groups: Vec<(&str, Vec<u64>)> = Vec::new();
    for step in steps {
        let Some(note) = step.note.as_deref() else {
            continue;
        };
        match groups.iter_mut().find(|(text, _)| *text == note) {
            Some((_, seqs)) => seqs.push(step.seq),
            None => groups.push((note, vec![step.seq])),
        }
    }
    if groups.is_empty() {
        return;
    }
    section(ui, tokens, "Notes", |ui| {
        for (note, seqs) in groups {
            let which = match seqs.as_slice() {
                [one] => format!("Step {one}"),
                many => {
                    let list: Vec<String> = many.iter().map(u64::to_string).collect();
                    format!("Steps {}", list.join(", "))
                }
            };
            ui.add(egui::Label::new(format!("{which}: {}", fmt::plain(note))).wrap());
        }
    });
}
