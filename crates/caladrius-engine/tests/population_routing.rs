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
    assert_eq!(new[0]["n_before_dose"], 2);
    assert!(old[0].get("n_before_dose").is_none());
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
    // `data.describe` lists the data checks only: the subject with two dose rows is refused by the
    // analysis, not listed there.
    let described = ok(&mut e, "data.describe", json!({ "worksheet": 1 }));
    assert!(described.get("refused").is_none(), "{described}");
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
            // Every case of the oracle names one affected subject, and that subject is the one
            // the check names first: its sentence is the check's, as it is.
            assert_eq!(affected.len(), 1, "{name}: one affected subject");
            assert_eq!(message, check.message, "{name}: subject {id}");
            for word in case["error"]["words"].as_array().unwrap() {
                let word = word.as_str().unwrap().to_lowercase();
                assert!(
                    message.to_lowercase().contains(&word),
                    "{name}: `{word}` is not in `{message}`"
                );
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

// ---- T-163, second round: the cases the first round left to chance -------------------------------

/// The result of `fit.run` with `params` on `csv` (worksheet 1), as the engine answers it.
fn fit_answer(csv: &str, covariates: &[&str], params: Value) -> Value {
    let mut e = engine_with(csv, covariates);
    let mut params = params;
    params["worksheet"] = json!(1);
    ok(&mut e, "fit.run", params)["result"].clone()
}

/// The entries of a `Fits` result, without the sentences that only the view adds.
fn entries(result: &Value) -> Vec<Value> {
    assert_eq!(result["kind"], "fits", "{result}");
    result["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| without(s.clone(), &["flag_messages", "status_message"]))
        .collect()
}

/// `event_indometh.csv` with the dose at time `offset` and, when asked, one observation of each
/// subject before it (time 1, below every shifted dose time used here). The times are dyadic
/// fractions, so adding the offset and taking it away again is exact.
fn indometh_dosed_at(offset: f64, with_pre_dose_row: bool) -> String {
    let source = read_file("data/population/event_indometh.csv");
    let mut out = String::from("ID,TIME,DV,AMT,EVID\n");
    let mut last = String::new();
    for line in source.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let cells: Vec<&str> = line.split(',').collect();
        if cells[0] != last {
            last = cells[0].to_owned();
            if with_pre_dose_row {
                out.push_str(&format!("{},1,0.5,,0\n", cells[0]));
            }
        }
        let time: f64 = cells[1].parse().unwrap();
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            cells[0],
            time + offset,
            cells[2],
            cells[3],
            cells[4]
        ));
    }
    out
}

/// `event_indometh.csv` with one more column, `name`, holding `cell` on the dose rows.
fn indometh_with_column(name: &str, cell: &str) -> String {
    let source = read_file("data/population/event_indometh.csv");
    let mut out = String::new();
    for (i, line) in source
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let cells: Vec<&str> = line.split(',').collect();
        let extra = if i == 0 {
            name
        } else if cells[4] == "1" {
            cell
        } else {
            ""
        };
        out.push_str(&format!("{line},{extra}\n"));
    }
    out
}

#[test]
fn a_fit_on_the_event_layout_with_a_dose_at_a_later_time_equals_the_fit_at_time_zero() {
    let at_zero = read_file("data/population/event_indometh.csv");
    let later = indometh_dosed_at(6.0, true);
    let model = "pk1.iv_bolus";

    // One subject named alone.
    let a = fit_answer(&at_zero, &[], json!({ "subject": "1", "model": model }));
    let b = fit_answer(&later, &[], json!({ "subject": "1", "model": model }));
    assert_eq!(a["outcome"], b["outcome"]);
    for key in ["dose", "n_observations", "n_missing", "n_blq"] {
        assert_eq!(a[key], b[key], "{key}");
    }
    assert!(a.get("n_before_dose").is_none() && a.get("notes").is_none());
    assert_eq!(b["n_before_dose"], 1);
    let note = b["notes"][0].as_str().unwrap();
    assert!(
        note.contains("subject 1") && note.contains("before the dose") && note.contains("time 6"),
        "{note}"
    );

    // Every subject, with the maximum-likelihood objective too (the same data, the same fits).
    for options in [
        json!({}),
        json!({ "objective": "ml", "error_model": "additive" }),
    ] {
        let p = json!({ "subject": "all", "model": model, "options": options });
        let a = entries(&fit_answer(&at_zero, &[], p.clone()));
        let b = entries(&fit_answer(&later, &[], p));
        assert_eq!(a.len(), 6);
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x["outcome"], y["outcome"], "subject {}", x["subject"]);
            assert_eq!(y["n_before_dose"], 1);
            assert_eq!(x["n_observations"], y["n_observations"]);
        }
    }
}

