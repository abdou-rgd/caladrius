//! The screen driven without a window by `egui_kittest`: pointer drags, clicks on points, edits and
//! keys, each checked on the state the engine's answer leaves.

use caladrius_proto_egui::app::App;
use caladrius_proto_egui::model::Status;
use caladrius_proto_egui::plot;
use egui::{Key, Modifiers, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn harness() -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .build_ui_state(|ui, app: &mut App| app.ui(ui), App::new(false));
    h.run();
    h
}

fn range(h: &Harness<'_, App>) -> (f64, f64) {
    h.state().screen.range().unwrap_or((f64::NAN, f64::NAN))
}

fn lambda_z(h: &Harness<'_, App>) -> String {
    h.state()
        .screen
        .headline()
        .iter()
        .find(|c| c.name == "lambda.z")
        .map(|c| c.text.clone())
        .unwrap_or_default()
}

fn center(h: &Harness<'_, App>, label: &str) -> Pos2 {
    h.get_by_label_contains(label).rect().center()
}

#[test]
fn it_opens_on_edge_blq_subject_3_fresh() {
    let h = harness();
    let s = &h.state().screen;
    assert_eq!(s.dataset, "edge_blq");
    assert_eq!(s.subject, "3");
    assert_eq!(s.status, Status::Fresh);
    assert_eq!(range(&h), (2.0, 8.0));
    assert!(s.seal.is_some());
    h.get_by_label("Analysis is fresh");
}

#[test]
fn a_drag_of_a_handle_runs_nca_and_changes_lambda_z() {
    let mut h = harness();
    let runs = h.state().screen.runs;
    let before = lambda_z(&h);
    let start = center(&h, "λz range start");
    let end = center(&h, "λz range end");
    // Time 4 on the (linear) time axis, between the two handles at times 2 and 8.
    let target = Pos2::new(
        start.x + (end.x - start.x) * (4.0 - 2.0) / (8.0 - 2.0),
        start.y,
    );
    h.hover_at(start);
    h.step();
    h.drag_at(start);
    h.step();
    h.hover_at(Pos2::new((start.x + target.x) / 2.0, start.y));
    h.step();
    h.hover_at(target);
    h.step();
    h.drop_at(target);
    h.run();
    assert!(
        h.state().screen.runs > runs,
        "the drag did not call nca.run"
    );
    assert_eq!(range(&h), (4.0, 8.0));
    assert_ne!(lambda_z(&h), before);
    assert!(h.state().screen.manual());
    assert_eq!(h.state().screen.status, Status::Fresh);
}

#[test]
fn a_click_on_a_point_leaves_it_out_then_puts_it_back() {
    let mut h = harness();
    let outer = h.get_by_label_contains("Concentration against time").rect();
    let point = |h: &Harness<'_, App>| -> Pos2 {
        let scene = h.state().screen.scene.clone();
        let panel = scene.as_ref().and_then(|s| s.panels.first());
        let p = panel.and_then(|p| {
            p.series
                .iter()
                .flat_map(|s| s.points.iter())
                .find(|pt| pt.x == 6.0 && !pt.blq)
                .map(|pt| plot::pixel(outer, p, pt.x, pt.y))
        });
        p.unwrap_or_default()
    };
    assert!(h.state().screen.used_times().contains(&6.0));
    let at = point(&h);
    h.hover_at(at);
    h.step();
    h.drag_at(at);
    h.step();
    h.drop_at(at);
    h.run();
    assert!(!h.state().screen.used_times().contains(&6.0));
    assert!(h.state().screen.manual());
    let at = point(&h);
    h.hover_at(at);
    h.step();
    h.drag_at(at);
    h.step();
    h.drop_at(at);
    h.run();
    assert!(h.state().screen.used_times().contains(&6.0));
}

#[test]
fn an_edit_marks_the_analysis_stale_and_run_makes_it_fresh() {
    let mut h = harness();
    let row = h.state().screen.table[1].sheet_row;
    let name = format!("Concentration at row {}", row + 1);
    h.get_by_label(&name).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.get_by_label(&name).type_text("2.5");
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().screen.status, Status::Stale);
    assert_eq!(h.state().screen.table[1].conc, 2.5);
    h.get_by_label("Analysis is stale");
    h.get_by_label("Run").click();
    h.run();
    assert_eq!(h.state().screen.status, Status::Fresh);
}

#[test]
fn an_engine_error_is_shown_as_its_sentence_in_place() {
    let mut h = harness();
    let row = h.state().screen.table[1].sheet_row;
    h.state_mut().screen.edit_cell(row, "conc", "not a number");
    h.run();
    let sentence = h.state().screen.error.clone().unwrap_or_default();
    assert!(!sentence.is_empty());
    h.get_by_label_contains(&sentence);
}

#[test]
fn the_keyboard_moves_each_handle_point_by_point() {
    let mut h = harness();
    // The handle has the focus: the arrows move it.
    h.get_by_label_contains("λz range start").focus();
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
    assert_eq!(range(&h), (4.0, 8.0));
    h.key_press(Key::ArrowLeft);
    h.run();
    assert_eq!(range(&h), (2.0, 8.0));
    // Without the focus on a handle: Alt moves the start, Alt+Shift the end.
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowRight);
    h.run();
    assert_eq!(range(&h), (4.0, 8.0));
    h.key_press_modifiers(Modifiers::ALT | Modifiers::SHIFT, Key::ArrowLeft);
    h.run();
    assert_eq!(range(&h), (4.0, 6.0));
    // A for back to automatic.
    h.key_press(Key::A);
    h.run();
    assert!(!h.state().screen.manual());
    assert_eq!(range(&h), (2.0, 8.0));
}

#[test]
fn a_candidate_row_sets_the_range() {
    let mut h = harness();
    h.get_by_label_contains("Candidate 1").click();
    h.run();
    assert_eq!(range(&h), (4.0, 8.0));
    h.get_by_label("Back to automatic").click();
    h.run();
    assert_eq!(range(&h), (2.0, 8.0));
}

#[test]
fn the_results_are_the_formatted_numbers_and_copy_gives_tab_separated_text() {
    let mut h = harness();
    h.state_mut().screen.all_open = true;
    h.run();
    let expected = h.state().screen.expected_cells();
    assert!(!expected.is_empty());
    for (key, text) in &expected {
        assert!(
            h.state()
                .screen
                .shown
                .iter()
                .any(|(k, t)| k == key && t == text),
            "{key} is not on the screen as `{text}`"
        );
    }
    h.key_press_modifiers(Modifiers::CTRL | Modifiers::SHIFT, Key::C);
    h.run();
    let copied = h.state().screen.copied.clone().unwrap_or_default();
    let mut lines = copied.lines();
    assert_eq!(lines.next(), Some("Parameter\tValue\tUnit"));
    assert!(copied.contains("Cmax\t6.3\t"));
}
