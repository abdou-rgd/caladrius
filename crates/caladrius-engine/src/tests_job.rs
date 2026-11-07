//! `Engine::begin` and `Engine::finish` (T-146): a fit that runs as a job gives the same answer
//! bytes, the same seal and the same history entry as `execute`, one job runs at a time, a job
//! that is cancelled leaves no trace, and the project cannot move under a job.

use serde_json::{Value, json};

use crate::*;

const TWO_SUBJECTS: &str = "Subject,Time (h),Conc (mg/L),Dose (mg)\n\
A,0,0,100\nA,0.5,2.195,100\nA,1,3.293,100\nA,2,3.971,100\nA,4,3.611,100\nA,6,2.989,100\nA,8,2.451,100\nA,12,1.643,100\nA,24,0.495,100\n\
B,0,0,100\nB,0.5,1.8,100\nB,1,2.9,100\nB,2,3.5,100\nB,4,3.2,100\nB,6,2.6,100\nB,8,,100\nB,12,1.4,100\nB,24,0.4,100\n";

const TIMES: [f64; 14] = [
    0.1, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 36.0, 48.0,
];

fn ok(engine: &mut Engine, id: &str, params: Value) -> Value {
    match engine.execute(id, params) {
        Ok(v) => v,
        Err(e) => panic!("{id} failed: {e}"),
    }
}

/// An engine with the oral profiles of two subjects.
fn oral() -> Engine {
    let mut e = Engine::new();
    ok(
        &mut e,
        "data.import",
        json!({ "name": "study", "csv": TWO_SUBJECTS }),
    );
    e
}

/// An engine with a simulated two-compartment profile and a small alternating error.
fn pk2() -> Engine {
    let mut e = Engine::new();
    let sim = ok(
        &mut e,
        "model.simulate",
        json!({ "model": "pk2.iv_bolus", "dose": 100, "params": { "cl": 2, "vc": 10, "q": 4, "vp": 8 }, "times": TIMES }),
    );
    let mut csv = String::from("Subject,Time (h),Conc (mg/L),Dose (mg)\n");
    let conc = sim["conc"].as_array().cloned().unwrap_or_default();
    for (i, (t, c)) in TIMES.iter().zip(&conc).enumerate() {
        let noise = if i % 2 == 0 { 1.01 } else { 0.99 };
        csv.push_str(&format!(
            "A,{t},{},100\n",
            c.as_f64().unwrap_or(0.0) * noise
        ));
    }
    ok(&mut e, "data.import", json!({ "name": "pk2", "csv": csv }));
    e
}

/// A case: its label, the engine, the parameters, and the status the answer must have (`None`
/// for a fit that is refused).
type Case = (&'static str, Engine, Value, Option<&'static str>);

/// The fits of the tests: one per model family and per way a fit ends.
fn cases() -> Vec<Case> {
    let oral_fit = json!({ "worksheet": 1, "subject": "A", "model": "pk1.oral_1" });
    let mut oral_limit = oral_fit.clone();
    oral_limit["options"] = json!({ "max_iterations": 2 });
    let mut oral_bad = oral_fit.clone();
    oral_bad["initial"] = json!({ "v": 0, "k": 0.1, "ka": 1 });
    let start = json!({ "cl": 1.6, "vc": 11, "q": 3.2, "vp": 9 });
    let pk2_fit = json!({ "worksheet": 1, "model": "pk2.iv_bolus", "initial": start });
    let mut pk2_limit = pk2_fit.clone();
    pk2_limit["options"] = json!({ "max_iterations": 2 });
    let mut pk2_bad = pk2_fit.clone();
    pk2_bad["initial"] = json!({ "cl": 0, "vc": 11, "q": 3.2, "vp": 9 });
    vec![
        ("pk1 converges", oral(), oral_fit, Some("converged")),
        (
            "pk1 stops on the iteration limit",
            oral(),
            oral_limit,
            Some("max_iterations"),
        ),
        ("pk1 fails", oral(), oral_bad, None),
        ("pk2 converges", pk2(), pk2_fit, Some("converged")),
        (
            "pk2 stops on the iteration limit",
            pk2(),
            pk2_limit,
            Some("max_iterations"),
        ),
        ("pk2 fails", pk2(), pk2_bad, None),
    ]
}