#[test]
fn the_nca_counts_the_rows_before_the_dose() {
    let later = indometh_dosed_at(6.0, true);
    let mut e = engine_with(&later, &[]);
    let subjects = nca_subjects(&mut e, json!({ "route": "iv_bolus" }));
    assert_eq!(subjects.len(), 6);
    for s in &subjects {
        assert_eq!(s["n_before_dose"], 1, "{s}");
        assert!(s["notes"][0].as_str().unwrap().contains("before_dose"));
    }
}

#[test]
fn an_infusion_is_fitted_with_the_duration_of_its_dose_row_from_dur_or_from_rate() {
    let model = "pk1.iv_infusion";
    // The reference: the same profiles in the profile layout, `dur` fixed by hand.
    let profile = read_file("data/indometh.csv");
    let by_hand = |dur: f64| {
        entries(&fit_answer(
            &profile,
            &[],
            json!({ "subject": "all", "model": model, "options": { "fixed": { "dur": dur } } }),
        ))
    };
    let want = by_hand(2.0);
    assert_eq!(want.len(), 6);
    let want_three = by_hand(3.0);

    // `dur` on the dose row, and the rate that gives the same duration (25 mg at 12.5 mg/h).
    for (column, cell) in [("DUR", "2"), ("RATE", "12.5")] {
        let table = indometh_with_column(column, cell);
        let got = entries(&fit_answer(
            &table,
            &[],
            json!({ "subject": "all", "model": model }),
        ));
        assert_eq!(got.len(), 6, "{column}");
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(
                g["outcome"], w["outcome"],
                "{column}: subject {}",
                g["subject"]
            );
            assert!(g.get("notes").is_none(), "{column}: {g}");
        }
    }

    // A duration fixed in the options wins over the file's, and a note says so.
    let table = indometh_with_column("DUR", "2");
    let got = entries(&fit_answer(
        &table,
        &[],
        json!({ "subject": "all", "model": model, "options": { "fixed": { "dur": 3.0 } } }),
    ));
    for (g, w) in got.iter().zip(&want_three) {
        assert_eq!(g["outcome"], w["outcome"], "subject {}", g["subject"]);
        let note = g["notes"][0].as_str().unwrap();
        assert!(
            note.contains("`dur` 3") && note.contains("dose row gives 2"),
            "{note}"
        );
    }

    // A `dur` among the initial estimates is fitted from there: the file's value is not held, and
    // a note says so.
    let mut e = engine_with(&table, &[]);
    let initial = ok(
        &mut e,
        "fit.initial_estimates",
        json!({ "worksheet": 1, "subject": "1", "model": model }),
    )["initial"]
        .clone();
    let mut start = initial.as_object().unwrap().clone();
    start.insert("dur".to_owned(), json!(2.5));
    let fitted = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "subject": "1", "model": model, "initial": start }),
    )["result"]
        .clone();
    let note = fitted["notes"][0].as_str().unwrap();
    assert!(
        note.contains("`dur` is among the initial estimates (2.5)")
            && note.contains("dose row gives 2"),
        "{note}"
    );
}

