//! Tests of the population dataset (T-150, `specs/data.md`): the records of both layouts, the
//! refusals as checks of the reading, and the rule that a refusal never takes the other subjects
//! of the table with it. The oracle files are in `tests/oracle_data.rs`; these cover the corners
//! that need no file.

use crate::*;

fn read(csv: &str) -> Reading {
    readings(csv.as_bytes(), &CsvOptions::default())
        .unwrap()
        .remove(0)
}

fn codes(reading: &Reading) -> Vec<&str> {
    reading.checks.iter().map(|c| c.code.as_str()).collect()
}

const EVENT: &str = "ID,TIME,DV,AMT,RATE,EVID\n\
1,0,,100,50,1\n1,0,0.4,,,0\n1,1,3.1,,,0\n1,4,<0.5,,,0\n\
2,0,,120,,1\n2,2,5.2,,,0\n";

#[test]
fn the_event_layout_gives_doses_and_observations_with_global_rows() {
    let r = read(EVENT);
    assert!(r.checks.is_empty(), "{:?}", r.checks);
    assert_eq!(r.dataset.layout, Layout::Event);
    let [one, two] = r.dataset.subjects.as_slice() else {
        panic!("two subjects");
    };
    assert_eq!((one.id.as_str(), one.n_rows, two.n_rows), ("1", 4, 2));
    let [dose] = one.doses.as_slice() else {
        panic!("one dose");
    };
    assert_eq!(dose.kind, DoseKind::Infusion);
    assert_eq!((dose.rate, dose.duration), (Some(50.0), Some(2.0)));
    assert_eq!(dose.row, Some(1));
    assert_eq!(
        (one.n_obs, one.n_observed, one.n_blq, one.n_missing),
        (3, 2, 1, 0)
    );
    // A dose and an observation at the same time are allowed in the event layout.
    assert_eq!(one.observations.first().and_then(|o| o.time), Some(0.0));
    assert_eq!(two.doses.first().map(|d| d.kind), Some(DoseKind::Bolus));
    assert_eq!(two.doses.first().and_then(|d| d.row), Some(5));
}

#[test]
fn a_refused_subject_is_marked_and_the_others_keep_their_records() {
    let r = read("ID,TIME,DV,AMT,EVID\n1,0,,,1\n1,1,3,,0\n2,0,,50,1\n2,1,4,,0\n");
    assert_eq!(codes(&r), ["dose_without_amount"]);
    let [one, two] = r.dataset.subjects.as_slice() else {
        panic!("two subjects");
    };
    assert_eq!(one.refusal.as_deref(), Some("dose_without_amount"));
    assert!(one.doses.is_empty());
    assert_eq!(two.refusal, None);
    assert_eq!((two.doses.len(), two.observations.len()), (1, 1));
}

#[test]
fn one_check_per_code_with_the_count_of_the_others() {
    let r = read("ID,TIME,DV,AMT,EVID\n1,0,,0,1\n1,1,3,,0\n2,0,,-5,1\n2,1,4,,0\n");
    assert_eq!(codes(&r), ["non_positive_amount"]);
    let message = &r.checks.first().unwrap().message;
    assert!(message.contains("row 1 of subject 1"), "{message}");
    assert!(message.contains("1 more of this kind"), "{message}");
}

#[test]
fn two_observations_at_one_time_are_still_duplicate_times() {
    let r = read("ID,TIME,DV,AMT,EVID\n1,0,,10,1\n1,0,1,,0\n1,2,3,,0\n1,2,4,,0\n");
    assert_eq!(codes(&r), ["duplicate_times"]);
    assert_eq!(
        r.dataset
            .subjects
            .first()
            .and_then(|s| s.refusal.as_deref()),
        Some("duplicate_times")
    );
}

