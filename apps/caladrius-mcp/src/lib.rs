#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! App layer: an MCP (Model Context Protocol) server over standard input and output that exposes
//! every command of the registry (`caladrius-engine`) to an agent such as Apothicaire.
//!
//! Transport: one JSON-RPC 2.0 message per line on stdin, one per line on stdout; nothing else is
//! ever written to stdout. Methods: `initialize`, `ping`, `tools/list`, `tools/call`; the
//! notifications of the protocol are accepted and ignored; any other method is a JSON-RPC
//! "method not found" error. Malformed input gets a JSON-RPC error and the server carries on.
//!
//! Tools: one per command, named like the command id with `.` written `_` (`nca.run` is
//! `nca_run`: some agent hosts restrict tool names to letters, digits, `_` and `-`); a call by the
//! original id works too. `inputSchema` and `outputSchema` are the registry's JSON Schemas. A
//! result is JSON text content plus `structuredContent`; a command that fails is a tool error
//! (`isError: true`) whose text is `<code>: <message>`. The server holds one [`Engine`]: the
//! worksheets and analyses of a session persist between calls, as in the UI.
//!
//! Every result also carries the seal of the command in its `_meta` (`caladrius/seal`: the
//! SHA-256 of the command, its parameters and its answer, and where it was made; `caladrius/at`:
//! the time). It is never in `content` or in `structuredContent`: what the model reads, and what
//! a number gate checks against, is exactly what it was before seals existed. That holds for the
//! three answers that would show seals of their own, the entries that `history_list` and
//! `project_save` list and the digests of a `project_replay` report: the model reads them without
//! those members, and the whole answer is in `_meta` under `caladrius/result` for the program
//! that keeps the file or checks the digests.

use std::io::{BufRead, Read, Write};

use caladrius_engine::{CommandInfo, Engine, describe};
use serde_json::{Value, json};

/// Protocol versions this server speaks, newest first. The client's version is echoed back when
/// it is in the list, else the newest is proposed (MCP lifecycle negotiation).
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// The longest line the server reads (a CSV can be large; a client that sends more without a
/// newline is not a client).
pub const MAX_LINE: usize = 64 * 1024 * 1024;

/// The most requests accepted in one batch.
pub const MAX_BATCH: usize = 100;

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// What the model reads once, at initialization.
const INSTRUCTIONS: &str = "Caladrius: pharmacokinetic analysis. Every tool is a command with a JSON schema. \
Typical session: data_import (CSV text in `csv`; roles and units are guessed from the headers and reported), \
data_describe to check them (data_set_column to fix a role or a unit), nca_run for a non-compartmental analysis \
or fit_run for a model fit (the answer holds the result and its status), export_table for CSV tables. \
Worksheets and analyses persist during the session; after data_set_cell or data_set_column an analysis is \
`stale` until analysis_run. project_save returns the whole project to keep. project_replay with no arguments runs the work of the session again and checks every step against its seal: report its verdict and its summary in words, they say whether it reproduces bit for bit or where it does not. To compare two analyses of one subject call analysis_compare: per shared parameter it gives both values with units, b-a, the relative difference in percent and the ratio, or `not_comparable` with the reason; quote those numbers, never compute them. Errors say what to fix.";

/// The keys of the `_meta` of a result (the prefix keeps them apart from other servers' keys).
pub const SEAL_KEY: &str = "caladrius/seal";
/// See [`SEAL_KEY`].
pub const AT_KEY: &str = "caladrius/at";
/// The whole answer of a command whose answer the model reads without its seals, times and
/// digests (see [`model_view`]).
pub const RESULT_KEY: &str = "caladrius/result";

/// The time of the system, in seconds since 1970 (the engine reads no clock itself).
fn system_clock() -> Option<i64> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    i64::try_from(elapsed.as_secs()).ok()
}

/// The tool name of a command id.
pub fn tool_name(command: &str) -> String {
    command.replace('.', "_")
}

/// One MCP session: an engine and the tool table.
pub struct Server {
    engine: Engine,
    tools: Vec<CommandInfo>,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn result_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

impl Server {
    /// A server with an empty project. Its engine dates the history with the clock of the system.
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_clock(system_clock);
        Self::with_engine(engine)
    }

    /// A server over an existing engine.
    pub fn with_engine(engine: Engine) -> Self {
        Self {
            engine,
            tools: describe(),
        }
    }

    /// The engine, for inspection.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The `tools/list` entry of a command.
    fn tool(command: &CommandInfo) -> Value {
        json!({
            "name": tool_name(&command.id),
            "title": command.title,
            "description": format!("{}. Command `{}`. {}", command.title, command.id, command.description),
            "inputSchema": command.params_schema,
            "outputSchema": command.result_schema,
            "annotations": {
                "title": command.title,
                "readOnlyHint": !command.mutates,
                "openWorldHint": false,
            },
        })
    }

