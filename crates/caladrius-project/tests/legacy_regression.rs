#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: helpers panic on purpose
//! Regression test of T-150 (specs/data.md DAT-LEG-01, DAT-LEG-02, OD-03): every legacy file loads
//! as it did before the event table was added.
//!
//! `legacy_pins/<file>.json` holds, for each file of `oracle/data/` (and `fit/`, `fit/ml/`) and the
//! profile-layout files of `oracle/data/population/`, what the code of `iter3/ml` at commit 5815119
//! gave, before the first edit of T-150: for three ways of reading the file, the table (columns with
//! role, unit and data, delimiter, decimal mark, notes) and the checks of every reading, the table of
//! `ImportedTable::from_csv`, then for the default reading the subjects, the profile of every
//! subject, the unit warnings, the derived units and the stored form of a project that holds the
//! worksheet. The files were written once by a throwaway test that called `snapshot::render` on each
//! file (not kept: the point of the pins is that they do not follow the importer).
//!
//! This test compares the text byte for byte. A difference is a finding to report, **never a reason
//! to rewrite a pin**. The stored form is also loaded and saved again, to check that a project saved
//! before the change opens and gives the same bytes.

#[path = "legacy_regression/snapshot.rs"]
mod snapshot;

use caladrius_project::Project;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The CSV a pin file was made from: its name is the path under `oracle/data` with `/` as `__`.
fn source_of(pin_name: &str) -> String {
    pin_name.trim_end_matches(".json").replace("__", "/")
}

fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[test]
fn legacy_files_read_exactly_as_pinned() {
    let pins = root().join("tests/legacy_pins");
    let mut names: Vec<String> = std::fs::read_dir(&pins)
        .unwrap()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    assert!(names.len() >= 27, "only {} pinned files", names.len());
    let mut differences = Vec::new();
    for name in &names {
        let file = source_of(name);
        let bytes = std::fs::read(root().join("../../oracle/data").join(&file))
            .unwrap_or_else(|e| panic!("cannot read oracle/data/{file}: {e}"));
        let pinned = lf(&std::fs::read_to_string(pins.join(name)).unwrap());
        let now = snapshot::render(&file, &bytes);
        if now != pinned {
            let at = now
                .bytes()
                .zip(pinned.bytes())
                .position(|(a, b)| a != b)
                .unwrap_or(now.len().min(pinned.len()));
            let show = |s: &str| {
                s.get(at.saturating_sub(60)..(at + 120).min(s.len()))
                    .unwrap_or("?")
                    .to_owned()
            };
            differences.push(format!(
                "{file}: first difference at byte {at}\n  pinned: ...{}\n  now:    ...{}",
                show(&pinned),
                show(&now)
            ));
        }
    }
    assert!(
        differences.is_empty(),
        "a legacy file no longer reads as pinned; report it, do not re-pin:\n{}",
        differences.join("\n")
    );
}

#[test]
fn projects_saved_before_the_change_still_open_and_save_the_same_bytes() {
    let pins = root().join("tests/legacy_pins");
    let mut checked = 0;
    for entry in std::fs::read_dir(&pins).unwrap().flatten() {
        let text = lf(&std::fs::read_to_string(entry.path()).unwrap());
        let pin: serde_json::Value = serde_json::from_str(&text).unwrap();
        let Some(saved) = pin["project_bytes"].as_str() else {
            continue;
        };
        let project = Project::from_bytes(saved.as_bytes())
            .unwrap_or_else(|e| panic!("{}: a saved project no longer opens: {e}", pin["file"]));
        let again = String::from_utf8(project.to_bytes().unwrap()).unwrap();
        assert_eq!(again, saved, "{}: saved form changed", pin["file"]);
        checked += 1;
    }
    assert!(checked >= 27, "only {checked} saved projects were checked");
}