#[test]
fn the_profile_layout_has_the_implied_dose_and_keeps_legacy_columns_other() {
    let r = read("subject,time,conc,dose,limit,rate\n1,0.5,3,100,0.5,50\n1,1,2,100,0.5,50\n");
    assert!(r.checks.is_empty(), "{:?}", r.checks);
    assert_eq!(r.dataset.layout, Layout::Profile);
    for name in ["limit", "rate"] {
        let column = r.table.columns.iter().find(|c| c.name == name).unwrap();
        assert_eq!(column.role, ColumnRole::Other, "{name}");
    }
    let subject = r.dataset.subjects.first().unwrap();
    let dose = subject.doses.first().unwrap();
    assert_eq!((dose.row, dose.time, dose.amount), (None, 0.0, 100.0));
    assert_eq!(subject.observations.first().map(|o| o.row), Some(1));
}

#[test]
fn two_doses_in_one_profile_are_the_check_different_doses() {
    let r = read("subject,time,conc,dose\n1,1,3,100\n1,2,2,150\n2,1,3,50\n");
    assert_eq!(codes(&r), ["different_doses"]);
    let message = &r.checks.first().unwrap().message;
    assert!(
        message.contains("subject 1 has two different doses (100 and 150)"),
        "{message}"
    );
    assert_eq!(r.dataset.subjects.get(1).unwrap().refusal, None);
}

#[test]
fn declared_covariates_are_one_value_per_subject() {
    let csv = "ID,TIME,DV,AMT,EVID,WT\n1,0,,10,1,70\n1,1,3,,0,\n2,0,,10,1,80\n2,1,3,,0,81\n";
    let options = CsvOptions {
        covariates: vec!["WT".to_owned()],
        ..CsvOptions::default()
    };
    let r = readings(csv.as_bytes(), &options).unwrap().remove(0);
    assert_eq!(codes(&r), ["covariate_varies"]);
    let one = r.dataset.subjects.first().unwrap();
    assert_eq!(
        one.covariates.get("WT"),
        Some(&Some(CovariateValue::Number(70.0)))
    );
    // Not declared: the same column is just carried.
    let r = read(csv);
    assert!(r.checks.is_empty());
    assert!(r.dataset.subjects.iter().all(|s| s.covariates.is_empty()));
}

/// What a file must give: no reading at all (the error code), or a reading whose checks have
/// exactly these codes, in this order.
enum Gives {
    Error(&'static str),
    Checks(&'static [&'static str]),
}

#[test]
fn odd_files_give_refusals_or_errors_never_a_crash() {
    use Gives::{Checks, Error};
    let cases: [(&str, Gives); 9] = [
        // No row at all, or no value in any row of the time or value column: no reading.
        ("ID,TIME,DV,EVID\n", Error("csv_no_reading")),
        ("ID,TIME,DV,AMT,EVID\n1,,,,\n", Error("csv_no_reading")),
        ("ID,TIME,DV,AMT,EVID\n,,,,\n,,,,\n", Error("csv_no_reading")),
        // Text where numbers are needed: the roles are not given, the event type is unknown, and
        // the subject then has nothing to analyse.
        (
            "ID,TIME,DV,AMT,EVID,CMT,MDV,CENS,LIMIT\n1,0,1,1,x,y,z,w,v\n",
            Checks(&["unknown_event", "subject_without_observation"]),
        ),
        // One observation and no subject column: one subject, nothing to refuse.
        ("TIME,DV,EVID\n0,1,0\n", Checks(&[])),
        (
            "TIME,DV,EVID,RATE,DUR,AMT\n0,,1,,,5\n1,2,0,,,\n",
            Checks(&[]),
        ),
        // A dose and no amount column at all.
        (
            "TIME,DV,EVID\n0,,1\n1,2,0\n",
            Checks(&["dose_without_amount"]),
        ),
        // Only a dose row: no observation. An event cell left empty: unknown event.
        (
            "ID,TIME,DV,AMT,EVID
1,0,,5,1
2,0,,5,1
2,1,3,,0
",
            Checks(&["subject_without_observation"]),
        ),
        (
            "ID,TIME,DV,AMT,EVID\n1,0,,5,\n1,1,2,,0\n",
            Checks(&["unknown_event"]),
        ),
    ];
    for (csv, expected) in cases {
        let found = readings(csv.as_bytes(), &CsvOptions::default());
        match (expected, found) {
            (Error(code), Err(e)) => assert_eq!(e.code, code, "{csv:?}: {}", e.message),
            (Checks(want), Ok(list)) => {
                let first = list
                    .first()
                    .unwrap_or_else(|| panic!("{csv:?}: no reading"));
                assert_eq!(codes(first), want, "{csv:?}: {:?}", first.checks);
            }
            (Error(code), Ok(list)) => {
                panic!(
                    "{csv:?}: wanted the error {code}, got {} readings",
                    list.len()
                )
            }
            (Checks(want), Err(e)) => {
                panic!(
                    "{csv:?}: wanted the checks {want:?}, got {} ({})",
                    e.code, e.message
                )
            }
        }
    }
}

