//! Tests of `plot.scene` and of `export.plot` on an NCA analysis (T-133), run through the
//! engine's own commands: import, then `nca.run` or `plot.xy`, then the scene. Data of the
//! repository's oracle (`oracle/data/edge_blq.csv`).

use serde_json::{Value, json};

use crate::schema::validate;
use crate::*;

const EDGE_BLQ: &str = include_str!("../../../oracle/data/edge_blq.csv");

fn ok(engine: &mut Engine, id: &str, params: Value) -> Value {
    match engine.execute(id, params) {
        Ok(v) => v,
        Err(e) => panic!("{id} failed: {e}"),
    }
}

fn err(engine: &mut Engine, id: &str, params: Value) -> CommandError {
    match engine.execute(id, params) {
        Ok(v) => panic!("{id} should have failed, gave {v}"),
        Err(e) => e,
    }
}

fn units() -> Value {
    json!({ "time": "h", "concentration": "mg/L", "dose": "mg" })
}

fn sheet() -> Engine {
    let mut e = Engine::new();
    ok(
        &mut e,
        "data.import",
        json!({ "name": "edge_blq", "csv": EDGE_BLQ }),
    );
    e
}

/// The oracle sheet imported, and the NCA of one subject with these extra options.
fn nca(subject: &str, options: Value) -> (Engine, u64) {
    let mut e = sheet();
    let mut opts = options;
    if let Some(map) = opts.as_object_mut() {
        map.insert("units".to_owned(), units());
    }
    let run = ok(
        &mut e,
        "nca.run",
        json!({ "worksheet": 1, "route": "extravascular", "subject": subject, "options": opts }),
    );
    let id = run["id"].as_u64().unwrap_or(0);
    (e, id)
}

fn series<'a>(scene: &'a Value, panel: usize, role: &str) -> Option<&'a Value> {
    scene["panels"][panel]["series"]
        .as_array()?
        .iter()
        .find(|s| s["role"] == role)
}

fn times(s: Option<&Value>) -> Vec<f64> {
    s.and_then(|s| s["points"].as_array())
        .map(|ps| ps.iter().filter_map(|p| p["x"].as_f64()).collect())
        .unwrap_or_default()
}

fn schema_of(id: &str) -> Value {
    describe()
        .into_iter()
        .find(|c| c.id == id)
        .map(|c| c.result_schema)
        .unwrap_or(Value::Null)
}

/// Validates an answer against the result schema of its command.
fn check(command: &str, answer: &Value) -> Result<(), String> {
    let schema = schema_of(command);
    validate::check(&schema, &schema, answer)
}

