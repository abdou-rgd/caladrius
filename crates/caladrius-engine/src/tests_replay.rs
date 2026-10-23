//! `project.replay`: a saved project is run again from its history and checked against its seals.

use serde_json::{Value, json};

use crate::replay::{Report, Status, Verdict};
use crate::schema::validate;
use crate::seal::ENGINE_VERSION;
use crate::*;

const CSV: &str = "Subject,Time (h),Conc (mg/L),Dose (mg)\n\
A,0,0,100\nA,0.5,2.195,100\nA,1,3.293,100\nA,2,3.971,100\nA,4,3.611,100\nA,8,2.451,100\nA,24,0.495,100\n\
B,0,0,100\nB,0.5,1.8,100\nB,1,2.9,100\nB,2,3.5,100\nB,4,3.2,100\nB,8,2.1,100\nB,24,0.4,100\n";

fn ok(engine: &mut Engine, id: &str, params: Value) -> Value {
    match engine.execute(id, params) {
        Ok(v) => v,
        Err(e) => panic!("{id} failed: {e}"),
    }
}

fn nca() -> Value {
    json!({ "worksheet": 1, "route": "extravascular" })
}

/// What a person does before saving: import, analyse, fit, simulate, edit a cell, analyse again,
/// export, make a mistake, and look at the project and its history.
fn session(engine: &mut Engine) {
    ok(
        engine,
        "data.import",
        json!({ "name": "study", "csv": CSV }),
    );
    ok(engine, "nca.run", nca());
    ok(
        engine,
        "fit.run",
        json!({ "worksheet": 1, "subject": "A", "model": "pk1.oral_1" }),
    );
    ok(
        engine,
        "model.simulate",
        json!({
            "model": "pk1.iv_bolus", "dose": 100, "params": { "v": 10, "k": 0.1 },
            "times": [0, 1, 2, 5, 10]
        }),
    );
    ok(
        engine,
        "data.set_cell",
        json!({ "worksheet": 1, "column": "Conc", "row": 3, "value": 4.0 }),
    );
    ok(engine, "nca.run", nca());
    ok(
        engine,
        "export.table",
        json!({ "table": "nca.parameters", "analysis": 4 }),
    );
    let _ = engine.execute("nca.run", json!({ "worksheet": 9 }));
    ok(engine, "project.describe", Value::Null);
    ok(engine, "project.save", Value::Null);
    ok(engine, "history.list", json!({ "from": 2, "limit": 3 }));
}

/// The saved file of a session, as a JSON document.
fn saved_session() -> Value {
    let mut engine = Engine::new();
    engine.set_clock(|| Some(1_000_000_000));
    session(&mut engine);
    saved(&mut engine)
}

