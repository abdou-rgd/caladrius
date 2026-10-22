//! Plot objects in the interface (T-110): "New plot" makes a plot of the project, its options
//! redraw it through `plot.xy`, it sits in the tree and opens again, a fit gives its diagnostic
//! plots, the SVG export is a request to the program around, and the drawing follows the theme.
//! Data invented for the tests; every point comes from an engine answer.

use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::app::{Action, Request, Selection, UiApp};
use crate::plotpage::{self, PlotPage, Source};
use crate::tests::harness;
use crate::theme::{ThemeMode, Tokens};

const CSV: &[u8] =
    b"subject,time (h),conc (mg/L),dose (mg)\nP,0,BLQ,50\nP,0.5,2.6,50\nP,1,4.2,50\nP,2,3.1,50\nP,4,1.9,50\nP,6,1.2,50\nP,8,0.7,50\n\
Q,0,0,50\nQ,0.5,2.2,50\nQ,1,3.9,50\nQ,2,2.8,50\nQ,4,1.7,50\nQ,6,0.9,50\nQ,8,0.5,50\n";

fn with_sheet() -> UiApp {
    let mut app = UiApp::new();
    app.load_csv("inv.csv", CSV);
    app.perform(vec![Action::ImportConfirm]);
    app
}

fn kinds(app: &UiApp) -> Vec<String> {
    app.overview
        .analyses
        .iter()
        .map(|a| a.kind.clone())
        .collect()
}

#[test]
fn new_plot_makes_a_plot_object_and_shows_its_page() {
    let mut app = with_sheet();
    app.perform(vec![Action::NewPlot]);
    let page = app.plot_page().unwrap();
    let id = page.analysis.unwrap();
    assert_eq!(page.source, Source::Worksheet { worksheet: 1 });
    assert_eq!(page.label, "Plot of inv, all subjects");
    assert_eq!(app.state.selection, Selection::Analysis(id));
    assert_eq!(kinds(&app), vec!["plot"]);
    let run = page.result.as_ref().unwrap();
    assert_eq!(run.panels.len(), 2);
    assert_eq!(run.panels[0].series.len(), 2);
    // The tree lists it and the page is drawn, with its options and both panels.
    let mut h = harness(app);
    h.run_steps(3);
    h.get_all_by_label("Plot of inv, all subjects")
        .next()
        .unwrap();
    h.get_by_label("Options");
    // The scale choice and the panel titles both read Linear and Semi-log.
    assert!(h.get_all_by_label("Linear").count() >= 2);
    assert!(h.get_all_by_label("Semi-log").count() >= 2);
    h.get_by_label("Export SVG…");
}

#[test]
fn options_redraw_the_same_object_through_the_engine() {
    let mut app = with_sheet();
    app.perform(vec![Action::NewPlot]);
    let id = app.plot_page().unwrap().analysis.unwrap();
    // Q alone, one panel per subject, semi-log only, a title.
    if let Some(page) = app.state.plot.as_mut() {
        page.all_subjects = false;
        page.subjects = vec!["Q".to_owned()];
        page.layout = "per_subject".to_owned();
        page.scale = "log".to_owned();
        page.title = "Q, semi-log".to_owned();
    }
    app.perform(vec![Action::PlotChanged]);
    let page = app.plot_page().unwrap();
    assert_eq!(page.analysis, Some(id));
    assert_eq!(kinds(&app), vec!["plot"]);
    let run = page.result.as_ref().unwrap();
    assert_eq!(run.title, "Q, semi-log");
    assert_eq!(run.panels.len(), 1);
    assert!(run.panels[0].y.log);
    assert_eq!(run.panels[0].series[0].name, "Q");
    assert_eq!(page.drawn_title, "Q, semi-log");
    // The engine stored exactly these options.
    let view = app
        .engine_mut()
        .execute("analysis.get", json!({ "analysis": id }))
        .unwrap();
    assert_eq!(view["spec"]["subjects"], json!(["Q"]));
    assert_eq!(view["spec"]["layout"], "per_subject");
    // A choice the engine refuses keeps the page and says why.
    if let Some(page) = app.state.plot.as_mut() {
        page.subjects.clear();
    }
    app.perform(vec![Action::PlotChanged]);
    let page = app.plot_page().unwrap();
    assert!(
        page.error
            .as_deref()
            .unwrap()
            .contains("at least one subject")
    );
    assert_eq!(page.analysis, Some(id));
}

