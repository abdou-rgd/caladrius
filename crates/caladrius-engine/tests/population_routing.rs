#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: helpers panic on purpose
//! T-163: the dataset routed into `nca.run` and `fit.run` (specs/data.md DAT-USE-01 to 03 and
//! DAT-ERR-08, specs/fit.md FIT-ML-20).
//!
//! Every expected value comes from a path that already existed and was tested on its own:
//! - the NCA of the event layout is compared, byte for byte, with the NCA of the same table in the
//!   profile layout, which still goes through the old worksheet path (`legacy_regression` pins it);
//! - the fits of `subject = all` are compared with the fit of each subject named alone;
//! - the unaffected subjects of a refused table are compared with the same table without the
//!   affected subjects.
//!
//! The files are `oracle/data/` and `oracle/data/population/`; the refusal cases are
//! `oracle/expected/data/refuse_*.json` (only read here).

use caladrius_engine::Engine;
use caladrius_project::{CsvOptions, readings};
use serde_json::{Value, json};
use std::path::PathBuf;

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

fn read_file(relative: &str) -> String {
    let path = oracle().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn read_json(relative: &str) -> Value {
    serde_json::from_str(&read_file(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

fn ok(engine: &mut Engine, id: &str, params: Value) -> Value {
    match engine.execute(id, params) {
        Ok(v) => v,
        Err(e) => panic!("{id} failed: {e}"),
    }
}

/// An engine with the CSV text imported as worksheet 1, the named columns declared as covariates.
fn engine_with(csv: &str, covariates: &[&str]) -> Engine {
    let mut engine = Engine::new();
    let columns: Vec<Value> = covariates
        .iter()
        .map(|name| json!({ "name": name, "role": "covariate" }))
        .collect();
    ok(
        &mut engine,
        "data.import",
        json!({ "name": "t", "csv": csv, "columns": columns }),
    );
    engine
}

/// The subjects of an `nca.run` answer, as JSON.
fn nca_subjects(engine: &mut Engine, params: Value) -> Vec<Value> {
    let mut params = params;
    params["worksheet"] = json!(1);
    let answer = ok(engine, "nca.run", params);
    answer["result"]["subjects"]
        .as_array()
        .unwrap_or_else(|| panic!("no subjects in {answer}"))
        .clone()
}

/// The subjects of a `nca.run` answer by label.
fn by_subject(subjects: &[Value]) -> std::collections::BTreeMap<String, Value> {
    subjects
        .iter()
        .map(|s| (s["subject"].as_str().unwrap().to_owned(), s.clone()))
        .collect()
}

fn without(mut value: Value, keys: &[&str]) -> Value {
    if let Some(map) = value.as_object_mut() {
        for key in keys {
            map.remove(*key);
        }
    }
    value
}

// ---- DAT-USE-01: the NCA of the event layout equals the profile layout ---------------------------

/// The event layout of a profile-layout file `subject,time,conc,dose`: a dose row at time 0 and the
/// observation rows of each subject. The cells are copied as text, so a blank, a `<LOQ` and a
/// negative value read the same way in both tables.
fn as_event_table(profile: &str) -> String {
    let mut lines = profile.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    assert_eq!(header, ["subject", "time", "conc", "dose"]);
    let mut order: Vec<String> = Vec::new();
    let mut doses: std::collections::BTreeMap<String, String> = Default::default();
    let mut rows: std::collections::BTreeMap<String, Vec<(String, String)>> = Default::default();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let cells: Vec<&str> = line.split(',').collect();
        let subject = cells[0].to_owned();
        if !order.contains(&subject) {
            order.push(subject.clone());
            doses.insert(subject.clone(), cells[3].to_owned());
        }
        rows.entry(subject)
            .or_default()
            .push((cells[1].to_owned(), cells[2].to_owned()));
    }
    let mut out = String::from("ID,TIME,DV,AMT,EVID\n");
    for subject in &order {
        out.push_str(&format!("{subject},0,,{},1\n", doses[subject]));
        for (time, conc) in &rows[subject] {
            out.push_str(&format!("{subject},{time},{conc},,0\n"));
        }
    }
    out
}

/// A profile-layout file the old path reads whole: the doses of a subject are all the same.
fn doses_are_constant(profile: &str) -> bool {
    let mut seen: std::collections::BTreeMap<&str, &str> = Default::default();
    profile.lines().skip(1).all(|line| {
        let cells: Vec<&str> = line.split(',').collect();
        *seen.entry(cells[0]).or_insert(cells[3]) == cells[3]
    })
}

fn same_nca_in_both_layouts(profile: &str, event: &str, route: Value, what: &str) {
    let mut a = engine_with(profile, &[]);
    let mut b = engine_with(event, &[]);
    let old = nca_subjects(&mut a, json!({ "route": route }));
    let new = nca_subjects(&mut b, json!({ "route": route }));
    assert!(!old.is_empty(), "{what}: no subjects");
    assert_eq!(old.len(), new.len(), "{what}: number of subjects");
    for (o, n) in old.iter().zip(&new) {
        assert_eq!(
            serde_json::to_string(o).unwrap(),
            serde_json::to_string(n).unwrap(),
            "{what}: subject {}",
            o["subject"]
        );
    }
}

#[test]
fn the_nca_of_the_event_files_equals_the_profile_files_byte_for_byte() {
    // Theoph and Indometh as the oracle wrote them in the event layout.
    same_nca_in_both_layouts(
        &read_file("data/theoph.csv"),
        &read_file("data/population/event_theoph.csv"),
        json!("extravascular"),
        "theoph",
    );
    same_nca_in_both_layouts(
        &read_file("data/indometh.csv"),
        &read_file("data/population/event_indometh.csv"),
        json!("iv_bolus"),
        "indometh",
    );
}

#[test]
fn the_nca_of_every_multi_subject_profile_file_equals_its_event_layout() {
    // Every file under oracle/data whose header is `subject,time,conc,dose` and that holds more than
    // one subject, written as an event table: the cells are the same, so the results must be.
    let mut files = 0;
    for entry in std::fs::read_dir(oracle().join("data")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }
        let profile = std::fs::read_to_string(&path).unwrap();
        let subjects: std::collections::BTreeSet<&str> = profile
            .lines()
            .skip(1)
            .map(|l| l.split(',').next().unwrap())
            .collect();
        if subjects.len() < 2 || !doses_are_constant(&profile) {
            continue;
        }
        files += 1;
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        for route in [
            json!("extravascular"),
            json!("iv_bolus"),
            json!({ "iv_infusion": { "duration": 1.0 } }),
        ] {
            same_nca_in_both_layouts(&profile, &as_event_table(&profile), route, &name);
        }
    }
    assert!(files >= 8, "only {files} files were compared");
}

#[test]
fn the_route_column_of_the_event_layout_is_read_like_the_profile_layout() {
    let profile = read_file("data/population/profile_multi.csv");
    let event = "ID,TIME,DV,AMT,EVID,ROUTE\n\
1,0,,100,1,extravascular\n1,0.5,3.1,,0,\n1,2,5.8,,0,\n1,8,2.0,,0,\n\
2,0,,120,1,extravascular\n2,0.5,2.7,,0,\n2,2,5.2,,0,\n2,8,<0.5,,0,\n\
3,0,,80,1,extravascular\n3,0.5,,,0,\n3,2,4.9,,0,\n3,8,1.4,,0,\n";
    let mut a = engine_with(&profile, &[]);
    let mut b = engine_with(event, &[]);
    let old = nca_subjects(&mut a, json!({}));
    let new = nca_subjects(&mut b, json!({}));
    assert_eq!(old.len(), 3);
    for (o, n) in old.iter().zip(&new) {
        assert_eq!(o.to_string(), n.to_string(), "subject {}", o["subject"]);
    }
}

#[test]
fn nca_without_a_subject_analyses_every_subject_of_both_layouts() {
    for file in ["data/theoph.csv", "data/population/event_theoph.csv"] {
        let mut e = engine_with(&read_file(file), &[]);
        let subjects = nca_subjects(&mut e, json!({ "route": "extravascular" }));
        let labels: Vec<&str> = subjects
            .iter()
            .map(|s| s["subject"].as_str().unwrap())
            .collect();
        assert_eq!(
            labels,
            [
                "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"
            ],
            "{file}"
        );
        assert!(subjects.iter().all(|s| s["outcome"].get("ok").is_some()));
    }
}

#[test]
fn a_dose_time_shifts_the_time_axis_and_rows_before_the_dose_are_left_out() {
    let at_zero = "ID,TIME,DV,AMT,EVID\n\
1,0,,100,1\n1,0.5,2.2,,0\n1,1,3.3,,0\n1,2,4.0,,0\n1,4,3.6,,0\n1,8,2.5,,0\n1,24,0.5,,0\n";
    // The same subject dosed at time 6: every time moves by 6, and two rows before the dose.
    let at_six = "ID,TIME,DV,AMT,EVID\n\
1,1,0.1,,0\n1,3,0.2,,0\n1,6,,100,1\n1,6.5,2.2,,0\n1,7,3.3,,0\n1,8,4.0,,0\n1,10,3.6,,0\n1,14,2.5,,0\n1,30,0.5,,0\n";
    let mut a = engine_with(at_zero, &[]);
    let mut b = engine_with(at_six, &[]);
    let old = nca_subjects(&mut a, json!({ "route": "extravascular" }));
    let new = nca_subjects(&mut b, json!({ "route": "extravascular" }));
    assert_eq!(old.len(), 1);
    assert_eq!(new.len(), 1);
    assert_eq!(old[0]["outcome"], new[0]["outcome"]);
    assert_eq!(old[0]["dose"], new[0]["dose"]);
    let notes = new[0]["notes"]
        .as_array()
        .expect("a note about the rows before the dose");
    assert_eq!(notes.len(), 1);
    let note = notes[0].as_str().unwrap();
    assert!(
        note.contains("2 observation rows are before the dose")
            && note.contains("before_dose")
            && note.contains("time 6"),
        "{note}"
    );
    assert!(
        old[0].get("notes").is_none(),
        "no note when nothing was left out"
    );
}

#[test]
fn an_infusion_takes_its_duration_from_dur_or_from_rate_and_the_option_wins() {
    let times = "1,1,4.2,,,0\n1,2,7.9,,,0\n1,4,6.1,,,0\n1,8,3.0,,,0\n1,12,1.6,,,0\n";
    let with_dur = format!("ID,TIME,DV,AMT,DUR,EVID\n1,0,,100,2,1\n{times}");
    // The same table in the profile layout, with the duration given in the route.
    let profile = "subject,time,conc,dose\n1,1,4.2,100\n1,2,7.9,100\n1,4,6.1,100\n1,8,3.0,100\n1,12,1.6,100\n";
    let mut old = engine_with(profile, &[]);
    let want = nca_subjects(
        &mut old,
        json!({ "route": { "iv_infusion": { "duration": 2.0 } } }),
    );
    let mut dur = engine_with(&with_dur, &[]);
    let got = nca_subjects(&mut dur, json!({}));
    assert_eq!(got[0]["outcome"], want[0]["outcome"]);
    assert_eq!(got[0]["route"], want[0]["route"]);
    assert!(got[0].get("notes").is_none());

    // The rate and the amount give the duration: 100 / 50 = 2.
    let with_rate = format!("ID,TIME,DV,AMT,RATE,EVID\n1,0,,100,50,1\n{times}");
    let mut rate = engine_with(&with_rate, &[]);
    let got = nca_subjects(&mut rate, json!({}));
    assert_eq!(got[0]["outcome"], want[0]["outcome"]);

    // An explicit duration in the options wins over the file's, with a note.
    let mut other = engine_with(&with_dur, &[]);
    let got = nca_subjects(
        &mut other,
        json!({ "route": { "iv_infusion": { "duration": 3.0 } } }),
    );
    let mut reference = engine_with(profile, &[]);
    let want3 = nca_subjects(
        &mut reference,
        json!({ "route": { "iv_infusion": { "duration": 3.0 } } }),
    );
    assert_eq!(got[0]["outcome"], want3[0]["outcome"]);
    let note = got[0]["notes"][0].as_str().expect("a note");
    assert!(
        note.contains("duration 3") && note.contains("dose row gives 2"),
        "{note}"
    );
}

#[test]
fn a_subject_with_several_dose_rows_is_refused_alone() {
    let table = "ID,TIME,DV,AMT,EVID\n\
1,0,,100,1\n1,1,5.0,,0\n1,2,6.0,,0\n1,4,4.0,,0\n1,8,2.0,,0\n\
2,0,,100,1\n2,1,5.5,,0\n2,12,,100,1\n2,13,9.0,,0\n2,14,8.0,,0\n\
3,0,,100,1\n3,1,4.5,,0\n3,2,5.5,,0\n3,4,3.5,,0\n3,8,1.5,,0\n";
    let mut e = engine_with(table, &[]);
    let subjects = nca_subjects(&mut e, json!({ "route": "extravascular" }));
    let map = by_subject(&subjects);
    assert_eq!(
        map["2"]["outcome"]["error"],
        "subject 2 has 2 dose rows; the NCA of this version analyses a single dose per profile"
    );
    assert!(map["1"]["outcome"].get("ok").is_some());
    assert!(map["3"]["outcome"].get("ok").is_some());
    // The unaffected subjects equal the table without subject 2.
    let rest = "ID,TIME,DV,AMT,EVID\n\
1,0,,100,1\n1,1,5.0,,0\n1,2,6.0,,0\n1,4,4.0,,0\n1,8,2.0,,0\n\
3,0,,100,1\n3,1,4.5,,0\n3,2,5.5,,0\n3,4,3.5,,0\n3,8,1.5,,0\n";
    let mut r = engine_with(rest, &[]);
    let without_two = by_subject(&nca_subjects(&mut r, json!({ "route": "extravascular" })));
    assert_eq!(map["1"], without_two["1"]);
    assert_eq!(map["3"], without_two["3"]);
}

#[test]
fn the_covariates_are_copied_into_each_result_and_change_nothing() {
    let event = read_file("data/population/event_theoph.csv");
    let mut with = engine_with(&event, &["WT"]);
    let mut plain = engine_with(&event, &[]);
    let a = nca_subjects(&mut with, json!({ "route": "extravascular" }));
    let b = nca_subjects(&mut plain, json!({ "route": "extravascular" }));
    assert_eq!(a.len(), 12);
    for (x, y) in a.iter().zip(&b) {
        assert!(x["covariates"]["WT"].is_number(), "{x}");
        assert!(y.get("covariates").is_none());
        assert_eq!(
            without(x.clone(), &["covariates"]).to_string(),
            y.to_string(),
            "subject {}",
            x["subject"]
        );
    }
    // The first subject weighs 79.6 kg in the file.
    assert_eq!(a[0]["covariates"]["WT"], 79.6);

    // The same for the fits, which also carry the covariates.
    let mut fit_with = engine_with(&event, &["WT"]);
    let mut fit_without = engine_with(&event, &[]);
    let p = json!({ "worksheet": 1, "subject": "all", "model": "pk1.oral_1" });
    let x = ok(&mut fit_with, "fit.run", p.clone());
    let y = ok(&mut fit_without, "fit.run", p);
    let xs = x["result"]["subjects"].as_array().unwrap();
    let ys = y["result"]["subjects"].as_array().unwrap();
    assert_eq!(xs.len(), 12);
    for (a, b) in xs.iter().zip(ys) {
        assert!(a["covariates"]["WT"].is_number());
        assert_eq!(
            without(a.clone(), &["covariates"]).to_string(),
            b.to_string()
        );
    }
}

// ---- DAT-USE-02: subject = all -------------------------------------------------------------------

/// The fit of `subject`, named alone, as the object a `Fits` entry holds.
fn single_fit(csv: &str, covariates: &[&str], subject: &str, model: &str) -> Value {
    let mut e = engine_with(csv, covariates);
    let answer = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "subject": subject, "model": model }),
    );
    without(
        answer["result"].clone(),
        &["kind", "flag_messages", "status_message"],
    )
}

