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
//! `legacy_pins/inputs__<file>.json` are the same for the three legacy-style inputs of
//! `tests/legacy_inputs/` that carry a check (T-150 review, Major 2): `backwards_times.csv`
//! (`time_not_increasing`), `duplicate_times.csv` (`duplicate_times`) and `two_doses.csv` (two
//! doses in one subject, which `Worksheet::profile` refuses with `dose_varies`). They were pinned
//! from the code of 5815119, not from the new code: `git archive 5815119` extracted outside the
//! repository, `tests/legacy_regression/snapshot.rs` copied into it with a throwaway test that wrote
//! `snapshot::render` of each input, run with its own target directory. The new code has not
//! produced any pin.
//!
//! One difference from the pins is allowed and named: `DIFFERENT_DOSES_FILES` lists the inputs on
//! which the new code adds one check, `different_doses` (DAT-ERR-06, T-150: it replaces the refusal
//! `dose_varies` at import). For them the new output must equal the pinned one plus that one entry
//! at the head of the `checks` of each reading, nothing else.
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

/// The inputs on which the one allowed difference shows: the new `different_doses` check.
const DIFFERENT_DOSES_FILES: &[&str] = &["two_doses.csv"];

/// The file a pin was made from: its text for `snapshot::render`, and its bytes. A pin named
/// `inputs__x.csv.json` was made from `tests/legacy_inputs/x.csv`, the others from `oracle/data`.
fn file_of(pin_name: &str) -> (String, Vec<u8>) {
    let file = source_of(pin_name);
    match file.strip_prefix("inputs/") {
        Some(input) => {
            let bytes = std::fs::read(root().join("tests/legacy_inputs").join(input))
                .unwrap_or_else(|e| panic!("cannot read tests/legacy_inputs/{input}: {e}"));
            (input.to_owned(), bytes)
        }
        None => {
            let bytes = std::fs::read(root().join("../../oracle/data").join(&file))
                .unwrap_or_else(|e| panic!("cannot read oracle/data/{file}: {e}"));
            (file, bytes)
        }
    }
}

/// `now` without the one allowed difference: the first check of each reading, when it is
/// `different_doses`, is taken out. Returns how many were taken out.
fn without_different_doses(now: &str) -> (serde_json::Value, usize) {
    let mut value: serde_json::Value = serde_json::from_str(now).unwrap();
    let mut removed = 0;
    if let Some(by_options) = value["by_options"].as_object_mut() {
        for entry in by_options.values_mut() {
            // `get_mut`, not indexing: indexing a value mutably adds the missing members.
            let Some(list) = entry
                .get_mut("readings")
                .and_then(|r| r.get_mut("ok"))
                .and_then(|r| r.as_array_mut())
            else {
                continue;
            };
            for reading in list {
                let Some(checks) = reading.get_mut("checks").and_then(|c| c.as_array_mut()) else {
                    continue;
                };
                if checks
                    .first()
                    .is_some_and(|c| c["code"] == "different_doses")
                {
                    checks.remove(0);
                    removed += 1;
                }
            }
        }
    }
    (value, removed)
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
    assert!(names.len() >= 30, "only {} pinned files", names.len());
    let mut differences = Vec::new();
    for name in &names {
        let (file, bytes) = file_of(name);
        let pinned = lf(&std::fs::read_to_string(pins.join(name)).unwrap());
        let mut now = snapshot::render(&file, &bytes);
        if DIFFERENT_DOSES_FILES.contains(&file.as_str()) {
            // The one allowed difference: take the new check out and compare what is left.
            let (rest, removed) = without_different_doses(&now);
            assert!(removed > 0, "{file}: the check different_doses is missing");
            let before: serde_json::Value = serde_json::from_str(&pinned).unwrap();
            let had = before["by_options"].as_object().map_or(0, |o| {
                o.values()
                    .filter_map(|e| e["readings"]["ok"].as_array())
                    .flatten()
                    .filter(|r| {
                        r["checks"]
                            .as_array()
                            .is_some_and(|c| c.iter().any(|x| x["code"] == "different_doses"))
                    })
                    .count()
            });
            assert_eq!(had, 0, "{file}: the pin itself has the new check");
            now = serde_json::to_string(&rest).unwrap();
            now.push('\n');
        }
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
    assert!(checked >= 30, "only {checked} saved projects were checked");
}