fn oral_params() -> Value {
    json!({ "worksheet": 1, "subject": "A", "model": "pk1.oral_1" })
}

fn project_text(e: &Engine) -> String {
    serde_json::to_string(e.project()).unwrap_or_default()
}

/// Records what an observer is offered.
struct Recorder {
    before: Vec<u32>,
    after: Vec<u32>,
    cancel_at: Option<u32>,
}

impl Recorder {
    fn new(cancel_at: Option<u32>) -> Self {
        Self {
            before: Vec::new(),
            after: Vec::new(),
            cancel_at,
        }
    }
}

impl Observer for Recorder {
    fn before_first_iteration(&mut self, progress: &Progress<'_>) -> Control {
        self.before.push(progress.iteration);
        Control::Continue
    }

    fn after_iteration(&mut self, progress: &Progress<'_>) -> Control {
        self.after.push(progress.iteration);
        if self.cancel_at == Some(progress.iteration) {
            Control::Cancel
        } else {
            Control::Continue
        }
    }
}

/// Drives a job one iteration at a time, as a host without threads does.
fn by_steps(mut job: Job) -> (Completed, Vec<u32>) {
    let mut seen = Vec::new();
    loop {
        match job.step() {
            Step::Progress(p) => seen.push(p.iteration),
            Step::Finished(result) => return (job.finished(result), seen),
        }
    }
}

