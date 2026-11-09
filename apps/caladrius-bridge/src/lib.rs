#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! App layer: the bridge between a web front and the engine. One call, two transports.
//!
//! A front end written for the web (React in a page, or in the Tauri webview) reaches the engine
//! through one call: [`Bridge::execute`], a request as text in, an answer as text out. The call
//! runs over two transports that carry the same text: a Web Worker that runs this crate as a
//! WebAssembly module (`wasm.rs`), and a Tauri command that calls it in process
//! (`apps/caladrius-tauri`). Both return the bytes the engine returns in Rust, which
//! `cargo xtask bridge` checks.
//!
//! # The request
//!
//! A JSON object `{"command": "<id>", "params": <value>}`. `command` is a command id of the
//! registry (`nca.run`, `plot.xy`...); `params` is the parameters of the command and may be left
//! out (it is then `null`, which commands without parameters accept). Any other key is a refusal,
//! so that a misspelt `param` is not silently dropped.
//!
//! # The answer
//!
//! One fixed shape, for success and for failure, with every key always present:
//!
//! ```text
//! {"at":null,"command":"nca.run","error":null,"ok":true,"result":{...},"seal":{"format":1,"sha256":"..."}}
//! ```
//!
//! | key | success | failure |
//! |---|---|---|
//! | `ok` | `true` | `false` |
//! | `command` | the id asked for | the id asked for, or `null` when the request had none |
//! | `result` | what `Engine::execute` returned | `null` |
//! | `error` | `null` | `{"code": "...", "message": "..."}` |
//! | `seal` | the seal of the history entry the command just recorded | the same, for a command the engine ran and refused; `null` when no command ran or the command carries no seal |
//! | `at` | the time of that entry when the engine has a clock, else `null` | the same |
//!
//! `seal` is the [`Seal`](caladrius_engine::Seal) of the entry: `format` and `sha256` always, and
//! `engine`, `commit` and `input` when the engine records provenance. `Engine::execute` does not
//! return it, so the bridge reads it from the last entry of `engine.history()`. The error codes of
//! the engine are its own (`unknown_command`, `invalid_parameters`...); the bridge adds one,
//! `invalid_request`, for a request that is not text of the shape above, and then no command ran
//! and nothing is recorded.
//!
//! The text is the canonical JSON of the engine (`caladrius_engine::seal::canonical_json`): keys
//! in byte order, no space, numbers as the shortest text that reads back as the same double. It
//! does not depend on the order in which a `serde_json` build keeps keys, so two builds that
//! return the same value return the same bytes.
//!
//! # Not in the bridge yet
//!
//! `project.load` and `project.replay` take the text of a saved file and are reached from Rust by
//! [`Engine::load_bytes`](caladrius_engine::Engine::load_bytes) and
//! [`Engine::replay_bytes`](caladrius_engine::Engine::replay_bytes); the bridge has no call for
//! bytes, no save, no open and no replay of a history. A front that needs them waits for a later
//! card. (Sent as ordinary requests with a `text` parameter they still go through `execute`, and
//! `project.load` replaces the project of the bridge's engine.)
//!
//! # Determinism
//!
//! The bridge reads no clock and no environment. A provenance (the version and the commit of the
//! build) goes in the seal when [`Bridge::new`] is used; [`Bridge::without_provenance`] leaves it
//! out, which is what `cargo xtask bridge` uses in its three runs, so that a different commit
//! cannot make the transports differ.

#[cfg(target_arch = "wasm32")]
mod wasm;

use caladrius_engine::seal::canonical_json;
use caladrius_engine::{CommandError, Engine};
use serde_json::{Map, Value, json};

/// One engine behind the single call.
#[derive(Debug, Default)]
pub struct Bridge {
    engine: Engine,
}

impl Bridge {
    /// A bridge with a new engine; the seals say which engine made them.
    pub fn new() -> Self {
        Self::default()
    }

    /// A bridge whose seals carry no version and no commit.
    pub fn without_provenance() -> Self {
        let mut engine = Engine::new();
        engine.set_provenance(false);
        Self { engine }
    }

    /// A bridge around an engine that already exists.
    pub fn with_engine(engine: Engine) -> Self {
        Self { engine }
    }

    /// The engine, for reading.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The call: a request as text in, the answer as text out (see the crate documentation for the
    /// shape of both). Never fails and never panics: whatever goes wrong is in the answer.
    pub fn execute(&mut self, request: &str) -> String {
        canonical_json(&self.answer(request)).0
    }

    /// As [`Bridge::execute`], the answer as a value instead of text.
    pub fn answer(&mut self, request: &str) -> Value {
        let (id, params) = match parse_request(request) {
            Ok(parsed) => parsed,
            Err((id, error)) => return shape(id.as_deref(), Err(error), None, None),
        };
        let outcome = self.engine.execute(&id, params);
        // `execute` records the command whether it succeeds or not, so the last entry is its own.
        let entry = self.engine.history().entries().last();
        let seal = entry
            .and_then(|e| e.seal.as_ref())
            .and_then(|s| serde_json::to_value(s).ok());
        let at = entry.and_then(|e| e.at.clone());
        shape(Some(&id), outcome, seal, at)
    }
}

/// The answer of a command, in its one shape.
fn shape(
    id: Option<&str>,
    outcome: Result<Value, CommandError>,
    seal: Option<Value>,
    at: Option<String>,
) -> Value {
    let (ok, result, error) = match outcome {
        Ok(result) => (true, result, Value::Null),
        Err(e) => (
            false,
            Value::Null,
            json!({ "code": e.code, "message": e.message }),
        ),
    };
    json!({
        "at": at,
        "command": id,
        "error": error,
        "ok": ok,
        "result": result,
        "seal": seal,
    })
}

/// `invalid_request`: no command ran, nothing is recorded.
fn invalid(id: Option<String>, message: String) -> (Option<String>, CommandError) {
    (id, CommandError::new("invalid_request", message))
}

/// The command id and the parameters of a request, or why it is not one.
fn parse_request(request: &str) -> Result<(String, Value), (Option<String>, CommandError)> {
    let value: Value = serde_json::from_str(request).map_err(|e| {
        invalid(
            None,
            format!(
                "the request is not valid JSON: {e}; send {{\"command\": \"nca.run\", \"params\": {{...}}}}"
            ),
        )
    })?;
    let Value::Object(mut map) = value else {
        return Err(invalid(
            None,
            "the request must be a JSON object {\"command\": \"<id>\", \"params\": {...}}"
                .to_owned(),
        ));
    };
    let id = match map.remove("command") {
        Some(Value::String(id)) if !id.is_empty() => id,
        _ => {
            return Err(invalid(
                None,
                "the request needs a `command`: the id of a command, as text (`nca.run`...)"
                    .to_owned(),
            ));
        }
    };
    let params = map.remove("params").unwrap_or(Value::Null);
    if let Some(extra) = first_key(&map) {
        return Err(invalid(
            Some(id),
            format!("unexpected key `{extra}` in the request; the keys are `command` and `params`"),
        ));
    }
    Ok((id, params))
}

fn first_key(map: &Map<String, Value>) -> Option<&String> {
    map.keys().next()
}

/// The commands of the registry with the schemas of their parameters and results, as canonical
/// JSON text: the same list `describe()` gives in Rust.
pub fn describe() -> String {
    let commands = serde_json::to_value(caladrius_engine::describe()).unwrap_or(Value::Null);
    canonical_json(&commands).0
}

#[cfg(test)]
mod tests;