/// The selected lambda z candidate of the first subject of an NCA analysis.
fn chosen(e: &mut Engine, id: u64) -> Value {
    let got = ok(e, "analysis.get", json!({ "analysis": id }));
    got["result"]["subjects"][0]["outcome"]["ok"]["lambda_z_candidates"]
        .as_array()
        .and_then(|c| c.iter().find(|c| c["selected"] == true))
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn the_nca_scene_puts_the_lambda_z_points_where_nca_run_chose_the_range() {
    let (mut e, id) = nca("3", json!({}));
    // The range of nca.run: 2 to 8 h, four points.
    let range = chosen(&mut e, id);
    assert_eq!(range["time_first"], 2.0);
    assert_eq!(range["time_last"], 8.0);
    assert_eq!(range["n_points"], 4);

    let answer = ok(&mut e, "plot.scene", json!({ "analysis": id }));
    assert_eq!(answer["kind"], "nca");
    assert_eq!(answer["subject"], "3");
    assert_eq!(answer["status"], "fresh");
    let scene = &answer["scene"];
    assert_eq!(scene["panels"].as_array().map(Vec::len), Some(1));
    let used = times(series(scene, 0, "lambda_z_used"));
    assert_eq!(used, vec![2.0, 4.0, 6.0, 8.0]);
    assert_eq!(times(series(scene, 0, "lambda_z_start")), vec![2.0]);

    // The line is the engine's intercept and lambda z, from the first point of the phase to the
    // last time, 61 points.
    let line = series(scene, 0, "lambda_z_fit")
        .and_then(|s| s["points"].as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(line.len(), 61);
    assert_eq!(line[0]["x"], 2.0);
    assert_eq!(line[60]["x"], 24.0);
    let a = range["intercept"].as_f64().unwrap_or(f64::NAN);
    let k = range["lambda_z"].as_f64().unwrap_or(f64::NAN);
    for p in &line {
        let t = p["x"].as_f64().unwrap_or(f64::NAN);
        let y = p["y"].as_f64().unwrap_or(f64::NAN);
        assert!((y - libm::exp(a - k * t)).abs() < 1e-12, "at {t}: {y}");
    }

    // The zeros after the last positive value are ordinary observed points.
    assert_eq!(
        times(series(scene, 0, "observed")),
        vec![0.0, 0.5, 1.0, 12.0, 24.0]
    );
    assert!(series(scene, 0, "replaced").is_none());
    assert!(series(scene, 0, "excluded").is_none());
    // No pixel, no colour in the data.
    let text = scene.to_string();
    assert!(!text.contains("color") && !text.contains("colour") && !text.contains("px"));
    // Axes, ticks and the legend are there.
    assert_eq!(scene["panels"][0]["x"]["title"], "Time (h)");
    assert_eq!(scene["panels"][0]["y"]["title"], "Concentration (mg/L)");
    assert_eq!(scene["legend"]["entries"][0]["name"], "Profile");
    assert_eq!(check("plot.scene", &answer), Ok(()));
}

#[test]
fn the_semi_log_nca_scene_is_in_log10_and_says_what_it_hides() {
    let (mut e, id) = nca("3", json!({}));
    let answer = ok(&mut e, "plot.scene", json!({ "analysis": id, "log": true }));
    let panel = &answer["scene"]["panels"][0];
    assert_eq!(panel["y"]["log"], true);
    assert_eq!(panel["x"]["log"], false);
    // The points are decades: 6.3 mg/L is log10(6.3).
    let y1 = series(&answer["scene"], 0, "observed")
        .and_then(|s| s["points"].as_array())
        .and_then(|p| p.iter().find(|p| p["x"] == 1.0))
        .and_then(|p| p["y"].as_f64())
        .unwrap_or(f64::NAN);
    assert!((y1 - libm::log10(6.3)).abs() < 1e-12);
    // The ticks are whole decades, labelled with their value.
    let labels: Vec<&str> = panel["y"]["ticks"]
        .as_array()
        .map(|t| t.iter().filter_map(|t| t["label"].as_str()).collect())
        .unwrap_or_default();
    assert!(labels.contains(&"1"), "{labels:?}");
    // The zeros (0, 12 and 24 h) have no logarithm: they are listed, with the sentence.
    assert_eq!(panel["hidden"]["count"], 3);
    assert!(
        panel["hidden"]["note"]
            .as_str()
            .is_some_and(|n| n.contains("3 point(s) at zero or below"))
    );
    assert_eq!(
        times(series(&answer["scene"], 0, "lambda_z_used")),
        vec![2.0, 4.0, 6.0, 8.0]
    );
    assert_eq!(check("plot.scene", &answer), Ok(()));
}

#[test]
fn replaced_and_excluded_points_have_their_own_roles() {
    // The trailing zeros (12 and 24 h) are replaced by 0.05 and kept out of the fit.
    let (mut e, id) = nca(
        "3",
        json!({
            "blq": { "position": { "first": "keep", "middle": "drop", "last": { "set": 0.05 } } },
            "lambda_z_selection": { "exclude_replaced": true }
        }),
    );
    let scene = ok(&mut e, "plot.scene", json!({ "analysis": id }))["scene"].clone();
    assert_eq!(times(series(&scene, 0, "replaced")), vec![12.0, 24.0]);
    assert_eq!(
        times(series(&scene, 0, "lambda_z_used")),
        vec![2.0, 4.0, 6.0, 8.0]
    );

    // A sample the user leaves out stays in the picture, and is not counted as used.
    let (mut e, id) = nca("3", json!({ "lambda_z_selection": { "exclude": [4.0] } }));
    let range = chosen(&mut e, id);
    let scene = ok(&mut e, "plot.scene", json!({ "analysis": id }))["scene"].clone();
    let used = times(series(&scene, 0, "lambda_z_used"));
    let left_out = times(series(&scene, 0, "excluded"));
    assert!(!used.contains(&4.0), "{used:?}");
    assert_eq!(
        used.len() as u64,
        range["n_points"].as_u64().unwrap_or(0),
        "used {used:?}, left out {left_out:?}"
    );
}

#[test]
fn a_blq_sample_that_the_policy_dropped_is_a_marker_below_the_axis() {
    // Subject 4 has a zero in the middle of the profile; the default policy drops it.
    let (mut e, id) = nca("4", json!({}));
    let scene = ok(&mut e, "plot.scene", json!({ "analysis": id }))["scene"].clone();
    let marker = series(&scene, 0, "observed")
        .and_then(|s| s["points"].as_array())
        .and_then(|p| p.iter().find(|p| p["blq"] == true))
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(marker["x"], 6.0);
    assert_eq!(
        scene["legend"]["below_limit"],
        "below the limit of quantification"
    );
}

#[test]
fn export_plot_draws_the_same_scene_for_an_nca_analysis() {
    let (mut e, id) = nca("3", json!({}));
    let one = ok(&mut e, "export.plot", json!({ "analysis": id }));
    assert_eq!(one["subject"], "3");
    assert_eq!(one["panels"], 1);
    assert_eq!(one["status"], "fresh");
    let name = one["file_name"].as_str().unwrap_or("");
    assert!(name.ends_with("-subject-3.svg"), "{name}");
    let svg = one["svg"].as_str().unwrap_or("");
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Used for λz") && svg.contains("λz fit"));
    // The semi-log export is the same scene on the other axis, named so.
    let log = ok(
        &mut e,
        "export.plot",
        json!({ "analysis": id, "log": true }),
    );
    assert!(
        log["file_name"]
            .as_str()
            .is_some_and(|n| n.ends_with("-semi-log.svg"))
    );
    assert_ne!(log["svg"], one["svg"]);
    assert_eq!(check("export.plot", &log), Ok(()));
}

#[test]
fn scene_requests_say_what_to_fix() {
    // Several subjects: name one.
    let mut e = sheet();
    let all = ok(
        &mut e,
        "nca.run",
        json!({ "worksheet": 1, "route": "extravascular", "options": { "units": units() } }),
    );
    let id = all["id"].clone();
    for command in ["plot.scene", "export.plot"] {
        let e1 = err(&mut e, command, json!({ "analysis": id }));
        assert_eq!(e1.code, "invalid_parameters");
        assert!(e1.message.contains("give `subject`"), "{}", e1.message);
        let e2 = err(&mut e, command, json!({ "analysis": id, "subject": "99" }));
        assert_eq!(e2.code, "unknown_subject");
        assert!(e2.message.contains("1, 2, 3"), "{}", e2.message);
    }
    let named = ok(
        &mut e,
        "plot.scene",
        json!({ "analysis": id, "subject": 3 }),
    );
    assert_eq!(named["subject"], "3");
    // A fit is neither a plot nor an NCA analysis.
    let fit = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "subject": "3", "model": "pk1.oral_1", "dose": 100 }),
    );
    assert_eq!(
        err(&mut e, "plot.scene", json!({ "analysis": fit["id"] })).code,
        "not_a_plot"
    );
    // Unknown analysis, unknown field.
    assert_eq!(
        err(&mut e, "plot.scene", json!({ "analysis": 99 })).code,
        "unknown_analysis"
    );
    assert_eq!(
        err(
            &mut e,
            "plot.scene",
            json!({ "analysis": id, "colour": "red" })
        )
        .code,
        "invalid_parameters"
    );
}