#[test]
fn a_model_that_does_not_read_the_duration_from_the_file_says_so() {
    let table = indometh_with_column("DUR", "2");
    let plain = read_file("data/population/event_indometh.csv");
    let model = "pk1.iv_bolus";
    let got = entries(&fit_answer(
        &table,
        &[],
        json!({ "subject": "all", "model": model }),
    ));
    let want = entries(&fit_answer(
        &plain,
        &[],
        json!({ "subject": "all", "model": model }),
    ));
    for (g, w) in got.iter().zip(&want) {
        assert_eq!(g["outcome"], w["outcome"], "subject {}", g["subject"]);
        let note = g["notes"][0].as_str().unwrap();
        assert!(
            note.contains("infusion of duration 2")
                && note.contains("pk1.iv_bolus does not read from the file"),
            "{note}"
        );
    }
}

#[test]
fn the_dose_of_the_analysis_wins_over_the_dose_row_of_an_event_table_with_a_note() {
    let plain = read_file("data/population/event_indometh.csv");
    let model = "pk1.iv_bolus";
    // The dose row says 25; the analysis says 30.
    let got = fit_answer(
        &plain,
        &[],
        json!({ "subject": "1", "model": model, "dose": 30.0 }),
    );
    assert_eq!(got["dose"], 30.0);
    let note = got["notes"][0].as_str().unwrap();
    assert!(
        note.contains("the dose 30 of the analysis is used") && note.contains("dose row gives 25"),
        "{note}"
    );
    // The same dose as the row's gives no note.
    let same = fit_answer(
        &plain,
        &[],
        json!({ "subject": "1", "model": model, "dose": 25.0 }),
    );
    assert!(same.get("notes").is_none());
    // The NCA, the same way.
    let mut e = engine_with(&plain, &[]);
    let subjects = nca_subjects(&mut e, json!({ "route": "iv_bolus", "dose": 30.0 }));
    assert_eq!(subjects[0]["dose"], 30.0);
    assert!(
        subjects[0]["notes"][0]
            .as_str()
            .unwrap()
            .contains("dose row gives 25")
    );
    // The profile layout keeps its old rule, without a note: the dose of the analysis wins.
    let profile = read_file("data/indometh.csv");
    let got = fit_answer(
        &profile,
        &[],
        json!({ "subject": "1", "model": model, "dose": 30.0 }),
    );
    assert_eq!(got["dose"], 30.0);
    assert!(got.get("notes").is_none());
}

#[test]
fn a_route_column_that_contradicts_an_infusion_dose_row_is_an_error_for_that_subject() {
    let infusion = |route: &str| {
        let table = format!(
            "ID,TIME,DV,AMT,DUR,ROUTE,EVID\n\
1,0,,100,2,{route},1\n1,1,4.2,,,,0\n1,2,7.9,,,,0\n1,4,6.1,,,,0\n1,8,3.0,,,,0\n1,12,1.6,,,,0\n"
        );
        let mut e = engine_with(&table, &[]);
        nca_subjects(&mut e, json!({}))
    };
    for contradiction in ["iv_bolus", "extravascular", "oral"] {
        let subjects = infusion(contradiction);
        let message = subjects[0]["outcome"]["error"].as_str().unwrap();
        assert!(
            message.contains("is an infusion but the route column says"),
            "{contradiction}: {message}"
        );
    }
    let unreadable = infusion("sideways");
    let message = unreadable[0]["outcome"]["error"].as_str().unwrap();
    assert!(
        message.contains("is an infusion, but the route `sideways` is not recognised"),
        "{message}"
    );
    // A route column that says infusion agrees with the dose row.
    for agreeing in ["iv_infusion", "infusion"] {
        let subjects = infusion(agreeing);
        assert!(subjects[0]["outcome"].get("ok").is_some(), "{agreeing}");
    }
}

