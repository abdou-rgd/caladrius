#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! Test layer: a fixed workload through [`Engine::execute`], and a fingerprint of what came back.
//!
//! The numbers of a result must not depend on the machine: the desktop application, the command
//! line, the MCP server and the page in the browser are the same engine. This crate is how that is
//! checked. A [`Workload`] is a list of sessions, each a script of commands for a fresh engine
//! (`data.import`, `nca.run`, `fit.run`, `model.simulate`, `project.replay`...). [`run`] executes
//! it and writes, for every step, one canonical JSON document (the result, or the error code and
//! message, and the seal of the engine: the SHA-256 of the command, its parameters and its
//! answer); the SHA-256 of that text is the digest. Two builds that print the same digest returned
//! the same bits, and made the same seals.
//!
//! The crate has no file access and no clock, so it compiles to WebAssembly: `cargo xtask digest`
//! builds the workload from the oracle and the command examples, runs it natively, and with
//! `--wasm` runs the same bytes of workload in the module of this crate under Node. The wasm
//! entry points are in `wasm.rs`.

mod diff;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use caladrius_engine::seal::{Sha256, canonical_json, sha256_hex};
pub use diff::{Diff, compare};

use caladrius_engine::{CommandError, Engine, Seal};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// A list of sessions. Each session starts from an empty engine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Workload {
    pub sessions: Vec<Session>,
}

/// A script of commands for one engine: what a user, the CLI or an agent would do in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// A name that tells what the session is, for example `nca/theoph`.
    pub name: String,
    pub steps: Vec<Step>,
}

/// One command, as [`Engine::execute`] takes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// Command id, for example `fit.run`.
    pub id: String,
    pub params: Value,
}

/// What a workload returned.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    sessions: Vec<RunSession>,
}

/// The canonical documents of one session.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSession {
    pub name: String,
    /// One canonical JSON document per step.
    pub lines: Vec<String>,
    /// Steps whose command returned an error (a refusal is a result too: its message is hashed).
    pub refused: usize,
    /// Numbers in the documents.
    pub numbers: usize,
}

impl RunSession {
    /// The SHA-256 of the session's documents, one per line.
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        for line in &self.lines {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        hasher.finalize_hex()
    }
}

impl Run {
    pub fn sessions(&self) -> &[RunSession] {
        &self.sessions
    }

    /// Steps executed.
    pub fn steps(&self) -> usize {
        self.sessions.iter().map(|s| s.lines.len()).sum()
    }

    /// Steps whose command returned an error.
    pub fn refused(&self) -> usize {
        self.sessions.iter().map(|s| s.refused).sum()
    }

    /// Numbers in the documents.
    pub fn numbers(&self) -> usize {
        self.sessions.iter().map(|s| s.numbers).sum()
    }

    /// The documents of every step, one per line: the text that is hashed and that `--dump` writes.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for line in self.sessions.iter().flat_map(|s| &s.lines) {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// The digest: the SHA-256 of [`Run::text`], 64 hexadecimal digits.
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        for line in self.sessions.iter().flat_map(|s| &s.lines) {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        hasher.finalize_hex()
    }
}

/// Executes every session of `workload` and canonicalizes what the commands returned.
pub fn run(workload: &Workload) -> Run {
    let sessions = workload
        .sessions
        .iter()
        .map(|session| {
            let mut engine = Engine::new();
            // The version and the commit of the build are not part of what is compared: a new
            // commit that computes the same bits must give the same digest.
            engine.set_provenance(false);
            let mut done = RunSession {
                name: session.name.clone(),
                lines: Vec::with_capacity(session.steps.len()),
                refused: 0,
                numbers: 0,
            };
            for (index, step) in session.steps.iter().enumerate() {
                let outcome = engine.execute(&step.id, step.params.clone());
                done.refused += usize::from(outcome.is_err());
                let seal = engine
                    .history()
                    .entries()
                    .last()
                    .and_then(|e| e.seal.clone());
                let (line, numbers) =
                    canonical_json(&document(session, index, step, outcome, seal));
                done.numbers += numbers;
                done.lines.push(line);
            }
            done
        })
        .collect();
    Run { sessions }
}

/// [`run`] for a workload written as JSON text. The error is a message about the text.
pub fn run_text(workload: &str) -> Result<Run, String> {
    let workload: Workload =
        serde_json::from_str(workload).map_err(|e| format!("the workload is not valid: {e}"))?;
    Ok(run(&workload))
}

