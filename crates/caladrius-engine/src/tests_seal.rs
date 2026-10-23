//! Seals through the engine: every command that ran has one, it is the same on every run, the
//! data a command read is hashed beside it, the time sits beside it and never inside, and all of
//! it survives a save and a reload.

use serde_json::{Value, json};

use crate::schema::validate;
use crate::seal::{ENGINE_VERSION, sha256_hex};
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

/// import, nca.run, a model, an export, a save: a short session of a user.
fn session(engine: &mut Engine) {
    ok(
        engine,
        "data.import",
        json!({ "name": "study", "csv": CSV }),
    );
    ok(engine, "nca.run", nca());
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
        "export.table",
        json!({ "table": "nca.parameters", "analysis": 2 }),
    );
    let _ = engine.execute("nca.run", json!({ "worksheet": 9 }));
    ok(engine, "project.describe", Value::Null);
}

fn seals(engine: &Engine) -> Vec<Option<String>> {
    engine
        .history()
        .entries()
        .iter()
        .map(|e| e.seal.as_ref().map(|s| s.sha256.clone()))
        .collect()
}

fn seal_at(engine: &Engine, seq: usize) -> Seal {
    engine
        .history()
        .entries()
        .get(seq - 1)
        .and_then(|e| e.seal.clone())
        .unwrap()
}

#[test]
fn every_command_that_ran_has_a_seal_except_the_load() {
    let mut engine = Engine::new();
    for command in describe() {
        let _ = engine.execute(&command.id, command.example.clone());
    }
    assert_eq!(engine.history().len(), command_ids().len());
    for entry in engine.history().entries() {
        if entry.command == "project.load" {
            assert!(entry.seal.is_none(), "the load carries no seal");
            continue;
        }
        let seal = entry
            .seal
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no seal", entry.command));
        assert_eq!(seal.format, 1);
        assert_eq!(seal.sha256.len(), 64, "{}", entry.command);
        assert!(
            seal.sha256
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        );
        assert_eq!(seal.engine.as_deref(), Some(ENGINE_VERSION));
        assert_eq!(seal.commit.as_deref(), seal::GIT_COMMIT);
    }
    // history.list shows them and its answer matches its schema.
    let list = ok(&mut engine, "history.list", Value::Null);
    let schema = describe()
        .into_iter()
        .find(|c| c.id == "history.list")
        .map(|c| c.result_schema)
        .unwrap();
    validate::check(&schema, &schema, &list).unwrap();
    assert!(list["entries"][0]["seal"]["sha256"].is_string());
    assert!(list["entries"][0].get("at").is_none(), "no clock, no time");
}

#[test]
fn the_same_session_gives_the_same_seals_on_any_engine() {
    let mut first = Engine::new();
    session(&mut first);
    let mut second = Engine::new();
    session(&mut second);
    assert_eq!(seals(&first), seals(&second));
    assert_eq!(first.history(), second.history());

    // Neither the provenance nor the clock is in the hash.
    let mut plain = Engine::new();
    plain.set_provenance(false);
    plain.set_clock(|| Some(1_000_000_000));
    session(&mut plain);
    assert_eq!(seals(&first), seals(&plain));
    let entry = plain.history().entries().first().unwrap();
    let seal = entry.seal.as_ref().unwrap();
    assert_eq!(
        (seal.engine.as_deref(), seal.commit.as_deref()),
        (None, None)
    );
    assert_eq!(entry.at.as_deref(), Some("2001-09-09T01:46:40Z"));
    assert!(seal.input.is_some(), "the input is deterministic: it stays");

    // Another parameter, or another command, is another seal.
    let mut other = Engine::new();
    ok(
        &mut other,
        "data.import",
        json!({ "name": "study", "csv": CSV }),
    );
    ok(
        &mut other,
        "nca.run",
        json!({ "worksheet": 1, "route": "extravascular", "options": { "auc_method": "linear" } }),
    );
    assert_eq!(seals(&other).first(), seals(&first).first());
    assert_ne!(seals(&other).get(1), seals(&first).get(1));
}

#[test]
fn a_failure_is_sealed_by_its_code_and_so_is_an_unknown_command() {
    let mut a = Engine::new();
    let mut b = Engine::new();
    for engine in [&mut a, &mut b] {
        assert!(
            engine
                .execute("nca.run", json!({ "worksheet": 9 }))
                .is_err()
        );
        assert!(engine.execute("no.such", Value::Null).is_err());
    }
    assert_eq!(seals(&a), seals(&b));
    let entries = a.history().entries();
    assert!(entries.iter().all(|e| !e.ok && e.seal.is_some()));
    // The same failure for other parameters is another seal.
    let mut c = Engine::new();
    assert!(c.execute("nca.run", json!({ "worksheet": 8 })).is_err());
    assert_ne!(seals(&a).first(), seals(&c).first());
}

