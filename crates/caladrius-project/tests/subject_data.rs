#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: helpers panic on purpose
//! T-163: the data of one subject for an analysis, from the dataset (DAT-USE-01 to 03, DAT-ERR-08,
//! FIT-ML-20). The counts of the dataset are compared with the counting that the run layer did
//! before (`Worksheet::profile`, then the flags of each row), which is what the fits pinned.

use caladrius_project::{
    CsvOptions, ImportedTable, ObservationCounts, Project, Purpose, Worksheet,
};
use std::path::PathBuf;

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

fn project_of(relative: &str) -> (Project, caladrius_project::WorksheetId) {
    let path = oracle().join(relative);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let table = ImportedTable::from_csv(&bytes, &CsvOptions::default()).unwrap();
    let mut project = Project::new("p");
    let id = project.add_worksheet("t", table.columns).unwrap();
    (project, id)
}

/// What the run layer counted before this change, from the profile of the old path.
fn old_counts(ws: &Worksheet, subject: &str) -> ObservationCounts {
    let profile = ws.profile(Some(subject)).unwrap();
    let n_blq = profile.blq.iter().filter(|b| **b).count();
    let n_missing = profile.conc.iter().filter(|c| !c.is_finite()).count();
    let n_used = profile
        .conc
        .iter()
        .zip(&profile.blq)
        .filter(|(c, below)| c.is_finite() && !**below)
        .count();
    ObservationCounts {
        n_used,
        n_missing,
        n_blq,
    }
}

#[test]
fn the_counts_of_the_dataset_equal_the_old_counting_on_every_legacy_file() {
    let mut subjects_checked = 0;
    for entry in std::fs::read_dir(oracle().join("data")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let (project, id) = project_of(&format!("data/{name}"));
        let ws = project.worksheet(id).unwrap();
        let dataset = ws.dataset();
        for subject in ws.subjects() {
            let Ok(old) = ws.profile(Some(&subject)) else {
                // The old path refuses it (a negative time, a varying dose...): nothing to compare.
                assert!(
                    ws.subject_data(&dataset, Some(&subject), Purpose::Fit)
                        .is_err(),
                    "{name}: subject {subject} is refused by the old path but not by the new one"
                );
                continue;
            };
            let data = ws
                .subject_data(&dataset, Some(&subject), Purpose::Fit)
                .unwrap_or_else(|e| panic!("{name}: subject {subject}: {e}"));
            assert_eq!(
                data.counts,
                old_counts(ws, &subject),
                "{name}: subject {subject}"
            );
            // The profile layout keeps the old path: the very same profile.
            // (A missing value is NaN, which is not equal to itself: compare the printed forms.)
            assert_eq!(
                format!("{:?}", data.profile),
                format!("{old:?}"),
                "{name}: subject {subject}"
            );
            subjects_checked += 1;
        }
    }
    assert!(subjects_checked >= 40, "only {subjects_checked} subjects");
}

#[test]
fn the_event_layout_counts_like_the_profile_layout_of_the_same_data() {
    for (profile, event) in [
        ("data/theoph.csv", "data/population/event_theoph.csv"),
        ("data/indometh.csv", "data/population/event_indometh.csv"),
    ] {
        let (pp, pid) = project_of(profile);
        let (ep, eid) = project_of(event);
        let (pws, ews) = (pp.worksheet(pid).unwrap(), ep.worksheet(eid).unwrap());
        let (pd, ed) = (pws.dataset(), ews.dataset());
        assert_eq!(pws.analysis_subjects(&pd), ews.analysis_subjects(&ed));
        for subject in pws.analysis_subjects(&pd) {
            let p = pws.subject_data(&pd, Some(&subject), Purpose::Fit).unwrap();
            let e = ews.subject_data(&ed, Some(&subject), Purpose::Fit).unwrap();
            assert_eq!(p.counts, e.counts, "{profile}: subject {subject}");
            assert_eq!(
                p.profile.time, e.profile.time,
                "{profile}: subject {subject}"
            );
            assert_eq!(
                p.profile.conc, e.profile.conc,
                "{profile}: subject {subject}"
            );
            assert_eq!(
                p.profile.dose, e.profile.dose,
                "{profile}: subject {subject}"
            );
        }
    }
}

#[test]
fn the_refusal_of_a_subject_is_its_own_sentence_and_code() {
    let (project, id) = project_of("data/population/event_theoph_dose0.csv");
    let ws = project.worksheet(id).unwrap();
    let dataset = ws.dataset();
    let refused = ws
        .subject_data(&dataset, Some("5"), Purpose::Nca)
        .unwrap_err();
    assert_eq!(refused.code, "non_positive_amount");
    assert!(
        refused.message.contains("row 49 of subject 5"),
        "{}",
        refused.message
    );
    for subject in ws.analysis_subjects(&dataset) {
        if subject != "5" {
            assert!(
                ws.subject_data(&dataset, Some(&subject), Purpose::Nca)
                    .is_ok(),
                "subject {subject}"
            );
        }
    }
}

#[test]
fn several_subjects_and_none_named_asks_which_one_in_both_layouts() {
    for file in ["data/theoph.csv", "data/population/event_theoph.csv"] {
        let (project, id) = project_of(file);
        let ws = project.worksheet(id).unwrap();
        let dataset = ws.dataset();
        let err = ws.subject_data(&dataset, None, Purpose::Fit).unwrap_err();
        assert_eq!(err.code, "subject_required", "{file}");
        let err = ws
            .subject_data(&dataset, Some("nobody"), Purpose::Fit)
            .unwrap_err();
        assert_eq!(err.code, "unknown_subject", "{file}");
    }
}
