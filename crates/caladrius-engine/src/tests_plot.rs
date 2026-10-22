//! Tests of `plot.xy` and `export.plot` (T-110): the points are the source's values as they are,
//! a fit is plotted from its stored result, the plot is stale when its source changes, and the
//! SVG is the drawing of the stored panels. Data invented for the tests.

use serde_json::{Value, json};

use crate::schema::validate;
use crate::*;

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

/// Two invented subjects; P has a BLQ row and a missing concentration, Q is written out of
/// time order.
const CSV: &str = "id,time (h),conc (mg/L),dose (mg)\n\
P,0,BLQ,50\nP,1,4.2,50\nP,2,3.1,50\nP,4,,50\nP,6,1.2,50\n\
Q,2,2.8,50\nQ,0,0,50\nQ,0.5,2.6,50\nQ,1,3.9,50\nQ,4,1.7,50\nQ,6,0.9,50\nQ,8,0.5,50\n";

fn engine() -> Engine {
    let mut e = Engine::new();
    ok(&mut e, "data.import", json!({ "name": "inv", "csv": CSV }));
    e
}

fn xy(series: &Value) -> Vec<(f64, f64, bool)> {
    series["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["x"].as_f64().unwrap(),
                p["y"].as_f64().unwrap(),
                p.get("blq").and_then(Value::as_bool).unwrap_or(false),
            )
        })
        .collect()
}