#[test]
fn a_refusal_before_any_command_ran_has_nothing_to_seal() {
    let mut engine = Engine::new();
    assert!(engine.execute_text("nca.run", "{not json").is_err());
    assert!(engine.import_csv("bad", &[0xff, 0xfe, 0x41]).is_err());
    assert!(engine.load_bytes(&[0xff, 0xfe]).is_err());
    assert_eq!(engine.history().len(), 3);
    assert!(
        engine
            .history()
            .entries()
            .iter()
            .all(|e| !e.ok && e.seal.is_none())
    );
}

#[test]
fn the_input_is_the_hash_of_the_csv_for_an_import_and_of_the_worksheets_for_the_rest() {
    let mut engine = Engine::new();
    ok(&mut engine, "data.preview", json!({ "csv": CSV }));
    ok(
        &mut engine,
        "data.import",
        json!({ "name": "study", "csv": CSV }),
    );
    let csv_hash = sha256_hex(CSV.as_bytes());
    assert_eq!(
        seal_at(&engine, 1).input.as_deref(),
        Some(csv_hash.as_str())
    );
    assert_eq!(
        seal_at(&engine, 2).input.as_deref(),
        Some(csv_hash.as_str())
    );

    ok(&mut engine, "nca.run", nca());
    ok(&mut engine, "nca.run", nca());
    let before = seal_at(&engine, 3).input.unwrap();
    assert_eq!(before.len(), 64);
    assert_ne!(before, csv_hash, "a worksheet is not its CSV text");
    assert_eq!(seal_at(&engine, 4).input.as_deref(), Some(before.as_str()));
    // An analysis is read through its worksheet.
    ok(&mut engine, "analysis.get", json!({ "analysis": 2 }));
    assert_eq!(seal_at(&engine, 5).input.as_deref(), Some(before.as_str()));

    // The input is what the command read, before it ran: an edit is sealed with the data it changed.
    ok(
        &mut engine,
        "data.set_cell",
        json!({ "worksheet": 1, "column": "Conc", "row": 2, "value": 9.5 }),
    );
    assert_eq!(seal_at(&engine, 6).input.as_deref(), Some(before.as_str()));
    ok(&mut engine, "nca.run", nca());
    let after = seal_at(&engine, 7).input.unwrap();
    assert_ne!(after, before, "one cell changed, the input changed");

    // Commands that read no stored data have none.
    ok(&mut engine, "project.describe", Value::Null);
    ok(&mut engine, "history.list", Value::Null);
    assert_eq!(seal_at(&engine, 8).input, None);
    assert_eq!(seal_at(&engine, 9).input, None);
    ok(
        &mut engine,
        "model.simulate",
        json!({ "model": "pk1.iv_bolus", "dose": 1, "params": { "v": 1, "k": 1 }, "times": [0, 1] }),
    );
    assert_eq!(seal_at(&engine, 10).input, None);
}

#[test]
fn the_seal_does_not_depend_on_the_order_of_the_keys_of_the_parameters() {
    let mut a = Engine::new();
    let mut b = Engine::new();
    ok(
        &mut a,
        "data.import",
        json!({ "name": "study", "csv": CSV }),
    );
    ok(
        &mut b,
        "data.import",
        json!({ "csv": CSV, "name": "study" }),
    );
    assert_eq!(seals(&a), seals(&b));
}

#[test]
fn large_parameters_are_sealed_whole_though_the_history_keeps_a_digest() {
    let mut csv = String::from("Time (h),Conc (mg/L),Dose (mg)\n");
    for i in 0..9000 {
        csv.push_str(&format!("{i},{},100\n", i % 7));
    }
    assert!(csv.len() > MAX_RECORDED_PARAMS_BYTES);
    let mut a = Engine::new();
    ok(&mut a, "data.import", json!({ "name": "big", "csv": csv }));
    let entry = a.history().entries().first().unwrap();
    assert_eq!(entry.params["omitted"], true);
    assert_eq!(
        entry.seal.as_ref().and_then(|s| s.input.as_deref()),
        Some(sha256_hex(csv.as_bytes()).as_str()),
        "the input hash is the way to check the file that was imported"
    );
    // One digit changed in the middle of the text: another seal.
    let changed = csv.replacen("\n3000,4,100\n", "\n3000,5,100\n", 1);
    assert_ne!(changed, csv);
    let mut b = Engine::new();
    ok(
        &mut b,
        "data.import",
        json!({ "name": "big", "csv": changed }),
    );
    assert_ne!(seals(&a), seals(&b));
}