    fn call_tool(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let Some(map) = params.as_object() else {
            return Err((
                INVALID_PARAMS,
                "`tools/call` needs params {name, arguments}".to_owned(),
            ));
        };
        let Some(name) = map.get("name").and_then(Value::as_str) else {
            return Err((
                INVALID_PARAMS,
                "`tools/call` needs a string `name`".to_owned(),
            ));
        };
        let Some(command) = self
            .tools
            .iter()
            .find(|c| c.id == name || tool_name(&c.id) == name)
            .map(|c| c.id.clone())
        else {
            let known: Vec<String> = self.tools.iter().map(|c| tool_name(&c.id)).collect();
            return Err((
                INVALID_PARAMS,
                format!("unknown tool `{name}`; the tools are: {}", known.join(", ")),
            ));
        };
        let arguments = map.get("arguments").cloned().unwrap_or(Value::Null);
        let (mut result, whole) = match self.engine.execute(&command, arguments) {
            Ok(value) => {
                // What the model reads, and the whole answer when that is not all of it.
                let (shown, whole) = match model_view(&command, &value) {
                    Some(view) => (view, Some(value)),
                    None => (value, None),
                };
                let text =
                    serde_json::to_string_pretty(&shown).unwrap_or_else(|_| shown.to_string());
                let mut result = json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false,
                });
                if shown.is_object() {
                    result["structuredContent"] = shown;
                }
                (result, whole)
            }
            Err(e) => (
                json!({
                    "content": [{ "type": "text", "text": format!("{}: {}", e.code, e.message) }],
                    "isError": true,
                }),
                None,
            ),
        };
        let mut meta = self.seal_meta();
        if let Some(whole) = whole {
            meta.get_or_insert_with(|| json!({}))[RESULT_KEY] = whole;
        }
        if let Some(meta) = meta {
            result["_meta"] = meta;
        }
        Ok(result)
    }

    /// The `_meta` of the result of the command the engine has just executed: its seal and the time
    /// it ran. `None` for a command that carries no seal.
    fn seal_meta(&self) -> Option<Value> {
        let entry = self.engine.history().entries().last()?;
        let seal = serde_json::to_value(entry.seal.as_ref()?).ok()?;
        let mut meta = json!({ SEAL_KEY: seal });
        if let Some(at) = &entry.at {
            meta[AT_KEY] = json!(at);
        }
        Some(meta)
    }

    /// Handles one decoded message. `None` for a notification or a response (no answer owed).
    fn handle_message(&mut self, message: &Value) -> Option<Value> {
        let Some(object) = message.as_object() else {
            return Some(error_response(
                Value::Null,
                INVALID_REQUEST,
                "a JSON-RPC message is an object",
            ));
        };
        // A response to a request of ours (we send none): ignore.
        if object.contains_key("result") || object.contains_key("error") {
            return None;
        }
        // No `id` key: a notification, which is never answered, however it is shaped.
        let id = object.get("id").cloned()?;
        let id_ok = matches!(&id, Value::Null | Value::String(_) | Value::Number(_));
        if !id_ok {
            return Some(error_response(
                Value::Null,
                INVALID_REQUEST,
                "`id` must be a string, a number or null",
            ));
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return Some(error_response(
                id,
                INVALID_REQUEST,
                "a request needs a string `method`",
            ));
        };
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Some(error_response(
                id,
                INVALID_REQUEST,
                "`jsonrpc` must be \"2.0\"",
            ));
        }
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        let outcome: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({
                "tools": self.tools.iter().map(Self::tool).collect::<Vec<_>>(),
            })),
            "tools/call" => self.call_tool(&params),
            other => Err((
                METHOD_NOT_FOUND,
                format!("method `{other}` is not supported"),
            )),
        };
        Some(match outcome {
            Ok(result) => result_response(id, result),
            Err((code, message)) => error_response(id, code, &message),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let requested = params.get("protocolVersion").and_then(Value::as_str);
        let version = requested
            .filter(|v| PROTOCOL_VERSIONS.contains(v))
            .unwrap_or(PROTOCOL_VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "caladrius-mcp",
                "title": "Caladrius",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": INSTRUCTIONS,
        })
    }

    /// Handles one line of input and returns the line to write back, if any. Never panics and
    /// never fails: bad input is answered with a JSON-RPC error.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let response = match serde_json::from_str::<Value>(line) {
            Err(e) => Some(error_response(
                Value::Null,
                PARSE_ERROR,
                &format!("parse error: {e}"),
            )),
            // A batch (JSON-RPC 2.0, MCP 2024-11-05): answer each request, in order.
            Ok(Value::Array(items)) => {
                if items.is_empty() {
                    Some(error_response(
                        Value::Null,
                        INVALID_REQUEST,
                        "an empty batch",
                    ))
                } else if items.len() > MAX_BATCH {
                    Some(error_response(
                        Value::Null,
                        INVALID_REQUEST,
                        &format!(
                            "a batch of {} requests is too many (at most {MAX_BATCH}); send them one by one",
                            items.len()
                        ),
                    ))
                } else {
                    let answers: Vec<Value> = items
                        .iter()
                        .filter_map(|m| self.handle_message(m))
                        .collect();
                    if answers.is_empty() {
                        None
                    } else {
                        Some(Value::Array(answers))
                    }
                }
            }
            Ok(message) => self.handle_message(&message),
        };
        response.map(|r| r.to_string())
    }

    /// Reads lines from `input` until it ends and writes the answers to `output`, flushing each.
    /// Bytes that are not UTF-8 are answered with a parse error; a line longer than [`MAX_LINE`]
    /// bytes is answered with an error and skipped up to its end. A byte order mark on the first
    /// line is ignored. Returns when the input ends or the output cannot be written.
    pub fn serve<R: BufRead, W: Write>(&mut self, input: R, output: W) -> std::io::Result<()> {
        self.serve_with_limit(input, output, MAX_LINE)
    }

    /// [`Server::serve`] with another longest line (the tests use a small one).
    pub fn serve_with_limit<R: BufRead, W: Write>(
        &mut self,
        mut input: R,
        mut output: W,
        max_line: usize,
    ) -> std::io::Result<()> {
        let mut buffer = Vec::new();
        let mut first = true;
        loop {
            buffer.clear();
            // Never read more than one line more a byte: a client that sends gigabytes without a
            // newline must not exhaust the memory.
            let limit = u64::try_from(max_line)
                .unwrap_or(u64::MAX)
                .saturating_add(1);
            let read = Read::take(&mut input, limit).read_until(b'\n', &mut buffer)?;
            if read == 0 {
                return Ok(());
            }
            let too_long = buffer.len() > max_line && buffer.last() != Some(&b'\n');
            let answer = if too_long {
                skip_line(&mut input)?;
                Some(
                    error_response(
                        Value::Null,
                        INVALID_REQUEST,
                        &format!(
                            "the line is too long (more than {max_line} bytes) and was skipped"
                        ),
                    )
                    .to_string(),
                )
            } else {
                let bytes = if first {
                    buffer.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&buffer)
                } else {
                    &buffer
                };
                match std::str::from_utf8(bytes) {
                    Ok(line) => self.handle_line(line),
                    Err(_) => Some(
                        error_response(
                            Value::Null,
                            PARSE_ERROR,
                            "parse error: the line is not valid UTF-8",
                        )
                        .to_string(),
                    ),
                }
            };
            first = false;
            if let Some(line) = answer {
                output.write_all(line.as_bytes())?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
    }
}