#[test]
fn a_worksheet_plot_holds_the_cells_as_they_are() {
    let mut e = engine();
    let v = ok(&mut e, "plot.xy", json!({ "worksheet": 1 }));
    assert_eq!(v["kind"], "plot");
    assert_eq!(v["label"], "Plot of inv, all subjects");
    assert_eq!(v["status"]["state"], "fresh");
    let r = &v["result"];
    assert_eq!(r["title"], "Plot of inv, all subjects");
    // Default scale: a linear and a semi-log panel, every subject overlaid.
    let panels = r["panels"].as_array().unwrap();
    assert_eq!(panels.len(), 2);
    assert_eq!(panels[0]["title"], "Linear");
    assert_eq!(panels[1]["title"], "Semi-log");
    assert_eq!(panels[0]["y"]["log"], false);
    assert_eq!(panels[1]["y"]["log"], true);
    assert_eq!(panels[0]["x"]["unit"], "h");
    assert_eq!(panels[0]["y"]["unit"], "mg/L");
    let p = &panels[0]["series"][0];
    assert_eq!(p["name"], "P");
    assert_eq!(p["role"], "subject");
    assert_eq!(p["slot"], 0);
    // The BLQ row is flagged at 0, the missing row is left out (and said), values unchanged.
    assert_eq!(
        xy(p),
        vec![
            (0.0, 0.0, true),
            (1.0, 4.2, false),
            (2.0, 3.1, false),
            (6.0, 1.2, false)
        ]
    );
    // Q is drawn in time order.
    let q = &panels[0]["series"][1];
    assert_eq!(q["slot"], 1);
    assert_eq!(
        xy(q),
        vec![
            (0.0, 0.0, false),
            (0.5, 2.6, false),
            (1.0, 3.9, false),
            (2.0, 2.8, false),
            (4.0, 1.7, false),
            (6.0, 0.9, false),
            (8.0, 0.5, false)
        ]
    );
    let notes: Vec<&str> = r["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    assert_eq!(notes.len(), 3, "{notes:?}");
    assert!(notes[0].starts_with("1 row(s) without a time or a concentration"));
    assert!(notes[1].starts_with("1 point(s) below the limit of quantification"));
    assert!(notes[2].starts_with("1 point(s) at zero or below have no logarithm"));
}

#[test]
fn subjects_scale_layout_and_title_are_options() {
    let mut e = engine();
    let v = ok(
        &mut e,
        "plot.xy",
        json!({ "worksheet": 1, "subjects": ["Q"], "scale": "log", "title": "Q alone" }),
    );
    assert_eq!(v["label"], "Plot of inv, subject Q");
    let r = &v["result"];
    assert_eq!(r["title"], "Q alone");
    assert_eq!(r["panels"].as_array().unwrap().len(), 1);
    // Q keeps its slot (its colour) when it is drawn alone.
    assert_eq!(r["panels"][0]["series"][0]["slot"], 1);
    // One panel per subject, linear only; the update keeps the same object.
    let id = v["id"].as_u64().unwrap();
    let v = ok(
        &mut e,
        "plot.xy",
        json!({ "analysis": id, "worksheet": 1, "layout": "per_subject", "scale": "linear" }),
    );
    assert_eq!(v["id"], id);
    let titles: Vec<&str> = v["result"]["panels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["P", "Q"]);
    // Both scales per subject: two panels each, the scale in the title.
    let v = ok(
        &mut e,
        "plot.xy",
        json!({ "analysis": id, "worksheet": 1, "layout": "per_subject", "subjects": ["Q", "P", "Q"] }),
    );
    let titles: Vec<&str> = v["result"]["panels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Q, linear", "Q, semi-log", "P, linear", "P, semi-log"]);
    // A subject given as a number is the same label.
    let mut n = Engine::new();
    ok(
        &mut n,
        "data.import",
        json!({ "name": "n", "csv": "subject,time,conc\n1,0,5\n1,1,3\n2,0,4\n2,1,2\n" }),
    );
    let v = ok(&mut n, "plot.xy", json!({ "worksheet": 1, "subjects": [2] }));
    assert_eq!(v["result"]["panels"][0]["series"][0]["name"], "2");
}

#[test]
fn a_single_profile_takes_the_observed_role() {
    let mut e = Engine::new();
    ok(
        &mut e,
        "data.import",
        json!({ "name": "one", "csv": "time,conc\n0,5\n1,3\n2,2\n" }),
    );
    let v = ok(&mut e, "plot.xy", json!({ "worksheet": 1, "scale": "linear" }));
    let s = &v["result"]["panels"][0]["series"][0];
    assert_eq!(s["role"], "observed");
    assert_eq!(s["name"], "observed");
    assert!(s.get("slot").is_none());
}

#[test]
fn wrong_plot_requests_say_what_to_fix_and_change_nothing() {
    let mut e = engine();
    let before = e.project().clone();
    let cases = [
        (json!({}), "invalid_parameters", "give `worksheet`"),
        (json!({ "worksheet": 1, "fit": 2 }), "invalid_parameters", "not both"),
        (json!({ "worksheet": 1, "view": "profile" }), "invalid_parameters", "a worksheet plot has none"),
        (json!({ "worksheet": 9 }), "unknown_worksheet", "inv"),
        (json!({ "worksheet": 1, "subjects": ["Z"] }), "unknown_subject", "P, Q"),
        (json!({ "worksheet": 1, "subjects": [] }), "no_subjects", "at least one subject"),
        (json!({ "worksheet": 1, "scale": "cubic" }), "invalid_parameters", "cubic"),
        (json!({ "worksheet": 1, "colour": "red" }), "invalid_parameters", "colour"),
        (json!({ "fit": 7 }), "unknown_analysis", "there is no analysis 7"),
    ];
    for (params, code, says) in cases {
        let e2 = err(&mut e, "plot.xy", params.clone());
        assert_eq!(e2.code, code, "{params}: {}", e2.message);
        assert!(e2.message.contains(says), "{params}: {}", e2.message);
    }
    assert_eq!(e.project(), &before);
    // A plot cannot be redrawn as another kind, nor another kind as a plot.
    let nca = ok(&mut e, "nca.run", json!({ "worksheet": 1, "route": "extravascular" }));
    let id = nca["id"].as_u64().unwrap();
    assert_eq!(
        err(&mut e, "plot.xy", json!({ "analysis": id, "worksheet": 1 })).code,
        "analysis_kind_changed"
    );
    assert_eq!(
        err(&mut e, "plot.xy", json!({ "fit": id })).code,
        "wrong_plot_source"
    );
    // No time column: said, with the way out.
    let mut bare = Engine::new();
    ok(
        &mut bare,
        "data.import",
        json!({ "name": "b", "csv": "time,conc\n0,1\n1,2\n" }),
    );
    ok(
        &mut bare,
        "data.set_column",
        json!({ "worksheet": 1, "column": "time", "role": "other" }),
    );
    let m = err(&mut bare, "plot.xy", json!({ "worksheet": 1 }));
    assert_eq!(m.code, "missing_column", "{}", m.message);
}

/// A fit of subject P, run, with its plot of `view`.
fn fit_and_plot(e: &mut Engine, view: &str, scale: &str) -> (u64, Value) {
    let fit = ok(
        e,
        "fit.run",
        json!({ "worksheet": 1, "subject": "Q", "model": "pk1.oral_1", "dose": 50 }),
    );
    let fit_id = fit["id"].as_u64().unwrap();
    let plot = ok(
        e,
        "plot.xy",
        json!({ "fit": fit_id, "view": view, "scale": scale }),
    );
    (fit_id, plot)
}

#[test]
fn a_fit_is_plotted_from_its_stored_result_never_fitted_again() {
    let mut e = engine();
    let (fit_id, plot) = fit_and_plot(&mut e, "observed_predicted", "linear");
    let stored = ok(&mut e, "analysis.get", json!({ "analysis": fit_id }));
    let obs = stored["result"]["outcome"]["ok"]["observations"]
        .as_array()
        .unwrap()
        .clone();
    let panel = &plot["result"]["panels"][0];
    assert_eq!(panel["guide"], "identity");
    assert_eq!(panel["x"]["label"], "Predicted concentration");
    assert_eq!(panel["y"]["unit"], "mg/L");
    // Each point is (predicted, observed) of the stored observation, bit for bit.
    let points = xy(&panel["series"][0]);
    assert_eq!(points.len(), obs.len());
    for (p, o) in points.iter().zip(&obs) {
        assert_eq!(p.0, o["predicted"].as_f64().unwrap());
        assert_eq!(p.1, o["observed"].as_f64().unwrap());
    }
    assert_eq!(
        plot["label"],
        "Observed vs predicted of Fit pk1.oral_1 to inv, subject Q"
    );

    // Residuals against time are the stored residuals; weighted ones too.
    for (view, key) in [
        ("residuals_time", "residual"),
        ("weighted_residuals_time", "weighted_residual"),
    ] {
        let v = ok(&mut e, "plot.xy", json!({ "fit": fit_id, "view": view, "scale": "linear" }));
        let panel = &v["result"]["panels"][0];
        assert_eq!(panel["guide"], "zero");
        for (p, o) in xy(&panel["series"][0]).iter().zip(&obs) {
            assert_eq!(p.0, o["time"].as_f64().unwrap());
            assert_eq!(p.1, o[key].as_f64().unwrap());
        }
        // Residuals can be negative: no log axis, said plainly.
        let m = err(&mut e, "plot.xy", json!({ "fit": fit_id, "view": view }));
        assert_eq!(m.code, "log_scale_not_possible");
        assert!(m.message.contains("use the linear scale"), "{}", m.message);
    }

    // The profile: observed points and the stored smooth curve.
    let v = ok(&mut e, "plot.xy", json!({ "fit": fit_id }));
    let panels = v["result"]["panels"].as_array().unwrap();
    assert_eq!(panels.len(), 2);
    let curve = stored["result"]["outcome"]["ok"]["curve"].as_array().unwrap();
    let predicted = &panels[0]["series"][1];
    assert_eq!(predicted["role"], "predicted");
    assert_eq!(predicted["style"], "line");
    assert_eq!(xy(predicted).len(), curve.len());
    assert_eq!(
        xy(predicted).last().unwrap().1,
        curve.last().unwrap()["conc"].as_f64().unwrap()
    );
}

#[test]
fn a_plot_goes_stale_with_its_source_and_is_redrawn_by_analysis_run() {
    let mut e = engine();
    let sheet = ok(&mut e, "plot.xy", json!({ "worksheet": 1 }));
    let sheet_id = sheet["id"].as_u64().unwrap();
    let (fit_id, plot) = fit_and_plot(&mut e, "profile", "both");
    let plot_id = plot["id"].as_u64().unwrap();
    ok(
        &mut e,
        "data.set_cell",
        json!({ "worksheet": 1, "column": "conc", "row": 1, "value": "4.4" }),
    );
    let s = ok(&mut e, "analysis.get", json!({ "analysis": sheet_id }));
    assert_eq!(s["status"]["reasons"][0]["code"], "input_changed");
    let s = ok(&mut e, "analysis.get", json!({ "analysis": plot_id }));
    assert_eq!(s["status"]["reasons"][0]["code"], "source_stale");
    // Redrawing the worksheet plot reads the new cell.
    let s = ok(&mut e, "analysis.run", json!({ "analysis": sheet_id }));
    assert_eq!(s["status"]["state"], "fresh");
    assert_eq!(s["result"]["panels"][0]["series"][0]["points"][1]["y"], 4.4);
    // Running the fit again: the plot shows an older result until it is redrawn.
    ok(&mut e, "analysis.run", json!({ "analysis": fit_id }));
    let s = ok(&mut e, "analysis.get", json!({ "analysis": plot_id }));
    assert_eq!(s["status"]["reasons"][0]["code"], "source_changed");
    let s = ok(&mut e, "analysis.run", json!({ "analysis": plot_id }));
    assert_eq!(s["status"]["state"], "fresh");
    // A fit shown by a plot is kept until the plot goes.
    let m = err(&mut e, "analysis.remove", json!({ "analysis": fit_id }));
    assert_eq!(m.code, "analysis_in_use");
    ok(&mut e, "analysis.remove", json!({ "analysis": plot_id }));
    ok(&mut e, "analysis.remove", json!({ "analysis": fit_id }));
    // Plots are saved with the project and come back the same.
    let bytes = e.save_bytes().unwrap();
    let mut back = Engine::new();
    back.load_bytes(&bytes).unwrap();
    assert_eq!(back.project(), e.project());
}

#[test]
fn a_failed_fit_or_a_fit_never_run_cannot_be_plotted() {
    let mut e = engine();
    let mut project = e.project().clone();
    let fit = project
        .add_analysis(
            project::AnalysisSpec::Fit(project::FitSpec {
                worksheet: project::WorksheetId(1),
                subject: Some("Q".to_owned()),
                model: caladrius_models::ModelId::Oral1,
                dose: Some(50.0),
                weighting: Default::default(),
                initial: Default::default(),
                options: Default::default(),
            }),
            None,
        )
        .unwrap();
    let mut e2 = Engine::with_project(project);
    let m = err(&mut e2, "plot.xy", json!({ "fit": fit.0 }));
    assert_eq!(m.code, "no_result");
    assert!(m.message.contains("has not been run"), "{}", m.message);
    // Nothing was stored by the failed call.
    assert_eq!(e2.project().analyses().len(), 1);
    let _ = &mut e;
}

#[test]
fn export_plot_gives_the_svg_of_the_stored_panels() {
    let mut e = engine();
    let v = ok(&mut e, "plot.xy", json!({ "worksheet": 1, "title": "Invented & co" }));
    let id = v["id"].as_u64().unwrap();
    let out = ok(&mut e, "export.plot", json!({ "analysis": id }));
    assert_eq!(out["format"], "svg");
    assert_eq!(out["status"], "fresh");
    assert_eq!(out["panels"], 2);
    assert_eq!(out["file_name"], "plot-of-inv-all-subjects.svg");
    let svg = out["svg"].as_str().unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"840\""));
    assert!(svg.contains("<title>Invented &amp; co</title>"));
    assert!(svg.contains("Concentration (mg/L)"));
    // The two subjects in the legend, and the BLQ marker explained.
    assert!(svg.contains(">P</text>") && svg.contains(">Q</text>"));
    assert!(svg.contains("below the limit of quantification"));
    // The same plot exports the same text (nothing depends on a clock or a random state).
    let again = ok(&mut e, "export.plot", json!({ "analysis": id }));
    assert_eq!(again["svg"], out["svg"]);
    // Panel size within bounds; another format, or a non-plot, is refused.
    let big = ok(&mut e, "export.plot", json!({ "analysis": id, "width": 600, "height": 400 }));
    assert!(big["svg"].as_str().unwrap().contains("width=\"1200\""));
    assert_eq!(
        err(&mut e, "export.plot", json!({ "analysis": id, "width": 50 })).code,
        "invalid_parameters"
    );
    assert_eq!(
        err(&mut e, "export.plot", json!({ "analysis": id, "format": "png" })).code,
        "invalid_parameters"
    );
    let nca = ok(&mut e, "nca.run", json!({ "worksheet": 1, "route": "extravascular" }));
    assert_eq!(
        err(&mut e, "export.plot", json!({ "analysis": nca["id"] })).code,
        "not_a_plot"
    );
    // A stale plot exports with its status.
    ok(
        &mut e,
        "data.set_cell",
        json!({ "worksheet": 1, "column": "conc", "row": 2, "value": "3" }),
    );
    let stale = ok(&mut e, "export.plot", json!({ "analysis": id }));
    assert_eq!(stale["status"], "stale");
    // The points are also a table.
    let t = ok(&mut e, "export.table", json!({ "table": "plot.points", "analysis": id }));
    assert_eq!(t["columns"], json!(["panel", "series", "x", "y", "blq"]));
    assert_eq!(t["rows"][0], json!(["Linear", "P", 0.0, 0.0, "true"]));
    assert_eq!(t["rows"].as_array().unwrap().len(), 2 * (4 + 7));
}

#[test]
fn plot_answers_match_their_schemas() {
    let mut e = engine();
    let (fit_id, _) = fit_and_plot(&mut e, "residuals_time", "linear");
    let schema = describe()
        .into_iter()
        .find(|c| c.id == "plot.xy")
        .unwrap()
        .result_schema;
    for params in [
        json!({ "worksheet": 1 }),
        json!({ "worksheet": 1, "layout": "per_subject", "scale": "log" }),
        json!({ "fit": fit_id, "view": "observed_predicted" }),
        json!({ "fit": fit_id, "view": "weighted_residuals_time", "scale": "linear" }),
    ] {
        let v = ok(&mut e, "plot.xy", params.clone());
        if let Err(why) = validate::check(&schema, &schema, &v) {
            panic!("{params}: {why}\n{v}");
        }
    }
}