fn saved(engine: &mut Engine) -> Value {
    let bytes = engine.save_bytes().unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// What a verifier says about a document: a fresh engine that knows nothing else.
fn replay_of(document: &Value) -> Report {
    let mut verifier = Engine::new();
    let answer = ok(
        &mut verifier,
        "project.replay",
        json!({ "text": document.to_string() }),
    );
    report(&answer)
}

fn report(answer: &Value) -> Report {
    let schema = describe()
        .into_iter()
        .find(|c| c.id == "project.replay")
        .map(|c| c.result_schema)
        .unwrap();
    validate::check(&schema, &schema, answer).unwrap_or_else(|e| panic!("{e}\n{answer}"));
    serde_json::from_value(answer.clone()).unwrap()
}

fn statuses(report: &Report) -> Vec<Status> {
    report.steps.iter().map(|s| s.status).collect()
}

fn history_mut(document: &mut Value) -> &mut Vec<Value> {
    document["history"].as_array_mut().unwrap()
}

/// A CSV of `subjects` profiles of 40 points (a rise and a fall, with only the four operations:
/// the tests of this crate do not use the functions whose rounding depends on the platform).
fn wide_csv(subjects: usize) -> String {
    let mut csv = String::from("Subject,Time (h),Conc (mg/L),Dose (mg)\n");
    for s in 1..=subjects {
        for t in 0..40 {
            let x = f64::from(t) * 0.5;
            let c = 8.0 * s as f64 / 7.0 * x / (1.0 + 0.1 * x * x);
            csv.push_str(&format!("S{s},{x},{c:.3},100\n"));
        }
    }
    csv
}

#[test]
fn a_saved_session_replays_and_every_seal_is_reproduced() {
    let document = saved_session();
    let n = document["history"].as_array().unwrap().len();
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Verified, "{}", r.summary);
    assert_eq!(r.steps.len(), n);
    assert_eq!(r.counts.matched as usize, n);
    assert_eq!(
        (r.counts.diverged, r.counts.no_seal, r.counts.not_replayable),
        (0, 0, 0)
    );
    assert!(r.first_divergence.is_none());
    for (i, step) in r.steps.iter().enumerate() {
        assert_eq!(step.seq as usize, i + 1);
        assert_eq!(step.status, Status::Match, "{step:?}");
        assert!(step.stored.is_some() && step.stored == step.recomputed);
        let stored = &document["history"][i]["seal"]["sha256"];
        assert_eq!(stored.as_str(), step.stored.as_deref());
    }
    // A failed step is replayed too: its seal is made of its error code.
    let failed = r.steps.iter().find(|s| s.command == "nca.run" && s.seq > 6);
    assert!(failed.is_some_and(|s| s.status == Status::Match));
    // The steps that echo the history see the history they saw the first time.
    for command in ["project.save", "history.list"] {
        let step = r.steps.iter().find(|s| s.command == command).unwrap();
        assert_eq!(step.status, Status::Match, "{command}");
    }
    // The project the steps make is the stored project.
    assert_eq!(r.state.matches, Some(true));
    assert_eq!(r.state.stored, r.state.replayed);
    assert!(r.state.differing.is_empty() && r.state.note.is_none());
    assert!(
        r.summary
            .starts_with(&format!("All {n} steps were replayed"))
    );
    // Who sealed and who replayed.
    assert_eq!(
        r.engine.as_ref().and_then(|e| e.version.as_deref()),
        Some(ENGINE_VERSION)
    );
    assert_eq!(r.sealed_by.len(), 1);
    assert_eq!(r.sealed_by[0].version.as_deref(), Some(ENGINE_VERSION));
}

#[test]
fn the_document_can_be_given_as_text_or_as_json_and_replays_the_same() {
    let document = saved_session();
    let mut a = Engine::new();
    let mut b = Engine::new();
    let by_text = ok(
        &mut a,
        "project.replay",
        json!({ "text": document.to_string() }),
    );
    let by_value = ok(&mut b, "project.replay", json!({ "project": document }));
    assert_eq!(by_text, by_value);
    // Giving both is refused, and giving a text that is not a project says so.
    let e = a
        .execute("project.replay", json!({ "text": "{}", "project": {} }))
        .unwrap_err();
    assert_eq!(e.code, "invalid_parameters");
    let e = a
        .execute("project.replay", json!({ "text": "not json" }))
        .unwrap_err();
    assert_eq!(e.code, "load_failed");
}

#[test]
fn the_session_itself_can_be_replayed_without_a_file() {
    let mut engine = Engine::new();
    session(&mut engine);
    let n = engine.history().len();
    let first = report(&ok(&mut engine, "project.replay", json!({})));
    assert_eq!(first.verdict, Verdict::Verified, "{}", first.summary);
    assert_eq!(first.steps.len(), n);
    // The replay is in the history, with no seal: it is the check, not a result.
    let last = engine.history().entries().last().unwrap();
    assert_eq!(last.command, "project.replay");
    assert!(last.seal.is_none() && last.ok);
    // Replayed again, it is a step like the others that carries nothing to compare, and it is
    // not run again.
    let second = report(&ok(&mut engine, "project.replay", Value::Null));
    assert_eq!(second.verdict, Verdict::Verified, "{}", second.summary);
    assert_eq!(second.steps.len(), n + 1);
    let step = second.steps.last().unwrap();
    assert_eq!(step.command, "project.replay");
    assert_eq!(step.status, Status::NoSeal);
    assert_eq!(second.counts.no_seal, 1);
    assert!(
        second.summary.contains("1 step has no seal"),
        "{}",
        second.summary
    );
}

