//! The command handler of the shell, called through Tauri's IPC on the mock runtime: no window on
//! the screen, no webview. `cargo xtask bridge` runs this file as the third transport.

use caladrius_bridge::Bridge;
use caladrius_tauri_lib::with_engine;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindowBuilder};

/// The shell, built with seals that carry no version and no commit: the run of `cargo xtask bridge`
/// leaves provenance out in all three transports.
fn shell() -> Result<(App<tauri::test::MockRuntime>, tauri::WebviewWindow<tauri::test::MockRuntime>), String> {
    let app = with_engine(mock_builder(), Bridge::without_provenance())
        .build(mock_context(noop_assets()))
        .map_err(|e| format!("the shell did not build: {e}"))?;
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .map_err(|e| format!("the window did not build: {e}"))?;
    Ok((app, window))
}

/// One call of `engine_execute`, as the page makes it: `invoke('engine_execute', { request })`.
fn invoke(window: &tauri::WebviewWindow<tauri::test::MockRuntime>, request: &str) -> Result<String, String> {
    let url = "http://tauri.localhost"
        .parse()
        .map_err(|e| format!("bad url: {e}"))?;
    let response = get_ipc_response(
        window,
        InvokeRequest {
            cmd: "engine_execute".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url,
            body: InvokeBody::Json(json!({ "request": request })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map_err(|e| format!("the command failed: {e}"))?;
    response
        .deserialize::<String>()
        .map_err(|e| format!("the answer is not text: {e}"))
}

#[test]
fn the_command_gives_the_answer_the_bridge_gives() -> Result<(), String> {
    let (_app, window) = shell()?;
    let mut reference = Bridge::without_provenance();
    for request in [
        r#"{"command":"data.preview","params":{"csv":"time,conc\n0,0\n1,5\n"}}"#,
        r#"{"command":"no.such.command"}"#,
        "not json",
        r#"{"command":"history.list"}"#,
    ] {
        assert_eq!(invoke(&window, request)?, reference.execute(request), "{request}");
    }
    Ok(())
}

#[test]
fn one_engine_serves_every_call() -> Result<(), String> {
    let (_app, window) = shell()?;
    invoke(&window, r#"{"command":"no.such.command"}"#)?;
    let list: Value = serde_json::from_str(&invoke(&window, r#"{"command":"history.list"}"#)?)
        .map_err(|e| e.to_string())?;
    assert_eq!(list["result"]["total"], 1);
    Ok(())
}

/// The script of `cargo xtask bridge`: the requests are in the file named by
/// `CALADRIUS_BRIDGE_REQUESTS` (a JSON list of texts); the answers go to the standard output on one
/// line. Without the variable the test has nothing to do.
#[test]
fn the_script_of_the_same_bytes_check() -> Result<(), String> {
    let Some(path) = std::env::var_os("CALADRIUS_BRIDGE_REQUESTS") else {
        return Ok(());
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read the requests: {e}"))?;
    let requests: Vec<String> =
        serde_json::from_str(&text).map_err(|e| format!("the requests are not a list of texts: {e}"))?;
    let (_app, window) = shell()?;
    let answers = requests
        .iter()
        .map(|request| invoke(&window, request))
        .collect::<Result<Vec<String>, String>>()?;
    let line = serde_json::to_string(&answers).map_err(|e| e.to_string())?;
    println!("BRIDGE-ANSWERS {line}");
    Ok(())
}
