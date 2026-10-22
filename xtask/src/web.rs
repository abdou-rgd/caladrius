//! `cargo xtask web [--out DIR]`: builds the browser application (`apps/caladrius-web`) into a
//! folder any static server can serve: `index.html`, `caladrius.js` and `caladrius_bg.wasm`.
//!
//! Steps: `cargo build --profile web --target wasm32-unknown-unknown -p caladrius-web`, then
//! `wasm-bindgen --target web`, then `wasm-opt -Os` when it is on the PATH (optional: it saves
//! about a fifth of the raw size, a few percent once compressed), then the page's `index.html`.
//! The task installs nothing: a missing tool gives the command that installs it. `wasm-bindgen`
//! must be the version of `wasm-bindgen` in `Cargo.lock`, or the generated glue does not match.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::console;
use crate::error::{Result, XtaskError};
use crate::workspace;

const TARGET: &str = "wasm32-unknown-unknown";
const CRATE: &str = "caladrius-web";
/// The Cargo profile of the page (root `Cargo.toml`): release, optimised for size.
const PROFILE: &str = "web";
/// The name of the generated files: `caladrius.js`, `caladrius_bg.wasm`.
const OUT_NAME: &str = "caladrius";
/// The WebAssembly features the Rust target emits, which `wasm-opt` must accept.
const WASM_OPT_FEATURES: [&str; 6] = [
    "--enable-bulk-memory",
    "--enable-nontrapping-float-to-int",
    "--enable-sign-ext",
    "--enable-mutable-globals",
    "--enable-reference-types",
    "--enable-multivalue",
];

/// The version of a package in the text of `Cargo.lock`.
fn locked_version(lock: &str, package: &str) -> Option<String> {
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() == format!("name = \"{package}\"") {
            let version = lines.next()?.trim();
            return version
                .strip_prefix("version = \"")
                .and_then(|v| v.strip_suffix('"'))
                .map(str::to_owned);
        }
    }
    None
}

/// The version `wasm-bindgen --version` prints (`wasm-bindgen 0.2.129`).
fn cli_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .nth(1)
        .map(|v| v.trim().to_owned())
}

fn install_message(version: &str) -> String {
    format!("install it with:\n    cargo install wasm-bindgen-cli --version {version} --locked")
}

/// Arguments of the `cargo build` of the page.
fn build_args() -> Vec<String> {
    [
        "build",
        "--profile",
        PROFILE,
        "--target",
        TARGET,
        "-p",
        CRATE,
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// Arguments of `wasm-bindgen` for the built module `wasm`.
fn bindgen_args(wasm: &Path, out: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![wasm.into()];
    for a in [
        "--target",
        "web",
        "--no-typescript",
        "--out-name",
        OUT_NAME,
        "--out-dir",
    ] {
        args.push(a.into());
    }
    args.push(out.into());
    args
}

/// Reads `--out DIR`; the default is `target/web-page` under the workspace (`target/web` is the profile's own folder).
fn out_dir(root: &Path, args: &[OsString]) -> std::result::Result<PathBuf, String> {
    match args {
        [] => Ok(root.join("target").join("web-page")),
        [flag, dir] if flag.to_str() == Some("--out") => Ok(PathBuf::from(dir)),
        [first, ..] => Err(format!(
            "unexpected argument `{}`; usage: cargo xtask web [--out DIR]",
            first.to_string_lossy()
        )),
    }
}

fn run_tool(program: &OsString, args: &[OsString], what: &str) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|source| {
            XtaskError::new(format!(
                "cannot run {}: {source}",
                program.to_string_lossy()
            ))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(XtaskError::new(format!("web: {what} failed ({status})")))
    }
}

fn size(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(m) => format!("{:.2} MB", m.len() as f64 / 1_000_000.0),
        Err(_) => "missing".to_owned(),
    }
}