#[test]
fn a_replay_changes_nothing_in_the_session_that_asks() {
    let document = saved_session();
    let mut engine = Engine::new();
    ok(
        &mut engine,
        "data.import",
        json!({ "name": "mine", "csv": "time,conc\n0,0\n1,5\n" }),
    );
    let project = engine.project().clone();
    let revision = engine.revision();
    let before = engine.history().entries().to_vec();
    ok(
        &mut engine,
        "project.replay",
        json!({ "text": document.to_string() }),
    );
    assert_eq!(engine.project(), &project);
    assert_eq!(engine.revision(), revision);
    assert_eq!(
        &engine.history().entries()[..before.len()],
        before.as_slice()
    );
    assert_eq!(engine.history().len(), before.len() + 1);
}

#[test]
fn a_one_cell_edit_of_a_stored_worksheet_is_detected() {
    let mut document = saved_session();
    let cell = &mut document["worksheets"][0]["columns"][2]["data"]["number"][5];
    let before = cell.as_f64().unwrap();
    *cell = json!(before + 0.001);
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Diverged, "{}", r.summary);
    // The history is intact, so every step reproduces its seal, and the stored project does not.
    assert_eq!(r.counts.matched as usize, r.steps.len());
    assert!(r.first_divergence.is_none());
    assert_eq!(r.state.matches, Some(false));
    assert_ne!(r.state.stored, r.state.replayed);
    assert!(
        r.state.differing.iter().any(|d| d == "worksheet 1"),
        "{:?}",
        r.state.differing
    );
    assert!(r.summary.contains("worksheet 1"), "{}", r.summary);
}

#[test]
fn a_one_cell_edit_of_the_data_in_the_history_is_the_first_divergence() {
    let mut document = saved_session();
    let params = &mut history_mut(&mut document)[0]["params"];
    let csv = params["csv"].as_str().unwrap().to_owned();
    let edited = csv.replacen("A,1,3.293,100", "A,1,3.294,100", 1);
    assert_ne!(csv, edited);
    params["csv"] = json!(edited);
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Diverged, "{}", r.summary);
    let first = r.first_divergence.clone().unwrap();
    assert_eq!((first.seq, first.command.as_str()), (1, "data.import"));
    assert_eq!(first.status, Status::Diverged);
    assert!(first.stored.is_some() && first.recomputed.is_some());
    assert_ne!(first.stored, first.recomputed);
    assert_eq!(r.steps[0], first);
    // What was computed from that data differs after it.
    assert!(r.counts.diverged > 1);
    assert!(
        r.summary
            .starts_with("Step 1 (data.import) does not reproduce its seal."),
        "{}",
        r.summary
    );
    // The summary holds no digest: what is read aloud is made of words and counts.
    assert!(!r.summary.contains(&first.stored.unwrap()[..12]));
}

#[test]
fn a_stored_result_that_was_edited_is_detected_in_the_project_and_not_in_the_steps() {
    let mut document = saved_session();
    // The Cmax of the first subject of the last NCA, as the application would show it.
    let analyses = document["analyses"].as_array_mut().unwrap();
    let nca = analyses
        .iter_mut()
        .rev()
        .find(|a| a["spec"]["kind"] == "nca")
        .unwrap();
    let id = nca["id"].as_u64().unwrap();
    let parameters = &mut nca["stored"]["result"]["subjects"][0]["outcome"]["ok"]["parameters"];
    let cmax = parameters
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["name"] == "cmax")
        .unwrap();
    cmax["value"]["value"] = json!(9.99);
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Diverged);
    assert!(r.first_divergence.is_none(), "no step differs");
    assert_eq!(r.state.differing, [format!("analysis {id}")]);
}

/// The text of a seal with its first digit changed.
fn flipped(seal: &str) -> String {
    let first = if seal.starts_with('0') { "1" } else { "0" };
    format!("{first}{}", &seal[1..])
}

#[test]
fn a_changed_seal_is_a_divergence_at_its_step() {
    // The last step of the file: nothing later lists it.
    let mut document = saved_session();
    let last = history_mut(&mut document).last_mut().unwrap();
    let stored = flipped(last["seal"]["sha256"].as_str().unwrap());
    last["seal"]["sha256"] = json!(stored);
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Diverged);
    let first = r.first_divergence.clone().unwrap();
    assert_eq!(first.seq as usize, r.steps.len());
    assert_eq!(first.stored.as_deref(), Some(stored.as_str()));
    assert_ne!(first.stored, first.recomputed);
    assert_eq!(r.counts.diverged, 1);
    assert_eq!(r.counts.matched as usize, r.steps.len() - 1);
    assert_eq!(r.state.matches, Some(true), "the project itself is intact");
    assert!(first.note.is_some());
}