fn all_fits(csv: &str, covariates: &[&str], model: &str) -> Vec<Value> {
    let mut e = engine_with(csv, covariates);
    let answer = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "subject": "all", "model": model }),
    );
    assert_eq!(answer["result"]["kind"], "fits");
    assert_eq!(answer["kind"], "fit");
    answer["result"]["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| without(s.clone(), &["flag_messages", "status_message"]))
        .collect()
}

#[test]
fn every_fit_of_subject_all_equals_the_fit_of_that_subject_named_alone() {
    for (file, model, expected_subjects) in [
        ("data/theoph.csv", "pk1.oral_1", 12),
        ("data/population/event_theoph.csv", "pk1.oral_1", 12),
        ("data/indometh.csv", "pk1.iv_bolus", 6),
        ("data/population/event_indometh.csv", "pk1.iv_bolus", 6),
    ] {
        let csv = read_file(file);
        let all = all_fits(&csv, &[], model);
        assert_eq!(all.len(), expected_subjects, "{file}");
        for fit in &all {
            let subject = fit["subject"].as_str().unwrap();
            let alone = single_fit(&csv, &[], subject, model);
            assert_eq!(
                fit.to_string(),
                alone.to_string(),
                "{file}: subject {subject}"
            );
        }
    }
}

#[test]
fn subject_all_gives_the_same_estimates_in_both_layouts() {
    let profile = all_fits(&read_file("data/theoph.csv"), &[], "pk1.oral_1");
    let event = all_fits(
        &read_file("data/population/event_theoph.csv"),
        &[],
        "pk1.oral_1",
    );
    for (p, e) in profile.iter().zip(&event) {
        assert_eq!(p.to_string(), e.to_string(), "subject {}", p["subject"]);
    }
}

