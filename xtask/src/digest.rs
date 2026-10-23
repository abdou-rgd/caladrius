//! `cargo xtask digest`: do the numbers of a result depend on the machine?
//!
//! The task builds a fixed workload (`workload`: the command examples and the NCA, fit and model
//! cases of `oracle/`), runs it through `Engine::execute` and prints one SHA-256 over the canonical
//! JSON of everything the engine returned. Two machines that print the same line returned the same
//! bits. The CI job `digest` runs it on Linux, Windows and macOS and, with `--wasm`, in the browser
//! build under Node, and fails when the lines differ (T-111).
//!
//! - `cargo xtask digest`: the workload in this process;
//! - `cargo xtask digest --wasm`: the same bytes of workload in the WebAssembly module of
//!   `caladrius-digest`, under Node (needs the `wasm32-unknown-unknown` target, `wasm-bindgen` at the
//!   version of `Cargo.lock`, and Node 20 or later; the task installs nothing);
//! - `--dump FILE` keeps the canonical text (one JSON document per step) next to the digest;
//! - `cargo xtask digest --diff LEFT RIGHT` compares two dumps: how many numbers differ and by how
//!   much, where a list has another length.
//!
//! The digest is the only line on the standard output; the rest goes to the standard error. The task
//! runs in the dev profile (`cargo xtask`); `cargo run --release -p xtask -- digest` runs the
//! optimised build, which must give the same line.

mod workload;

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::console;
use crate::error::{Result, XtaskError};
use crate::web::{bindgen_cli, run_tool, target_dir};
use crate::workspace;

const TARGET: &str = "wasm32-unknown-unknown";
const CRATE: &str = "caladrius-digest";
/// The Cargo profile of the module: the one of the browser page (root `Cargo.toml`), so that the
/// build checked is the build shipped.
const PROFILE: &str = "web";
/// The Node script that loads the module, relative to the workspace root.
const RUNNER: &str = "xtask/digest/run.cjs";
const USAGE: &str =
    "usage: cargo xtask digest [--wasm] [--dump FILE]\n       cargo xtask digest --diff LEFT RIGHT";

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// Run the workload and print its digest.
    Run { wasm: bool, dump: Option<PathBuf> },
    /// Compare two dumps.
    Diff { left: PathBuf, right: PathBuf },
}

/// Reads the arguments that follow `digest`; the error is a message for the user.
pub fn parse(args: &[OsString]) -> std::result::Result<Mode, String> {
    let mut wasm = false;
    let mut dump = None;
    let mut diff = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.to_str() {
            Some("--wasm") => wasm = true,
            Some("--dump") => {
                let file = rest.next().ok_or("`--dump` needs a file")?;
                dump = Some(PathBuf::from(file));
            }
            Some("--diff") => {
                let (Some(left), Some(right)) = (rest.next(), rest.next()) else {
                    return Err("`--diff` needs two files".to_owned());
                };
                diff = Some((PathBuf::from(left), PathBuf::from(right)));
            }
            _ => {
                return Err(format!(
                    "unexpected argument `{}`; {USAGE}",
                    arg.to_string_lossy()
                ));
            }
        }
    }
    match diff {
        Some(_) if wasm || dump.is_some() => Err(format!(
            "`--diff` compares two dumps and takes no other option; {USAGE}"
        )),
        Some((left, right)) => Ok(Mode::Diff { left, right }),
        None => Ok(Mode::Run { wasm, dump }),
    }
}

pub fn run(args: &[OsString]) -> Result<()> {
    match parse(args).map_err(XtaskError::new)? {
        Mode::Diff { left, right } => compare_dumps(&left, &right),
        Mode::Run { wasm, dump } => {
            let root = workspace::root()?;
            let workload = workload::build()?;
            let text = serde_json::to_string(&workload)
                .map_err(|e| XtaskError::new(format!("cannot write the workload as JSON: {e}")))?;
            console::err(&format!(
                "digest: workload of {} sessions and {} steps, {} bytes, sha256 {}",
                workload.sessions.len(),
                workload
                    .sessions
                    .iter()
                    .map(|s| s.steps.len())
                    .sum::<usize>(),
                text.len(),
                caladrius_digest::sha256_hex(text.as_bytes())
            ));
            let digest = if wasm {
                run_wasm(&root, &text, dump.as_deref())?
            } else {
                run_native(&text, dump.as_deref())?
            };
            console::out(&digest);
            Ok(())
        }
    }
}

fn write_dump(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(|e| XtaskError::io("create", dir, &e))?;
    }
    fs::write(path, text).map_err(|e| XtaskError::io("write", path, &e))
}

fn run_native(workload: &str, dump: Option<&Path>) -> Result<String> {
    let started = Instant::now();
    let run = caladrius_digest::run_text(workload).map_err(XtaskError::new)?;
    let seconds = started.elapsed().as_secs_f64();
    console::err(&format!(
        "digest: native {}/{} ({} build), {} steps ({} answered by an error), {} numbers, {seconds:.2} s",
        env::consts::OS,
        env::consts::ARCH,
        if cfg!(debug_assertions) {
            "dev"
        } else {
            "release"
        },
        run.steps(),
        run.refused(),
        run.numbers(),
    ));
    if let Some(path) = dump {
        write_dump(path, &run.text())?;
    }
    Ok(run.digest())
}

/// Arguments of the `cargo rustc` that builds the module: the library as a `cdylib`, which the
/// native build does not make (a native `cdylib` nobody loads would only slow the linking).
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

/// Arguments of `wasm-bindgen` for the built module: the CommonJS glue Node loads with `require`.
fn bindgen_args(wasm: &Path, out: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![wasm.into()];
    for a in ["--target", "nodejs", "--no-typescript", "--out-dir"] {
        args.push(a.into());
    }
    args.push(out.into());
    args
}