fn message_of(csv: &str, code: &str) -> String {
    let r = read(csv);
    r.checks
        .iter()
        .find(|c| c.code == code)
        .map(|c| c.message.clone())
        .unwrap_or_else(|| panic!("no check {code} in {:?}", r.checks))
}

#[test]
fn a_dose_without_amount_says_what_is_missing() {
    // No amount column: the sentence says so and does not name a column `amt`.
    let none = message_of("TIME,DV,EVID\n0,,1\n1,2,0\n", "dose_without_amount");
    assert!(none.contains("the table has no amount column"), "{none}");
    assert!(!none.contains("in column `amt`"), "{none}");
    // An amount column of text: it is named, and said to hold text.
    let text = message_of(
        "TIME,DV,EVID,AMT\n0,,1,ten\n1,2,0,\n",
        "dose_without_amount",
    );
    assert!(text.contains("column `AMT` holds text"), "{text}");
    // An empty cell of an amount column that exists: as before.
    let empty = message_of(
        "TIME,DV,EVID,AMT\n0,,1,\n1,2,0,\n2,,1,5\n",
        "dose_without_amount",
    );
    assert!(empty.contains("give the amount in column `AMT`"), "{empty}");
}

#[test]
fn a_subject_without_observation_counts_its_dose_rows_in_the_singular_too() {
    let one = message_of(
        "ID,TIME,DV,AMT,EVID\n1,0,,5,1\n2,0,,5,1\n2,1,3,,0\n",
        "subject_without_observation",
    );
    assert!(one.contains("no observation (only 1 dose row)"), "{one}");
    let two = message_of(
        "ID,TIME,DV,AMT,EVID\n1,0,,5,1\n1,4,,5,1\n2,0,,5,1\n2,1,3,,0\n",
        "subject_without_observation",
    );
    assert!(two.contains("no observation (only 2 dose rows)"), "{two}");
    let none = message_of(
        "ID,TIME,DV,AMT,EVID\n1,0,2,,x\n2,0,,5,1\n2,1,3,,0\n",
        "subject_without_observation",
    );
    assert!(!none.contains("only 0"), "{none}");
    assert!(none.contains("has no observation row"), "{none}");
}

#[test]
fn rate_duration_and_route_on_an_observation_row_are_ignored_with_a_note() {
    let csv = "ID,TIME,DV,AMT,EVID,RATE,DUR,ROUTE\n\
        1,0,,100,1,50,,iv\n1,1,3,,0,50,2,iv\n1,2,2,,0,0,0,\n1,3,1,,0,,,iv\n";
    let r = read(csv);
    assert!(r.checks.is_empty(), "{:?}", r.checks);
    let notes = r.table.notes.join("\n");
    // The rate is filled on one observation row, the duration on one, the route on two.
    assert!(
        notes.contains("column `RATE` has a value on 1 observation row (the first is row 2)"),
        "{notes}"
    );
    assert!(
        notes.contains("column `DUR` has a value on 1 observation row (the first is row 2)"),
        "{notes}"
    );
    assert!(
        notes.contains("column `ROUTE` has a value on 2 observation rows (the first is row 2)"),
        "{notes}"
    );
    assert!(notes.contains("ignored on observation rows"), "{notes}");
    // Ignored: the dose is the dose row's own (an infusion of 50 over 2).
    let subject = r.dataset.subjects.first().unwrap();
    assert_eq!(subject.observations.len(), 3);
    let dose = subject.doses.first().unwrap();
    assert_eq!((dose.rate, dose.duration), (Some(50.0), Some(2.0)));
    // A table whose observation rows leave them empty (or 0) has no such note.
    let clean = read(EVENT);
    assert!(
        !clean
            .table
            .notes
            .iter()
            .any(|n| n.contains("observation row")),
        "{:?}",
        clean.table.notes
    );
}