#[test]
fn the_time_is_beside_the_seal_and_only_when_there_is_a_clock() {
    let mut engine = Engine::new();
    ok(&mut engine, "project.describe", Value::Null);
    assert!(engine.history().entries()[0].at.is_none());
    engine.set_clock(|| Some(951_782_400));
    ok(&mut engine, "project.describe", Value::Null);
    engine.set_clock(|| None);
    ok(&mut engine, "project.describe", Value::Null);
    let entries = engine.history().entries();
    assert_eq!(entries[1].at.as_deref(), Some("2000-02-29T00:00:00Z"));
    assert!(
        entries[2].at.is_none(),
        "a clock that does not know the time"
    );
}

#[test]
fn a_save_is_sealed_without_the_times_of_the_entries_it_lists() {
    let run = |clock: Clock| {
        let mut engine = Engine::new();
        engine.set_clock(clock);
        session(&mut engine);
        ok(&mut engine, "project.save", Value::Null);
        ok(&mut engine, "history.list", Value::Null);
        engine
    };
    let early = run(|| Some(1_000_000_000));
    let late = run(|| Some(1_900_000_000));
    assert_ne!(
        early.history().entries()[0].at,
        late.history().entries()[0].at
    );
    assert_eq!(seals(&early), seals(&late));
}

#[test]
fn seals_and_times_are_saved_and_come_back_the_same_after_a_reload() {
    let mut engine = Engine::new();
    engine.set_clock(|| Some(1_000_000_000));
    session(&mut engine);
    let bytes = engine.save_bytes().unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    let saved = document["history"].as_array().unwrap();
    assert_eq!(saved.len(), engine.history().len());
    assert!(saved.iter().all(|e| e["seal"]["sha256"].is_string()));
    assert!(saved.iter().all(|e| e["at"] == "2001-09-09T01:46:40Z"));

    let mut fresh = Engine::new();
    let loaded = fresh.load_bytes(&bytes).unwrap();
    assert_eq!(loaded["history_restored"], true);
    let n = engine.history().len();
    assert_eq!(
        fresh.history().entries()[..n],
        engine.history().entries()[..n]
    );
    assert_eq!(seals(&fresh)[..n], seals(&engine)[..n]);
    // Saved again, the same entries are written the same.
    let again = fresh.save_bytes().unwrap();
    let again: Value = serde_json::from_slice(&again).unwrap();
    assert_eq!(again["history"].as_array().unwrap()[..n], saved[..n]);
}

#[test]
fn a_project_saved_before_seals_existed_loads_with_entries_that_have_none() {
    let mut engine = Engine::new();
    session(&mut engine);
    let bytes = engine.save_bytes().unwrap();
    let mut document: Value = serde_json::from_slice(&bytes).unwrap();
    for entry in document["history"].as_array_mut().unwrap() {
        let map = entry.as_object_mut().unwrap();
        map.remove("seal");
        map.remove("at");
    }
    let mut fresh = Engine::new();
    let loaded = ok(&mut fresh, "project.load", json!({ "project": document }));
    assert_eq!(loaded["history_restored"], true);
    assert_eq!(fresh.project(), engine.project());
    let n = engine.history().len();
    assert!(
        fresh.history().entries()[..n]
            .iter()
            .all(|e| e.seal.is_none() && e.at.is_none())
    );
    // What is done next is sealed.
    ok(&mut fresh, "project.describe", Value::Null);
    assert!(fresh.history().entries().last().unwrap().seal.is_some());
}

#[test]
fn an_import_skips_the_stamp_line_of_an_export_and_seals_the_text_it_was_given() {
    let stamp = seal::stamp_line("export.table", &seal_of_nothing());
    let stamped = seal::csv_with_stamp(CSV, &stamp);
    let mut plain = Engine::new();
    let mut with_stamp = Engine::new();
    let a = ok(
        &mut plain,
        "data.import",
        json!({ "name": "w", "csv": CSV }),
    );
    let b = ok(
        &mut with_stamp,
        "data.import",
        json!({ "name": "w", "csv": stamped }),
    );
    assert_eq!(a, b);
    assert_eq!(plain.project(), with_stamp.project());
    assert_ne!(
        seals(&plain),
        seals(&with_stamp),
        "another text, another seal"
    );
    let preview = ok(&mut with_stamp, "data.preview", json!({ "csv": stamped }));
    let expected = ok(&mut plain, "data.preview", json!({ "csv": CSV }));
    assert_eq!(preview, expected);
}

fn seal_of_nothing() -> Seal {
    Seal {
        format: 1,
        sha256: "0".repeat(64),
        engine: None,
        commit: None,
        input: None,
    }
}
