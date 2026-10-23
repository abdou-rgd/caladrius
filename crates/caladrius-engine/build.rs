//! Records the git commit the engine is built from, for the provenance of the seals
//! (`caladrius_engine::seal::GIT_COMMIT`). The commit is `CALADRIUS_GIT_COMMIT` when the build
//! environment sets it (a build outside a git checkout, for instance from a source archive), else
//! `git rev-parse HEAD`; when neither is available there is no commit and the seals say none. A
//! tree with uncommitted changes carries the commit it started from.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=CALADRIUS_GIT_COMMIT");
    println!("cargo:rerun-if-changed=build.rs");
    let given = std::env::var("CALADRIUS_GIT_COMMIT")
        .ok()
        .map(|c| c.trim().to_owned())
        .filter(|c| is_commit(c));
    let commit = given.or_else(|| git(&["rev-parse", "HEAD"]).filter(|c| is_commit(c)));
    if let Some(commit) = commit {
        println!("cargo:rustc-env=CALADRIUS_GIT_COMMIT={commit}");
    }
    // Build again when the commit moves. Only files that exist: Cargo reads a missing one as
    // changed at every build.
    let mut watched = vec![git(&["rev-parse", "--git-path", "HEAD"])];
    watched.push(git(&["rev-parse", "--git-path", "packed-refs"]));
    if let Some(head) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watched.push(git(&["rev-parse", "--git-path", &head]));
    }
    for path in watched.into_iter().flatten() {
        if Path::new(&path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

/// 40 hexadecimal digits (SHA-1) or 64 (SHA-256 repositories).
fn is_commit(text: &str) -> bool {
    (text.len() == 40 || text.len() == 64) && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}