#[test]
fn declared_covariate_names_match_the_header_without_regard_to_case() {
    let csv = "ID,TIME,DV,AMT,EVID,Wt\n1,0,,10,1,70\n1,1,3,,0,70\n";
    for declared in ["wt", "WT", "Wt"] {
        let options = CsvOptions {
            covariates: vec![declared.to_owned()],
            ..CsvOptions::default()
        };
        let r = readings(csv.as_bytes(), &options).unwrap().remove(0);
        let column = r.table.columns.iter().find(|c| c.name == "Wt").unwrap();
        assert_eq!(column.role, ColumnRole::Covariate, "declared as {declared}");
        assert!(
            !r.table
                .notes
                .iter()
                .any(|n| n.contains("no column of that name")),
            "{:?}",
            r.table.notes
        );
        assert_eq!(
            r.dataset
                .subjects
                .first()
                .and_then(|s| s.covariates.get("Wt")),
            Some(&Some(CovariateValue::Number(70.0)))
        );
    }
    // A name that matches no header is still a note.
    let options = CsvOptions {
        covariates: vec!["weight".to_owned()],
        ..CsvOptions::default()
    };
    let r = readings(csv.as_bytes(), &options).unwrap().remove(0);
    assert!(
        r.table
            .notes
            .iter()
            .any(|n| n.contains("no column of that name"))
    );
}

#[test]
fn a_rate_or_duration_that_is_not_finite_when_derived_is_refused() {
    // 1e308 / 1e-10 does not fit a number: the duration is infinite.
    let a = message_of(
        "ID,TIME,DV,AMT,EVID,RATE\n1,0,,1e308,1,1e-10\n1,1,3,,0,\n",
        "rate_duration_mismatch",
    );
    assert!(
        a.contains("duration (the amount divided by the rate) is inf"),
        "{a}"
    );
    assert!(a.contains("not a finite number"), "{a}");
    // The same from a tiny duration: the rate is infinite.
    let b = message_of(
        "ID,TIME,DV,AMT,EVID,DUR\n1,0,,1e308,1,1e-10\n1,1,3,,0,\n",
        "rate_duration_mismatch",
    );
    assert!(
        b.contains("rate (the amount divided by the duration) is inf"),
        "{b}"
    );
    // The subject is refused and the others keep their records.
    let r = read("ID,TIME,DV,AMT,EVID,RATE\n1,0,,1e308,1,1e-10\n1,1,3,,0,\n2,0,,5,1,\n2,1,3,,0,\n");
    assert_eq!(
        r.dataset
            .subjects
            .first()
            .and_then(|s| s.refusal.as_deref()),
        Some("rate_duration_mismatch")
    );
    assert_eq!(
        r.dataset.subjects.get(1).and_then(|s| s.refusal.as_deref()),
        None
    );
}

#[test]
fn the_compartment_column_is_kept_and_only_several_endpoints_are_refused() {
    let one = read("ID,TIME,DV,AMT,EVID,CMT\n1,0,,10,1,1\n1,1,3,,0,2\n1,2,2,,0,2\n");
    assert!(one.checks.is_empty(), "{:?}", one.checks);
    let two = read(
        "ID,TIME,DV,AMT,EVID,CMT\n1,0,,10,1,1\n1,1,3,,0,2\n1,2,2,,0,3\n2,0,,10,1,1\n2,1,3,,0,2\n",
    );
    assert_eq!(codes(&two), ["several_endpoints"]);
    let message = &two.checks.first().unwrap().message;
    assert!(
        message.contains("subject 1 are in several compartments (2 and 3)"),
        "{message}"
    );
    assert_eq!(
        two.dataset
            .subjects
            .get(1)
            .and_then(|s| s.refusal.as_deref()),
        None
    );
    // A column of names is the same: compartments are compared as written.
    let names =
        read("ID,TIME,DV,AMT,EVID,CMT\n1,0,,10,1,depot\n1,1,3,,0,central\n1,2,2,,0,central\n");
    assert!(names.checks.is_empty(), "{:?}", names.checks);
}