#[test]
fn subject_all_with_the_maximum_likelihood_objective_equals_each_subject_named_alone() {
    let csv = read_file("data/population/event_indometh.csv");
    let options = json!({ "objective": "ml", "error_model": "additive" });
    let all = entries(&fit_answer(
        &csv,
        &[],
        json!({ "subject": "all", "model": "pk1.iv_bolus", "options": options }),
    ));
    assert_eq!(all.len(), 6);
    let mut fitted = 0;
    for fit in &all {
        let subject = fit["subject"].as_str().unwrap();
        let mut e = engine_with(&csv, &[]);
        let alone = e.execute(
            "fit.run",
            json!({ "worksheet": 1, "subject": subject, "model": "pk1.iv_bolus", "options": options }),
        );
        match alone {
            Ok(answer) => {
                fitted += 1;
                let alone = without(
                    answer["result"].clone(),
                    &["kind", "flag_messages", "status_message"],
                );
                assert_eq!(fit.to_string(), alone.to_string(), "subject {subject}");
                assert_eq!(fit["outcome"]["ok"]["objective"], "ml", "subject {subject}");
            }
            // A fit that cannot start is refused alone and is that subject's error in `all`.
            Err(refusal) => assert_eq!(
                Value::String(refusal.message),
                fit["outcome"]["error"],
                "subject {subject}"
            ),
        }
    }
    assert!(fitted >= 1, "no subject was fitted by maximum likelihood");
}

#[test]
fn a_request_that_is_wrong_for_every_subject_is_refused_as_a_whole_and_stores_nothing() {
    let csv = read_file("data/population/event_indometh.csv");
    let mut e = engine_with(&csv, &[]);
    let request = |subject: &str| json!({ "worksheet": 1, "subject": subject, "model": "pk1.iv_bolus", "initial": { "zzz": 1.0 } });
    let alone = e.execute("fit.run", request("1")).unwrap_err();
    assert_eq!(alone.code, "fit_error");
    let entries_before = e.history().len();
    let all = e.execute("fit.run", request("all")).unwrap_err();
    assert_eq!(all.code, "fit_error");
    assert_eq!(all.message, alone.message);
    assert!(e.project().analyses().is_empty(), "nothing was stored");
    let last = e.history().entries().last().unwrap();
    assert_eq!(e.history().len(), entries_before + 1);
    assert!(!last.ok, "the refusal is a failed entry, not a success");

    // An option out of its domain is the same.
    let bad = json!({ "worksheet": 1, "subject": "all", "model": "pk1.iv_bolus",
        "options": { "confidence_level": 7.0 } });
    let single = json!({ "worksheet": 1, "subject": "1", "model": "pk1.iv_bolus",
        "options": { "confidence_level": 7.0 } });
    let a = e.execute("fit.run", single).unwrap_err();
    let b = e.execute("fit.run", bad).unwrap_err();
    assert_eq!(a.message, b.message);
    assert!(e.project().analyses().is_empty());
}

#[test]
fn a_subject_that_cannot_start_keeps_its_dose_and_counts_in_its_result() {
    // Subject b has two observations, fewer than the three parameters of the model.
    let csv = "subject,time,conc,dose\n\
a,0.5,2.2,100\na,1,3.3,100\na,2,4.0,100\na,4,3.6,100\na,8,2.5,100\na,12,1.7,100\na,24,0.5,100\n\
b,1,3.0,100\nb,4,2.0,100\n";
    for table in [csv.to_owned(), as_event_table(csv)] {
        let all = entries(&fit_answer(
            &table,
            &[],
            json!({ "subject": "all", "model": "pk1.oral_1" }),
        ));
        assert_eq!(all.len(), 2);
        let b = &all[1];
        assert!(b["outcome"].get("error").is_some(), "{b}");
        assert_eq!(b["dose"], 100.0);
        assert_eq!(b["n_observations"], 2);
        assert_eq!(b["n_missing"], 0);
    }
}

#[test]
fn a_subject_labelled_all_is_that_subject_in_both_layouts_with_a_note() {
    let profile = "subject,time,conc,dose\n\
all,0,0,100\nall,0.5,2.2,100\nall,1,3.3,100\nall,2,4.0,100\nall,4,3.6,100\nall,8,2.5,100\nall,24,0.5,100\n\
b,0,0,100\nb,0.5,1.8,100\nb,1,2.9,100\nb,2,3.5,100\nb,4,3.2,100\nb,8,2.1,100\nb,24,0.4,100\n";
    for (layout, table) in [
        ("profile", profile.to_owned()),
        ("event", as_event_table(profile)),
    ] {
        let result = fit_answer(
            &table,
            &[],
            json!({ "subject": "all", "model": "pk1.oral_1" }),
        );
        assert_eq!(result["kind"], "fit", "{layout}");
        assert_eq!(result["subject"], "all", "{layout}");
        let note = result["notes"][0].as_str().unwrap();
        assert!(
            note.contains("`all` is the label of a subject") && note.contains("not every subject"),
            "{layout}: {note}"
        );
        // The same subject fitted by a name that is not the keyword has no such note.
        let other = fit_answer(
            &table,
            &[],
            json!({ "subject": "b", "model": "pk1.oral_1" }),
        );
        assert!(other.get("notes").is_none(), "{layout}");
    }
}

