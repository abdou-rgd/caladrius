//! A fit that runs for a while, as a value: `Engine::begin`, `Job::run` (or `Job::step`), then
//! `Engine::finish` (`docs/long-running-commands.md` sections 1 and 2).
//!
//! The engine marks one job as running. While it runs, every command that changes the project is
//! refused with a sentence naming the job (nothing is recorded for it), and `finish` also checks
//! that [`Engine::revision`] did not move, so the entry that `finish` records replays against the
//! project the fit saw. A [`Job`] owns its fit and holds no borrow of the engine and no lock; it is
//! `Send`, and it needs no thread of its own: a host that wants one runs [`Job::run`] on it, a
//! host without one (WebAssembly) calls [`Job::step`] between two ticks of its event loop.

use caladrius_fit::{Control, FitResult, FitRun, ModelId, Observer, Outcome, Progress, Step};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use crate::Engine;
use crate::commands::{self, FitJobData};
use crate::error::CommandError;

/// The id of the one command that can run as a job.
pub const FIT_RUN: &str = "fit.run";

/// A number no other engine of the process has. It only tells engines apart: it is not in an
/// answer, a seal or the history, so it does not touch rule 10.
pub(crate) fn new_identity() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// What the engine remembers of the job that is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Running {
    pub(crate) id: u64,
    pub(crate) command: &'static str,
}

/// What `begin` took from the project and from the caller, kept until `finish`.
struct Ticket {
    id: u64,
    /// The engine that began the job.
    engine: u64,
    /// The parameters as received (what the entry of the history and the seal carry).
    received: Value,
    /// The hash of the worksheet(s) the fit reads, taken before it ran.
    input: Option<String>,
    /// `Engine::revision()` at `begin`.
    revision: u64,
    data: FitJobData,
}

/// A fit that has begun: the observations and the initial estimates (FIT-INI-01) are taken, the
/// iterations have not started. `Send`; owns everything it needs.
pub struct Job {
    ticket: Ticket,
    run: FitRun<ModelId>,
}

/// A job that ended, by itself or because it was cancelled, waiting for [`Engine::finish`].
pub struct Completed {
    ticket: Ticket,
    outcome: Outcome,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("id", &self.ticket.id)
            .field("iteration", &self.run.progress().iteration)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Completed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Completed")
            .field("id", &self.ticket.id)
            .field("cancelled_after", &self.cancelled_after())
            .finish_non_exhaustive()
    }
}

impl Job {
    /// The number of the job (1 for the first one the engine began). A host numbers its own jobs
    /// as it likes; this one is not sealed and not in the history.
    pub fn id(&self) -> u64 {
        self.ticket.id
    }

    /// The state after the last committed iteration (iteration 0 before the first step).
    pub fn progress(&self) -> Progress<'_> {
        self.run.progress()
    }

    /// Exactly one iteration (see [`FitRun::step`]). When it returns [`Step::Finished`], hand the
    /// result to [`Job::finished`].
    pub fn step(&mut self) -> Step<'_> {
        self.run.step()
    }

    /// The end of a job driven with [`Job::step`]: the result that [`Step::Finished`] carried.
    pub fn finished(self, result: FitResult) -> Completed {
        Completed {
            ticket: self.ticket,
            outcome: Outcome::Done(result),
        }
    }

    /// Runs to the end, offering the initial estimates and each committed iteration to `observer`
    /// (see [`FitRun::run`]). The observer can only say continue or cancel; it changes no number.
    pub fn run(self, observer: &mut dyn Observer) -> Completed {
        Completed {
            ticket: self.ticket,
            outcome: self.run.run(observer),
        }
    }

    /// Gives up between two iterations. [`Engine::finish`] then stores nothing and records
    /// nothing, and lets the engine run other commands again.
    pub fn cancel(self) -> Completed {
        Completed {
            ticket: self.ticket,
            outcome: self.run.cancel(),
        }
    }
}

impl Completed {
    /// `Some(n)` when the job was cancelled after iteration `n`: nothing will be stored.
    pub fn cancelled_after(&self) -> Option<u32> {
        match &self.outcome {
            Outcome::Cancelled {
                after_iteration, ..
            } => Some(*after_iteration),
            Outcome::Done(_) => None,
        }
    }

    /// The rows of the minimisation trace (FIT-OUT-11) of a cancelled job, for a screen: a record,
    /// never an answer.
    pub fn trace(&self) -> &[caladrius_fit::TraceRow] {
        match &self.outcome {
            Outcome::Cancelled { trace, .. } => trace,
            Outcome::Done(result) => result.trace(),
        }
    }
}

/// An observer that never cancels: what `execute` uses.
pub(crate) struct NeverCancel;

impl Observer for NeverCancel {
    fn after_iteration(&mut self, _progress: &Progress<'_>) -> Control {
        Control::Continue
    }
}

/// How `finish_job` ended: an answer or an error that `execute` or `finish` records, or a refusal
/// that is recorded nowhere (the job was cancelled, or it is not the running one).
pub(crate) enum Settled {
    Recorded(Result<Value, CommandError>),
    Unrecorded(CommandError),
}

impl Engine {
    /// The number of the job that is running, if one is.
    pub fn running_job(&self) -> Option<u64> {
        self.running.map(|r| r.id)
    }

    /// The way out for a host that lost its job (a `Job` or `Completed` dropped, a worker that
    /// panicked, a command that was aborted): if `id` is the running job, clears the mark and
    /// returns true. Nothing is recorded and the project is untouched. Any other `id` changes
    /// nothing and returns false.
    pub fn abandon_job(&mut self, id: u64) -> bool {
        if self.running.is_some_and(|r| r.id == id) {
            self.running = None;
            true
        } else {
            false
        }
    }

