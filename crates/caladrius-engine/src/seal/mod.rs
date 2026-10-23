//! Seals: a fingerprint of what a command was asked and what it answered, and where it was made.
//!
//! The seal of a command is the SHA-256 of a canonical text made of the command id, its
//! parameters as they were received and its outcome: the result, or the code of the error. The
//! text is canonical (keys in byte order, no space, numbers as the shortest text that reads back
//! the same double), so two engines that compute the same bits write the same text. The engine
//! returns the same bits on every machine (AGENTS.md, rule 10), so a seal is the same on every
//! machine: anyone who runs the same command on the same data with the same engine gets the same
//! seal, and one flipped bit anywhere in the answer gives another.
//!
//! ```text
//! {"command":"nca.run","format":"caladrius-seal-1","outcome":{"ok":true,"result":{...}},"params":{...}}
//! ```
//!
//! Next to the hash the seal says where it was made: the version of the engine, the git commit it
//! was built from when that was known at build time, and the SHA-256 of the data the command
//! read (the CSV text of an import, the worksheets a command refers to). That is provenance, and
//! it is not in the hashed text: a later version that computes the same bits has to give the same
//! hash. The time a command ran is another such fact; it sits next to the seal
//! ([`crate::HistoryEntry::at`]) and the engine reads no clock itself: the front end hands it one
//! ([`crate::Engine::set_clock`]), so the crate still compiles to WebAssembly.
//!
//! Two things are not sealed: `project.load` (the file it brings in carries its own seals, and its
//! outcome depends on whether the session already had a history) and `project.replay` (it is the
//! check itself). A result that echoes the history (`project.save`, `history.list`) is hashed
//! without the times and the provenance of the entries it lists, which are not deterministic.

mod canonical;
mod embed;
mod sha256;

use std::borrow::Cow;
use std::collections::BTreeSet;

use caladrius_project::{AnalysisId, Project, WorksheetId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CommandError;

pub use canonical::canonical_json;
pub use embed::{
    STAMP_MARK, csv_with_stamp, json_with_seal, stamp_line, strip_stamp, svg_with_stamp,
};
pub use sha256::{Sha256, sha256_hex};

/// The version of the sealed text. A seal of another format is not recomputed.
pub const FORMAT: u32 = 1;

/// How many hexadecimal digits of a hash a screen shows.
pub const SHORT: usize = 12;

/// The tag written inside the sealed text.
const TAG: &str = "caladrius-seal-1";

/// The version of this engine (the version of the workspace).
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The git commit this engine was built from, when the build knew it (`build.rs`).
pub const GIT_COMMIT: Option<&str> = option_env!("CALADRIUS_GIT_COMMIT");

/// A clock: seconds since 1970-01-01 00:00:00 UTC, or `None` when there is none.
pub type Clock = fn() -> Option<i64>;

/// The seal of one command: the hash, and where it was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seal {
    /// Version of the sealed text ([`FORMAT`]).
    pub format: u32,
    /// SHA-256 of the sealed text: 64 lower-case hexadecimal digits.
    pub sha256: String,
    /// Version of the engine that made the seal. Provenance, not hashed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// Git commit that engine was built from, when known at build time. Provenance, not hashed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// SHA-256 of the data the command read: the CSV text of `data.preview` and `data.import`, the
    /// canonical text of the worksheets the parameters refer to (as they were before the command
    /// ran) for the others. Absent when the command read no stored data. Provenance, not hashed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
}

impl Seal {
    /// The first [`SHORT`] digits of the hash, what a screen shows.
    pub fn short(&self) -> &str {
        self.sha256.get(..SHORT).unwrap_or(&self.sha256)
    }
}

/// True for the commands that get a seal.
pub(crate) fn is_sealed(command: &str) -> bool {
    !matches!(command, "project.load" | "project.replay")
}

/// The text that is hashed: the command, the parameters as received and the outcome.
pub(crate) fn sealed_text(
    command: &str,
    params: &Value,
    outcome: &Result<Value, CommandError>,
) -> String {
    use canonical::{write_string, write_value};
    // The keys are written in byte order, as `canonical_json` would write them.
    let mut out = String::from("{\"command\":");
    write_string(command, &mut out);
    out.push_str(",\"format\":\"");
    out.push_str(TAG);
    out.push_str("\",\"outcome\":");
    match outcome {
        Ok(result) => {
            out.push_str("{\"ok\":true,\"result\":");
            write_value(&without_volatile(command, result), &mut out);
            out.push('}');
        }
        Err(error) => {
            out.push_str("{\"error\":");
            write_string(&error.code, &mut out);
            out.push_str(",\"ok\":false}");
        }
    }
    out.push_str(",\"params\":");
    write_value(params, &mut out);
    out.push('}');
    out
}

