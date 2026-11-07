#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! The `caladrius-cli` binary: arguments in, answer on standard output, `error: ...` on standard
//! error and exit code 1 on any failure (and on a replay that finds a difference). See the
//! library for what it does.

use std::io::{Write, stderr, stdin, stdout};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set by Ctrl-C while a fit runs: the fit is cancelled at its next iteration.
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

fn main() -> ExitCode {
    let args = match caladrius_cli::text_args(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            let _ = writeln!(stderr(), "error: {e}");
            return ExitCode::from(1);
        }
    };
    // Ctrl-C cancels a fit between two iterations (nothing is stored); a second Ctrl-C ends the
    // program at once. For any other command Ctrl-C keeps its default behaviour, and so it does
    // if the handler cannot be installed.
    if args.first().map(String::as_str) == Some("fit.run") {
        let _ = ctrlc::set_handler(|| {
            if INTERRUPTED.swap(true, Ordering::SeqCst) {
                std::process::exit(130);
            }
        });
    }
    let mut show = |line: &str| {
        let _ = writeln!(stderr(), "{line}");
    };
    let interrupted = || INTERRUPTED.load(Ordering::SeqCst);
    match caladrius_cli::run_with(&args, &mut stdin(), &mut show, &interrupted) {
        Ok(output) => {
            for note in &output.notes {
                let _ = writeln!(stderr(), "{note}");
            }
            // A closed pipe (`| head`) is not a failure of the command.
            let _ = stdout().write_all(output.stdout.as_bytes());
            ExitCode::from(output.exit_code)
        }
        Err(e) => {
            let _ = writeln!(stderr(), "error: {e}");
            ExitCode::from(1)
        }
    }
}
