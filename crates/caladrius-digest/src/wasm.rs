//! The entry points of the WebAssembly module, for Node (`cargo xtask digest --wasm`).
//!
//! The module is built with `cargo rustc --crate-type cdylib` and wrapped by `wasm-bindgen
//! --target nodejs`: `run_workload` takes the text of a workload and gives back the canonical text
//! of the results, `sha256_hex` hashes a text with the same code the native build uses.

use wasm_bindgen::prelude::wasm_bindgen;

/// Runs a workload (JSON text) and returns the canonical documents, one per line.
#[wasm_bindgen]
pub fn run_workload(workload: &str) -> Result<String, String> {
    crate::run_text(workload).map(|run| run.text())
}

/// The SHA-256 of `text` as UTF-8, 64 hexadecimal digits.
#[wasm_bindgen]
pub fn sha256_text(text: &str) -> String {
    crate::sha256_hex(text.as_bytes())
}
