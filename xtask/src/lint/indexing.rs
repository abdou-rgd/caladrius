//! The indexing step of `cargo xtask lint`: `clippy::indexing_slicing` denied in the numerical crates.
//!
//! `AGENTS.md` (golden rule 6) forbids `[i]` indexing on input-derived data. The crates that hold the
//! numerical core are held to it by clippy itself, on their library targets only (test code keeps its
//! indexing). The other crates are checked in review until their existing sites are cleaned up.

use std::env;
use std::ffi::OsString;
use std::process::Command;

use crate::console;
use crate::error::{Result, XtaskError};

/// The crates held to the rule: the numerical core.
pub const CRATES: &[&str] = &[
    "caladrius-nca",
    "caladrius-models",
    "caladrius-fit",
    "caladrius-project",
];

/// The clippy lint that reports `[i]` on slices and maps and `[a..b]` on slices and strings.
const LINT: &str = "clippy::indexing_slicing";

/// Arguments of the `cargo` invocation: `clippy -p <crate>... --lib --message-format short -- -D <lint>`.
fn clippy_args(crates: &[&str]) -> Vec<String> {
    let mut args = vec!["clippy".to_owned()];
    for name in crates {
        args.push("-p".to_owned());
        args.push((*name).to_owned());
    }
    for word in ["--lib", "--message-format", "short", "--", "-D", LINT] {
        args.push(word.to_owned());
    }
    args
}

/// The lines of clippy's short output that report the lint. The short format omits the lint name, so a
/// finding is recognised by the lint's two messages (`file:line:col: error: indexing may panic`,
/// `... slicing may panic`), or by the lint name when the format carries it.
fn findings(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| {
            line.contains(LINT)
                || line.contains(": indexing may panic")
                || line.contains(": slicing may panic")
        })
        .collect()
}

/// One-line summary of the step.
fn summary(count: usize) -> String {
    format!(
        "lint: indexing_slicing {} in {} ({} crates, lib targets)",
        if count == 0 {
            "ok, 0 findings".to_owned()
        } else {
            format!("{count} finding(s)")
        },
        CRATES.join(", "),
        CRATES.len()
    )
}

/// Runs clippy on the numerical crates; an error (exit 1) when it reports the lint or cannot run.
pub fn run() -> Result<()> {
    let args = clippy_args(CRATES);
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = Command::new(&cargo)
        .args(&args)
        .output()
        .map_err(|source| {
            XtaskError::new(format!("cannot run {}: {source}", cargo.to_string_lossy()))
        })?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let found = findings(&text);
    for line in &found {
        console::out(line);
    }
    if !found.is_empty() {
        return Err(XtaskError::new(summary(found.len())));
    }
    if !output.status.success() {
        // Clippy failed for another reason (a compile error, a missing component): show it.
        console::err(text.trim_end());
        return Err(XtaskError::new(format!(
            "lint: `cargo {}` failed",
            args.join(" ")
        )));
    }
    console::out(&summary(0));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_is_clippy_on_the_four_crates_lib_only_with_the_lint_denied() {
        let args = clippy_args(CRATES);
        assert_eq!(args.first().map(String::as_str), Some("clippy"));
        for name in [
            "caladrius-nca",
            "caladrius-models",
            "caladrius-fit",
            "caladrius-project",
        ] {
            assert!(args.windows(2).any(|pair| pair == ["-p", name]), "{name}");
        }
        assert!(args.iter().any(|arg| arg == "--lib"));
        assert!(!args.iter().any(|arg| arg == "--all-targets"));
        assert!(args.ends_with(&["--".to_owned(), "-D".to_owned(), LINT.to_owned()]));
    }

    #[test]
    fn only_the_lint_lines_are_findings() {
        let output = "\
    Checking caladrius-nca v0.1.0 (C:/x)\n\
crates/caladrius-nca/src/a.rs:10:5: error: indexing may panic [clippy::indexing_slicing]\n\
crates/caladrius-nca/src/b.rs:3:1: warning: unused import: `x`\n\
error: could not compile `caladrius-nca` due to 1 previous error\n";
        assert_eq!(findings(output).len(), 1);
        assert!(findings("    Finished `dev` profile\n").is_empty());
    }

    #[test]
    fn the_summary_names_the_count_and_the_crates() {
        assert!(summary(0).contains("ok, 0 findings"));
        assert!(summary(3).contains("3 finding(s)"));
        assert!(summary(0).contains("caladrius-project"));
    }
}