fn run_wasm(root: &Path, workload: &str, dump: Option<&Path>) -> Result<String> {
    if !crate::wasm::target_installed()? {
        return Err(XtaskError::new(crate::wasm::missing_target_message()));
    }
    let bindgen = bindgen_cli(root, "digest")?;
    let node = env::var_os("NODE").unwrap_or_else(|| OsString::from("node"));
    let node_version = Command::new(&node)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .ok_or_else(|| {
            XtaskError::new(
                "digest: node is not on the PATH; the wasm32 run needs Node 20 or later",
            )
        })?;

    let work = target_dir(root).join("digest");
    let module_dir = work.join("wasm");
    fs::create_dir_all(&module_dir).map_err(|e| XtaskError::io("create", &module_dir, &e))?;
    let workload_path = work.join("workload.json");
    fs::write(&workload_path, workload).map_err(|e| XtaskError::io("write", &workload_path, &e))?;

    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let build: Vec<OsString> = build_args().into_iter().map(OsString::from).collect();
    console::err(&format!("digest: cargo {}", build_args().join(" ")));
    run_tool("digest", &cargo, &build, "cargo rustc")?;
    let wasm = target_dir(root)
        .join(TARGET)
        .join(PROFILE)
        .join("caladrius_digest.wasm");
    run_tool(
        "digest",
        &bindgen,
        &bindgen_args(&wasm, &module_dir),
        "wasm-bindgen",
    )?;
    let size = fs::metadata(&wasm).map(|m| m.len()).unwrap_or_default();
    console::err(&format!(
        "digest: module {} bytes, node {node_version}",
        size
    ));

    let mut command = Command::new(&node);
    command
        .arg(root.join(RUNNER))
        .arg(&module_dir)
        .arg(&workload_path);
    if let Some(path) = dump {
        command.arg(path);
    }
    let output = command
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| XtaskError::new(format!("cannot run {}: {e}", node.to_string_lossy())))?;
    if !output.status.success() {
        return Err(XtaskError::new(format!(
            "digest: the WebAssembly run failed ({})",
            output.status
        )));
    }
    let line = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if is_digest(&line) {
        Ok(line)
    } else {
        Err(XtaskError::new(format!(
            "digest: the WebAssembly run printed {line:?} instead of a digest"
        )))
    }
}

/// 64 lower-case hexadecimal digits.
fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn compare_dumps(left: &Path, right: &Path) -> Result<()> {
    let read = |path: &Path| {
        fs::read_to_string(path).map_err(|source| XtaskError::io("read", path, &source))
    };
    let diff = caladrius_digest::compare(&read(left)?, &read(right)?).map_err(XtaskError::new)?;
    console::out(&diff.report());
    if diff.identical() {
        console::out("digest: the two dumps are identical");
        Ok(())
    } else {
        Err(XtaskError::new("digest: the two dumps differ"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> std::result::Result<Mode, String> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse(&args)
    }

    #[test]
    fn the_options_are_recognised() {
        assert_eq!(
            parsed(&[]),
            Ok(Mode::Run {
                wasm: false,
                dump: None
            })
        );
        assert_eq!(
            parsed(&["--wasm", "--dump", "out/x.ndjson"]),
            Ok(Mode::Run {
                wasm: true,
                dump: Some(PathBuf::from("out/x.ndjson"))
            })
        );
        assert_eq!(
            parsed(&["--diff", "a", "b"]),
            Ok(Mode::Diff {
                left: PathBuf::from("a"),
                right: PathBuf::from("b")
            })
        );
    }

    #[test]
    fn a_missing_or_unknown_argument_is_a_usage_error() {
        assert!(parsed(&["--dump"]).unwrap_err().contains("needs a file"));
        assert!(parsed(&["--diff", "a"]).unwrap_err().contains("two files"));
        assert!(
            parsed(&["--fast"])
                .unwrap_err()
                .contains("unexpected argument `--fast`")
        );
        assert!(parsed(&["--diff", "a", "b", "--wasm"]).is_err());
        assert!(parsed(&["--dump", "d", "--diff", "a", "b"]).is_err());
    }

    #[test]
    fn the_module_is_the_library_built_as_a_cdylib_for_wasm_with_the_page_profile() {
        assert_eq!(
            build_args().join(" "),
            "rustc -p caladrius-digest --lib --crate-type cdylib --profile web --target wasm32-unknown-unknown"
        );
        let args: Vec<String> = bindgen_args(Path::new("m.wasm"), Path::new("out"))
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args.join(" "),
            "m.wasm --target nodejs --no-typescript --out-dir out"
        );
    }

    #[test]
    fn a_digest_is_sixty_four_lower_case_hexadecimal_digits() {
        assert!(is_digest(&"0123456789abcdef".repeat(4)));
        assert!(!is_digest(&"0123456789ABCDEF".repeat(4)));
        assert!(!is_digest("abc"));
        assert!(!is_digest(&format!("{} ", "a".repeat(63))));
    }

    #[test]
    fn the_node_runner_calls_the_functions_the_module_exports() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let runner = fs::read_to_string(root.join(RUNNER)).unwrap_or_default();
        let exports =
            fs::read_to_string(root.join("crates").join(CRATE).join("src").join("wasm.rs"))
                .unwrap_or_default();
        for name in ["run_workload", "sha256_text"] {
            assert!(
                runner.contains(&format!("digest.{name}(")),
                "{name} in the runner"
            );
            assert!(
                exports.contains(&format!("pub fn {name}(")),
                "{name} in wasm.rs"
            );
        }
        assert!(runner.contains("caladrius_digest.js"));
    }
}