/// A result that lists history entries, without what is not deterministic in them: the time of
/// each entry and the engine and commit of its seal.
fn without_volatile<'a>(command: &str, result: &'a Value) -> Cow<'a, Value> {
    let pointer = match command {
        "project.save" => "/project/history",
        "history.list" => "/entries",
        _ => return Cow::Borrowed(result),
    };
    let mut copy = result.clone();
    if let Some(Value::Array(entries)) = copy.pointer_mut(pointer) {
        for entry in entries {
            if let Value::Object(map) = entry {
                map.remove("at");
                if let Some(Value::Object(seal)) = map.get_mut("seal") {
                    seal.remove("engine");
                    seal.remove("commit");
                }
            }
        }
    }
    Cow::Owned(copy)
}

/// The seal of an outcome. `input` is the hash of the data the command read, `provenance` says
/// whether the version and the commit of this build are written next to the hash.
pub(crate) fn seal(
    command: &str,
    params: &Value,
    outcome: &Result<Value, CommandError>,
    input: Option<String>,
    provenance: bool,
) -> Seal {
    let text = sealed_text(command, params, outcome);
    Seal {
        format: FORMAT,
        sha256: sha256_hex(text.as_bytes()),
        engine: provenance.then(|| ENGINE_VERSION.to_owned()),
        commit: if provenance {
            GIT_COMMIT.map(str::to_owned)
        } else {
            None
        },
        input,
    }
}

// ---- the data a command reads -------------------------------------------------------------

/// The hash of the data a command is about to read, before it runs: the CSV text for an import,
/// else the worksheets its parameters refer to (`worksheet`, or the worksheet of an `analysis`,
/// `fit`, `a` or `b`), as the canonical text of the list of them in order of id.
pub(crate) fn input_hash(project: &Project, command: &str, params: &Value) -> Option<String> {
    if matches!(command, "data.preview" | "data.import") {
        let csv = params.get("csv").and_then(Value::as_str)?;
        return Some(sha256_hex(csv.as_bytes()));
    }
    let sheets: Vec<Value> = worksheets_read(project, params)
        .into_iter()
        .filter_map(|id| project.worksheet(id).ok())
        .filter_map(|sheet| serde_json::to_value(sheet).ok())
        .collect();
    if sheets.is_empty() {
        return None;
    }
    let (text, _) = canonical_json(&Value::Array(sheets));
    Some(sha256_hex(text.as_bytes()))
}

fn worksheets_read(project: &Project, params: &Value) -> BTreeSet<WorksheetId> {
    let mut ids = BTreeSet::new();
    if let Some(n) = params.get("worksheet").and_then(Value::as_u64) {
        ids.insert(WorksheetId(n));
    }
    for key in ["analysis", "fit", "a", "b"] {
        if let Some(n) = params.get(key).and_then(Value::as_u64) {
            sheets_of_analysis(project, AnalysisId(n), &mut ids, 2);
        }
    }
    ids
}

/// The worksheet an analysis reads, and, for a plot of a fit, the one its source reads.
fn sheets_of_analysis(
    project: &Project,
    id: AnalysisId,
    ids: &mut BTreeSet<WorksheetId>,
    hops: u8,
) {
    let Ok(analysis) = project.analysis(id) else {
        return;
    };
    if let Some(sheet) = analysis.spec().worksheet() {
        ids.insert(sheet);
    }
    if hops > 0 {
        if let Some(source) = analysis.spec().source_analysis() {
            sheets_of_analysis(project, source, ids, hops - 1);
        }
    }
}

// ---- time ---------------------------------------------------------------------------------

/// `seconds` since 1970-01-01 00:00:00 UTC as `YYYY-MM-DDTHH:MM:SSZ` (RFC 3339, proleptic
/// Gregorian calendar).
pub fn rfc3339(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    // Days since 1970-01-01 to a civil date: H. Hinnant's algorithm, on an era of 400 years.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests;
