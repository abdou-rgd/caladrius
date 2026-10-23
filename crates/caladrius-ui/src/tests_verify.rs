//! Tests of "Verify a project": a saved file is replayed on an engine of its own, the page lists
//! every step with the start of its seal, a changed number is found, the project in the window is
//! not touched, and the File menu, the palette and the page reach the same request.

use caladrius_engine::replay::{Status, Verdict};
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use crate::app::{Action, NoticeKind, Request, Selection, UiApp, UiState};
use crate::tests::{harness, with_analysis, with_oral};
use crate::theme::ThemeMode;

/// The bytes of a saved project: an import, an analysis, a fit, a simulation, an edit of the data
/// and the analysis run again.
fn saved_bytes() -> Vec<u8> {
    let mut app = with_analysis();
    app.perform(vec![
        Action::NewFit,
        Action::RunFit,
        Action::NewSimulation,
        Action::SimSave,
        Action::SetCell {
            worksheet: 1,
            row: 3,
            column: "Conc".to_owned(),
            text: "3,4".to_owned(),
        },
        Action::RunAgain(2),
        Action::SaveProject,
    ]);
    app.take_requests()
        .into_iter()
        .find_map(|request| match request {
            Request::SaveProject { bytes, .. } => Some(bytes),
            _ => None,
        })
        .expect("a save was requested")
}

fn document_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("a saved project is JSON")
}

fn bytes_of(document: &Value) -> Vec<u8> {
    serde_json::to_vec_pretty(document).expect("a document is JSON")
}

/// The position, from 1, and the parameters of the `data.import` entry of a saved project's history.
fn import_entry(document: &mut Value) -> (u64, &mut Value) {
    let history = document["history"].as_array_mut().expect("a history");
    let at = history
        .iter()
        .position(|entry| entry["command"] == "data.import")
        .expect("an import in the history");
    (at as u64 + 1, &mut history[at]["params"])
}

/// An application that has just verified `bytes`.
fn verified(name: &str, bytes: &[u8]) -> UiApp {
    let mut app = UiApp::new();
    assert!(app.verify_project(name, bytes), "{:?}", app.notice_text());
    app
}

#[test]
fn a_saved_project_verifies_and_the_page_lists_each_step_with_the_start_of_its_seal() {
    let bytes = saved_bytes();
    let app = verified("study.caladrius.json", &bytes);
    assert_eq!(app.state.selection, Selection::Verification);
    let report = app.verification.clone().unwrap().report;
    assert_eq!(report.verdict, Verdict::Verified, "{}", report.summary);
    assert!(report.steps.len() >= 7, "{}", report.steps.len());
    assert!(report.steps.iter().all(|s| s.status == Status::Match));
    assert_eq!(app.notice_text(), Some("study.caladrius.json: verified."));
    assert_eq!(
        app.notice.as_ref().map(|n| n.kind.clone()),
        Some(NoticeKind::Info)
    );

    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_label("Verify a project");
    h.get_by_label("Verified");
    h.get_by_label("study.caladrius.json");
    assert_eq!(h.query_all_by_label("matches").count(), report.steps.len());
    assert!(h.query_all_by_label("differs").next().is_none());
    for step in &report.steps {
        let seal = step.stored.as_deref().unwrap();
        assert_eq!(seal.len(), 64);
        // The stored and the recomputed seal are the same text on a row that matches.
        assert!(
            h.query_all_by_label(&seal[..12]).count() >= 2,
            "step {} shows the first twelve digits of its seal",
            step.seq
        );
        assert!(
            h.query_all_by_label(seal).next().is_none(),
            "only the first twelve digits are written on the page"
        );
    }
    // What the replay rests on is told: the steps, the project and the engine.
    h.get_by_label_contains("matching");
    h.get_by_label_contains("is the one the steps produce");
    // The engine that sealed the steps and the one that replays them: the same line twice, and
    // the version in the status bar.
    let engine = format!("Caladrius {}", crate::VERSION);
    assert_eq!(h.query_all_by_label_contains(&engine).count(), 3);
}

#[test]
fn verifying_a_file_leaves_the_project_in_the_window_alone() {
    let bytes = saved_bytes();
    let mut app = with_analysis();
    app.perform(vec![Action::NewFit]);
    let project = app.engine().project().clone();
    let entries = app.engine().history().len();
    let revision = app.engine().revision();
    let dirty = app.is_dirty();
    let name = app.file_name().map(str::to_owned);
    assert!(app.verify_project("other.caladrius.json", &bytes));
    assert_eq!(app.engine().project(), &project);
    assert_eq!(app.engine().history().len(), entries);
    assert_eq!(app.engine().revision(), revision);
    assert_eq!(app.is_dirty(), dirty);
    assert_eq!(app.file_name().map(str::to_owned), name);
    assert!(app.take_requests().is_empty());
    assert!(app.guard().is_none());
}