#[test]
fn a_save_seals_the_history_before_it_so_an_earlier_seal_cannot_change_unnoticed() {
    // Step 3 is a fit. Its seal is changed in the file: it diverges, and so do the steps that
    // list the history after it (the saves and the listing), because their answers hold the seal
    // of every entry they list.
    let mut document = saved_session();
    let seal = &mut history_mut(&mut document)[2]["seal"]["sha256"];
    *seal = json!(flipped(seal.as_str().unwrap()));
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Diverged);
    assert_eq!(r.first_divergence.as_ref().map(|s| s.seq), Some(3));
    let diverged: Vec<&str> = r
        .steps
        .iter()
        .filter(|s| s.status == Status::Diverged)
        .map(|s| s.command.as_str())
        .collect();
    assert_eq!(
        diverged,
        ["fit.run", "project.save", "history.list", "project.save"]
    );
    assert_eq!(r.state.matches, Some(true));
}

#[test]
fn a_changed_parameter_in_the_history_is_detected_though_the_answer_does_not_change() {
    // The options written in full are the default options: the answer of the command is the same
    // text, and the parameters, which are part of the seal, are not.
    let mut document = saved_session();
    let entries = history_mut(&mut document);
    let step = entries
        .iter_mut()
        .find(|e| e["command"] == "nca.run")
        .unwrap();
    step["params"]["options"] = json!({ "auc_method": "lin_up_log_down" });
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Diverged, "{}", r.summary);
    let first = r.first_divergence.unwrap();
    assert_eq!((first.seq, first.command.as_str()), (2, "nca.run"));
}

#[test]
fn a_project_saved_before_seals_replays_with_no_seal_recorded_per_step() {
    let mut document = saved_session();
    let n = document["history"].as_array().unwrap().len();
    for entry in history_mut(&mut document) {
        let map = entry.as_object_mut().unwrap();
        map.remove("seal");
        map.remove("at");
    }
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Unsealed, "{}", r.summary);
    assert_eq!(r.steps.len(), n);
    assert!(statuses(&r).iter().all(|s| *s == Status::NoSeal));
    assert!(r.steps.iter().all(|s| s.stored.is_none()));
    assert!(
        r.steps.iter().all(|s| s.recomputed.is_some()),
        "what the steps give now is shown"
    );
    assert_eq!(r.counts.no_seal as usize, n);
    assert!(r.first_divergence.is_none());
    // The project is still compared, and it is the one the steps make.
    assert_eq!(r.state.matches, Some(true));
    assert!(r.sealed_by.is_empty());
    assert!(
        r.summary.contains("none of them carries a seal"),
        "{}",
        r.summary
    );

    // Edited, the old format is still caught by the comparison of the project.
    let cell = &mut document["worksheets"][0]["columns"][2]["data"]["number"][2];
    *cell = json!(7.5);
    let edited = replay_of(&document);
    assert_eq!(edited.verdict, Verdict::Diverged);
    assert_eq!(edited.state.matches, Some(false));
}

#[test]
fn a_file_without_a_history_has_nothing_to_replay() {
    let mut document = saved_session();
    document.as_object_mut().unwrap().remove("history");
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Empty);
    assert!(r.steps.is_empty());
    assert_eq!(r.state.matches, None);
    assert!(r.state.note.is_some());
    assert!(r.summary.contains("nothing to replay"));
    // A session with nothing done is the same.
    let mut engine = Engine::new();
    let r = report(&ok(&mut engine, "project.replay", json!({})));
    assert_eq!(r.verdict, Verdict::Empty);
}