#[test]
fn a_plot_analysis_has_a_scene_and_the_export_draws_it() {
    let mut e = sheet();
    let plot = ok(
        &mut e,
        "plot.xy",
        json!({ "worksheet": 1, "scale": "both" }),
    );
    let id = plot["id"].clone();
    let answer = ok(&mut e, "plot.scene", json!({ "analysis": id }));
    assert_eq!(answer["kind"], "plot");
    assert!(answer.get("subject").is_none());
    let panels = answer["scene"]["panels"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(panels.len(), 2);
    assert_eq!(panels[0]["y"]["log"], false);
    assert_eq!(panels[1]["y"]["log"], true);
    let first = &panels[0]["series"][0];
    assert_eq!(first["role"], "subject");
    assert_eq!(first["slot"], 0);
    // The logarithmic panel says which points it cannot show.
    assert!(panels[1]["hidden"]["count"].as_u64().unwrap_or(0) > 0);
    assert!(
        panels[1]["hidden"]["note"]
            .as_str()
            .is_some_and(|n| n.contains("no logarithm"))
    );
    assert_eq!(check("plot.scene", &answer), Ok(()));
    // `log` keeps the panels of one axis.
    let only_log = ok(&mut e, "plot.scene", json!({ "analysis": id, "log": true }));
    assert_eq!(
        only_log["scene"]["panels"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(only_log["scene"]["panels"][0]["y"]["log"], true);
    let svg = ok(&mut e, "export.plot", json!({ "analysis": id }));
    assert_eq!(svg["panels"], 2);
    // A linear-only plot has no logarithmic panel to keep.
    let lin = ok(
        &mut e,
        "plot.xy",
        json!({ "worksheet": 1, "scale": "linear" }),
    );
    let e1 = err(
        &mut e,
        "plot.scene",
        json!({ "analysis": lin["id"], "log": true }),
    );
    assert_eq!(e1.code, "no_such_panel");
    // A plot has no subject to name.
    assert_eq!(
        err(
            &mut e,
            "plot.scene",
            json!({ "analysis": id, "subject": "3" })
        )
        .code,
        "invalid_parameters"
    );
}

#[test]
fn the_scene_command_is_in_the_history() {
    let (mut e, id) = nca("3", json!({}));
    ok(&mut e, "plot.scene", json!({ "analysis": id }));
    let history = ok(&mut e, "history.list", json!({}));
    assert!(history.to_string().contains("plot.scene"));
}
