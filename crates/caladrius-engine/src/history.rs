//! The history of executed commands: append-only, in order, failures included.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CommandError;
use crate::seal::Seal;

/// Parameters larger than this (as JSON text) are not kept: the entry holds their size and a digest.
pub const MAX_RECORDED_PARAMS_BYTES: usize = 64 * 1024;

/// The parameters as the history keeps them: unchanged up to [`MAX_RECORDED_PARAMS_BYTES`], else
/// `{"omitted": true, "bytes": <size>, "digest": "fnv1a64:<16 hex digits>"}`. The digest is a
/// fingerprint to tell two large inputs apart, not a cryptographic hash.
pub(crate) fn recorded(params: Value) -> Value {
    let text = params.to_string();
    if text.len() <= MAX_RECORDED_PARAMS_BYTES {
        return params;
    }
    let digest = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    serde_json::json!({
        "omitted": true,
        "bytes": text.len(),
        "digest": format!("fnv1a64:{digest:016x}"),
    })
}

/// The size in bytes of the parameters that [`recorded`] left out, if `params` is the note that
/// stands in for them (and not parameters of a command).
pub(crate) fn omitted_bytes(params: &Value) -> Option<u64> {
    let map = params.as_object()?;
    let is_note = map.len() == 3
        && map.get("omitted") == Some(&Value::Bool(true))
        && map.get("digest").is_some_and(Value::is_string);
    if is_note {
        map.get("bytes").and_then(Value::as_u64)
    } else {
        None
    }
}

/// Checks that the positions of saved entries run from 1 without a gap. The error says which
/// entry is out of place.
pub(crate) fn check_positions(entries: &[HistoryEntry]) -> Result<(), String> {
    for (i, e) in entries.iter().enumerate() {
        if e.seq != i as u64 + 1 {
            return Err(format!(
                "the history of the project is damaged: entry {} has position {}",
                i + 1,
                e.seq
            ));
        }
    }
    Ok(())
}

/// One executed command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Position, from 1, without gaps.
    pub seq: u64,
    /// Command id.
    pub command: String,
    /// The parameters as received.
    pub params: Value,
    /// Whether it succeeded.
    pub ok: bool,
    /// The error, when it failed.
    pub error: Option<CommandError>,
    /// True when it succeeded and changed the project (a command that only reads does not).
    pub changed_project: bool,
    /// The seal of the command and its answer (see [`crate::seal`]). Absent in a project saved
    /// before seals existed, and for the commands that carry none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seal: Option<Seal>,
    /// When it ran, as `YYYY-MM-DDTHH:MM:SSZ` (UTC), when the program that ran it gave the engine
    /// a clock. It sits next to the seal and is not in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

/// The entries so far. There is no way to remove or change one: the only operation that adds is
/// private to the crate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    entries: Vec<HistoryEntry>,
}

impl History {
    /// Every entry, oldest first.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True before the first command.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adopts the history of a saved project. Only when this one is empty, so what happened in
    /// this session is never replaced; checked: positions run from 1 without a gap.
    pub(crate) fn adopt(&mut self, entries: Vec<HistoryEntry>) -> Result<bool, CommandError> {
        if !self.entries.is_empty() {
            return Ok(false);
        }
        check_positions(&entries).map_err(|message| CommandError::new("load_failed", message))?;
        self.entries = entries;
        Ok(true)
    }

    /// Appends the entry of a command. `params` are the parameters as received, kept up to
    /// [`MAX_RECORDED_PARAMS_BYTES`].
    pub(crate) fn push(
        &mut self,
        command: &str,
        params: Value,
        outcome: &Result<Value, CommandError>,
        mutates: bool,
        stamp: Stamp,
    ) {
        let seq = self.entries.len() as u64 + 1;
        self.entries.push(HistoryEntry {
            seq,
            command: command.to_owned(),
            params: recorded(params),
            ok: outcome.is_ok(),
            error: outcome.as_ref().err().cloned(),
            changed_project: mutates && outcome.is_ok(),
            seal: stamp.seal,
            at: stamp.at,
        });
    }

    /// For the scratch engine of a replay: makes its history the stored one up to entry `index`
    /// (from 0), so that a command that echoes the history (`project.save`, `history.list`) sees
    /// what it saw when it ran for the first time. Puts the stored entry in place of the one the
    /// engine just recorded for a command it ran (`executed`), or appends it for a step that was
    /// not run.
    pub(crate) fn follow(&mut self, stored: &[HistoryEntry], index: usize, executed: bool) {
        if executed {
            self.entries.pop();
        }
        if let Some(entry) = stored.get(index) {
            self.entries.push(entry.clone());
        }
        // A command that adopted a history of its own would break the numbering: start over from
        // the stored entries (this does not happen with a file that was written by the engine).
        if self.entries.len() != index + 1 {
            self.entries = stored.iter().take(index + 1).cloned().collect();
        }
    }
}

/// What goes next to the outcome of a command in its entry: the seal and the time.
#[derive(Debug, Clone, Default)]
pub(crate) struct Stamp {
    pub(crate) seal: Option<Seal>,
    pub(crate) at: Option<String>,
}