#[test]
fn an_error_in_one_subject_is_that_subjects_result() {
    // Subject 5 has a dose of 0 in this Theoph: its check refuses it, the others are fitted.
    let csv = read_file("data/population/event_theoph_dose0.csv");
    let all = all_fits(&csv, &[], "pk1.oral_1");
    assert_eq!(all.len(), 12);
    for fit in &all {
        let subject = fit["subject"].as_str().unwrap();
        if subject == "5" {
            let message = fit["outcome"]["error"].as_str().unwrap();
            assert!(
                message.contains("row 49 of subject 5") && message.to_lowercase().contains("amt"),
                "{message}"
            );
            assert!(fit["outcome"].get("error").is_some());
        } else {
            assert!(fit["outcome"].get("ok").is_some(), "subject {subject}");
            let alone = single_fit(&csv, &[], subject, "pk1.oral_1");
            assert_eq!(fit.to_string(), alone.to_string(), "subject {subject}");
        }
    }
    // The same subject named alone is refused with the same sentence.
    let mut e = engine_with(&csv, &[]);
    let refusal = e
        .execute(
            "fit.run",
            json!({ "worksheet": 1, "subject": "5", "model": "pk1.oral_1" }),
        )
        .unwrap_err();
    assert_eq!(Value::String(refusal.message), all[4]["outcome"]["error"]);
}

