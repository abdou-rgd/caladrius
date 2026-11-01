#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! Developer tasks for the Caladrius workspace, run as `cargo xtask <task>` (alias in `.cargo/config.toml`).

mod conformance;
mod console;
mod digest;
mod error;
mod layers;
mod lint;
mod review;
mod wasm;
mod web;
mod workspace;

use std::ffi::OsString;
use std::process::ExitCode;

use crate::error::Result;

const USAGE: &str = "\
usage: cargo xtask <task>

tasks:
  layers       print the layer table and check the dependency rules between crates
  lint         check the mechanical rules of AGENTS.md (names, private/, deny lints, tolerances, board,
               and clippy::indexing_slicing denied in the four numerical crates);
               one `file:line rule message` per finding, exit 1 on any (exceptions: xtask/lint_allow.toml)
  wasm         cargo check the L0 to L3 crates for wasm32-unknown-unknown
  web          build the browser application into target/web-page (`web --out DIR` elsewhere);
               needs wasm-bindgen-cli at the version of Cargo.lock, uses wasm-opt when present
  conformance  run the engine on every oracle case and write docs/conformance.md
  review       check the figures of docs/validation.md and the specs against the files that own them,
               and that the README states no figure of its own
               (`review --strict` also fails on a note); reads only, writes nothing
  digest       run a fixed workload (the command examples, the NCA, fit and model cases of oracle/)
               through the engine and print one SHA-256 over the canonical JSON of the results: the
               same line on every machine, or a number depends on the platform; `--wasm` runs the same
               workload in the browser build under Node, `--dump FILE` keeps the canonical text,
               `--diff LEFT RIGHT` compares two dumps
  help         show this message";

/// The tasks this binary can run.
#[derive(Debug, PartialEq, Eq)]
enum Task {
    Layers,
    Lint,
    Wasm,
    /// The folder arguments (`--out DIR`), checked by the task.
    Web(Vec<OsString>),
    Conformance,
    /// `strict`: a note is a failure too.
    Review {
        strict: bool,
    },
    /// The arguments after `digest` (`--wasm`, `--dump FILE`, `--diff LEFT RIGHT`), checked by the task.
    Digest(Vec<OsString>),
    Help,
}

/// Reads the command line (without the program name); the error is a message for the user.
fn parse_task(args: &[OsString]) -> std::result::Result<Task, String> {
    match args {
        [] => Err("missing task".to_owned()),
        [task] => match task.to_str() {
            Some("layers") => Ok(Task::Layers),
            Some("lint") => Ok(Task::Lint),
            Some("wasm") => Ok(Task::Wasm),
            Some("web") => Ok(Task::Web(Vec::new())),
            Some("conformance") => Ok(Task::Conformance),
            Some("review") => Ok(Task::Review { strict: false }),
            Some("digest") => Ok(Task::Digest(Vec::new())),
            Some("help" | "-h" | "--help") => Ok(Task::Help),
            _ => Err(format!("unknown task `{}`", task.to_string_lossy())),
        },
        [task, flag] if task.to_str() == Some("review") && flag.to_str() == Some("--strict") => {
            Ok(Task::Review { strict: true })
        }
        [task, rest @ ..] if task.to_str() == Some("web") => Ok(Task::Web(rest.to_vec())),
        [task, rest @ ..] if task.to_str() == Some("digest") => Ok(Task::Digest(rest.to_vec())),
        [_, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
    }
}

fn main() -> ExitCode {
    // `args_os`, not `args`: the latter panics on an argument that is not valid Unicode.
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let task = match parse_task(&args) {
        Ok(task) => task,
        Err(message) => {
            console::err(&format!("error: {message}\n\n{USAGE}"));
            return ExitCode::from(2);
        }
    };
    let outcome: Result<()> = match task {
        Task::Layers => layers::run(),
        Task::Lint => lint::run(),
        Task::Wasm => wasm::run(),
        Task::Web(args) => web::run(&args),
        Task::Conformance => conformance::run(),
        Task::Review { strict } => review::run(strict),
        Task::Digest(args) => digest::run(&args),
        Task::Help => {
            console::out(USAGE);
            Ok(())
        }
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            console::err(&format!("error: {error}"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> std::result::Result<Task, String> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse_task(&args)
    }

    #[test]
    fn known_tasks_are_recognised() {
        assert_eq!(parse(&["layers"]), Ok(Task::Layers));
        assert_eq!(parse(&["lint"]), Ok(Task::Lint));
        assert_eq!(parse(&["wasm"]), Ok(Task::Wasm));
        assert_eq!(parse(&["web"]), Ok(Task::Web(Vec::new())));
        assert_eq!(
            parse(&["web", "--out", "dist"]),
            Ok(Task::Web(vec![
                OsString::from("--out"),
                OsString::from("dist")
            ]))
        );
        assert_eq!(parse(&["conformance"]), Ok(Task::Conformance));
        assert_eq!(
            parse(&["conformance", "--private"]),
            Err("unexpected argument `--private`".to_owned())
        );
        assert_eq!(parse(&["review"]), Ok(Task::Review { strict: false }));
        assert_eq!(
            parse(&["review", "--strict"]),
            Ok(Task::Review { strict: true })
        );
        assert_eq!(parse(&["digest"]), Ok(Task::Digest(Vec::new())));
        assert_eq!(
            parse(&["digest", "--wasm", "--dump", "d.ndjson"]),
            Ok(Task::Digest(vec![
                OsString::from("--wasm"),
                OsString::from("--dump"),
                OsString::from("d.ndjson")
            ]))
        );
        assert_eq!(parse(&["help"]), Ok(Task::Help));
        assert_eq!(parse(&["--help"]), Ok(Task::Help));
        assert_eq!(parse(&["-h"]), Ok(Task::Help));
    }

    #[test]
    fn a_missing_unknown_or_extra_argument_is_a_usage_error() {
        assert_eq!(parse(&[]), Err("missing task".to_owned()));
        assert_eq!(parse(&["bogus"]), Err("unknown task `bogus`".to_owned()));
        assert_eq!(
            parse(&["layers", "--fast"]),
            Err("unexpected argument `--fast`".to_owned())
        );
        assert_eq!(
            parse(&["lint", "--fix"]),
            Err("unexpected argument `--fix`".to_owned())
        );
        assert_eq!(
            parse(&["layers", "--private"]),
            Err("unexpected argument `--private`".to_owned())
        );
        assert_eq!(
            parse(&["conformance", "--fast"]),
            Err("unexpected argument `--fast`".to_owned())
        );
        assert_eq!(
            parse(&["review", "--private"]),
            Err("unexpected argument `--private`".to_owned())
        );
    }
}
