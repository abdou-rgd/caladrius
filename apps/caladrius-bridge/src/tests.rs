use super::*;

const CSV: &str = "subject,time,conc,dose\n1,0,0,100\n1,1,6,100\n1,2,5,100\n1,4,3,100\n1,8,1.5,100\n1,12,0.7,100\n";

fn request(command: &str, params: Value) -> String {
    json!({ "command": command, "params": params }).to_string()
}

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

#[test]
fn a_command_answers_with_its_result_and_its_seal() {
    let mut bridge = Bridge::without_provenance();
    let text = bridge.execute(&request("data.import", json!({ "name": "t", "csv": CSV })));
    let answer = parsed(&text);
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["command"], "data.import");
    assert_eq!(answer["error"], Value::Null);
    assert_eq!(answer["at"], Value::Null);
    assert!(answer["result"].is_object());
    // The seal is the one of the history entry the command recorded.
    let recorded = bridge
        .engine()
        .history()
        .entries()
        .last()
        .and_then(|e| e.seal.clone());
    assert_eq!(
        answer["seal"],
        serde_json::to_value(recorded).unwrap_or(Value::Null)
    );
    assert_eq!(answer["seal"]["sha256"].as_str().map(str::len), Some(64));
    assert_eq!(answer["seal"].get("engine"), None, "no provenance");
}

#[test]
fn the_answer_is_the_bytes_of_the_engine_in_canonical_form() {
    let mut bridge = Bridge::without_provenance();
    let mut engine = Engine::new();
    engine.set_provenance(false);
    let params = json!({ "name": "t", "csv": CSV });
    let text = bridge.execute(&request("data.import", params.clone()));
    let direct = engine.execute("data.import", params).unwrap_or(Value::Null);
    assert_eq!(parsed(&text)["result"], direct);
    // Keys in byte order.
    assert!(
        text.starts_with("{\"at\":null,\"command\":\"data.import\",\"error\":null,\"ok\":true,")
    );
}

#[test]
fn provenance_goes_in_the_seal_unless_it_is_turned_off() {
    let with = parsed(&Bridge::new().execute(&request("units.check", json!({}))));
    let without = parsed(&Bridge::without_provenance().execute(&request("units.check", json!({}))));
    assert!(with["seal"]["engine"].is_string());
    assert_eq!(without["seal"].get("engine"), None);
    // The hash does not depend on it.
    assert_eq!(with["seal"]["sha256"], without["seal"]["sha256"]);
}

#[test]
fn a_command_that_fails_has_the_same_shape_and_a_seal() {
    let mut bridge = Bridge::without_provenance();
    let unknown = parsed(&bridge.execute(&request("no.such", json!({}))));
    assert_eq!(unknown["ok"], false);
    assert_eq!(unknown["result"], Value::Null);
    assert_eq!(unknown["error"]["code"], "unknown_command");
    assert!(unknown["seal"]["sha256"].is_string());
    let invalid = parsed(&bridge.execute(&request("nca.run", json!({ "wrong": 1 }))));
    assert_eq!(invalid["error"]["code"], "invalid_parameters");
    let keys = |v: &Value| -> Vec<String> {
        v.as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    };
    assert_eq!(keys(&unknown), keys(&invalid));
    assert_eq!(
        keys(&unknown),
        ["at", "command", "error", "ok", "result", "seal"]
    );
}

#[test]
fn params_may_be_left_out() {
    let mut bridge = Bridge::without_provenance();
    let answer = parsed(&bridge.execute(r#"{"command":"history.list"}"#));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["result"]["total"], 0);
}

#[test]
fn a_request_that_is_not_one_is_refused_without_running_anything() {
    let mut bridge = Bridge::without_provenance();
    for (text, id) in [
        ("not json", Value::Null),
        ("[1]", Value::Null),
        ("{}", Value::Null),
        (r#"{"command": 3}"#, Value::Null),
        (r#"{"command": ""}"#, Value::Null),
        (
            r#"{"command":"history.list","param":{}}"#,
            json!("history.list"),
        ),
    ] {
        let answer = parsed(&bridge.execute(text));
        assert_eq!(answer["ok"], false, "{text}");
        assert_eq!(answer["error"]["code"], "invalid_request", "{text}");
        assert_eq!(answer["command"], id, "{text}");
        assert_eq!(answer["seal"], Value::Null, "{text}");
        assert_eq!(answer["at"], Value::Null, "{text}");
    }
    assert!(bridge.engine().history().is_empty());
}

#[test]
fn a_command_without_a_seal_answers_with_a_null_seal() {
    let mut bridge = Bridge::without_provenance();
    let answer = parsed(&bridge.execute(&request("project.load", json!({ "text": "{}" }))));
    assert_eq!(answer["command"], "project.load");
    assert_eq!(answer["seal"], Value::Null);
}

#[test]
fn describe_lists_every_command() {
    let commands = parsed(&describe());
    let ids: Vec<&str> = commands
        .as_array()
        .map(|a| a.iter().filter_map(|c| c["id"].as_str()).collect())
        .unwrap_or_default();
    assert_eq!(ids, caladrius_engine::command_ids());
}

#[test]
fn the_history_command_sees_what_the_bridge_ran() {
    let mut bridge = Bridge::without_provenance();
    bridge.execute(&request("data.import", json!({ "name": "t", "csv": CSV })));
    bridge.execute(&request("no.such", json!({})));
    let list = parsed(&bridge.execute(&request("history.list", json!({}))));
    assert_eq!(list["result"]["total"], 2);
}