#[test]
fn seals_are_stable_across_a_save_and_a_reload() {
    let mut first = Engine::new();
    first.set_clock(|| Some(1_000_000_000));
    session(&mut first);
    let document = saved(&mut first);
    let bytes = serde_json::to_vec(&document).unwrap();
    let n = document["history"].as_array().unwrap().len();

    // Reloaded in another engine, then replayed there: the seals it carries are the same, and
    // what the same command seals now is the same.
    let mut second = Engine::new();
    let loaded = second.load_bytes(&bytes).unwrap();
    assert_eq!(loaded["history_restored"], true);
    let stored: Vec<Option<String>> = first
        .history()
        .entries()
        .iter()
        .map(|e| e.seal.as_ref().map(|s| s.sha256.clone()))
        .collect();
    let reloaded: Vec<Option<String>> = second
        .history()
        .entries()
        .iter()
        .take(n)
        .map(|e| e.seal.as_ref().map(|s| s.sha256.clone()))
        .collect();
    assert_eq!(reloaded, stored[..n]);
    // The simulation of the session, asked again after the reload, is sealed the same.
    let again = json!({
        "model": "pk1.iv_bolus", "dose": 100, "params": { "v": 10, "k": 0.1 },
        "times": [0, 1, 2, 5, 10]
    });
    ok(&mut second, "model.simulate", again);
    let simulated = second.history().entries().last().unwrap();
    let original = first
        .history()
        .entries()
        .iter()
        .find(|e| e.command == "model.simulate")
        .unwrap();
    assert_eq!(
        simulated.seal.as_ref().map(|s| &s.sha256),
        original.seal.as_ref().map(|s| &s.sha256)
    );

    // The reloaded session replays as a whole: the file, the load and the step after it.
    let r = report(&ok(&mut second, "project.replay", json!({})));
    assert_eq!(r.verdict, Verdict::Verified, "{}", r.summary);
    let load = r
        .steps
        .iter()
        .find(|s| s.command == "project.load")
        .unwrap();
    assert_eq!(load.status, Status::NoSeal);
    // And saved again, then replayed in a third engine.
    let second_file = saved(&mut second);
    let r = replay_of(&second_file);
    assert_eq!(r.verdict, Verdict::Verified, "{}", r.summary);
    assert_eq!(
        r.steps.len(),
        second_file["history"].as_array().unwrap().len()
    );
}

#[test]
fn steps_whose_parameters_were_not_kept_are_reported_and_what_depends_on_them_is_blocked() {
    let big = wide_csv(100);
    assert!(big.len() > MAX_RECORDED_PARAMS_BYTES);
    let mut engine = Engine::new();
    ok(
        &mut engine,
        "data.import",
        json!({ "name": "big", "csv": big }),
    );
    ok(&mut engine, "nca.run", nca());
    ok(&mut engine, "data.describe", json!({ "worksheet": 1 }));
    let document = saved(&mut engine);
    assert_eq!(document["history"][0]["params"]["omitted"], true);
    let r = replay_of(&document);
    assert_eq!(r.verdict, Verdict::Incomplete, "{}", r.summary);
    assert_eq!(r.steps[0].status, Status::NotReplayable);
    assert!(
        r.steps[0]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("too large")),
        "{:?}",
        r.steps[0]
    );
    assert!(r.steps[0].stored.is_some() && r.steps[0].recomputed.is_none());
    assert!(r.steps[1..].iter().all(|s| s.status == Status::Blocked));
    assert_eq!(r.counts.not_replayable, 1);
    assert_eq!(r.counts.blocked as usize, r.steps.len() - 1);
    assert!(r.first_divergence.is_none(), "nothing was found to differ");
    assert_eq!(r.state.matches, None);
    assert!(
        r.state
            .note
            .as_deref()
            .is_some_and(|n| n.contains("step 1"))
    );
    // The input hash is the way to check the file itself.
    assert_eq!(
        document["history"][0]["seal"]["input"].as_str(),
        Some(crate::seal::sha256_hex(big.as_bytes()).as_str())
    );
}

#[test]
fn a_step_that_only_reads_with_large_parameters_does_not_stop_the_replay() {
    let big = wide_csv(100);
    assert!(big.len() > MAX_RECORDED_PARAMS_BYTES);
    let mut engine = Engine::new();
    ok(
        &mut engine,
        "data.import",
        json!({ "name": "study", "csv": CSV }),
    );
    ok(&mut engine, "data.preview", json!({ "csv": big }));
    ok(&mut engine, "nca.run", nca());
    let document = saved(&mut engine);
    assert_eq!(document["history"][1]["params"]["omitted"], true);
    let r = replay_of(&document);
    assert_eq!(
        statuses(&r),
        [
            Status::Match,
            Status::NotReplayable,
            Status::Match,
            Status::Match
        ]
    );
    assert_eq!(r.verdict, Verdict::Incomplete);
    assert_eq!(r.state.matches, Some(true), "the project is still compared");
    assert!(
        r.summary.contains("could not be run again"),
        "{}",
        r.summary
    );
}

