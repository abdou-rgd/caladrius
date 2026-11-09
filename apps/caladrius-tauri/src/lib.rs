#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! The desktop shell of the web front: a Tauri 2 window and one command.
//!
//! `engine_execute` takes the request of the bridge as text (`{"command": "<id>", "params": ...}`)
//! and returns its answer as text, both documented in `caladrius-bridge`. The command is the second
//! transport of the one call (the first is a Web Worker running the same engine as WebAssembly). The
//! window shows the web front (`apps/caladrius-front`, built to its `dist/`, or its dev server at the
//! `devUrl` of `tauri.conf.json`): the page finds `__TAURI_INTERNALS__`, uses the Tauri transport and
//! starts no worker, so a fit runs in this process with the same numbers as in the browser. The
//! capability `capabilities/default.json` allows this command and the two measure commands, no other.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use caladrius_bridge::Bridge;
use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

/// The engine of the application: one, behind a lock, shared by every call of the window.
pub struct EngineState(Mutex<Bridge>);

impl EngineState {
    pub fn new(bridge: Bridge) -> Self {
        Self(Mutex::new(bridge))
    }

    /// The call of the bridge. A lock left poisoned by a panic elsewhere is taken as it is: the
    /// bridge keeps no half-written state, and the history shows what ran.
    pub fn execute(&self, request: &str) -> String {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .execute(request)
    }
}

/// The one command of the shell. `async` in Tauri's sense: it runs on a worker thread, not on the
/// thread of the window, so that a long fit does not freeze it.
#[tauri::command(async)]
fn engine_execute(state: State<'_, EngineState>, request: String) -> String {
    state.execute(&request)
}

/// The measure mode (the wave 3 brief): when the variable `CALADRIUS_MEASURE` names a file, the page
/// runs its measure once the first frame is on screen and hands the JSON back with `measure_write`,
/// which writes it to that file and, three seconds later (the memory is sampled then), ends the
/// application.
pub struct MeasureState {
    path: Option<PathBuf>,
    started: Instant,
}

impl MeasureState {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self { path, started: Instant::now() }
    }

    /// From the environment of the process.
    pub fn from_env() -> Self {
        Self::new(std::env::var_os("CALADRIUS_MEASURE").filter(|p| !p.is_empty()).map(PathBuf::from))
    }
}

/// The answer of `measure_requested`: whether to measure, and how long the process has run (the
/// page adds the time since this answer to date its first frame from the start of the process).
#[derive(Serialize)]
pub struct MeasureAsk {
    requested: bool,
    since_start_ms: f64,
}

#[tauri::command]
fn measure_requested(state: State<'_, MeasureState>) -> MeasureAsk {
    MeasureAsk {
        requested: state.path.is_some(),
        since_start_ms: state.started.elapsed().as_secs_f64() * 1000.0,
    }
}

/// Writes the JSON of the measure to the file of `CALADRIUS_MEASURE`, then ends the application after
/// three seconds. Without the variable the call is refused: the window cannot write a file of its choice.
#[tauri::command]
fn measure_write<R: Runtime>(app: AppHandle<R>, state: State<'_, MeasureState>, json: String) -> Result<(), String> {
    let Some(path) = state.path.as_ref() else {
        return Err("the measure was not requested (CALADRIUS_MEASURE is not set)".to_string());
    };
    std::fs::write(path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        app.exit(0);
    });
    Ok(())
}

/// Gives `builder` its engine and its commands. The runtime is a parameter so that the tests can
/// use the mock one.
pub fn with_engine<R: Runtime>(builder: tauri::Builder<R>, bridge: Bridge) -> tauri::Builder<R> {
    builder
        .manage(EngineState::new(bridge))
        .manage(MeasureState::from_env())
        .invoke_handler(tauri::generate_handler![engine_execute, measure_requested, measure_write])
}

/// The folder where WebView2 keeps the data of the window: `%LOCALAPPDATA%\<identifier>` on Windows
/// (it is created by the webview, and a refusal ends in a panic deep inside it). Elsewhere the
/// platform decides and there is nothing to check.
pub fn webview_folder(local_app_data: Option<OsString>, identifier: &str) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    local_app_data.filter(|v| !v.is_empty()).map(|base| Path::new(&base).join(identifier))
}

/// Makes sure the webview folder can be created, and says in one sentence what to do when it cannot.
pub fn ensure_webview_folder(folder: &Path) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|error| {
        format!(
            "Caladrius cannot open its window: the folder {} for the web view cannot be created ({error}). Free that location or make it writable, then start Caladrius again.",
            folder.display()
        )
    })
}

/// Starts the application. The seals say which engine made them (version and commit).
pub fn run() {
    let context = tauri::generate_context!();
    if let Some(folder) = webview_folder(std::env::var_os("LOCALAPPDATA"), &context.config().identifier) {
        if let Err(sentence) = ensure_webview_folder(&folder) {
            eprintln!("{sentence}");
            std::process::exit(1);
        }
    }
    let builder = with_engine(tauri::Builder::default(), Bridge::new());
    if let Err(error) = builder.run(context) {
        eprintln!("caladrius: the window could not start: {error}");
        std::process::exit(1);
    }
}