#[test]
fn one_changed_number_in_the_history_is_the_first_difference_and_the_page_shows_both_seals() {
    let mut document = document_of(&saved_bytes());
    let (at, params) = import_entry(&mut document);
    let csv = params["csv"]
        .as_str()
        .unwrap()
        .replacen("1,3.293,100", "1,3.294,100", 1);
    params["csv"] = json!(csv);
    let app = verified("edited.caladrius.json", &bytes_of(&document));
    let report = app.verification.clone().unwrap().report;
    assert_eq!(report.verdict, Verdict::Diverged, "{}", report.summary);
    let first = report.first_divergence.clone().unwrap();
    assert_eq!((first.seq, first.command.as_str()), (at, "data.import"));
    // The status bar says so in the colour of an error.
    assert_eq!(
        app.notice_text(),
        Some("edited.caladrius.json: does not reproduce.")
    );
    assert_eq!(
        app.notice.as_ref().map(|n| n.kind.clone()),
        Some(NoticeKind::Error)
    );

    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_label("Does not reproduce");
    assert!(h.query_all_by_label("differs").count() >= 1);
    // The first step that differs: the seal that was stored and the one computed now, twelve
    // digits each, side by side on the row of the step.
    let (stored, now) = (first.stored.unwrap(), first.recomputed.unwrap());
    assert_ne!(stored[..12], now[..12]);
    h.get_by_label(&stored[..12]);
    h.get_by_label(&now[..12]);
    h.get_by_label_contains(&format!(
        "Step {at} (data.import) does not reproduce its seal."
    ));
    // The row of the first step that differs is marked, and only that one.
    h.get_by_label("first difference");
}

#[test]
fn an_edited_result_in_the_file_shows_in_the_project_line_though_every_step_matches() {
    let mut document = document_of(&saved_bytes());
    document["worksheets"][0]["columns"][2]["data"]["number"][2] = json!(7.5);
    let app = verified("edited.caladrius.json", &bytes_of(&document));
    let report = app.verification.clone().unwrap().report;
    assert_eq!(report.verdict, Verdict::Diverged, "{}", report.summary);
    assert!(report.first_divergence.is_none());
    assert_eq!(report.state.matches, Some(false));

    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_label("Does not reproduce");
    h.get_by_label_contains("is not the one the steps produce: worksheet 1 differs");
    assert!(h.query_all_by_label("differs").next().is_none());
    assert!(h.query_by_label("first difference").is_none());
}

#[test]
fn a_project_saved_before_seals_says_so_on_every_step() {
    let mut document = document_of(&saved_bytes());
    for entry in document["history"].as_array_mut().unwrap() {
        let map = entry.as_object_mut().unwrap();
        map.remove("seal");
        map.remove("at");
    }
    let app = verified("old.caladrius.json", &bytes_of(&document));
    let report = app.verification.clone().unwrap().report;
    assert_eq!(report.verdict, Verdict::Unsealed, "{}", report.summary);
    assert_eq!(
        app.notice_text(),
        Some("old.caladrius.json: no seal to compare.")
    );
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_label("No seal to compare");
    assert_eq!(
        h.query_all_by_label("no seal recorded").count(),
        report.steps.len()
    );
    h.get_by_label_contains("no step carries a seal");
    assert!(h.query_all_by_label("matches").next().is_none());
}