#[test]
fn a_project_opened_from_a_file_replays_from_the_history_the_file_carried() {
    // The file is small enough to be kept whole in the history, or too large for that.
    for (label, csv) in [("small", CSV.to_owned()), ("large", wide_csv(40))] {
        let mut first = Engine::new();
        ok(
            &mut first,
            "data.import",
            json!({ "name": "study", "csv": csv }),
        );
        ok(&mut first, "nca.run", nca());
        let file = serde_json::to_vec(&saved(&mut first)).unwrap();

        // Opened, extended, saved, and replayed from the second file.
        let mut second = Engine::new();
        second.load_bytes(&file).unwrap();
        let load = second.history().entries().last().unwrap();
        assert_eq!(load.command, "project.load");
        let kept = load.params.get("omitted").is_none();
        assert_eq!(
            kept,
            label == "small",
            "{label}: only a small file is kept whole"
        );
        ok(&mut second, "data.describe", json!({ "worksheet": 1 }));
        let r = replay_of(&saved(&mut second));
        assert_eq!(r.verdict, Verdict::Verified, "{label}: {}", r.summary);
        let opened = r
            .steps
            .iter()
            .find(|s| s.command == "project.load")
            .unwrap();
        assert_eq!(opened.status, Status::NoSeal, "{label}");
        assert!(opened.stored.is_none());
        assert_eq!(r.state.matches, Some(true), "{label}");
        assert_eq!(
            r.summary.contains("is not in the history"),
            !kept,
            "{label}: {}",
            r.summary
        );

        // The file was edited by one cell before it was opened.
        let mut tampered: Value = serde_json::from_slice(&file).unwrap();
        tampered["worksheets"][0]["columns"][2]["data"]["number"][1] = json!(123.456);
        let mut third = Engine::new();
        third
            .load_bytes(&serde_json::to_vec(&tampered).unwrap())
            .unwrap();
        ok(&mut third, "data.describe", json!({ "worksheet": 1 }));
        let r = replay_of(&saved(&mut third));
        assert_eq!(r.verdict, Verdict::Diverged, "{label}: {}", r.summary);
        if kept {
            // The file is in the history: the step that opened it says what is not as it was.
            let first = r.first_divergence.unwrap();
            assert_eq!(first.command, "project.load", "{label}");
            assert!(
                first
                    .note
                    .as_deref()
                    .is_some_and(|n| n.contains("worksheet 1")),
                "{label}: {first:?}"
            );
            assert_eq!(
                r.state.matches,
                Some(true),
                "{label}: it goes on from the file"
            );
        } else {
            // It is not: the steps after it ran on another project than the one they are
            // replayed on, which the project at the end shows, and the report says where the
            // trust was.
            assert_eq!(r.state.matches, Some(false), "{label}");
            assert!(
                r.summary.contains("is not in the history"),
                "{label}: {}",
                r.summary
            );
        }
    }
}

#[test]
fn a_project_opened_from_a_file_without_a_history_can_only_be_replayed_if_the_file_was_kept() {
    // A document without history, opened: the load is the first step and the only source of
    // the data.
    let mut first = Engine::new();
    ok(
        &mut first,
        "data.import",
        json!({ "name": "study", "csv": CSV }),
    );
    let mut plain = saved(&mut first);
    plain.as_object_mut().unwrap().remove("history");
    let mut small = Engine::new();
    small
        .load_bytes(&serde_json::to_vec(&plain).unwrap())
        .unwrap();
    ok(&mut small, "nca.run", nca());
    let r = replay_of(&saved(&mut small));
    assert_eq!(r.verdict, Verdict::Verified, "{}", r.summary);
    assert_eq!(r.steps[0].command, "project.load");
    assert_eq!(r.steps[0].status, Status::NoSeal);

    // The same with a file too large to be kept in the history: nothing to start from.
    let mut wide = Engine::new();
    ok(
        &mut wide,
        "data.import",
        json!({ "name": "wide", "csv": wide_csv(100) }),
    );
    let mut plain = saved(&mut wide);
    plain.as_object_mut().unwrap().remove("history");
    let mut large = Engine::new();
    large
        .load_bytes(&serde_json::to_vec(&plain).unwrap())
        .unwrap();
    assert_eq!(large.history().entries()[0].params["omitted"], true);
    ok(&mut large, "data.describe", json!({ "worksheet": 1 }));
    let r = replay_of(&saved(&mut large));
    assert_eq!(r.verdict, Verdict::Incomplete, "{}", r.summary);
    assert_eq!(r.steps[0].status, Status::NotReplayable);
    assert_eq!(r.steps[1].status, Status::Blocked);
}

