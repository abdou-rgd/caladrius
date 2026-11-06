//! `cargo xtask bridge`: do the three ways into the engine return the same bytes?
//!
//! A web front reaches the engine through one call, `Bridge::execute` (`apps/caladrius-bridge`),
//! over two transports: a Web Worker that runs the engine as WebAssembly, and a Tauri command on
//! the desktop. This task runs the fixed script `apps/caladrius-bridge/script.json` (the import of
//! `oracle/data/edge_blq.csv`, `nca.run` on subject 3, `plot.xy`, `export.plot`, `history.list`, an
//! unknown command, invalid parameters, a request that is not JSON) three times, each on a new
//! engine:
//!
//! 1. in this process, through `Bridge::execute` in Rust;
//! 2. in the WebAssembly module of `caladrius-bridge` under Node, as `cargo xtask digest --wasm`
//!    does (needs the `wasm32-unknown-unknown` target, `wasm-bindgen` at the version of
//!    `Cargo.lock` and Node 20 or later);
//! 3. through the command handler of the Tauri shell (`apps/caladrius-tauri`), called by a test of
//!    the shell on the mock runtime, without a window (`cargo test` of the shell: the first build
//!    takes minutes).
//!
//! It prints `bridge: k of n answers identical on 3 transports` and fails unless k is n. What is
//! turned off, so that a difference between runs cannot be blamed on the run: the provenance of the
//! seals (the version and the commit of the build) is left out in all three runs, and no clock is
//! given to the engine, so `at` is `null`. Nothing else in an answer depends on the run.
//!
//! - `cargo xtask bridge`: the three transports;
//! - `cargo xtask bridge --no-tauri`: Rust and WebAssembly only (prints `on 2 transports`);
//! - `cargo xtask bridge --dump DIR`: also keeps the answers of each transport in `DIR`.
//!
//! The task also writes the browser build of the module (`wasm-bindgen --target web`, what the Web
//! Worker loads) to `target/bridge/web` and says how big it is.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::console;
use crate::error::{Result, XtaskError};
use crate::web::{bindgen_cli, run_tool, target_dir};
use crate::workspace;

const TARGET: &str = "wasm32-unknown-unknown";
const CRATE: &str = "caladrius-bridge";
/// The profile of the browser page (root `Cargo.toml`): the module checked is the one shipped.
const PROFILE: &str = "web";
const SCRIPT: &str = "apps/caladrius-bridge/script.json";
const SHELL_MANIFEST: &str = "apps/caladrius-tauri/Cargo.toml";
/// Where the test of the shell reads the requests, and the line it prints the answers on.
const REQUESTS_ENV: &str = "CALADRIUS_BRIDGE_REQUESTS";
const ANSWERS_PREFIX: &str = "BRIDGE-ANSWERS ";
const USAGE: &str = "usage: cargo xtask bridge [--no-tauri] [--dump DIR]";

/// The Node program that runs the requests in the WebAssembly module. Written next to the module
/// at each run; it prints one JSON document with the answers and `describe()`.
const NODE_RUNNER: &str = r#"'use strict';
const fs = require('node:fs');
const path = require('node:path');
const [moduleDir, requestsPath] = process.argv.slice(2);
const wasm = require(path.resolve(moduleDir, 'caladrius_bridge.js'));
const requests = JSON.parse(fs.readFileSync(requestsPath, 'utf8'));
const bridge = new wasm.Bridge(false);
const answers = requests.map((request) => bridge.execute(request));
bridge.free();
process.stdout.write(JSON.stringify({ answers, describe: wasm.describe() }) + '\n');
"#;

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub tauri: bool,
    pub dump: Option<PathBuf>,
}

/// Reads the arguments that follow `bridge`; the error is a message for the user.
pub fn parse(args: &[OsString]) -> std::result::Result<Options, String> {
    let mut options = Options {
        tauri: true,
        dump: None,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.to_str() {
            Some("--no-tauri") => options.tauri = false,
            Some("--dump") => {
                let dir = rest.next().ok_or("`--dump` needs a folder")?;
                options.dump = Some(PathBuf::from(dir));
            }
            _ => {
                return Err(format!(
                    "unexpected argument `{}`; {USAGE}",
                    arg.to_string_lossy()
                ));
            }
        }
    }
    Ok(options)
}

