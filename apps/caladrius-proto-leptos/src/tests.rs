//! The tests of the screen's logic against the real engine (`caladrius_bridge::Bridge`, the one the page
//! links): the map of the semi-log plot with hidden points, a drag of the range, an edit that makes the
//! analysis stale, and every result cell formatted by `caladrius-format`.
// `powf` is only here to check the engine's log10 against the sheet, in a test (the page never calls it).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods
)]

use caladrius_bridge::Bridge;
use caladrius_format as fmt;
use serde_json::{Value, json};

use crate::app::{EDGE_CSV, NCA50_CSV};
use crate::measure::expected_cells;
use crate::plotmap::{Frame, plot_rect, px_of_x, py_of_y, selectable, x_of_px, y_of_py};
use crate::results::{adj_text, all_cells, count, headline_cells, number};
use crate::session::{CellRole, Engine, Session, State, Status, stale_sentence};

/// The bridge, writing down every call it is given.
struct Recording {
    inner: Bridge,
    calls: Vec<(String, Value)>,
}

impl Engine for Recording {
    fn call(&mut self, command: &str, params: Value) -> crate::session::Answer {
        self.calls.push((command.to_owned(), params.clone()));
        self.inner.call(command, params)
    }
}

fn recording(csv: &str, name: &str, subject: Option<&str>) -> Session<Recording> {
    let mut s = Session::new(Recording {
        inner: Bridge::new(),
        calls: Vec::new(),
    });
    s.load_dataset(name, csv, subject);
    s
}

fn frame() -> Frame {
    Frame {
        width: 800.0,
        height: 400.0,
        left: 64.0,
        right: 18.0,
        top: 16.0,
        bottom: 44.0,
    }
}

#[test]
fn the_semi_log_map_places_every_sample_and_leaves_the_hidden_ones_out() {
    let s = recording(EDGE_CSV, "edge_blq", None);
    let f = frame();
    let r = plot_rect(&f);
    let mut subjects_with_hidden = 0;
    for subject in s.state().info.as_ref().unwrap().subjects.clone() {
        let mut one = recording(EDGE_CSV, "edge_blq", Some(&subject));
        one.set_log(true);
        let st = one.state();
        let Some(panel) = st.scene.as_ref().and_then(|sc| sc.panels.first()) else {
            continue;
        };
        assert!(
            panel.y.log,
            "the engine draws the concentration axis in log10"
        );
        // The ends of the axes land on the ends of the drawing area, and the map is its own inverse.
        assert!((px_of_x(&f, panel, panel.x.min) - r.x0).abs() < 1e-9);
        assert!((px_of_x(&f, panel, panel.x.max) - r.x1).abs() < 1e-9);
        assert!((py_of_y(&f, panel, panel.y.min) - r.y1).abs() < 1e-9);
        assert!((py_of_y(&f, panel, panel.y.max) - r.y0).abs() < 1e-9);
        let samples = selectable(panel);
        assert!(!samples.is_empty());
        for p in &samples {
            let (px, py) = (px_of_x(&f, panel, p.x), py_of_y(&f, panel, p.y));
            assert!(
                px >= r.x0 - 1e-9 && px <= r.x1 + 1e-9 && py >= r.y0 - 1e-9 && py <= r.y1 + 1e-9,
                "subject {subject}: ({px}, {py}) is outside the plot"
            );
            assert!((x_of_px(&f, panel, px) - p.x).abs() < 1e-9);
            assert!((y_of_py(&f, panel, py) - p.y).abs() < 1e-9);
            // The y of the scene is log10 of the concentration of the sheet: nothing is taken again here.
            let row = st
                .rows
                .iter()
                .find(|row| row.subject == subject && row.time == p.x)
                .unwrap();
            let conc = row.conc.unwrap();
            assert!(
                (10f64.powf(p.y) - conc).abs() <= 1e-9 * conc.abs().max(1.0),
                "subject {subject} t={}: 10^{} is not {conc}",
                p.x,
                p.y
            );
        }
        if let Some(hidden) = panel.hidden.as_ref().filter(|h| h.count > 0) {
            subjects_with_hidden += 1;
            assert_eq!(
                hidden.points.len(),
                hidden.count,
                "the scene lists every point it leaves out"
            );
            for h in &hidden.points {
                // A hidden sample is not a place a click can land on, and it is a zero of the sheet.
                assert!(
                    !samples.iter().any(|p| p.x == h.x),
                    "subject {subject}: t={} is both hidden and drawn",
                    h.x
                );
                let row = st
                    .rows
                    .iter()
                    .find(|row| row.subject == subject && row.time == h.x)
                    .unwrap();
                assert!(
                    row.conc.is_none_or(|c| c <= 0.0),
                    "subject {subject}: a hidden sample must have no positive concentration"
                );
            }
        }
    }
    assert!(
        subjects_with_hidden > 0,
        "edge_blq is the dataset that has points a log axis cannot show"
    );
}