pub fn run(args: &[OsString]) -> Result<()> {
    let root = workspace::root()?;
    let out = out_dir(&root, args).map_err(XtaskError::new)?;
    if !crate::wasm::target_installed()? {
        return Err(XtaskError::new(crate::wasm::missing_target_message()));
    }
    let lock_path = root.join("Cargo.lock");
    let lock =
        std::fs::read_to_string(&lock_path).map_err(|e| XtaskError::io("read", &lock_path, &e))?;
    let wanted = locked_version(&lock, "wasm-bindgen")
        .ok_or_else(|| XtaskError::new("web: Cargo.lock names no wasm-bindgen"))?;
    let bindgen = env::var_os("WASM_BINDGEN").unwrap_or_else(|| OsString::from("wasm-bindgen"));
    let found = Command::new(&bindgen)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| cli_version(&String::from_utf8_lossy(&o.stdout)));
    match found {
        None => {
            return Err(XtaskError::new(format!(
                "web: wasm-bindgen is not on the PATH; {}",
                install_message(&wanted)
            )));
        }
        Some(v) if v != wanted => {
            return Err(XtaskError::new(format!(
                "web: wasm-bindgen {v} does not match the wasm-bindgen {wanted} of Cargo.lock; {}",
                install_message(&wanted)
            )));
        }
        Some(_) => {}
    }

    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let build: Vec<OsString> = build_args().into_iter().map(OsString::from).collect();
    console::out(&format!("web: cargo {}", build_args().join(" ")));
    run_tool(&cargo, &build, "cargo build")?;

    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let module = target_dir
        .join(TARGET)
        .join(PROFILE)
        .join(format!("{CRATE}.wasm"));
    std::fs::create_dir_all(&out).map_err(|e| XtaskError::io("create", &out, &e))?;
    run_tool(&bindgen, &bindgen_args(&module, &out), "wasm-bindgen")?;

    let wasm = out.join(format!("{OUT_NAME}_bg.wasm"));
    let wasm_opt = env::var_os("WASM_OPT").unwrap_or_else(|| OsString::from("wasm-opt"));
    let has_opt = Command::new(&wasm_opt)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if has_opt {
        let mut opt: Vec<OsString> = vec!["-Os".into()];
        opt.extend(WASM_OPT_FEATURES.iter().map(OsString::from));
        opt.extend([wasm.clone().into(), "-o".into(), wasm.clone().into()]);
        run_tool(&wasm_opt, &opt, "wasm-opt")?;
    } else {
        console::out("web: wasm-opt not found, the module is left as wasm-bindgen wrote it");
    }

    let page = root.join("apps").join(CRATE).join("index.html");
    let target = out.join("index.html");
    std::fs::copy(&page, &target).map_err(|e| XtaskError::io("copy", &page, &e))?;
    console::out(&format!(
        "web: ok, {} (index.html, {OUT_NAME}.js {}, {OUT_NAME}_bg.wasm {})",
        out.display(),
        size(&out.join(format!("{OUT_NAME}.js"))),
        size(&wasm)
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_locked_version_is_read_from_cargo_lock() {
        let lock = "[[package]]\nname = \"wasm-bindgen-shared\"\nversion = \"9.9.9\"\n\n[[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.129\"\nsource = \"x\"\n";
        assert_eq!(
            locked_version(lock, "wasm-bindgen").as_deref(),
            Some("0.2.129")
        );
        assert_eq!(locked_version(lock, "eframe"), None);
    }

    #[test]
    fn the_cli_version_is_the_second_word() {
        assert_eq!(
            cli_version("wasm-bindgen 0.2.129\n").as_deref(),
            Some("0.2.129")
        );
        assert_eq!(cli_version(""), None);
        assert!(install_message("0.2.129").ends_with("--version 0.2.129 --locked"));
    }

    #[test]
    fn the_page_is_built_with_the_size_profile_for_the_browser() {
        assert_eq!(
            build_args().join(" "),
            "build --profile web --target wasm32-unknown-unknown -p caladrius-web"
        );
        let args = bindgen_args(Path::new("m.wasm"), Path::new("out"));
        let text: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            text.join(" "),
            "m.wasm --target web --no-typescript --out-name caladrius --out-dir out"
        );
    }

    #[test]
    fn the_output_folder_defaults_to_target_web() {
        let root = Path::new("/w");
        assert_eq!(out_dir(root, &[]), Ok(PathBuf::from("/w/target/web-page")));
        let args = [OsString::from("--out"), OsString::from("dist/app")];
        assert_eq!(out_dir(root, &args), Ok(PathBuf::from("dist/app")));
        assert!(out_dir(root, &[OsString::from("--bogus")]).is_err());
    }
}
