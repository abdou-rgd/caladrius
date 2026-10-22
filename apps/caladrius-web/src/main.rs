#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! App layer: the browser application, `caladrius-ui` compiled to WebAssembly and drawn by eframe
//! on a WebGL canvas. Same interface and same engine as the desktop; files are read in the page and
//! saved as downloads, settings stay in the browser's local storage, nothing is sent anywhere.
//! `cargo xtask web` builds the page; `index.html` is its shell.

#[cfg(target_arch = "wasm32")]
mod browser;
// Outside wasm32 only the tests use it.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod host;

#[cfg(target_arch = "wasm32")]
fn main() {
    browser::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    eprintln!(
        "caladrius-web is the browser application: build the page with `cargo xtask web` and \
         serve the folder it writes. On this computer, run `caladrius`."
    );
    std::process::ExitCode::from(1)
}