#[test]
fn a_drag_of_the_start_handle_calls_nca_run_with_the_expected_parameters() {
    let mut s = recording(EDGE_CSV, "edge_blq", Some("3"));
    let before = s.state().clone();
    let (start, end) = before.range().unwrap();
    let analysis = before.analysis.unwrap();
    let worksheet = before.info.as_ref().unwrap().id;
    let target = before
        .samples()
        .into_iter()
        .find(|t| *t < start)
        .expect("a sample before the start of the range");
    s.engine_mut_for_test().calls.clear();
    s.set_range_start(target);
    let calls = s.engine().calls.clone();
    let commands: Vec<&str> = calls.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(
        commands,
        ["nca.run", "plot.scene"],
        "a drag runs the analysis and asks for its scene, nothing else"
    );
    let params = &calls[0].1;
    assert_eq!(params["worksheet"], json!(worksheet));
    assert_eq!(params["subject"], json!("3"));
    assert_eq!(params["route"], json!("extravascular"));
    assert_eq!(params["dose"], Value::Null);
    assert_eq!(
        params["analysis"],
        json!(analysis),
        "the same analysis is run again, not a new one"
    );
    assert_eq!(
        params["options"]["lambda_z_selection"]["manual"],
        json!({ "range": { "start": target, "end": end } })
    );
    assert_eq!(
        calls[1].1,
        json!({ "analysis": analysis, "subject": "3", "log": true })
    );
    let after = s.state();
    assert_eq!(
        after.range().map(|r| r.0),
        Some(target),
        "the range on the plot follows the answer"
    );
    assert!(after.is_manual());
    assert_eq!(after.status(), Status::Fresh);
}

#[test]
fn a_range_with_fewer_than_two_samples_is_refused_without_calling_the_engine() {
    let mut s = recording(EDGE_CSV, "edge_blq", Some("3"));
    let (_, end) = s.state().range().unwrap();
    s.engine_mut_for_test().calls.clear();
    s.set_range(end, end);
    assert!(s.engine().calls.is_empty());
    assert!(
        s.state()
            .error
            .as_deref()
            .is_some_and(|e| e.contains("at least 2 points"))
    );
}

#[test]
fn an_edit_makes_the_analysis_stale_until_it_runs_again() {
    let mut s = recording(EDGE_CSV, "edge_blq", Some("3"));
    assert_eq!(s.state().status(), Status::Fresh);
    let row = s
        .state()
        .rows
        .iter()
        .find(|r| r.subject == "3" && r.conc.is_some_and(|c| c > 0.0))
        .unwrap()
        .clone();
    s.edit_cell(row.index, CellRole::Concentration, "99.5");
    let st = s.state();
    assert_eq!(st.status(), Status::Stale);
    assert!(!st.stale_reasons.is_empty());
    assert!(stale_sentence(&st.stale_reasons).starts_with("Out of date"));
    assert_eq!(
        st.rows.iter().find(|r| r.index == row.index).unwrap().conc,
        Some(99.5)
    );
    s.run();
    assert_eq!(s.state().status(), Status::Fresh);
}

#[test]
fn a_value_the_engine_does_not_understand_is_refused_in_place() {
    let mut s = recording(EDGE_CSV, "edge_blq", Some("3"));
    let row = s
        .state()
        .rows
        .iter()
        .find(|r| r.subject == "3")
        .unwrap()
        .clone();
    s.edit_cell(row.index, CellRole::Concentration, "not a number");
    assert!(s.state().error.is_some());
    assert_eq!(s.state().status(), Status::Fresh);
}

/// What the formatter says for the raw value of a parameter.
fn written(name: &str, v: f64) -> String {
    if name.contains("r.squared") {
        fmt::fit_quality(v)
    } else {
        fmt::number(v)
    }
}

fn check_cells(state: &State) -> usize {
    let (Some(subject), Some(ok)) = (state.subject_result(), state.ok()) else {
        return 0;
    };
    let units = state.units();
    let mut checked = 0;
    for cells in [
        headline_cells(subject, ok, &units, true),
        all_cells(subject, ok, &units, true),
    ] {
        for c in cells {
            let p = ok.parameters.iter().find(|p| p.name == c.key).unwrap();
            match p.value.value {
                Some(v) => assert_eq!(c.text, written(&c.key, v), "{}", c.key),
                None => assert_eq!(c.text, "not calculated", "{}", c.key),
            }
            checked += 1;
        }
    }
    for (i, c) in ok.lambda_z_candidates.iter().enumerate() {
        let expected = expected_cells(state);
        assert_eq!(expected[&format!("c{i}:n")], count(c.n_points));
        assert_eq!(expected[&format!("c{i}:from")], number(c.time_first));
        assert_eq!(expected[&format!("c{i}:lambda_z")], fmt::number(c.lambda_z));
        assert_eq!(expected[&format!("c{i}:adj")], adj_text(c));
        checked += 4;
    }
    checked
}

#[test]
fn every_result_cell_is_what_caladrius_format_writes_for_the_value_of_the_answer() {
    let mut total = 0;
    for (csv, name, subjects) in [
        (EDGE_CSV, "edge_blq", vec!["1", "2", "3", "4", "5"]),
        (NCA50_CSV, "nca50", vec!["1", "2", "17", "50"]),
    ] {
        for subject in subjects {
            let s = recording(csv, name, Some(subject));
            if s.state().subject_result().is_some() {
                total += check_cells(s.state());
            }
        }
    }
    assert!(
        total > 150,
        "the cells of the answers were checked, not an empty list ({total})"
    );
}

#[test]
fn the_screen_never_formats_a_number_by_itself() {
    // Every number on screen goes through `caladrius-format`: no precision, no exponent, no rounding in the
    // files that write text for the person. (`plotmap::coord` rounds pixel positions for SVG attributes.)
    let files = [
        ("app.rs", include_str!("app.rs")),
        ("data_panel.rs", include_str!("data_panel.rs")),
        ("plot_panel.rs", include_str!("plot_panel.rs")),
        ("results_panel.rs", include_str!("results_panel.rs")),
        ("results.rs", include_str!("results.rs")),
        ("roles.rs", include_str!("roles.rs")),
    ];
    for (name, text) in files {
        for banned in [
            "{:.",
            "{:e",
            "{:E",
            "to_fixed",
            ".round()",
            ".trunc()",
            "toFixed",
            "toPrecision",
        ] {
            assert!(
                !text.contains(banned),
                "{name} contains {banned:?}: a number is written there without caladrius-format"
            );
        }
    }
}