#[test]
fn a_plot_opens_again_from_the_tree_and_goes_stale_with_its_data() {
    let mut app = with_sheet();
    app.perform(vec![Action::NewPlot]);
    let id = app.plot_page().unwrap().analysis.unwrap();
    app.perform(vec![
        Action::Select(Selection::Worksheet(1)),
        Action::SetCell {
            worksheet: 1,
            row: 2,
            column: "conc".to_owned(),
            text: "4.5".to_owned(),
        },
        Action::Select(Selection::Analysis(id)),
    ]);
    let page = app.plot_page().unwrap();
    assert!(page.status.is_stale());
    assert!(page.status.sentence().unwrap().contains("the data changed"));
    // "Run again" redraws it with the new cell.
    app.perform(vec![Action::RunAgain(id)]);
    let page = app.plot_page().unwrap();
    assert!(!page.status.is_stale());
    let p = &page.result.as_ref().unwrap().panels[0].series[0];
    assert_eq!(p.points[2].y, 4.5);
    // The page state round-trips as data (golden rule 7); the drawing is not part of it.
    let text = serde_json::to_string(page).unwrap();
    let back: PlotPage = serde_json::from_str(&text).unwrap();
    assert_eq!(back.source, page.source);
    assert!(back.result.is_none());
}

#[test]
fn a_fit_offers_its_diagnostic_plots_as_objects() {
    let mut app = with_sheet();
    let fit = app
        .engine_mut()
        .execute(
            "fit.run",
            json!({ "worksheet": 1, "subject": "Q", "model": "pk1.oral_1" }),
        )
        .unwrap();
    let fit_id = fit["id"].as_u64().unwrap();
    app.refresh_overview();
    app.perform(vec![Action::Select(Selection::Analysis(fit_id))]);
    let mut h = harness(app);
    h.run_steps(3);
    h.get_by_label("Keep a plot of this fit:");
    // Below the fold of the page: clicked through accessibility, as a screen reader would.
    h.get_by_label("Observed vs predicted").click_accesskit();
    h.run_steps(2);
    let app = h.state_mut();
    let page = app.plot_page().unwrap();
    assert_eq!(
        page.source,
        Source::Fit {
            fit: fit_id,
            view: "observed_predicted".to_owned()
        }
    );
    assert_eq!(page.result.as_ref().unwrap().panels.len(), 2);
    // Residuals are linear only.
    app.perform(vec![Action::NewFitPlot {
        fit: fit_id,
        view: "residuals_time".to_owned(),
    }]);
    let page = app.plot_page().unwrap();
    assert_eq!(page.scale, "linear");
    assert!(page.error.is_none());
    assert_eq!(kinds(app), vec!["fit", "plot", "plot"]);
}

#[test]
fn export_asks_the_program_to_save_an_svg() {
    let mut app = with_sheet();
    app.perform(vec![Action::NewPlot]);
    let id = app.plot_page().unwrap().analysis.unwrap();
    app.take_requests();
    app.perform(vec![Action::ExportPlot(id)]);
    let requests = app.take_requests();
    let [Request::SaveFile { suggested, bytes }] = requests.as_slice() else {
        panic!("{requests:?}");
    };
    assert_eq!(suggested, "plot-of-inv-all-subjects.svg");
    assert!(bytes.starts_with(b"<svg "));
    app.file_saved("plot.svg");
    assert_eq!(app.notice_text(), Some("Saved plot.svg."));
}