#[test]
fn an_unknown_worksheet_is_reported_before_the_keyword_all() {
    let mut e = Engine::new();
    for subject in ["all", "1"] {
        let params = json!({ "worksheet": 99, "subject": subject, "model": "pk1.oral_1" });
        let by_execute = e.execute("fit.run", params.clone()).unwrap_err();
        assert_eq!(by_execute.code, "unknown_worksheet", "{subject}");
        let by_begin = e.begin("fit.run", params).unwrap_err();
        assert_eq!(by_begin.code, "unknown_worksheet", "{subject}");
    }
}

#[test]
fn refusing_all_as_a_job_leaves_no_entry_in_the_history() {
    let mut e = engine_with(&read_file("data/indometh.csv"), &[]);
    let before = e.history().len();
    let err = e
        .begin(
            "fit.run",
            json!({ "worksheet": 1, "subject": "all", "model": "pk1.iv_bolus" }),
        )
        .unwrap_err();
    assert_eq!(err.code, "subject_all_not_a_job");
    assert_eq!(e.history().len(), before, "a refusal is not a run");
    assert!(e.running_job().is_none());
    // A job that fails for another reason is still recorded, as it was.
    let err = e
        .begin(
            "fit.run",
            json!({ "worksheet": 1, "subject": "nobody", "model": "pk1.iv_bolus" }),
        )
        .unwrap_err();
    assert_eq!(err.code, "unknown_subject");
    assert_eq!(e.history().len(), before + 1);
}

// ---- the profile layout: a covariate that varies refuses its subject (DAT-ERR-05, DAT-ERR-08) --------

const PROFILE_WITH_WEIGHT: &str = "subject,time,conc,dose,WT\n\
a,0.5,3.1,100,70\na,2,5.8,100,70\na,4,4.6,100,70\na,8,2.9,100,70\na,12,1.8,100,70\n\
b,0.5,2.7,100,70\nb,2,5.2,100,80\nb,4,4.0,100,80\nb,8,2.5,100,80\nb,12,1.5,100,80\n\
c,0.5,3.4,100,65\nc,2,6.0,100,65\nc,4,4.9,100,65\nc,8,3.1,100,65\nc,12,2.0,100,65\n";