#[test]
fn the_default_is_unchanged_several_subjects_and_none_named_is_refused() {
    for file in ["data/theoph.csv", "data/population/event_theoph.csv"] {
        let mut e = engine_with(&read_file(file), &[]);
        let err = e
            .execute("fit.run", json!({ "worksheet": 1, "model": "pk1.oral_1" }))
            .unwrap_err();
        assert_eq!(err.code, "subject_required", "{file}");
    }
}

#[test]
fn a_subject_labelled_all_is_still_that_subject() {
    let csv = "subject,time,conc,dose\n\
all,0,0,100\nall,0.5,2.2,100\nall,1,3.3,100\nall,2,4.0,100\nall,4,3.6,100\nall,8,2.5,100\nall,24,0.5,100\n\
b,0,0,100\nb,0.5,1.8,100\nb,1,2.9,100\nb,2,3.5,100\nb,4,3.2,100\nb,8,2.1,100\nb,24,0.4,100\n";
    let mut e = engine_with(csv, &[]);
    let answer = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "subject": "all", "model": "pk1.oral_1" }),
    );
    assert_eq!(answer["result"]["kind"], "fit");
    assert_eq!(answer["result"]["subject"], "all");
}

#[test]
fn subject_all_is_not_a_job_and_the_engine_says_so() {
    let mut e = engine_with(&read_file("data/indometh.csv"), &[]);
    let err = e
        .begin(
            "fit.run",
            json!({ "worksheet": 1, "subject": "all", "model": "pk1.iv_bolus" }),
        )
        .unwrap_err();
    assert_eq!(err.code, "subject_all_not_a_job");
}