/// The requests of the script as the texts that are sent. An object becomes its compact JSON text,
/// a string is sent as it stands; `{"$file": path}` becomes the text of the file at `root/path`.
pub fn requests(script: &str, root: &Path) -> Result<Vec<String>> {
    let script: Value = serde_json::from_str(script)
        .map_err(|e| XtaskError::new(format!("bridge: {SCRIPT} is not valid JSON: {e}")))?;
    let list = script
        .get("requests")
        .and_then(Value::as_array)
        .ok_or_else(|| XtaskError::new(format!("bridge: {SCRIPT} has no `requests` list")))?;
    list.iter()
        .enumerate()
        .map(|(index, item)| match item {
            Value::String(text) => Ok(text.clone()),
            Value::Object(_) => {
                let mut item = item.clone();
                inline_files(&mut item, root)?;
                serde_json::to_string(&item).map_err(|e| {
                    XtaskError::new(format!("bridge: request {index} cannot be written: {e}"))
                })
            }
            _ => Err(XtaskError::new(format!(
                "bridge: request {index} of {SCRIPT} is neither an object nor a string"
            ))),
        })
        .collect()
}

/// Replaces every `{"$file": path}` in `value` by the text of the file.
fn inline_files(value: &mut Value, root: &Path) -> Result<()> {
    match value {
        Value::Object(map) => {
            if map.len() == 1 {
                if let Some(Value::String(relative)) = map.get("$file") {
                    let path = root.join(relative);
                    let text =
                        fs::read_to_string(&path).map_err(|e| XtaskError::io("read", &path, &e))?;
                    *value = Value::String(text);
                    return Ok(());
                }
            }
            for item in map.values_mut() {
                inline_files(item, root)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                inline_files(item, root)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// How many answers are the same text on every transport, and the index of the first that is not.
pub fn compare(runs: &[(&str, &[String])]) -> (usize, Option<usize>) {
    let Some((_, first)) = runs.first() else {
        return (0, None);
    };
    let mut same = 0;
    let mut first_difference = None;
    for (index, answer) in first.iter().enumerate() {
        if runs
            .iter()
            .all(|(_, other)| other.get(index) == Some(answer))
        {
            same += 1;
        } else if first_difference.is_none() {
            first_difference = Some(index);
        }
    }
    (same, first_difference)
}

pub fn run(args: &[OsString]) -> Result<()> {
    let options = parse(args).map_err(XtaskError::new)?;
    let root = workspace::root()?;
    let script_path = root.join(SCRIPT);
    let script =
        fs::read_to_string(&script_path).map_err(|e| XtaskError::io("read", &script_path, &e))?;
    let requests = requests(&script, &root)?;
    let work = target_dir(&root).join("bridge");
    fs::create_dir_all(&work).map_err(|e| XtaskError::io("create", &work, &e))?;
    let requests_path = work.join("requests.json");
    let requests_json = serde_json::to_string(&requests)
        .map_err(|e| XtaskError::new(format!("bridge: cannot write the requests: {e}")))?;
    fs::write(&requests_path, &requests_json)
        .map_err(|e| XtaskError::io("write", &requests_path, &e))?;

    let native = run_native(&requests);
    let wasm = run_wasm(&root, &work, &requests_path)?;
    if wasm.answers.len() != requests.len() {
        return Err(XtaskError::new(format!(
            "bridge: {} requests but the WebAssembly module answered {}",
            requests.len(),
            wasm.answers.len()
        )));
    }
    let tauri = if options.tauri {
        Some(run_tauri(&root, &work, &requests_path)?)
    } else {
        None
    };

    let mut runs: Vec<(&str, &[String])> = vec![("rust", &native.answers), ("wasm", &wasm.answers)];
    if let Some(answers) = &tauri {
        runs.push(("tauri", answers));
    }
    if let Some(dir) = &options.dump {
        for (name, answers) in &runs {
            write_answers(dir, name, answers)?;
        }
    }
    let (same, first_difference) = compare(&runs);
    let describe_same = native.describe == wasm.describe;
    console::err(&format!(
        "bridge: describe() is {} bytes in Rust and {} in WebAssembly: {}",
        native.describe.len(),
        wasm.describe.len(),
        if describe_same {
            "identical"
        } else {
            "DIFFERENT"
        }
    ));
    console::out(&format!(
        "bridge: {same} of {} answers identical on {} transports",
        requests.len(),
        runs.len()
    ));
    if let Some(index) = first_difference {
        return Err(XtaskError::new(format!(
            "bridge: the answers to request {index} differ\n{}",
            explain(index, &requests, &runs)
        )));
    }
    if !describe_same {
        return Err(XtaskError::new(
            "bridge: describe() differs between Rust and WebAssembly",
        ));
    }
    Ok(())
}

/// Where the request is and what each transport answered, cut to a readable length.
fn explain(index: usize, requests: &[String], runs: &[(&str, &[String])]) -> String {
    let cut = |text: &str| -> String {
        let head: String = text.chars().take(200).collect();
        if head.len() < text.len() {
            format!("{head}... ({} bytes)", text.len())
        } else {
            head
        }
    };
    let mut out = format!(
        "request: {}",
        cut(requests.get(index).map_or("", String::as_str))
    );
    for (name, answers) in runs {
        out.push_str(&format!(
            "\n  {name}: {}",
            cut(answers.get(index).map_or("(none)", String::as_str))
        ));
    }
    out
}

fn write_answers(dir: &Path, name: &str, answers: &[String]) -> Result<()> {
    fs::create_dir_all(dir).map_err(|e| XtaskError::io("create", dir, &e))?;
    let path = dir.join(format!("{name}.ndjson"));
    let mut text = String::new();
    for answer in answers {
        text.push_str(answer);
        text.push('\n');
    }
    fs::write(&path, text).map_err(|e| XtaskError::io("write", &path, &e))
}

/// What one transport returned: the answers, and `describe()` when it has one.
struct Run {
    answers: Vec<String>,
    describe: String,
}

/// The reference: the bridge in this process, on a new engine without provenance.
fn run_native(requests: &[String]) -> Run {
    let mut bridge = caladrius_bridge::Bridge::without_provenance();
    Run {
        answers: requests.iter().map(|r| bridge.execute(r)).collect(),
        describe: caladrius_bridge::describe(),
    }
}

/// Arguments of the `cargo rustc` that builds the module: the library as a `cdylib` for wasm32.
fn build_args() -> Vec<String> {
    [
        "rustc",
        "-p",
        CRATE,
        "--lib",
        "--crate-type",
        "cdylib",
        "--profile",
        PROFILE,
        "--target",
        TARGET,
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// Arguments of `wasm-bindgen`: `target` is `nodejs` (what this task runs) or `web` (what the Web
/// Worker loads).
fn bindgen_args(wasm: &Path, out: &Path, target: &str) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![wasm.into()];
    for a in ["--target", target, "--no-typescript", "--out-dir"] {
        args.push(a.into());
    }
    args.push(out.into());
    args
}

fn node() -> OsString {
    env::var_os("NODE").unwrap_or_else(|| OsString::from("node"))
}

fn run_wasm(root: &Path, work: &Path, requests_path: &Path) -> Result<Run> {
    if !crate::wasm::target_installed()? {
        return Err(XtaskError::new(crate::wasm::missing_target_message()));
    }
    let bindgen = bindgen_cli(root, "bridge")?;
    let node = node();
    let node_version = Command::new(&node)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .ok_or_else(|| {
            XtaskError::new(
                "bridge: node is not on the PATH; the wasm32 run needs Node 20 or later",
            )
        })?;

    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let build: Vec<OsString> = build_args().into_iter().map(OsString::from).collect();
    console::err(&format!("bridge: cargo {}", build_args().join(" ")));
    run_tool("bridge", &cargo, &build, "cargo rustc")?;
    let wasm = target_dir(root)
        .join(TARGET)
        .join(PROFILE)
        .join("caladrius_bridge.wasm");

    let node_dir = work.join("node");
    let web_dir = work.join("web");
    for (dir, target) in [(&node_dir, "nodejs"), (&web_dir, "web")] {
        fs::create_dir_all(dir).map_err(|e| XtaskError::io("create", dir, &e))?;
        run_tool(
            "bridge",
            &bindgen,
            &bindgen_args(&wasm, dir, target),
            "wasm-bindgen",
        )?;
    }
    let size = |dir: &Path| {
        fs::metadata(dir.join("caladrius_bridge_bg.wasm"))
            .map(|m| m.len())
            .unwrap_or_default()
    };
    console::err(&format!(
        "bridge: module {} bytes from cargo, {} bytes after wasm-bindgen (browser build in {}), node {node_version}",
        fs::metadata(&wasm).map(|m| m.len()).unwrap_or_default(),
        size(&web_dir),
        web_dir.display()
    ));

    let runner = work.join("run.cjs");
    fs::write(&runner, NODE_RUNNER).map_err(|e| XtaskError::io("write", &runner, &e))?;
    let output = Command::new(&node)
        .arg(&runner)
        .arg(&node_dir)
        .arg(requests_path)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| XtaskError::new(format!("cannot run {}: {e}", node.to_string_lossy())))?;
    if !output.status.success() {
        return Err(XtaskError::new(format!(
            "bridge: the WebAssembly run failed ({})",
            output.status
        )));
    }
    let document: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        XtaskError::new(format!("bridge: the WebAssembly run printed no JSON: {e}"))
    })?;
    let answers = strings(document.get("answers"))
        .ok_or_else(|| XtaskError::new("bridge: the WebAssembly run gave no list of answers"))?;
    let describe = document
        .get("describe")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok(Run { answers, describe })
}

/// A JSON list of strings.
fn strings(value: Option<&Value>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// The answers the command handler of the Tauri shell gives: the test of the shell reads the
/// requests from the file named by `CALADRIUS_BRIDGE_REQUESTS` and prints the answers on one line (the
/// test harness may put its own `test ... ` text before it, so the prefix is looked for in the line).
fn run_tauri(root: &Path, work: &Path, requests_path: &Path) -> Result<Vec<String>> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    console::err(&format!(
        "bridge: cargo test --manifest-path {SHELL_MANIFEST} (the first build of Tauri takes minutes)"
    ));
    let output = Command::new(&cargo)
        .args(["test", "--manifest-path"])
        .arg(root.join(SHELL_MANIFEST))
        .args(["--test", "bridge", "--", "--nocapture", "--test-threads=1"])
        .env(REQUESTS_ENV, requests_path)
        .env("CARGO_TARGET_DIR", work.join("tauri-target"))
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| XtaskError::new(format!("cannot run cargo: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(XtaskError::new(format!(
            "bridge: the test of the Tauri shell failed ({})\n{stdout}",
            output.status
        )));
    }
    let line = stdout
        .lines()
        .find_map(|l| l.split_once(ANSWERS_PREFIX).map(|(_, rest)| rest))
        .ok_or_else(|| {
            XtaskError::new(format!(
                "bridge: the test of the Tauri shell printed no `{}` line",
                ANSWERS_PREFIX.trim_end()
            ))
        })?;
    let document: Value = serde_json::from_str(line)
        .map_err(|e| XtaskError::new(format!("bridge: the Tauri answers are not JSON: {e}")))?;
    strings(Some(&document))
        .ok_or_else(|| XtaskError::new("bridge: the Tauri answers are not a list of texts"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> std::result::Result<Options, String> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse(&args)
    }

    #[test]
    fn the_options_are_recognised() {
        assert_eq!(
            parsed(&[]),
            Ok(Options {
                tauri: true,
                dump: None
            })
        );
        assert_eq!(
            parsed(&["--no-tauri", "--dump", "out"]),
            Ok(Options {
                tauri: false,
                dump: Some(PathBuf::from("out"))
            })
        );
        assert!(parsed(&["--dump"]).unwrap_err().contains("needs a folder"));
        assert!(
            parsed(&["--fast"])
                .unwrap_err()
                .contains("unexpected argument `--fast`")
        );
    }

    #[test]
    fn the_script_gives_objects_as_compact_text_and_strings_as_they_stand() {
        let script = r#"{"requests": [{"command": "history.list"}, "not json", {"command": "x", "params": {"a": 1}}]}"#;
        let got = requests(script, Path::new(".")).unwrap_or_default();
        assert_eq!(
            got,
            [
                r#"{"command":"history.list"}"#,
                "not json",
                r#"{"command":"x","params":{"a":1}}"#
            ]
        );
        assert!(requests("{}", Path::new(".")).is_err());
        assert!(requests(r#"{"requests": [3]}"#, Path::new(".")).is_err());
    }

    #[test]
    fn a_file_reference_becomes_the_text_of_the_file() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let script = r#"{"requests": [{"command": "data.import", "params": {"csv": {"$file": "oracle/data/edge_blq.csv"}}}]}"#;
        let got = requests(script, &root).unwrap_or_default();
        let value: Value = got
            .first()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or(Value::Null);
        let csv = value["params"]["csv"].as_str().unwrap_or_default();
        assert!(csv.starts_with("subject,time,conc,dose\n"), "{csv:.40}");
        assert!(
            requests(
                r#"{"requests": [{"command": "x", "params": {"csv": {"$file": "no/such/file"}}}]}"#,
                &root
            )
            .is_err()
        );
    }

    #[test]
    fn the_script_in_the_repository_has_at_least_seven_requests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let script = fs::read_to_string(root.join(SCRIPT)).unwrap_or_default();
        let got = requests(&script, &root).unwrap_or_default();
        assert!(got.len() >= 7, "{} requests", got.len());
        for command in [
            "data.import",
            "nca.run",
            "plot.xy",
            "export.plot",
            "history.list",
            "no.such.command",
        ] {
            assert!(
                got.iter().any(|r| r.contains(command)),
                "{command} in the script"
            );
        }
    }

    #[test]
    fn the_native_run_answers_every_request_of_the_script_in_one_shape() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let script = fs::read_to_string(root.join(SCRIPT)).unwrap_or_default();
        let list = requests(&script, &root).unwrap_or_default();
        let run = run_native(&list);
        assert_eq!(run.answers.len(), list.len());
        let ok: Vec<bool> = run
            .answers
            .iter()
            .map(|a| {
                serde_json::from_str::<Value>(a)
                    .ok()
                    .and_then(|v| v["ok"].as_bool())
                    .unwrap_or(false)
            })
            .collect();
        // Seven requests succeed, the five others are refusals: the answers say so.
        assert_eq!(ok.iter().filter(|ok| **ok).count(), 7, "{ok:?}");
        // Twice the same script gives the same bytes.
        assert_eq!(run.answers, run_native(&list).answers);
    }

    #[test]
    fn answers_are_counted_one_by_one() {
        let a = vec!["1".to_owned(), "2".to_owned(), "3".to_owned()];
        let b = vec!["1".to_owned(), "x".to_owned(), "3".to_owned()];
        let short = vec!["1".to_owned()];
        assert_eq!(compare(&[("a", &a), ("a2", &a)]), (3, None));
        assert_eq!(compare(&[("a", &a), ("b", &b)]), (2, Some(1)));
        assert_eq!(compare(&[("a", &a), ("s", &short)]), (1, Some(1)));
        assert_eq!(compare(&[]), (0, None));
    }

    #[test]
    fn the_module_is_the_library_built_as_a_cdylib_for_wasm_with_the_page_profile() {
        assert_eq!(
            build_args().join(" "),
            "rustc -p caladrius-bridge --lib --crate-type cdylib --profile web --target wasm32-unknown-unknown"
        );
        let args: Vec<String> = bindgen_args(Path::new("m.wasm"), Path::new("out"), "web")
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args.join(" "),
            "m.wasm --target web --no-typescript --out-dir out"
        );
    }

    #[test]
    fn the_node_runner_calls_what_the_module_exports() {
        let exports = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("apps/caladrius-bridge/src/wasm.rs"),
        )
        .unwrap_or_default();
        assert!(exports.contains("js_name = Bridge"));
        assert!(exports.contains("pub fn execute("));
        assert!(exports.contains("pub fn describe("));
        assert!(NODE_RUNNER.contains("new wasm.Bridge(false)"));
        assert!(NODE_RUNNER.contains("bridge.execute("));
        assert!(NODE_RUNNER.contains("wasm.describe()"));
    }
}