/// The document of one step: where it is, what the command returned and its seal (the commands
/// that carry none, `project.load` and `project.replay`, have `null`).
fn document(
    session: &Session,
    index: usize,
    step: &Step,
    outcome: Result<Value, CommandError>,
    seal: Option<Seal>,
) -> Value {
    let mut doc = Map::new();
    doc.insert("at".to_owned(), json!(format!("{}#{index}", session.name)));
    doc.insert("id".to_owned(), json!(step.id));
    match outcome {
        Ok(result) => {
            doc.insert("ok".to_owned(), Value::Bool(true));
            doc.insert("result".to_owned(), result);
        }
        Err(error) => {
            doc.insert("ok".to_owned(), Value::Bool(false));
            doc.insert(
                "error".to_owned(),
                json!({ "code": error.code, "message": error.message }),
            );
        }
    }
    doc.insert(
        "seal".to_owned(),
        seal.and_then(|s| serde_json::to_value(s).ok())
            .unwrap_or(Value::Null),
    );
    Value::Object(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "Time (h),Conc (mg/L),Dose (mg)\n0,0,100\n0.5,2.1,100\n1,3.3,100\n2,3.9,100\n4,3.6,100\n8,2.4,100\n12,1.6,100\n24,0.5,100\n";

    fn small() -> Workload {
        Workload {
            sessions: vec![
                Session {
                    name: "nca/small".to_owned(),
                    steps: vec![
                        Step {
                            id: "data.import".to_owned(),
                            params: json!({ "name": "small", "csv": CSV }),
                        },
                        Step {
                            id: "nca.run".to_owned(),
                            params: json!({ "worksheet": 1, "route": "extravascular" }),
                        },
                        Step {
                            id: "no.such.command".to_owned(),
                            params: Value::Null,
                        },
                    ],
                },
                Session {
                    name: "model/bolus".to_owned(),
                    steps: vec![Step {
                        id: "model.simulate".to_owned(),
                        params: json!({
                            "model": "pk1.iv_bolus", "dose": 100,
                            "params": { "v": 10, "k": 0.1 }, "times": [0, 1, 2, 5, 10]
                        }),
                    }],
                },
            ],
        }
    }

    #[test]
    fn a_workload_gives_one_document_per_step_and_a_stable_digest() {
        let first = run(&small());
        let second = run(&small());
        assert_eq!(first.steps(), 4);
        assert_eq!(first.refused(), 1, "the unknown command is a result too");
        assert!(first.numbers() > 50, "{}", first.numbers());
        assert_eq!(first.text().lines().count(), 4);
        assert_eq!(first.digest(), second.digest());
        assert_eq!(first.digest().len(), 64);
        assert_eq!(first.digest(), sha256_hex(first.text().as_bytes()));
    }

    #[test]
    fn every_document_says_where_it_is_and_what_came_back() {
        let run = run(&small());
        let docs: Vec<Value> = run
            .text()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(docs[1]["at"], "nca/small#1");
        assert_eq!(docs[1]["id"], "nca.run");
        assert_eq!(docs[1]["ok"], true);
        assert!(docs[1]["result"].is_object());
        assert_eq!(docs[2]["ok"], false);
        assert_eq!(docs[2]["error"]["code"], "unknown_command");
        assert_eq!(docs[3]["result"]["conc"][0], 10.0);
    }

    #[test]
    fn a_changed_parameter_changes_the_digest_and_the_diff_finds_it() {
        let a = run(&small());
        let mut other = small();
        other.sessions[1].steps[0].params["params"]["k"] = json!(0.1000000000000001);
        let b = run(&other);
        assert_ne!(a.digest(), b.digest());
        assert_ne!(a.sessions()[0].digest(), b.sessions()[1].digest());
        assert_eq!(a.sessions()[0].digest(), b.sessions()[0].digest());
        let diff = compare(&a.text(), &b.text()).unwrap();
        assert!(!diff.identical());
        assert!(diff.numbers_differing > 0);
        assert!(compare(&a.text(), &a.text()).unwrap().identical());
    }

    #[test]
    fn a_workload_survives_its_own_json_text() {
        let workload = small();
        let text = serde_json::to_string(&workload).unwrap();
        assert_eq!(serde_json::from_str::<Workload>(&text).unwrap(), workload);
        assert_eq!(run_text(&text).unwrap().digest(), run(&workload).digest());
        assert!(run_text("[1, 2]").unwrap_err().contains("not valid"));
    }

    #[test]
    fn a_session_starts_from_an_empty_engine() {
        let step = |id: &str, params: Value| Step {
            id: id.to_owned(),
            params,
        };
        let import = step("data.import", json!({ "name": "w", "csv": CSV }));
        let nca = step(
            "nca.run",
            json!({ "worksheet": 1, "route": "extravascular" }),
        );
        let workload = Workload {
            sessions: vec![
                Session {
                    name: "first".to_owned(),
                    steps: vec![import],
                },
                Session {
                    name: "second".to_owned(),
                    steps: vec![nca],
                },
            ],
        };
        let run = run(&workload);
        assert_eq!(run.sessions()[0].refused, 0);
        assert_eq!(
            run.sessions()[1].refused,
            1,
            "worksheet 1 does not exist here"
        );
    }
}