#[test]
fn the_stored_analysis_of_subject_all_runs_again_to_the_same_result() {
    let mut e = engine_with(&read_file("data/indometh.csv"), &[]);
    let first = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "subject": "all", "model": "pk1.iv_bolus" }),
    );
    let id = first["id"].clone();
    let again = ok(&mut e, "analysis.run", json!({ "analysis": id }));
    assert_eq!(first["result"], again["result"]);
}

// ---- FIT-ML-20: the counts -----------------------------------------------------------------------

#[test]
fn the_counts_of_a_fit_are_those_of_the_dataset() {
    // One missing cell, one value below the limit, nine measured.
    let csv = "subject,time,conc,dose\n\
1,0,0,100\n1,0.5,2.2,100\n1,1,,100\n1,2,4.0,100\n1,4,3.6,100\n1,6,<0.5,100\n1,8,2.5,100\n1,12,1.7,100\n1,16,1.1,100\n1,24,0.6,100\n";
    let mut e = engine_with(csv, &[]);
    let answer = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "model": "pk1.oral_1" }),
    );
    let result = &answer["result"];
    assert_eq!(result["n_observations"], 8);
    assert_eq!(result["n_missing"], 1);
    assert_eq!(result["n_blq"], 1);
    // The same table in the event layout counts the same.
    let event = as_event_table(csv);
    let mut b = engine_with(&event, &[]);
    let answer = ok(
        &mut b,
        "fit.run",
        json!({ "worksheet": 1, "model": "pk1.oral_1" }),
    );
    for key in ["n_observations", "n_missing", "n_blq"] {
        assert_eq!(answer["result"][key], result[key], "{key}");
    }
}