#[test]
fn a_file_that_is_not_a_project_says_why_and_changes_nothing() {
    let mut app = with_analysis();
    let project = app.engine().project().clone();
    let selection = app.state.selection.clone();
    assert!(!app.verify_project("notes.json", b"{ not a project"));
    let said = app.notice_text().unwrap().to_owned();
    assert!(
        said.starts_with("notes.json cannot be verified: this is not a readable Caladrius project"),
        "{said}"
    );
    assert_eq!(
        app.notice.as_ref().map(|n| n.kind.clone()),
        Some(NoticeKind::Error)
    );
    assert!(!app.verify_project("binary.json", &[0xff, 0xfe, 0x00]));
    assert!(
        app.notice_text().unwrap().contains("not UTF-8"),
        "{:?}",
        app.notice_text()
    );
    assert!(!app.verify_project("other.json", br#"{"format":"something-else"}"#));
    assert_eq!(app.engine().project(), &project);
    assert_eq!(app.state.selection, selection);
    assert!(app.verification.is_none());
}

#[test]
fn closing_the_page_goes_back_to_what_was_on_screen_and_a_failed_file_keeps_the_report() {
    let bytes = saved_bytes();
    let mut app = with_analysis();
    let shown = app.state.selection.clone();
    assert!(matches!(shown, Selection::Analysis(_)));
    assert!(app.verify_project("a.caladrius.json", &bytes));
    assert_eq!(app.state.selection, Selection::Verification);
    // A second file replaces the report and keeps the way back; one that fails changes nothing.
    assert!(app.verify_project("b.caladrius.json", &bytes));
    assert!(!app.verify_project("notes.json", b"{ nope"));
    assert_eq!(app.verification.as_ref().unwrap().name, "b.caladrius.json");
    assert_eq!(app.state.selection, Selection::Verification);
    app.perform(vec![Action::CloseVerification]);
    assert_eq!(app.state.selection, shown);
    assert!(app.verification.is_none());
    // Closing a page that is not there does nothing.
    app.perform(vec![Action::CloseVerification]);
    assert_eq!(app.state.selection, shown);
}

#[test]
fn the_file_menu_the_palette_and_the_command_all_ask_for_a_file_to_verify() {
    let mut app = with_oral();
    assert!(app.is_dirty());
    app.perform(vec![Action::VerifyProject]);
    assert_eq!(app.take_requests(), vec![Request::VerifyProject]);
    // Nothing is replaced, so there is no question about the unsaved changes.
    assert!(app.guard().is_none());
    // The palette lists it with the actions of the interface, and the engine's own command
    // `project.replay` is the same action.
    assert!(
        app.palette_entries()
            .iter()
            .any(|e| e.key == "ui:verify_project" && e.action == Action::VerifyProject)
    );
    app.perform(vec![Action::PaletteRun("cmd:project.replay".to_owned())]);
    assert_eq!(app.take_requests(), vec![Request::VerifyProject]);
    assert!(app.notice_text().is_none_or(|t| !t.contains("no page yet")));

    let mut h = harness(app);
    h.run_steps(3);
    h.get_by_label("File").click();
    h.run_steps(3);
    h.get_by_label_contains("Verify a project…").click();
    h.run_steps(3);
    assert_eq!(h.state_mut().take_requests(), vec![Request::VerifyProject]);
}

#[test]
fn the_buttons_of_the_page_ask_for_another_file_or_close_it() {
    let bytes = saved_bytes();
    let mut app = with_oral();
    let shown = app.state.selection.clone();
    assert!(app.verify_project("study.caladrius.json", &bytes));
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_label("Verify another project…").click();
    h.run_steps(3);
    assert_eq!(h.state_mut().take_requests(), vec![Request::VerifyProject]);
    h.get_by_label("Close").click();
    h.run_steps(3);
    assert_eq!(h.state().state.selection, shown);
    assert!(h.query_by_label("Verify a project").is_none());
}

#[test]
fn the_page_without_a_report_is_the_welcome_page_and_the_state_is_data() {
    let mut app = UiApp::new();
    app.state.selection = Selection::Verification;
    let text = serde_json::to_string(&app.state).unwrap();
    assert!(text.contains("\"verification\""), "{text}");
    let back: UiState = serde_json::from_str(&text).unwrap();
    assert_eq!(back.selection, Selection::Verification);
    let mut h = harness(app);
    h.run_steps(3);
    assert!(h.query_by_label("Verify a project").is_none());
}

#[test]
fn the_page_draws_for_every_verdict_in_both_themes() {
    let good = saved_bytes();
    let mut edited = document_of(&good);
    import_entry(&mut edited).1["csv"] = json!("Time,Conc,Dose\n0,0,10\n1,5,10\n");
    let mut unsealed = document_of(&good);
    for entry in unsealed["history"].as_array_mut().unwrap() {
        entry.as_object_mut().unwrap().remove("seal");
    }
    // A project without a history has nothing to replay.
    let mut empty = document_of(&good);
    empty["history"] = json!([]);
    for dark in [false, true] {
        for bytes in [
            good.clone(),
            bytes_of(&edited),
            bytes_of(&unsealed),
            bytes_of(&empty),
        ] {
            let mut app = verified("any.caladrius.json", &bytes);
            app.state.mode = if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            let mut h = harness(app);
            h.run_steps(3);
            h.get_by_label("Verify a project");
        }
    }
}