#[test]
fn a_profile_layout_subject_with_a_varying_covariate_is_refused_like_in_the_event_layout() {
    let check = {
        let options: CsvOptions = serde_json::from_value(json!({ "covariates": ["WT"] })).unwrap();
        let found = readings(PROFILE_WITH_WEIGHT.as_bytes(), &options).unwrap();
        found[0]
            .checks
            .first()
            .expect("the reading checks it")
            .clone()
    };
    assert_eq!(check.code, "covariate_varies");

    let mut e = engine_with(PROFILE_WITH_WEIGHT, &["WT"]);
    let subjects = by_subject(&nca_subjects(&mut e, json!({ "route": "extravascular" })));
    let message = subjects["b"]["outcome"]["error"].as_str().unwrap();
    assert_eq!(message, check.message);
    assert!(
        message.contains("covariate `WT` takes several values for subject b")
            && message.contains("70 at row 6")
            && message.contains("80 at row 7"),
        "{message}"
    );
    // The others run, and give what they give without the affected subject.
    let rest = "subject,time,conc,dose,WT\n".to_owned()
        + &PROFILE_WITH_WEIGHT
            .lines()
            .skip(1)
            .filter(|l| !l.starts_with("b,"))
            .collect::<Vec<_>>()
            .join("\n")
        + "\n";
    let mut r = engine_with(&rest, &["WT"]);
    let reference = by_subject(&nca_subjects(&mut r, json!({ "route": "extravascular" })));
    for id in ["a", "c"] {
        assert_eq!(
            subjects[id].to_string(),
            reference[id].to_string(),
            "subject {id}"
        );
    }

    // A fit of every subject, and of the subject alone, refuse it with the same sentence.
    let all = entries(&fit_answer(
        PROFILE_WITH_WEIGHT,
        &["WT"],
        json!({ "subject": "all", "model": "pk1.oral_1" }),
    ));
    assert_eq!(all[1]["outcome"]["error"], message);
    assert!(all[0]["outcome"].get("ok").is_some() && all[2]["outcome"].get("ok").is_some());
    let mut e = engine_with(PROFILE_WITH_WEIGHT, &["WT"]);
    let alone = e
        .execute(
            "fit.run",
            json!({ "worksheet": 1, "subject": "b", "model": "pk1.oral_1" }),
        )
        .unwrap_err();
    assert_eq!(alone.message, message);

    // The event layout of the same data refuses it with the same words.
    let event = "ID,TIME,DV,AMT,EVID,WT\n\
b,0,,100,1,70\nb,0.5,2.7,,0,70\nb,2,5.2,,0,80\nb,4,4.0,,0,80\n";
    let mut ev = engine_with(event, &["WT"]);
    let twin = nca_subjects(&mut ev, json!({ "route": "extravascular" }));
    assert!(
        twin[0]["outcome"]["error"]
            .as_str()
            .unwrap()
            .contains("covariate `WT` takes several values for subject b"),
    );
}

