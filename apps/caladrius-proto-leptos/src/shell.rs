//! The Tauri 2 shell, seen from the page. In the shell the engine stays in this WebAssembly module;
//! the shell only gives the window and the two commands of the measure mode (`measure_requested`,
//! `measure_write`). They are reached through `window.__TAURI_INTERNALS__.invoke`, which Tauri puts in
//! every page it serves.

use js_sys::{Function, Object, Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

fn internals() -> Option<JsValue> {
    let win: JsValue = web_sys::window()?.into();
    let value = Reflect::get(&win, &JsValue::from_str("__TAURI_INTERNALS__")).ok()?;
    (!value.is_undefined() && !value.is_null()).then_some(value)
}

/// True in the window of the Tauri shell, false in a browser.
pub fn in_tauri() -> bool {
    internals().is_some()
}

async fn invoke(command: &str, args: &Object) -> Result<JsValue, String> {
    let internals = internals().ok_or_else(|| "this page is not in the Tauri shell".to_owned())?;
    let call = Reflect::get(&internals, &JsValue::from_str("invoke"))
        .map_err(|_| "the shell has no invoke".to_owned())?;
    let call = call
        .dyn_into::<Function>()
        .map_err(|_| "the shell has no invoke".to_owned())?;
    let promise = call
        .call2(&internals, &JsValue::from_str(command), args)
        .map_err(|e| format!("{command}: {e:?}"))?
        .dyn_into::<Promise>()
        .map_err(|_| format!("{command}: no promise"))?;
    JsFuture::from(promise).await.map_err(|e| {
        format!(
            "{command}: {}",
            e.as_string().unwrap_or_else(|| format!("{e:?}"))
        )
    })
}

/// The answer of `measure_requested`: whether to measure, and how long the process has run (the page
/// adds the time since this answer to date its first frame from the start of the process).
pub struct MeasureAsk {
    pub requested: bool,
    pub since_start_ms: f64,
}

pub async fn measure_requested() -> Option<MeasureAsk> {
    let answer = invoke("measure_requested", &Object::new()).await.ok()?;
    let requested = Reflect::get(&answer, &JsValue::from_str("requested"))
        .ok()?
        .as_bool()?;
    let since_start_ms = Reflect::get(&answer, &JsValue::from_str("since_start_ms"))
        .ok()?
        .as_f64()?;
    Some(MeasureAsk {
        requested,
        since_start_ms,
    })
}

/// Hands the JSON of the measure to the shell, which writes it to the file of `CALADRIUS_MEASURE`.
pub async fn measure_write(json: &str) -> Result<(), String> {
    let args = Object::new();
    Reflect::set(&args, &JsValue::from_str("json"), &JsValue::from_str(json))
        .map_err(|_| "cannot build the arguments".to_owned())?;
    invoke("measure_write", &args).await.map(|_| ())
}
