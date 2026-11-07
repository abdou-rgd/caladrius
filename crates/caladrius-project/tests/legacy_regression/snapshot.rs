//! What today's importer and worksheet give for a file, as one JSON text.
//!
//! This module uses only the API that existed before T-150 (`readings`, `ImportedTable::from_csv`,
//! `Project`, `Worksheet::profile`), so that the same code produced the pinned files at 5815119 and
//! checks them after the change. It serializes the `table` and the `checks` of a reading separately,
//! never the whole `Reading`, because a reading gained a `dataset` (a new field is not a change of
//! the old ones).

use caladrius_project::{CsvOptions, ImportedTable, Project, readings};
use serde_json::{Map, Value, json};

/// The variants of the options a file is read with, besides the default one.
fn variants() -> Vec<(&'static str, CsvOptions)> {
    vec![
        ("default", CsvOptions::default()),
        (
            "comma_point",
            CsvOptions {
                delimiter: Some(','),
                decimal_comma: Some(false),
                ..CsvOptions::default()
            },
        ),
        (
            "semicolon_comma",
            CsvOptions {
                delimiter: Some(';'),
                decimal_comma: Some(true),
                ..CsvOptions::default()
            },
        ),
    ]
}

fn to_value<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).unwrap_or(Value::Null)
}

/// The outcome of a fallible call, as data: the value, or the error code and message.
fn outcome<T: serde::Serialize>(r: Result<T, caladrius_project::ProjectError>) -> Value {
    match r {
        Ok(v) => json!({ "ok": to_value(&v) }),
        Err(e) => json!({ "error": { "code": e.code, "message": e.message } }),
    }
}

/// The pinned text of one file: for each option variant, every reading (table and checks), the table
/// of `from_csv`, and, for the default reading, the stored form of a project that holds its
/// worksheet, the subjects, the profile of every subject (and of none), the unit warnings and the
/// derived units.
pub fn render(file: &str, bytes: &[u8]) -> String {
    let mut by_options = Map::new();
    for (label, options) in variants() {
        let mut entry = Map::new();
        entry.insert("from_csv".to_owned(), outcome(ImportedTable::from_csv(bytes, &options)));
        let found = readings(bytes, &options).map(|list| {
            list.iter()
                .map(|r| {
                    let whole = to_value(r);
                    json!({ "table": whole["table"], "checks": whole["checks"] })
                })
                .collect::<Vec<_>>()
        });
        entry.insert("readings".to_owned(), outcome(found));
        by_options.insert(label.to_owned(), Value::Object(entry));
    }
    let mut out = Map::new();
    out.insert("file".to_owned(), json!(file));
    out.insert("pinned_at".to_owned(), json!("5815119"));
    out.insert("by_options".to_owned(), Value::Object(by_options));
    if let Ok(table) = ImportedTable::from_csv(bytes, &CsvOptions::default()) {
        let mut project = Project::new("pinned");
        if let Ok(id) = project.add_worksheet("sheet", table.columns) {
            if let Ok(ws) = project.worksheet(id) {
                let subjects = ws.subjects();
                let mut profiles = Map::new();
                profiles.insert("(none)".to_owned(), json!(format!("{:?}", ws.profile(None))));
                for s in &subjects {
                    profiles.insert(s.clone(), json!(format!("{:?}", ws.profile(Some(s)))));
                }
                out.insert("subjects".to_owned(), json!(subjects));
                out.insert("profiles".to_owned(), Value::Object(profiles));
                out.insert("unit_warnings".to_owned(), to_value(&ws.unit_warnings()));
                out.insert("derived_units".to_owned(), to_value(&ws.derived_units()));
            }
            if let Ok(saved) = project.to_bytes() {
                out.insert(
                    "project_bytes".to_owned(),
                    json!(String::from_utf8_lossy(&saved)),
                );
            }
        }
    }
    let mut text = serde_json::to_string(&Value::Object(out)).unwrap_or_default();
    text.push('\n');
    text
}