/// Discards input up to and including the next newline (or the end of the input).
fn skip_line<R: BufRead>(input: &mut R) -> std::io::Result<()> {
    loop {
        let (used, done) = {
            let available = input.fill_buf()?;
            if available.is_empty() {
                return Ok(());
            }
            match available.iter().position(|b| *b == b'\n') {
                Some(i) => (i + 1, true),
                None => (available.len(), false),
            }
        };
        input.consume(used);
        if done {
            return Ok(());
        }
    }
}

/// The tool table as `tools/list` returns it, without a session.
pub fn tools() -> Vec<Value> {
    describe().iter().map(Server::tool).collect()
}

/// What the model reads of an answer, when that is not the whole answer: the entries that
/// `history.list` and `project.save` list without their seals and times, and a `project.replay`
/// report without its digests and engine versions. A hash is a string of digits that a number
/// gate reads as numbers, and it is no use to a model that never computes anything: the program
/// that keeps the file or checks the digests finds the whole answer in `_meta`. `None` for every
/// other answer, which is shown as it is.
fn model_view(command: &str, answer: &Value) -> Option<Value> {
    let mut view = answer.clone();
    let changed = match command {
        "history.list" => without(view.pointer_mut("/entries"), &["seal", "at"]),
        "project.save" => without(view.pointer_mut("/project/history"), &["seal", "at"]),
        "project.replay" => {
            let mut changed = without(Some(&mut view), &["engine", "sealed_by"]);
            changed |= without(view.pointer_mut("/state"), &["stored", "replayed"]);
            changed |= without(view.pointer_mut("/steps"), &["stored", "recomputed"]);
            changed |= without(
                view.pointer_mut("/first_divergence"),
                &["stored", "recomputed"],
            );
            changed
        }
        _ => false,
    };
    changed.then_some(view)
}

/// Removes the members `keys` from `value`, or from each element when it is an array. True when
/// one was there.
fn without(value: Option<&mut Value>, keys: &[&str]) -> bool {
    let mut removed = false;
    match value {
        Some(Value::Object(map)) => {
            for key in keys {
                removed |= map.remove(*key).is_some();
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                removed |= without(Some(item), keys);
            }
        }
        _ => {}
    }
    removed
}

#[cfg(test)]
mod tests;