#[test]
fn mdv_leaves_a_row_out_and_other_values_are_refused() {
    let r = read("ID,TIME,DV,AMT,EVID,MDV\n1,0,,10,1,1\n1,1,3,,0,0\n1,2,2,,0,1\n1,3,1,,0,\n");
    assert!(r.checks.is_empty(), "{:?}", r.checks);
    let s = r.dataset.subjects.first().unwrap();
    // The row with mdv = 1 is a record that is not used (counted as missing), the others are kept.
    let statuses: Vec<_> = s.observations.iter().map(|o| o.status).collect();
    assert_eq!(
        statuses,
        [
            ObservationStatus::Observed,
            ObservationStatus::Missing,
            ObservationStatus::Observed
        ]
    );
    assert_eq!((s.n_obs, s.n_observed, s.n_missing), (3, 2, 1));
    // The value of a skipped row is not read.
    assert_eq!(s.observations.get(1).and_then(|o| o.value), None);
    let bad = read("ID,TIME,DV,AMT,EVID,MDV\n1,0,,10,1,\n1,1,3,,0,2\n");
    assert_eq!(codes(&bad), ["unknown_mdv"]);
}

#[test]
fn cens_marks_a_row_below_the_limit_and_keeps_the_limit() {
    let csv = "ID,TIME,DV,AMT,EVID,CENS,LIMIT\n\
        1,0,,10,1,,\n1,1,3,,0,0,\n1,2,0.5,,0,1,\n1,3,0.4,,0,1,0.1\n1,4,<0.5,,0,,\n1,5,<0.3,,0,1,0.1\n";
    let r = read(csv);
    assert!(r.checks.is_empty(), "{:?}", r.checks);
    let s = r.dataset.subjects.first().unwrap();
    let rows: Vec<_> = s
        .observations
        .iter()
        .map(|o| (o.row, o.status, o.value, o.limit, o.lower_limit))
        .collect();
    assert_eq!(
        rows,
        [
            (2, ObservationStatus::Observed, Some(3.0), None, None),
            // cens = 1 and only the value cell: the value cell is the limit, the value is not kept.
            (3, ObservationStatus::Blq, None, Some(0.5), None),
            // cens = 1 with the value cell and a limit column: the column is the lower bound.
            (4, ObservationStatus::Blq, None, Some(0.4), Some(0.1)),
            // A `<0.5` text cell: the limit is the number after the sign.
            (5, ObservationStatus::Blq, None, Some(0.5), None),
            // Both a `<` cell and a censored row: the cell's number is the limit.
            (6, ObservationStatus::Blq, None, Some(0.3), Some(0.1)),
        ]
    );
    assert_eq!((s.n_obs, s.n_observed, s.n_blq), (5, 1, 4));
    // A censored row whose value cell is empty is refused: the limit is missing.
    let missing = read("ID,TIME,DV,AMT,EVID,CENS,LIMIT\n1,0,,10,1,,\n1,1,,,0,1,0.1\n1,2,2,,0,,\n");
    assert_eq!(codes(&missing), ["unsupported_censoring"]);
    let message = &missing.checks.first().unwrap().message;
    assert!(message.contains("the limit is missing"), "{message}");
    // Right-censored and unknown codes are refused.
    let right = read("ID,TIME,DV,AMT,EVID,CENS\n1,0,,10,1,\n1,1,3,,0,-1\n");
    assert_eq!(codes(&right), ["unsupported_censoring"]);
    // A word without a number, and a row not censored, give no limit; the field is not written.
    let plain = read("ID,TIME,DV,AMT,EVID\n1,0,,10,1\n1,1,BLQ,,0\n1,2,4,,0\n");
    let o = plain
        .dataset
        .subjects
        .first()
        .unwrap()
        .observations
        .first()
        .unwrap();
    assert_eq!(
        (o.status, o.limit, o.lower_limit),
        (ObservationStatus::Blq, None, None)
    );
    let json = serde_json::to_string(o).unwrap();
    assert!(!json.contains("limit"), "{json}");
    // The word BLQ on a censored row has no number to give: no limit, rather than 0.
    let word = read("ID,TIME,DV,AMT,EVID,CENS\n1,0,,10,1,\n1,1,BLQ,,0,1\n1,2,4,,0,\n");
    let o = word
        .dataset
        .subjects
        .first()
        .unwrap()
        .observations
        .first()
        .unwrap();
    assert_eq!((o.status, o.limit), (ObservationStatus::Blq, None));
}