// ---- DAT-ERR-08: a refused subject ---------------------------------------------------------------

/// Which column holds the subject, and the delimiter, of a refusal case.
fn layout_of_case(case: &Value) -> (char, usize) {
    let delimiter = case["delimiter"].as_str().unwrap().chars().next().unwrap();
    // A profile-layout case lists no roles: the subject is the first column.
    let position = case["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c.as_str().unwrap().ends_with(":subject"))
        .unwrap_or(0);
    (delimiter, position)
}

/// The CSV text without the rows of `drop`.
fn without_subjects(csv: &str, delimiter: char, column: usize, drop: &[String]) -> String {
    let mut out = String::new();
    for (i, line) in csv.lines().enumerate() {
        let subject = line.split(delimiter).nth(column).unwrap_or("");
        if i == 0 || !drop.iter().any(|d| d == subject) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn refusal_cases() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(oracle().join("expected/data"))
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_prefix("refuse_")
                .and_then(|n| n.strip_suffix(".json"))
                .map(|n| format!("refuse_{n}"))
        })
        .collect();
    names.sort();
    names
}

fn covariates_of_case(case: &Value) -> Vec<String> {
    case["options"]["covariates"]
        .as_array()
        .map(|a| a.iter().map(|c| c.as_str().unwrap().to_owned()).collect())
        .unwrap_or_default()
}