#[test]
fn a_damaged_history_or_a_wrong_file_is_refused_with_its_code() {
    let mut engine = Engine::new();
    let mut run = |params: Value| engine.execute("project.replay", params).unwrap_err().code;

    let mut gap = saved_session();
    history_mut(&mut gap)[1]["seq"] = json!(7);
    assert_eq!(run(json!({ "project": gap })), "history_damaged");

    let mut unreadable = saved_session();
    history_mut(&mut unreadable)[0]
        .as_object_mut()
        .unwrap()
        .remove("command");
    assert_eq!(run(json!({ "project": unreadable })), "history_damaged");

    let mut not_a_list = saved_session();
    not_a_list["history"] = json!("nothing");
    assert_eq!(run(json!({ "project": not_a_list })), "history_damaged");

    let mut other = saved_session();
    other["format"] = json!("something-else");
    assert_eq!(run(json!({ "project": other })), "load_wrong_format");

    let mut newer = saved_session();
    newer["format_version"] = json!(99);
    assert_eq!(run(json!({ "project": newer })), "load_newer_version");

    assert_eq!(run(json!({ "text": "[1, 2" })), "load_failed");
    assert_eq!(run(json!({ "project": 5 })), "load_failed");
}

#[test]
fn the_report_says_which_engine_sealed_the_steps_and_which_one_replays_them() {
    let mut document = saved_session();
    for entry in history_mut(&mut document) {
        entry["seal"]["engine"] = json!("0.0.1");
        entry["seal"]["commit"] = json!("a".repeat(40));
    }
    let mut verifier = Engine::new();
    let r = report(&ok(
        &mut verifier,
        "project.replay",
        json!({ "project": document }),
    ));
    // The version is provenance, not part of the hash: another engine that computes the same
    // bits verifies the file.
    assert_eq!(r.verdict, Verdict::Verified, "{}", r.summary);
    assert_eq!(r.sealed_by.len(), 1);
    assert_eq!(r.sealed_by[0].version.as_deref(), Some("0.0.1"));
    assert_eq!(
        r.sealed_by[0].commit.as_deref(),
        Some("a".repeat(40).as_str())
    );
    let this = r.engine.unwrap();
    assert_eq!(this.version.as_deref(), Some(ENGINE_VERSION));
    assert_eq!(this.commit.as_deref(), seal::GIT_COMMIT);

    // An engine that leaves its version out of its seals leaves it out of the report too.
    let mut plain = Engine::new();
    plain.set_provenance(false);
    let answer = ok(
        &mut plain,
        "project.replay",
        json!({ "project": saved_session() }),
    );
    assert!(answer.get("engine").is_none());
}

#[test]
fn a_seal_of_another_format_is_reported_and_not_compared() {
    let mut document = saved_session();
    // The last entry: no later step lists it, so only this step is concerned.
    history_mut(&mut document).last_mut().unwrap()["seal"]["format"] = json!(2);
    let r = replay_of(&document);
    let last = r.steps.last().unwrap();
    assert_eq!(last.status, Status::NoSeal);
    assert!(
        last.note.as_deref().is_some_and(|n| n.contains("format 2")),
        "{last:?}"
    );
    assert_eq!(r.verdict, Verdict::Verified, "{}", r.summary);
    assert_eq!(r.counts.no_seal, 1);
}

#[test]
fn a_replay_is_the_same_on_every_run() {
    let document = saved_session();
    let mut a = Engine::new();
    let mut b = Engine::new();
    let first = ok(&mut a, "project.replay", json!({ "project": document }));
    let second = ok(&mut b, "project.replay", json!({ "project": document }));
    assert_eq!(first, second);
    // And the report is plain data that reads back.
    let back: Report = serde_json::from_value(first.clone()).unwrap();
    assert_eq!(serde_json::to_value(back).unwrap(), first);
}