#[test]
fn a_limit_after_the_less_than_sign_is_kept_in_a_profile_file_too() {
    let r = read("subject,time,conc,dose\n1,0.5,3,100\n1,2,< 0.25,100\n1,4,BLQ,100\n");
    assert!(r.checks.is_empty(), "{:?}", r.checks);
    let limits: Vec<_> = r
        .dataset
        .subjects
        .first()
        .unwrap()
        .observations
        .iter()
        .map(|o| (o.status, o.limit))
        .collect();
    assert_eq!(
        limits,
        [
            (ObservationStatus::Observed, None),
            (ObservationStatus::Blq, Some(0.25)),
            (ObservationStatus::Blq, None)
        ]
    );
    // The serialized form of the table is what it was: a BLQ cell is 0 and marked, no limit.
    let json = serde_json::to_value(&r.table).unwrap();
    let mut members: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    assert_eq!(members, ["columns", "decimal_comma", "delimiter", "notes"]);
    let conc = r.table.columns.iter().find(|c| c.name == "conc").unwrap();
    assert_eq!(
        conc.data,
        ColumnData::Number(vec![Some(3.0), Some(0.0), Some(0.0)])
    );
    assert_eq!(conc.blq, [1, 2]);
}

#[test]
fn the_event_types_are_the_five_codes_and_their_words() {
    // A table with a dose row, then an observation row (or a row of the type under test).
    let with = |event: &str| {
        let is_dose = event == "dose" || event == "Dose" || event == "1";
        let (dv, amt) = if is_dose { ("", "5") } else { ("3", "") };
        let first = if is_dose { "" } else { "1,0,,10,1\n" };
        read(&format!(
            "ID,TIME,DV,AMT,EVID\n{first}1,1,{dv},{amt},{event}\n1,2,2,,0\n"
        ))
    };
    // Accepted: 0 and 1, and the words in any case.
    for word in ["0", "obs", "OBS", "Observation", "1", "dose", "Dose"] {
        let r = with(word);
        assert!(r.checks.is_empty(), "{word}: {:?}", r.checks);
    }
    // Refused as unsupported: 2, 3 and 4 with their meaning.
    for (word, meaning) in [("2", "other"), ("3", "reset"), ("4", "reset and dose")] {
        let r = with(word);
        assert_eq!(codes(&r), ["unsupported_event"], "{word}");
        let message = &r.checks.first().unwrap().message;
        assert!(
            message.contains(&format!("(`{meaning}`)")),
            "{word}: {message}"
        );
        assert!(
            message.contains("supported types are 0 (observation) and 1 (dose)"),
            "{message}"
        );
    }
    // Anything else is an unknown event, and so is an empty cell (never read as 0 or 1).
    for word in ["5", "-1", "0.5", "visit", ""] {
        let r = with(word);
        assert_eq!(
            codes(&r).first(),
            Some(&"unknown_event"),
            "{word:?}: {:?}",
            r.checks
        );
    }
}

/// The position of a role in `ColumnRole::ALL`. The match has no wildcard arm: a new role does not
/// compile until it is placed here, and the test below then fails until `ALL` lists it.
fn position(role: ColumnRole) -> usize {
    match role {
        ColumnRole::Time => 0,
        ColumnRole::Concentration => 1,
        ColumnRole::Subject => 2,
        ColumnRole::Dose => 3,
        ColumnRole::Route => 4,
        ColumnRole::Event => 5,
        ColumnRole::Rate => 6,
        ColumnRole::Dur => 7,
        ColumnRole::Cmt => 8,
        ColumnRole::Mdv => 9,
        ColumnRole::Cens => 10,
        ColumnRole::Limit => 11,
        ColumnRole::Covariate => 12,
        ColumnRole::Other => 13,
    }
}