#[test]
fn the_raw_data_card_and_the_palette_lead_to_a_plot() {
    let app = with_sheet();
    let mut h = harness(app);
    h.run_steps(3);
    h.get_by_label("Keep as a plot").click();
    h.run_steps(2);
    let app = h.state();
    assert_eq!(kinds(app), vec!["plot"]);
    let entries = app.palette_entries();
    assert!(entries.iter().any(|e| e.action == Action::NewPlot));
    let export = entries.iter().find(|e| e.id == "export.plot").unwrap();
    let id = app.plot_page().unwrap().analysis.unwrap();
    assert_eq!(export.action, Action::ExportPlot(id));
}

#[test]
fn the_export_colours_are_the_light_theme() {
    let t = Tokens::embedded(ThemeMode::Light).unwrap();
    let hex = |c: egui::Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
    let svg = caladrius_engine::SVG_COLORS;
    assert_eq!(hex(t.colors.plot_background.color()), svg.background);
    assert_eq!(hex(t.colors.text.color()), svg.text);
    assert_eq!(hex(t.colors.text_muted.color()), svg.text_muted);
    assert_eq!(hex(t.colors.plot_grid.color()), svg.grid);
    assert_eq!(hex(t.colors.series_observed.color()), svg.observed);
    assert_eq!(hex(t.colors.series_fit.color()), svg.predicted);
    assert_eq!(hex(t.colors.series_other.color()), svg.other);
    let subjects: Vec<String> = t
        .colors
        .series_subjects
        .iter()
        .map(|c| hex(c.color()))
        .collect();
    assert_eq!(subjects, svg.subjects.to_vec());
}

#[test]
fn panel_axes_follow_the_guide_and_the_scale() {
    use caladrius_engine::project::{
        PlotAxis, PlotGuide, PlotPanel, PlotPoint, PlotSeries, SeriesRole, SeriesStyle,
    };
    let axis = |log| PlotAxis {
        label: "v".to_owned(),
        unit: None,
        log,
    };
    let panel = |points: Vec<(f64, f64)>, guide, x_log, y_log| PlotPanel {
        title: String::new(),
        x: axis(x_log),
        y: axis(y_log),
        series: vec![PlotSeries {
            name: "s".to_owned(),
            role: SeriesRole::Observed,
            style: SeriesStyle::Points,
            points: points
                .into_iter()
                .map(|(x, y)| PlotPoint { x, y, blq: false })
                .collect(),
        }],
        guide,
    };
    // Residuals: centred on zero.
    let (a, _) = plotpage::panel_axes(&panel(
        vec![(1.0, -2.0), (2.0, 1.0)],
        Some(PlotGuide::Zero),
        false,
        false,
    ))
    .unwrap();
    assert!((a.y.0 + a.y.1).abs() < 1e-12 && a.y.1 > 2.0);
    // Observed vs predicted: the same range on both axes.
    let (a, _) = plotpage::panel_axes(&panel(
        vec![(1.0, 3.0), (4.0, 2.0)],
        Some(PlotGuide::Identity),
        false,
        false,
    ))
    .unwrap();
    assert_eq!(a.x, a.y);
    let (a, _) = plotpage::panel_axes(&panel(
        vec![(1.0, 3.0), (40.0, 2.0)],
        Some(PlotGuide::Identity),
        true,
        true,
    ))
    .unwrap();
    assert!(a.x_log && a.log);
    // A log axis with nothing positive has nothing to show.
    assert!(plotpage::panel_axes(&panel(vec![(1.0, 0.0)], None, false, true)).is_none());
    // Concentrations: from zero.
    let (a, blq) =
        plotpage::panel_axes(&panel(vec![(0.0, 2.0), (4.0, 5.0)], None, false, false)).unwrap();
    assert!(a.y.0 <= 0.0 && blq == 0.0);
}

#[test]
fn the_plot_page_draws_in_both_themes_and_narrow() {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        let mut app = with_sheet();
        app.state.mode = mode;
        app.perform(vec![Action::NewPlot]);
        let mut h = egui_kittest::Harness::builder()
            // A narrow window (test size, not a layout value): panels stack.
            .with_size(egui::Vec2::new(700.0, 900.0))
            .build_state(|ctx, app: &mut UiApp| app.ui(ctx), app);
        h.run_steps(3);
        h.get_by_label("below the limit of quantification");
    }
}
