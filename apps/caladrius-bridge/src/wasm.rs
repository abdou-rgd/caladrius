//! The WebAssembly module (`wasm-bindgen --target web` for the Web Worker and the page,
//! `--target nodejs` for `cargo xtask bridge`).
//!
//! It exports the one call (`Bridge.execute`), `describe()`, and the formatting functions of
//! `caladrius-format`. The formatting functions take their display (significant digits and decimal
//! comma) as arguments: a module has one thread and the page may hold two instances of it, so no
//! setting is kept between calls.

use caladrius_format::Display;
use wasm_bindgen::prelude::wasm_bindgen;

/// The engine of a worker. One instance owns one engine and its history.
#[wasm_bindgen(js_name = Bridge)]
pub struct WasmBridge {
    inner: crate::Bridge,
}

#[wasm_bindgen(js_class = Bridge)]
impl WasmBridge {
    /// A new engine. `provenance` false leaves the version and the commit out of the seals.
    #[wasm_bindgen(constructor)]
    pub fn new(provenance: bool) -> WasmBridge {
        let inner = if provenance {
            crate::Bridge::new()
        } else {
            crate::Bridge::without_provenance()
        };
        WasmBridge { inner }
    }

    /// The call: a request as JSON text in, the sealed answer as JSON text out.
    pub fn execute(&mut self, request: &str) -> String {
        self.inner.execute(request)
    }
}

/// The commands with the schemas of their parameters and results, as JSON text.
#[wasm_bindgen]
pub fn describe() -> String {
    crate::describe()
}

fn display(digits: u8, comma: bool) -> Display {
    Display { digits, comma }
}

/// `caladrius_format::number_with`.
#[wasm_bindgen(js_name = formatNumber)]
pub fn format_number(x: f64, digits: u8, comma: bool) -> String {
    caladrius_format::number_with(x, display(digits, comma))
}

/// `caladrius_format::fit_quality_with`.
#[wasm_bindgen(js_name = formatFitQuality)]
pub fn format_fit_quality(x: f64, digits: u8, comma: bool) -> String {
    caladrius_format::fit_quality_with(x, display(digits, comma))
}

/// `caladrius_format::exact_with`.
#[wasm_bindgen(js_name = formatExact)]
pub fn format_exact(x: f64, digits: u8, comma: bool) -> String {
    caladrius_format::exact_with(x, display(digits, comma))
}

/// `caladrius_format::exact_point`.
#[wasm_bindgen(js_name = formatExactPoint)]
pub fn format_exact_point(x: f64) -> String {
    caladrius_format::exact_point(x)
}

/// `caladrius_format::parse_number`: `undefined` for text that is not a finite number.
#[wasm_bindgen(js_name = parseNumber)]
pub fn parse_number(text: &str) -> Option<f64> {
    caladrius_format::parse_number(text)
}

/// `caladrius_format::parameter_label`.
#[wasm_bindgen(js_name = parameterLabel)]
pub fn parameter_label(name: &str, extravascular: bool) -> String {
    caladrius_format::parameter_label(name, extravascular)
}

/// `caladrius_format::parameter_unit`. `derived` is the JSON object of derived units that the
/// engine names (`{"auc": "h*mg/L", ...}`); text that is not one counts as none.
#[wasm_bindgen(js_name = parameterUnit)]
pub fn parameter_unit(
    name: &str,
    time: Option<String>,
    conc: Option<String>,
    derived: &str,
) -> Option<String> {
    let derived: std::collections::BTreeMap<String, String> =
        serde_json::from_str(derived).unwrap_or_default();
    caladrius_format::parameter_unit(name, time.as_deref(), conc.as_deref(), &derived)
}

/// `caladrius_format::with_labels`.
#[wasm_bindgen(js_name = withLabels)]
pub fn with_labels(sentence: &str) -> String {
    caladrius_format::with_labels(sentence)
}

/// `caladrius_format::plain`.
#[wasm_bindgen]
pub fn plain(text: &str) -> String {
    caladrius_format::plain(text)
}

/// `caladrius_format::words`.
#[wasm_bindgen]
pub fn words(code: &str) -> String {
    caladrius_format::words(code)
}