#[test]
fn every_column_role_is_in_the_list_the_schema_is_built_from() {
    for (i, role) in ColumnRole::ALL.iter().enumerate() {
        assert_eq!(position(*role), i, "{} is out of place in ALL", role.id());
    }
    let largest = ColumnRole::ALL
        .iter()
        .map(|r| position(*r))
        .max()
        .unwrap_or(0);
    assert_eq!(largest + 1, ColumnRole::ALL.len());
    for role in ColumnRole::ALL {
        let json = serde_json::to_value(role).unwrap();
        assert_eq!(json.as_str(), Some(role.id()), "serde id differs from id()");
    }
}

/// What `Worksheet::profile` says about every subject of the table: the code of its refusal, or
/// nothing. The check `different_doses` must fire for exactly the subjects it refuses with
/// `dose_varies`, because it replaces that refusal at import (T-150 review, fix 1).
fn dose_refusals_of_profile(csv: &str) -> (Vec<String>, Vec<String>) {
    let r = read(csv);
    let mut project = Project::new("agreement");
    let id = project
        .add_worksheet("sheet", r.table.columns.clone())
        .unwrap();
    let sheet = project.worksheet(id).unwrap();
    let refused_by_profile: Vec<String> = sheet
        .subjects()
        .into_iter()
        .filter(|s| {
            sheet
                .profile(Some(s))
                .is_err_and(|e| e.code == "dose_varies")
        })
        .collect();
    let refused_by_check: Vec<String> = r
        .dataset
        .subjects
        .iter()
        .filter(|s| s.refusal.as_deref() == Some("different_doses"))
        .map(|s| s.id.clone())
        .collect();
    (refused_by_profile, refused_by_check)
}

#[test]
fn different_doses_fires_exactly_when_profile_refuses_with_dose_varies() {
    // A near-equal pair: 0.1 + 0.2 and 0.3 differ in the last bit, so `!=` says different, and so
    // must the check (it compares the parsed numbers with the same `!=`, not with a tolerance).
    // An equal pair written differently (100 and 100.0) is one dose for both.
    let csv = "subject,time,conc,dose\n\
        near,1,3,0.30000000000000004\nnear,2,2,0.3\n\
        equal,1,3,100\nequal,2,2,100.0\n\
        far,1,3,100\nfar,2,2,150\n\
        late,1,3,\nlate,2,2,100\nlate,3,1,100\n";
    let (profile, check) = dose_refusals_of_profile(csv);
    assert_eq!(profile, ["near", "far"]);
    assert_eq!(check, profile);
    // The two doses of `near` really are two numbers, not a printing difference.
    assert_ne!(0.30000000000000004_f64, 0.3_f64);
}

#[test]
fn a_blank_subject_cell_is_not_in_the_profile_of_a_subject_and_not_in_its_doses() {
    // `profile` skips the row whose subject cell is blank, so its dose cannot make the doses of
    // subject 1 differ, with one subject as with several.
    let csv = "subject,time,conc,dose\n1,1,3,100\n,2,2,150\n1,3,1,100\n";
    let (profile, check) = dose_refusals_of_profile(csv);
    assert_eq!(profile, Vec::<String>::new());
    assert_eq!(check, profile);
    let r = read(csv);
    assert_eq!(r.dataset.subjects.first().map(|s| s.n_rows), Some(2));
}

#[test]
fn a_dose_check_of_a_profile_file_stays_with_the_reading_the_file_gives() {
    // The ranking of the readings leaves the dataset checks of a profile file out of its key (a
    // wrong delimiter or decimal mark must not climb over the right reading); the pinned files of
    // `tests/legacy_regression.rs` hold the order of every legacy file. Here: the reading with the
    // dose problem is still the first, and its checks are the dataset check alone.
    let csv = "subject;time;conc;dose
1;1;3;100
1;2;2;150
";
    let all = readings(csv.as_bytes(), &CsvOptions::default()).unwrap();
    assert_eq!(all.first().map(|r| r.table.delimiter), Some(';'));
    assert_eq!(all.first().map(codes), Some(vec!["different_doses"]));
}