#[test]
fn an_affected_subject_is_refused_with_the_sentence_of_its_check_and_the_others_run() {
    let cases = refusal_cases();
    assert!(cases.len() >= 25, "{} refusal cases found", cases.len());
    let mut refused = 0;
    for name in &cases {
        let case = read_json(&format!("expected/data/{name}.json"));
        let csv = read_file(&format!("data/{}", case["file"].as_str().unwrap()));
        let covariates = covariates_of_case(&case);
        let covariates: Vec<&str> = covariates.iter().map(String::as_str).collect();
        let affected: Vec<String> = case["affected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect();
        let (delimiter, column) = layout_of_case(&case);

        // The sentence of the check, from the reading that the importer gives.
        let options: CsvOptions = serde_json::from_value(case["options"].clone()).unwrap();
        let found = readings(csv.as_bytes(), &options).unwrap();
        let check = found[0]
            .checks
            .first()
            .unwrap_or_else(|| panic!("{name}: a refused table has a check"));
        assert_eq!(
            check.code,
            case["error"]["code"].as_str().unwrap(),
            "{name}"
        );

        let mut e = engine_with(&csv, &covariates);
        let subjects = by_subject(&nca_subjects(&mut e, json!({ "route": "extravascular" })));

        refused += affected.len();
        for id in &affected {
            let got = &subjects[id];
            assert!(
                got["outcome"].get("error").is_some(),
                "{name}: subject {id}"
            );
            let message = got["outcome"]["error"].as_str().unwrap();
            // The first subject the problem names carries the sentence of the check as it is.
            if case["error"]["subject"].as_str() == Some(id.as_str()) {
                assert_eq!(message, check.message, "{name}: subject {id}");
            }
            for word in case["error"]["words"].as_array().unwrap() {
                let word = word.as_str().unwrap().to_lowercase();
                if case["error"]["subject"].as_str() == Some(id.as_str()) {
                    assert!(
                        message.to_lowercase().contains(&word),
                        "{name}: `{word}` is not in `{message}`"
                    );
                }
            }
        }

        // The subjects the problem does not affect run, and give what they give on a table without
        // the problem.
        let clean = without_subjects(&csv, delimiter, column, &affected);
        let mut c = engine_with(&clean, &covariates);
        let reference = by_subject(&nca_subjects(&mut c, json!({ "route": "extravascular" })));
        for (id, result) in &subjects {
            if affected.contains(id) {
                continue;
            }
            assert_eq!(
                result.to_string(),
                reference[id].to_string(),
                "{name}: unaffected subject {id}"
            );
        }
        assert_eq!(
            subjects.len() - affected.len().min(subjects.len()),
            reference.len(),
            "{name}: unaffected subjects"
        );
    }
    assert!(
        refused >= 24,
        "only {refused} refused subjects were checked"
    );
}

#[test]
fn a_fit_of_every_subject_refuses_the_affected_subject_like_the_nca_does() {
    for name in refusal_cases() {
        let case = read_json(&format!("expected/data/{name}.json"));
        let csv = read_file(&format!("data/{}", case["file"].as_str().unwrap()));
        let covariates = covariates_of_case(&case);
        let covariates: Vec<&str> = covariates.iter().map(String::as_str).collect();
        let affected: Vec<String> = case["affected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect();
        let (delimiter, column) = layout_of_case(&case);
        let model = "pk1.oral_1";

        let all = all_fits(&csv, &covariates, model);
        let mut nca = engine_with(&csv, &covariates);
        let nca = by_subject(&nca_subjects(&mut nca, json!({ "route": "extravascular" })));
        for fit in &all {
            let id = fit["subject"].as_str().unwrap();
            if affected.iter().any(|a| a == id) {
                assert!(
                    fit["outcome"].get("error").is_some(),
                    "{name}: subject {id}"
                );
                assert_eq!(
                    fit["outcome"]["error"], nca[id]["outcome"]["error"],
                    "{name}: subject {id}: the fit and the NCA refuse with one sentence"
                );
            }
        }
        let clean = without_subjects(&csv, delimiter, column, &affected);
        let reference = all_fits(&clean, &covariates, model);
        for fit in &all {
            let id = fit["subject"].as_str().unwrap();
            if affected.iter().any(|a| a == id) {
                continue;
            }
            let want = reference
                .iter()
                .find(|r| r["subject"] == fit["subject"])
                .unwrap_or_else(|| panic!("{name}: no reference for subject {id}"));
            assert_eq!(
                fit.to_string(),
                want.to_string(),
                "{name}: unaffected subject {id}"
            );
        }
    }
}
