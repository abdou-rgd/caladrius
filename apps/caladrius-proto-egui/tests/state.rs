use caladrius_proto_egui::state::{Handle, Screen};

const EDGE: &str = include_str!("../../../oracle/data/edge_blq.csv");
const NCA50: &str = include_str!("../../../prototypes/nca50/nca50.csv");

#[test]
fn edge_blq_runs_and_a_handle_changes_lambda_z() {
    let mut s = Screen::new();
    assert_eq!(s.load_csv("edge_blq", EDGE, "3"), Ok(()));
    assert_eq!(s.subject, "3");
    assert_eq!(s.range(), Some((2.0, 8.0)));
    let lambda_z = |s: &Screen| {
        s.headline()
            .iter()
            .find(|c| c.name == "lambda.z")
            .map(|c| c.text.clone())
    };
    let before = lambda_z(&s);
    assert!(s.drag_handle(Handle::Start, 4.0), "{:?}", s.notice);
    assert_eq!(s.range(), Some((4.0, 8.0)));
    assert_ne!(lambda_z(&s), before);
    // The start cannot pass the end: the screen says so and does not call the engine.
    let runs = s.runs;
    assert!(!s.drag_handle(Handle::Start, 8.0));
    assert_eq!(s.runs, runs);
    assert!(s.notice.is_some());
}

#[test]
fn a_bad_cell_gives_the_engine_sentence_and_no_crash() {
    let mut s = Screen::new();
    assert_eq!(s.load_csv("edge_blq", EDGE, "3"), Ok(()));
    let row = s.table[1].sheet_row;
    s.edit_cell(row, "conc", "abc");
    assert!(s.error.as_deref().is_some_and(|e| !e.is_empty()));
}

#[test]
fn nca50_manual_ranges_run() {
    let mut s = Screen::new();
    assert_eq!(s.load_csv("nca50", NCA50, "1"), Ok(()));
    assert_eq!(s.subjects.len(), 50);
    for k in 0..100usize {
        let subject = format!("{}", 1 + k % 50);
        s.select_subject(&subject, false);
        let times = s.eligible_times();
        let n = [3usize, 4, 5, 6][k % 4];
        let start = times[times.len() - n];
        assert!(
            s.drag_handle(Handle::Start, start),
            "drag {k}: {:?}",
            s.notice
        );
        assert_eq!(s.error, None, "drag {k}");
        assert_eq!(s.range().map(|r| r.0), Some(start), "drag {k}");
    }
}