#[test]
fn data_describe_names_the_refused_subjects_with_their_sentence() {
    // Profile layout: the varying covariate.
    let mut e = engine_with(PROFILE_WITH_WEIGHT, &["WT"]);
    let described = ok(&mut e, "data.describe", json!({ "worksheet": 1 }));
    let refused = described["refused"].as_array().unwrap();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["subject"], "b");
    assert_eq!(refused[0]["code"], "covariate_varies");
    let nca = nca_subjects(&mut e, json!({ "route": "extravascular" }));
    assert_eq!(
        Value::String(refused[0]["message"].as_str().unwrap().to_owned()),
        by_subject(&nca)["b"]["outcome"]["error"]
    );

    // Event layout: every case of the refusal files that names a subject lists it, with the
    // sentence its NCA gives.
    let mut listed = 0;
    for name in refusal_cases() {
        let case = read_json(&format!("expected/data/{name}.json"));
        let affected: Vec<String> = case["affected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect();
        if affected.is_empty() {
            continue;
        }
        let csv = read_file(&format!("data/{}", case["file"].as_str().unwrap()));
        let covariates = covariates_of_case(&case);
        let covariates: Vec<&str> = covariates.iter().map(String::as_str).collect();
        let mut e = engine_with(&csv, &covariates);
        let described = ok(&mut e, "data.describe", json!({ "worksheet": 1 }));
        let shown: Vec<String> = described["refused"]
            .as_array()
            .unwrap_or_else(|| panic!("{name}: no `refused` in {described}"))
            .iter()
            .map(|r| r["subject"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(shown, affected, "{name}");
        let nca = by_subject(&nca_subjects(&mut e, json!({ "route": "extravascular" })));
        for r in described["refused"].as_array().unwrap() {
            let id = r["subject"].as_str().unwrap();
            assert_eq!(
                Value::String(r["message"].as_str().unwrap().to_owned()),
                nca[id]["outcome"]["error"],
                "{name}: subject {id}"
            );
            listed += 1;
        }
    }
    assert!(listed >= 20, "only {listed} refused subjects were listed");

    // A table without a problem answers as before: no `refused` member.
    let mut clean = engine_with(&read_file("data/theoph.csv"), &[]);
    let described = ok(&mut clean, "data.describe", json!({ "worksheet": 1 }));
    assert!(described.get("refused").is_none());
}

#[test]
fn several_subjects_with_the_same_problem_each_carry_their_own_sentence() {
    // Subjects 1 and 3 each have a weight that changes; subject 2 does not. The sentences name
    // their own subject and their own rows.
    let table = "ID,TIME,DV,AMT,EVID,WT\n\
1,0,,100,1,70\n1,1,5.0,,0,70\n1,2,6.0,,0,75\n\
2,0,,100,1,60\n2,1,5.5,,0,60\n2,2,6.5,,0,60\n\
3,0,,100,1,80\n3,1,4.5,,0,81\n3,2,5.5,,0,81\n";
    let mut e = engine_with(table, &["WT"]);
    let subjects = by_subject(&nca_subjects(&mut e, json!({ "route": "extravascular" })));
    let sentence = |id: &str, first: &str, first_row: usize, second: &str, second_row: usize| {
        format!(
            "covariate `WT` takes several values for subject {id} ({first} at row {first_row} and {second} at row {second_row}); in this version a covariate has one value per subject. Give one value per subject, or declare the column as not a covariate"
        )
    };
    assert_eq!(
        subjects["1"]["outcome"]["error"],
        sentence("1", "70", 1, "75", 3)
    );
    assert_eq!(
        subjects["3"]["outcome"]["error"],
        sentence("3", "80", 7, "81", 8)
    );
    assert!(subjects["2"]["outcome"].get("ok").is_some());
    // The fits of every subject say the same.
    let all = entries(&fit_answer(
        table,
        &["WT"],
        json!({ "subject": "all", "model": "pk1.oral_1" }),
    ));
    assert_eq!(
        all[0]["outcome"]["error"],
        subjects["1"]["outcome"]["error"]
    );
    assert_eq!(
        all[2]["outcome"]["error"],
        subjects["3"]["outcome"]["error"]
    );
}

#[test]
fn a_partial_initial_refuses_only_the_subject_whose_dose_row_lacks_the_duration() {
    // The estimates of `k` and `v` are given, `dur` is not: it comes from each subject's dose row.
    // Subject 3 has no duration on its dose row, so for that subject alone `dur` is missing. That
    // is the data of one subject; the other subjects must be fitted.
    let mut table = String::new();
    for (i, line) in read_file("data/population/event_indometh.csv")
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let cells: Vec<&str> = line.split(',').collect();
        let extra = if i == 0 {
            "DUR"
        } else if cells[4] == "1" && cells[0] != "3" {
            "2"
        } else {
            ""
        };
        table.push_str(&format!(
            "{line},{extra}
"
        ));
    }
    let mut e = engine_with(&table, &[]);
    let model = "pk1.iv_infusion";
    let generated = ok(
        &mut e,
        "fit.initial_estimates",
        json!({ "worksheet": 1, "subject": "1", "model": model }),
    )["initial"]
        .clone();
    let mut initial = serde_json::Map::new();
    for name in ["k", "v"] {
        assert!(generated.get(name).is_some(), "{generated}");
        initial.insert(name.to_owned(), generated[name].clone());
    }
    let result = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "subject": "all", "model": model, "initial": initial }),
    )["result"]
        .clone();
    let fits = entries(&result);
    assert_eq!(fits.len(), 6, "{result}");
    for fit in &fits {
        let id = fit["subject"].as_str().unwrap();
        if id == "3" {
            let error = fit["outcome"]["error"].as_str().unwrap();
            assert!(error.contains("dur"), "{error}");
            assert_eq!(fit["dose"], 25.0, "the subject keeps its dose and counts");
        } else {
            assert!(fit["outcome"].get("ok").is_some(), "subject {id}: {fit}");
        }
    }

    // An estimate that is wrong for every subject is still the request's: `v` out of its domain
    // refuses the whole request.
    let wrong = json!({ "worksheet": 1, "subject": "all", "model": model, "initial": { "k": 0.2, "v": -1.0 } });
    let before = e.project().analyses().len();
    let refused = e.execute("fit.run", wrong).unwrap_err();
    assert_eq!(refused.code, "fit_error");
    assert_eq!(e.project().analyses().len(), before);
}