fn assert_same_session(label: &str, a: &Engine, b: &Engine) {
    assert_eq!(
        a.history().entries(),
        b.history().entries(),
        "{label}: history entries (with their seals)"
    );
    assert_eq!(project_text(a), project_text(b), "{label}: project");
    assert_eq!(a.revision(), b.revision(), "{label}: revision");
    assert_eq!(b.running_job(), None, "{label}: no job left running");
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[test]
fn a_job_gives_the_bytes_the_seal_and_the_entry_of_execute() {
    for (label, engine, params, status) in cases() {
        // `execute` is the reference.
        let mut reference = engine.clone();
        let wanted = reference.execute("fit.run", params.clone());
        match (&wanted, status) {
            (Ok(answer), Some(status)) => assert_eq!(
                answer["result"]["outcome"]["ok"]["status"], status,
                "{label}: the case does not end as intended"
            ),
            (Err(_), None) => {}
            (other, expected) => panic!("{label}: {other:?} against {expected:?}"),
        }
        let before_begin = engine.history().len();

        // `begin`, one `step` at a time, `finish`.
        let mut stepped = engine.clone();
        let stepped_answer = match stepped.begin("fit.run", params.clone()) {
            Ok(job) => {
                assert_eq!(stepped.running_job(), Some(job.id()), "{label}");
                let (done, seen) = by_steps(job);
                let expected: Vec<u32> = (1..=count(seen.len())).collect();
                assert_eq!(seen, expected, "{label}: iterations are 1, 2, 3...");
                stepped.finish(done)
            }
            Err(e) => {
                assert_eq!(stepped.running_job(), None, "{label}");
                Err(e)
            }
        };

        // `begin`, `run` with an observer that watches, `finish`.
        let mut watched = engine.clone();
        let mut recorder = Recorder::new(None);
        let watched_answer = match watched.begin("fit.run", params.clone()) {
            Ok(job) => watched.finish(job.run(&mut recorder)),
            Err(e) => Err(e),
        };

        for (how, session, answer) in [
            ("steps", &stepped, &stepped_answer),
            ("observer", &watched, &watched_answer),
        ] {
            let label = format!("{label} ({how})");
            assert_eq!(answer, &wanted, "{label}: the answer");
            if let (Ok(a), Ok(w)) = (answer, &wanted) {
                assert_eq!(a.to_string(), w.to_string(), "{label}: the bytes");
            }
            assert_same_session(&label, &reference, session);
            assert_eq!(session.history().len(), before_begin + 1, "{label}");
        }
        if status.is_some() {
            assert_eq!(recorder.before, [0], "{label}");
            assert!(!recorder.after.is_empty(), "{label}");
            assert_eq!(
                recorder.after,
                (1..=count(recorder.after.len())).collect::<Vec<_>>(),
                "{label}"
            );
        }
        // The entry carries a seal, and it is the one of `execute` (compared above).
        let entry = stepped.history().entries().last();
        assert!(entry.is_some_and(|e| e.seal.is_some()), "{label}: sealed");
    }
}

#[test]
fn a_job_is_send_and_holds_no_borrow_of_the_engine() {
    fn assert_send<T: Send>() {}
    assert_send::<Job>();
    assert_send::<Completed>();
    let mut e = oral();
    let job = e.begin("fit.run", oral_params());
    // The engine can be used (read-only) while the job exists.
    assert!(e.execute("project.describe", Value::Null).is_ok());
    assert!(job.is_ok());
}

#[test]
fn while_a_job_runs_a_command_that_changes_the_project_is_refused_and_not_recorded() {
    let mut e = oral();
    let params = oral_params();
    let Ok(job) = e.begin("fit.run", params.clone()) else {
        panic!("begin failed");
    };
    let entries = e.history().entries().to_vec();
    let project = project_text(&e);
    let revision = e.revision();

    let attempts = [
        (
            "nca.run",
            json!({ "worksheet": 1, "route": "extravascular" }),
        ),
        ("data.import", json!({ "name": "x", "csv": TWO_SUBJECTS })),
        ("project.new", Value::Null),
        ("fit.run", params.clone()),
        (
            "model.simulate",
            json!({ "model": "pk1.iv_bolus", "dose": 100, "params": { "v": 10, "k": 0.1 }, "times": [0, 1], "store": true }),
        ),
    ];
    for (id, p) in attempts {
        let refusal = e.execute(id, p).err();
        let message = refusal
            .as_ref()
            .map(|r| r.message.clone())
            .unwrap_or_default();
        assert_eq!(
            refusal.as_ref().map(|r| r.code.as_str()),
            Some("job_running"),
            "{id}: {refusal:?}"
        );
        assert!(
            message.contains("job 1") && message.contains("fit.run"),
            "{id}: {message}"
        );
    }
    // A second begin is refused in the same way.
    let second = e.begin("fit.run", params.clone()).err();
    assert!(
        second
            .as_ref()
            .is_some_and(|r| r.code == "job_running" && r.message.contains("job 1")),
        "{second:?}"
    );
    assert_eq!(
        e.history().entries(),
        entries.as_slice(),
        "history unchanged"
    );
    assert_eq!(project_text(&e), project, "project unchanged");
    assert_eq!(e.revision(), revision);

    // Commands that only read still run (and are recorded, as always).
    ok(&mut e, "project.describe", Value::Null);
    ok(
        &mut e,
        "model.simulate",
        json!({ "model": "pk1.iv_bolus", "dose": 100, "params": { "v": 10, "k": 0.1 }, "times": [0, 1] }),
    );
    assert_eq!(e.history().len(), entries.len() + 2);

    // The job ends, and the engine takes commands that change the project again.
    let answer = e.finish(job.run(&mut Recorder::new(None)));
    assert!(answer.is_ok(), "{answer:?}");
    assert_eq!(e.running_job(), None);
    ok(
        &mut e,
        "nca.run",
        json!({ "worksheet": 1, "route": "extravascular" }),
    );
}

#[test]
fn a_moved_revision_makes_finish_refuse_and_store_nothing() {
    let mut e = oral();
    let Ok(job) = e.begin("fit.run", oral_params()) else {
        panic!("begin failed");
    };
    let done = job.run(&mut Recorder::new(None));
    let entries = e.history().entries().to_vec();
    let project = project_text(&e);
    // Nothing can move the revision through the commands while a job runs; this is the second
    // check, so it is moved by hand.
    e.revision = e.revision.wrapping_add(1);
    let refusal = e.finish(done).err();
    assert!(
        refusal.as_ref().is_some_and(|r| r.code == "project_changed"
            && r.message.contains("changed while job 1 ran")
            && r.message.contains("nothing was stored")),
        "{refusal:?}"
    );
    assert_eq!(e.history().entries(), entries.as_slice());
    assert_eq!(project_text(&e), project);
    assert_eq!(e.running_job(), None, "the dead job no longer blocks");
    ok(&mut e, "fit.run", oral_params());
}

#[test]
fn a_job_of_another_engine_is_not_taken() {
    let mut a = oral();
    let mut b = oral();
    let (Ok(job_a), Ok(job_b)) = (
        a.begin("fit.run", oral_params()),
        b.begin("fit.run", oral_params()),
    ) else {
        panic!("begin failed");
    };
    // Both are job 1 of their engine: make `b` believe it runs another one.
    let done_a = job_a.run(&mut Recorder::new(None));
    b.running = Some(job::Running {
        id: 7,
        command: "fit.run",
    });
    let refusal = b.finish(done_a).err();
    assert!(
        refusal
            .as_ref()
            .is_some_and(|r| r.code == "not_the_running_job"),
        "{refusal:?}"
    );
    assert_eq!(b.running_job(), Some(7), "the running job is untouched");
    drop(job_b);
}

#[test]
fn a_cancelled_job_leaves_the_project_and_the_history_as_before_begin() {
    let params = oral_params();
    // How many iterations the fit takes, to cancel after the last one.
    let mut probe = oral();
    let Ok(job) = probe.begin("fit.run", params.clone()) else {
        panic!("begin failed");
    };
    let (done, seen) = by_steps(job);
    let _ = probe.finish(done);
    let last = seen.last().copied().unwrap_or(0);
    assert!(last >= 2, "the case should take a few iterations");

    // Cancel before the first iteration, after the first, and after the last one (the fit had
    // converged and only its statistics were left: it is cancelled all the same).
    for cancel_at in [0, 1, last] {
        let mut e = oral();
        let history = e.history().entries().to_vec();
        let project = project_text(&e);
        let revision = e.revision();
        let Ok(job) = e.begin("fit.run", params.clone()) else {
            panic!("begin failed");
        };
        let done = if cancel_at == 0 {
            // Before the first iteration: the host just gives up.
            job.cancel()
        } else {
            job.run(&mut Recorder::new(Some(cancel_at)))
        };
        assert_eq!(done.cancelled_after(), Some(cancel_at));
        assert_eq!(
            done.trace().len(),
            usize::try_from(cancel_at).unwrap_or(0) + 1,
            "rows 0 to {cancel_at}"
        );
        let refusal = e.finish(done).err();
        let expected = format!("fit cancelled after iteration {cancel_at}; nothing was stored");
        assert_eq!(
            refusal
                .as_ref()
                .map(|r| (r.code.as_str(), r.message.as_str())),
            Some(("fit_cancelled", expected.as_str())),
            "cancel at {cancel_at}"
        );
        assert_eq!(
            e.history().entries(),
            history.as_slice(),
            "history, {cancel_at}"
        );
        assert_eq!(project_text(&e), project, "project, {cancel_at}");
        assert_eq!(e.revision(), revision, "revision, {cancel_at}");
        assert_eq!(
            e.running_job(),
            None,
            "the engine is free again, {cancel_at}"
        );
        // And the same fit then runs as if nothing had happened.
        let again = ok(&mut e, "fit.run", params.clone());
        assert_eq!(again["result"]["outcome"]["ok"]["status"], "converged");
    }
}

#[test]
fn only_fit_run_runs_as_a_job() {
    let mut e = oral();
    let refusal = e.begin("nca.run", json!({ "worksheet": 1 })).err();
    assert!(
        refusal
            .as_ref()
            .is_some_and(|r| r.code == "not_a_job" && r.message.contains("fit.run")),
        "{refusal:?}"
    );
    assert_eq!(e.running_job(), None);
    assert_eq!(e.history().len(), 1, "only the import");
}

/// Captured at fa306d4 (before `fit.run` went through jobs), with the provenance off: the SHA-256
/// of the answer bytes (the error text for a refusal), the SHA-256 of the seal and the hash of the
/// input. `execute` and `begin`/`finish` must both still give them.
const GOLDEN: [(&str, &str, &str, &str); 6] = [
    (
        "pk1 converges",
        "ffef529abbba0a2d9286328a76e3fca7aedcce4cc7bf868b1eea50e01ff6aae4",
        "7de6e47f34a5cf65378eb75beb08936890adda83290baf37232ffcd18fe3ddfa",
        "e4f4cf193bb4f41a5ba7e5d77a1a0f2d5ec54637a7746ef901a5edacacefcbdc",
    ),
    (
        "pk1 stops on the iteration limit",
        "3e62a8210a05ae6fe660d45a2b550abef36f7d1183f37b33679e76579e49ff7e",
        "2e3da314500818d83e38637e0a3a92a4258d1dd6b5a7e4bebb3e5a6784c8e4b8",
        "e4f4cf193bb4f41a5ba7e5d77a1a0f2d5ec54637a7746ef901a5edacacefcbdc",
    ),
    (
        "pk1 fails",
        "5b622742654eda28f10f16e86548c9bb6732fadf761c4d955e258371c593c758",
        "4af0e759be860acba670350188b090786b460172c0f55b2f927f72922a3f2cc0",
        "e4f4cf193bb4f41a5ba7e5d77a1a0f2d5ec54637a7746ef901a5edacacefcbdc",
    ),
    (
        "pk2 converges",
        "4c5143e306ff54489f65448e540bfb7f1b900e8d7a9dade1e90fd27b9f3916c4",
        "ea94bab83f4a4f2ba51cd23603c6e44a01c9f41922bace3f937a6a09fa06037f",
        "8b1c4f51f2fab2337721e583b92eb86ee93215858112c9e50c23f95a31453394",
    ),
    (
        "pk2 stops on the iteration limit",
        "cb0b787831b3318a24749633c2449a4c3c0b54b0f2b58efd9b33e6add56ed0d4",
        "ab6625a62c772fbb32bbf4f1902dd880c6e010749710569cf9a6c042f98cc6a5",
        "8b1c4f51f2fab2337721e583b92eb86ee93215858112c9e50c23f95a31453394",
    ),
    (
        "pk2 fails",
        "a3bf11f494620d528fde32fc6ed4eae06ea64bf94d1b735bd4ce592805c5ba6f",
        "f175d7d0c39a3043b113da9b2c8f76277dcc5ff4e48e19f573a794862571ad57",
        "8b1c4f51f2fab2337721e583b92eb86ee93215858112c9e50c23f95a31453394",
    ),
];

#[test]
fn the_answers_and_seals_are_those_of_the_engine_before_jobs() {
    use crate::seal::sha256_hex;
    let digest = |answer: &Result<Value, CommandError>| {
        let text = match answer {
            Ok(v) => v.to_string(),
            Err(e) => e.to_string(),
        };
        sha256_hex(text.as_bytes())
    };
    let seal_of = |e: &Engine| {
        let seal = e.history().entries().last().and_then(|x| x.seal.clone());
        (
            seal.as_ref().map(|s| s.sha256.clone()).unwrap_or_default(),
            seal.and_then(|s| s.input).unwrap_or_default(),
        )
    };
    for ((label, engine, params, _), golden) in cases().into_iter().zip(GOLDEN) {
        assert_eq!(label, golden.0);
        let mut by_execute = engine.clone();
        by_execute.set_provenance(false);
        let answer = by_execute.execute("fit.run", params.clone());
        assert_eq!(digest(&answer), golden.1, "{label}: answer, execute");
        let (seal, input) = seal_of(&by_execute);
        assert_eq!(
            (seal.as_str(), input.as_str()),
            (golden.2, golden.3),
            "{label}: seal, execute"
        );

        let mut by_job = engine.clone();
        by_job.set_provenance(false);
        let answer = match by_job.begin("fit.run", params) {
            Ok(job) => by_job.finish(job.run(&mut Recorder::new(None))),
            Err(e) => Err(e),
        };
        assert_eq!(digest(&answer), golden.1, "{label}: answer, job");
        let (seal, input) = seal_of(&by_job);
        assert_eq!(
            (seal.as_str(), input.as_str()),
            (golden.2, golden.3),
            "{label}: seal, job"
        );
    }
}

#[test]
fn a_job_of_another_engine_is_refused_even_with_the_same_number() {
    let mut a = oral();
    let mut b = oral();
    let (Ok(job_a), Ok(job_b)) = (
        a.begin("fit.run", oral_params()),
        b.begin("fit.run", oral_params()),
    ) else {
        panic!("begin failed");
    };
    assert_eq!(job_a.id(), 1);
    assert_eq!(job_b.id(), 1, "both engines hand out job 1");
    let done_a = job_a.run(&mut Recorder::new(None));
    let b_history = b.history().entries().to_vec();
    let b_project = project_text(&b);
    let refusal = b.finish(done_a).err();
    assert!(
        refusal
            .as_ref()
            .is_some_and(|r| r.code == "not_the_running_job"),
        "{refusal:?}"
    );
    assert_eq!(b.history().entries(), b_history.as_slice());
    assert_eq!(project_text(&b), b_project, "A's fit is not stored in B");
    assert_eq!(b.running_job(), Some(1), "B's own job is untouched");
    // And each engine still ends its own job.
    assert!(b.finish(job_b.run(&mut Recorder::new(None))).is_ok());
    assert_eq!(a.running_job(), Some(1));
}

#[test]
fn abandon_job_is_the_way_out_for_a_job_that_was_lost() {
    let mut e = oral();
    let Ok(job) = e.begin("fit.run", oral_params()) else {
        panic!("begin failed");
    };
    let id = job.id();
    drop(job); // the host lost it
    assert_eq!(e.running_job(), Some(id));
    let refusal = e.execute("project.new", Value::Null).err();
    assert!(refusal.is_some_and(|r| r.code == "job_running"));
    let entries = e.history().entries().to_vec();
    let project = project_text(&e);
    assert!(!e.abandon_job(id + 1), "another number changes nothing");
    assert_eq!(e.running_job(), Some(id));
    assert!(e.abandon_job(id));
    assert!(!e.abandon_job(id), "nothing left to clear");
    assert_eq!(e.running_job(), None);
    assert_eq!(
        e.history().entries(),
        entries.as_slice(),
        "nothing recorded"
    );
    assert_eq!(project_text(&e), project);
    ok(
        &mut e,
        "nca.run",
        json!({ "worksheet": 1, "route": "extravascular" }),
    );
    // A lost `Completed` is the same: abandoned, then the next job is numbered on.
    let Ok(job) = e.begin("fit.run", oral_params()) else {
        panic!("begin failed");
    };
    let lost = job.run(&mut Recorder::new(None));
    assert!(e.abandon_job(2));
    assert!(
        e.finish(lost).is_err(),
        "an abandoned job cannot be finished"
    );
    assert_eq!(
        e.history().entries().len(),
        entries.len() + 1,
        "only the nca.run"
    );
}

#[test]
fn a_clone_made_during_a_job_has_no_running_job_and_its_own_identity() {
    let mut e = oral();
    let Ok(job) = e.begin("fit.run", oral_params()) else {
        panic!("begin failed");
    };
    let mut copy = e.clone();
    assert_eq!(copy.running_job(), None);
    ok(&mut copy, "project.describe", Value::Null);
    ok(&mut copy, "fit.run", oral_params());
    // The job of the original cannot be finished on the copy, even if it were running a job 1.
    let Ok(job_copy) = oral().begin("fit.run", oral_params()) else {
        panic!("begin failed");
    };
    drop(job_copy);
    let done = job.run(&mut Recorder::new(None));
    let refusal = copy.finish(done).err();
    assert!(refusal.is_some_and(|r| r.code == "not_the_running_job"));
    assert_eq!(e.running_job(), Some(1), "the original still runs its job");
}
