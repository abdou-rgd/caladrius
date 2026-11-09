//! Golden files of `export.plot` (T-133): the SVG of a set of plots, captured before `svg.rs` was
//! rewritten on top of the plot scene, compared byte for byte. A change to the drawing that is on
//! purpose changes these files in the same commit, and the commit says why. Data invented for the
//! tests.
//!
//! To write the files again, run
//! `cargo test -p caladrius-engine print_golden_plots -- --ignored --nocapture` and split the
//! output at the `=== name` lines.

use serde_json::{Value, json};

use crate::*;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/export_plot");

fn ok(engine: &mut Engine, id: &str, params: Value) -> Value {
    match engine.execute(id, params) {
        Ok(v) => v,
        Err(e) => panic!("{id} failed: {e}"),
    }
}

/// Two subjects, P with a point below the limit and a missing value, Q out of time order.
const CSV: &str = "id,time (h),conc (mg/L),dose (mg)\n\
P,0,BLQ,50\nP,1,4.2,50\nP,2,3.1,50\nP,4,,50\nP,6,1.2,50\n\
Q,2,2.8,50\nQ,0,0,50\nQ,0.5,2.6,50\nQ,1,3.9,50\nQ,4,1.7,50\nQ,6,0.9,50\nQ,8,0.5,50\n";

/// Two points below the limit in a row, then a decay: the markers sit under the lowest decade.
const BLQ: &str = "id,time,conc\n\
A,0,BLQ\nA,0.5,BLQ\nA,1,12.5\nA,2,8.1\nA,4,3.3\nA,8,0.42\nA,12,0.09\n\
B,0,BLQ\nB,1,9.7\nB,2,7.2\nB,4,2.9\nB,8,BLQ\nB,12,BLQ\n";

/// Every value is zero: a logarithmic axis has nothing to show.
const ZEROS: &str = "time,conc\n0,0\n1,0\n2,0\n";

/// One profile: the `observed` role, no legend.
const SINGLE: &str =
    "time (h),conc (ng/mL)\n0,0\n0.25,31.5\n0.5,52\n1,60.5\n2,38.2\n4,17.9\n8,3.8\n24,0.011\n";

/// Ten subjects: the colours come round, the legend wraps.
fn many() -> String {
    let mut csv = String::from("id,time,conc\n");
    for s in 1..=10 {
        for (t, c) in [(0.0, 0.0), (1.0, 8.0), (2.0, 5.0), (6.0, 2.0)] {
            csv.push_str(&format!("S{s},{t},{}\n", c + f64::from(s) * 0.5));
        }
    }
    csv
}

fn sheet(csv: &str) -> Engine {
    let mut e = Engine::new();
    ok(&mut e, "data.import", json!({ "name": "inv", "csv": csv }));
    e
}

fn svg(e: &mut Engine, plot: Value, export: Value) -> String {
    let id = ok(e, "plot.xy", plot)["id"].clone();
    let mut params = export;
    params["analysis"] = id;
    let out = ok(e, "export.plot", params);
    out["svg"].as_str().unwrap_or_default().to_owned()
}

fn fitted() -> (Engine, u64) {
    let mut e = sheet(CSV);
    let fit = ok(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "subject": "Q", "model": "pk1.oral_1", "dose": 50 }),
    );
    let id = fit["id"].as_u64().unwrap_or_default();
    (e, id)
}

/// The name and the SVG of every case.
fn cases() -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let mut e = sheet(CSV);
    for (name, scale) in [("linear", "linear"), ("log", "log"), ("both", "both")] {
        let s = svg(
            &mut e,
            json!({ "worksheet": 1, "scale": scale, "title": "Invented & <co>" }),
            json!({}),
        );
        out.push((name, s));
    }
    out.push((
        "both_per_subject",
        svg(
            &mut e,
            json!({ "worksheet": 1, "layout": "per_subject" }),
            json!({}),
        ),
    ));
    out.push((
        "custom_size",
        svg(
            &mut e,
            json!({ "worksheet": 1 }),
            json!({ "width": 600, "height": 400 }),
        ),
    ));
    out.push((
        "small_size",
        svg(
            &mut e,
            json!({ "worksheet": 1, "layout": "per_subject" }),
            json!({ "width": 200, "height": 200 }),
        ),
    ));

    let (mut e, fit) = fitted();
    out.push((
        "fit_profile",
        svg(&mut e, json!({ "fit": fit, "scale": "both" }), json!({})),
    ));
    out.push((
        "fit_profile_linear",
        svg(&mut e, json!({ "fit": fit, "scale": "linear" }), json!({})),
    ));
    for (name, view) in [
        ("residuals_zero", "residuals_time"),
        ("weighted_residuals_zero", "weighted_residuals_time"),
    ] {
        out.push((
            name,
            svg(
                &mut e,
                json!({ "fit": fit, "view": view, "scale": "linear" }),
                json!({}),
            ),
        ));
    }
    out.push((
        "observed_predicted_identity",
        svg(
            &mut e,
            json!({ "fit": fit, "view": "observed_predicted", "scale": "linear" }),
            json!({}),
        ),
    ));
    out.push((
        "observed_predicted_identity_both",
        svg(
            &mut e,
            json!({ "fit": fit, "view": "observed_predicted" }),
            json!({}),
        ),
    ));

    let mut e = sheet(BLQ);
    out.push((
        "below_the_limit",
        svg(&mut e, json!({ "worksheet": 1 }), json!({})),
    ));
    out.push((
        "below_the_limit_per_subject",
        svg(
            &mut e,
            json!({ "worksheet": 1, "layout": "per_subject", "subjects": ["B"] }),
            json!({}),
        ),
    ));
    let mut e = sheet(ZEROS);
    out.push((
        "log_nothing_to_show",
        svg(
            &mut e,
            json!({ "worksheet": 1, "scale": "both" }),
            json!({}),
        ),
    ));
    let mut e = sheet(SINGLE);
    out.push((
        "single_profile",
        svg(&mut e, json!({ "worksheet": 1 }), json!({})),
    ));
    let mut e = sheet(&many());
    out.push((
        "ten_subjects_wrapped_legend",
        svg(&mut e, json!({ "worksheet": 1 }), json!({ "width": 300 })),
    ));
    out
}

#[test]
fn export_plot_is_byte_identical_to_its_golden_files() {
    let cases = cases();
    assert!(cases.len() >= 9, "the nine cases of the card at least");
    let mut same = 0;
    let mut differ = Vec::new();
    for (name, svg) in &cases {
        let path = format!("{DIR}/{name}.svg");
        match std::fs::read(&path) {
            Ok(golden) if golden == svg.as_bytes() => same += 1,
            Ok(_) => differ.push(format!("{name}: the bytes differ")),
            Err(e) => differ.push(format!("{name}: cannot read {path}: {e}")),
        }
    }
    assert!(
        differ.is_empty(),
        "{same} of {} identical: {differ:?}",
        cases.len()
    );
}

/// Prints the cases, to write the golden files again (see the top of the file).
#[test]
#[ignore = "prints the golden files; run on purpose"]
fn print_golden_plots() {
    for (name, svg) in cases() {
        println!("=== {name}");
        print!("{svg}");
    }
}
