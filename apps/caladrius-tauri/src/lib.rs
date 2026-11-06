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
//! transport of the one call (the first is a Web Worker running the same engine as WebAssembly); the
//! window is a placeholder page (`dist/`) that calls it once and shows the sealed answer. The
//! capability `capabilities/default.json` allows this command and no other.

use std::sync::{Mutex, PoisonError};

use caladrius_bridge::Bridge;
use tauri::{Runtime, State};

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

/// Gives `builder` its engine and its one command. The runtime is a parameter so that the tests
/// can use the mock one.
pub fn with_engine<R: Runtime>(builder: tauri::Builder<R>, bridge: Bridge) -> tauri::Builder<R> {
    builder
        .manage(EngineState::new(bridge))
        .invoke_handler(tauri::generate_handler![engine_execute])
}

/// Starts the application. The seals say which engine made them (version and commit).
pub fn run() {
    let builder = with_engine(tauri::Builder::default(), Bridge::new());
    if let Err(error) = builder.run(tauri::generate_context!()) {
        eprintln!("caladrius: the window could not start: {error}");
        std::process::exit(1);
    }
}