    /// The refusal of a command that would change the project under the running job, if one runs.
    pub(crate) fn refuse_while_running(&self, id: &str, params: &Value) -> Option<CommandError> {
        let running = self.running?;
        let changes = match commands::find(id) {
            Some(command) => {
                command.mutates
                    || (id == "model.simulate"
                        && params
                            .get("store")
                            .is_some_and(|s| s.as_bool().unwrap_or(true)))
            }
            None => false,
        };
        changes.then(|| {
            CommandError::new(
                "job_running",
                format!(
                    "job {} (`{}`) is running and `{id}` would change the project under it; wait for the job to end, or cancel it, then run `{id}` again",
                    running.id, running.command
                ),
            )
        })
    }

    /// Begins `fit.run`: validates the parameters exactly as [`Engine::execute`] does, reads the
    /// observations and the initial estimates (FIT-INI-01) from the project, takes the hash of the
    /// input and [`Engine::revision`], and marks the job as running. The fit has not started.
    ///
    /// Run it with [`Job::run`] (or [`Job::step`]), then hand it to [`Engine::finish`]. Parameters
    /// that are not valid are recorded as a failed `fit.run`, as `execute` records them. While the
    /// job runs, a second `begin` and every command that changes the project are refused with a
    /// sentence naming it (read-only commands still run); nothing is recorded for a refusal.
    pub fn begin(&mut self, id: &str, params: Value) -> Result<Job, CommandError> {
        let received = params.clone();
        match self.begin_job(id, params) {
            Ok(job) => Ok(job),
            Err(e) => {
                // A refusal because of the running job is not a run of the command. Nothing
                // changed the project since `begin_job` started, so the hash is still the one
                // the fit would have read.
                if e.code != "job_running" && id == FIT_RUN {
                    let input = self.input_hash_of(id, &received);
                    self.record(id, received, input, &Err(e.clone()), true);
                }
                Err(e)
            }
        }
    }

    /// Ends a job: stores the analysis, records one `fit.run` entry with its seal, lets the engine
    /// run other commands again and returns the answer, byte for byte what `execute("fit.run",
    /// ..)` gives with the same parameters and project.
    ///
    /// A job that was cancelled stores nothing and records nothing, whatever its state: even a
    /// fit that had converged and only had its statistics left is not stored when the cancel
    /// reached it. The error then has the code `fit_cancelled` and the sentence "fit cancelled
    /// after iteration N; nothing was stored". `finish` also refuses, with a sentence and without
    /// storing, when the project changed since `begin` ([`Engine::revision`] moved) or when the job
    /// is not the one the engine is running; the revision check ends the job, the other does not.
    /// This one function takes a cancelled job too, so there is no separate `abandon`.
    pub fn finish(&mut self, completed: Completed) -> Result<Value, CommandError> {
        let id = FIT_RUN;
        let received = completed.ticket.received.clone();
        let input = completed.ticket.input.clone();
        match self.finish_job(completed) {
            Settled::Unrecorded(e) => Err(e),
            Settled::Recorded(outcome) => {
                if outcome.is_ok() {
                    self.revision = self.revision.wrapping_add(1);
                }
                self.record(id, received, input, &outcome, true);
                outcome
            }
        }
    }

    /// `begin` without the entry: shared with `execute`, which records the command itself.
    pub(crate) fn begin_job(&mut self, id: &str, params: Value) -> Result<Job, CommandError> {
        if id != FIT_RUN {
            return Err(CommandError::new(
                "not_a_job",
                format!("`{id}` does not run as a job; the only command that does is `{FIT_RUN}`"),
            ));
        }
        if let Some(running) = self.running {
            return Err(CommandError::new(
                "job_running",
                format!(
                    "job {} (`{}`) is already running; wait for it to end, or cancel it, before starting another",
                    running.id, running.command
                ),
            ));
        }
        let received = params.clone();
        let input = self.input_hash_of(id, &received);
        let (run, data) = commands::begin_fit(self, params)?;
        self.jobs_begun = self.jobs_begun.wrapping_add(1);
        let job_id = self.jobs_begun;
        self.running = Some(Running {
            id: job_id,
            command: FIT_RUN,
        });
        Ok(Job {
            ticket: Ticket {
                id: job_id,
                engine: self.identity,
                received,
                input,
                revision: self.revision,
                data,
            },
            run,
        })
    }

    /// `finish` without the entry.
    pub(crate) fn finish_job(&mut self, completed: Completed) -> Settled {
        let Completed { ticket, outcome } = completed;
        if ticket.engine != self.identity || self.running.map(|r| r.id) != Some(ticket.id) {
            return Settled::Unrecorded(CommandError::new(
                "not_the_running_job",
                format!(
                    "job {} is not the job this engine is running; nothing was stored",
                    ticket.id
                ),
            ));
        }
        self.running = None;
        let result = match outcome {
            Outcome::Cancelled {
                after_iteration, ..
            } => {
                return Settled::Unrecorded(CommandError::new(
                    "fit_cancelled",
                    format!("fit cancelled after iteration {after_iteration}; nothing was stored"),
                ));
            }
            Outcome::Done(result) => result,
        };
        if self.revision != ticket.revision {
            return Settled::Unrecorded(CommandError::new(
                "project_changed",
                format!(
                    "the project changed while job {} ran, so its fit no longer matches it; nothing was stored. Run `{FIT_RUN}` again",
                    ticket.id
                ),
            ));
        }
        Settled::Recorded(commands::finish_fit(self, ticket.data, result))
    }

    fn input_hash_of(&self, id: &str, received: &Value) -> Option<String> {
        if crate::seal::is_sealed(id) {
            crate::seal::input_hash(&self.project, id, received)
        } else {
            None
        }
    }
}
