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

#[test]
fn odd_files_give_refusals_or_errors_never_a_crash() {
    for csv in [
        "ID,TIME,DV,EVID\n",
        "ID,TIME,DV,AMT,EVID\n1,,,,\n",
        "ID,TIME,DV,AMT,EVID\n,,,,\n,,,,\n",
        "ID,TIME,DV,AMT,EVID,CMT,MDV,CENS,LIMIT\n1,0,1,1,x,y,z,w,v\n",
        "TIME,DV,EVID\n0,1,0\n",
        "TIME,DV,EVID,RATE,DUR,AMT\n0,,1,,,5\n1,2,0,,,\n",
    ] {
        let _ = readings(csv.as_bytes(), &CsvOptions::default());
    }
}
