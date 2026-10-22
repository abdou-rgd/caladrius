//! The typed table (T-103): data typed or pasted into the table give the same worksheet and the
//! same NCA as the same data imported from a CSV file, and the checks of an import apply.

use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use crate::app::{Action, Selection, UiApp, UiState};
use crate::entry::{DataEntry, EntryRow, Field};
use crate::tests::harness;

/// A synthetic oral profile: ten samples, one dose of 100 mg.
const TIMES: [&str; 10] = ["0", "0.25", "0.5", "1", "2", "4", "6", "8", "12", "24"];
const CONCS: [&str; 10] = [
    "0", "1.279", "2.195", "3.293", "3.971", "3.611", "2.989", "2.451", "1.643", "0.495",
];

/// The same profile as a CSV file, with the column names the table writes.
fn csv_file() -> String {
    let mut out = "Time (h),Concentration (mg/L),Dose (mg)\n".to_owned();
    for (t, c) in TIMES.iter().zip(CONCS) {
        out.push_str(&format!("{t},{c},100\n"));
    }
    out
}

fn imported() -> UiApp {
    let mut app = UiApp::new();
    app.load_csv("Typed data.csv", csv_file().as_bytes());
    app.perform(vec![Action::ImportConfirm]);
    app
}

/// The table filled by `fill`, then "Create worksheet".
fn typed(fill: impl FnOnce(&mut DataEntry)) -> UiApp {
    let mut app = UiApp::new();
    app.perform(vec![Action::OpenEntry]);
    assert_eq!(app.state.selection, Selection::Entry);
    if let Some(entry) = app.state.entry.as_mut() {
        fill(entry);
    }
    app.perform(vec![Action::EntryCreate]);
    app
}

fn type_every_row(entry: &mut DataEntry, dose_on_every_row: bool) {
    entry.rows = TIMES
        .iter()
        .zip(CONCS)
        .enumerate()
        .map(|(i, (t, c))| EntryRow {
            subject: String::new(),
            time: (*t).to_owned(),
            concentration: c.to_owned(),
            dose: if dose_on_every_row || i == 0 {
                "100".to_owned()
            } else {
                String::new()
            },
        })
        .collect();
}

/// The cells and columns of worksheet 1.
fn sheet(app: &mut UiApp) -> Value {
    app.engine_mut()
        .execute(
            "export.table",
            json!({ "table": "worksheet", "worksheet": 1 }),
        )
        .unwrap()
}

/// The description of worksheet 1 (roles, units, subjects, doses), without its name.
fn described(app: &mut UiApp) -> Value {
    let mut v = app
        .engine_mut()
        .execute(
            "data.describe",
            json!({ "worksheet": 1, "preview_rows": 0 }),
        )
        .unwrap();
    v["worksheet"]["name"] = Value::Null;
    v
}

/// The NCA the "New analysis" button gives, as the engine stores it: specification and result.
fn nca(app: &mut UiApp) -> String {
    app.perform(vec![Action::NewAnalysis]);
    let id = app.nca_page().and_then(|p| p.analysis).unwrap();
    let v = app
        .engine_mut()
        .execute("analysis.get", json!({ "analysis": id }))
        .unwrap();
    assert!(
        v["result"]["subjects"][0]["outcome"].get("ok").is_some(),
        "{v}"
    );
    serde_json::to_string(&json!([v["spec"], v["result"]])).unwrap()
}

#[test]
fn typed_pasted_and_imported_data_give_the_same_worksheet_and_the_same_nca() {
    let mut file = imported();
    let reference = nca(&mut file);

    // Typed by hand, the dose on every row: the very worksheet of the file.
    let mut by_hand = typed(|e| type_every_row(e, true));
    assert_eq!(by_hand.state.selection, Selection::Worksheet(1));
    assert!(by_hand.state.entry.is_none());
    assert_eq!(sheet(&mut by_hand), sheet(&mut file));
    assert_eq!(described(&mut by_hand), described(&mut file));
    assert_eq!(nca(&mut by_hand), reference);

    // Pasted from a French spreadsheet: a header line, tabs, decimal commas, CRLF.
    let mut french = String::from("Temps (h)\tConc (mg/L)\tDose (mg)\r\n");
    for (t, c) in TIMES.iter().zip(CONCS) {
        french.push_str(&format!(
            "{}\t{}\t100\r\n",
            t.replace('.', ","),
            c.replace('.', ",")
        ));
    }
    let mut pasted = typed(|e| {
        e.paste(None, &french);
    });
    assert_eq!(sheet(&mut pasted), sheet(&mut file));
    assert_eq!(nca(&mut pasted), reference);

    // The dose typed once: the worksheet keeps the empty cells, the NCA is the same.
    let mut once = typed(|e| type_every_row(e, false));
    assert_eq!(nca(&mut once), reference);
}

#[test]
fn several_subjects_are_read_like_a_file_with_a_subject_column() {
    let csv = "Subject,Time (h),Concentration (mg/L),Dose (mg)\nA,0,0,100\nA,1,3.2,100\nA,2,4,100\nA,4,3.1,100\nA,8,1.9,100\nB,0,0,50\nB,1,1.5,50\nB,2,2.1,50\nB,4,1.6,50\nB,8,0.9,50\n";
    let mut file = UiApp::new();
    file.load_csv("Typed data.csv", csv.as_bytes());
    file.perform(vec![Action::ImportConfirm]);
    let mut by_hand = typed(|e| {
        e.subjects = true;
        e.rows.clear();
        for line in csv.lines().skip(1) {
            let cells: Vec<&str> = line.split(',').collect();
            e.rows.push(EntryRow {
                subject: cells[0].to_owned(),
                time: cells[1].to_owned(),
                concentration: cells[2].to_owned(),
                dose: cells[3].to_owned(),
            });
        }
    });
    assert_eq!(sheet(&mut by_hand), sheet(&mut file));
    assert_eq!(described(&mut by_hand), described(&mut file));
    let subjects = described(&mut by_hand)["worksheet"]["subjects"].clone();
    assert_eq!(subjects.as_array().map(Vec::len), Some(2), "{subjects}");
}

#[test]
fn a_table_with_a_check_waits_for_the_same_yes_as_a_file() {
    // Time 1 twice: the import of a file shows a check and needs the person's yes.
    let rows = [
        ["0", "0"],
        ["1", "3.2"],
        ["1", "3.0"],
        ["4", "2"],
        ["8", "1"],
    ];
    let mut csv = "Time (h),Concentration (mg/L),Dose (mg)\n".to_owned();
    for [t, c] in rows {
        csv.push_str(&format!("{t},{c},100\n"));
    }
    let mut file = UiApp::new();
    file.load_csv("Typed data.csv", csv.as_bytes());
    let file_checks = file
        .pending_import()
        .unwrap()
        .chosen_reading()
        .unwrap()
        .checks
        .clone();
    assert!(!file_checks.is_empty());

    let mut app = typed(|e| {
        e.rows = rows
            .iter()
            .map(|[t, c]| EntryRow {
                time: (*t).to_owned(),
                concentration: (*c).to_owned(),
                dose: "100".to_owned(),
                ..EntryRow::default()
            })
            .collect();
    });
    // Nothing stored; the same checks are shown on the table.
    assert!(app.engine().project().worksheets().is_empty());
    assert_eq!(app.state.selection, Selection::Entry);
    let entry = app.state.entry.as_ref().unwrap();
    let review = entry.review.as_ref().unwrap();
    assert_eq!(review.chosen_reading().unwrap().checks, file_checks);
    assert!(entry.review_is_current());
    // The yes is for this table only: an edit takes it back.
    if let Some(e) = app.state.entry.as_mut() {
        if let Some(r) = e.review.as_mut() {
            r.acknowledged = true;
        }
        e.rows[4].concentration = "1.1".to_owned();
        assert!(!e.review_is_current());
    }
    app.perform(vec![Action::EntryCreate]);
    assert!(app.engine().project().worksheets().is_empty());
    // A yes on the table as it is now creates the worksheet, with the file's cells.
    if let Some(r) = app.state.entry.as_mut().and_then(|e| e.review.as_mut()) {
        r.acknowledged = true;
    }
    app.perform(vec![Action::EntryCreate]);
    assert_eq!(app.engine().project().worksheets().len(), 1);
    assert_eq!(app.state.selection, Selection::Worksheet(1));
}

#[test]
fn a_cell_the_engine_cannot_read_is_said_like_in_a_file() {
    // A concentration typed as a word: no reading gives a numeric concentration column. The table
    // gets the file's error, nothing is stored and nothing typed is lost.
    let app = typed(|e| {
        type_every_row(e, true);
        e.rows[3].concentration = "abc".to_owned();
    });
    let csv = csv_file().replace("\n1,3.293,", "\n1,abc,");
    let mut file = UiApp::new();
    file.load_csv("Typed data.csv", csv.as_bytes());
    let file_error = file.pending_import().unwrap().error.clone().unwrap();
    assert!(app.engine().project().worksheets().is_empty());
    let entry = app.state.entry.as_ref().unwrap();
    assert_eq!(
        entry.review.as_ref().unwrap().error.as_deref(),
        Some(file_error.as_str())
    );
    assert_eq!(entry.rows[3].concentration, "abc");
}

#[test]
fn an_empty_table_creates_nothing_and_cancel_drops_the_table() {
    let mut app = typed(|_| {});
    assert!(app.engine().project().worksheets().is_empty());
    assert_eq!(app.state.selection, Selection::Entry);
    app.perform(vec![Action::EntryCancel]);
    assert!(app.state.entry.is_none());
    assert_eq!(app.state.selection, Selection::Welcome);
}

#[test]
fn the_table_is_kept_while_the_person_looks_elsewhere_and_round_trips_as_data() {
    let mut app = UiApp::new();
    app.perform(vec![Action::OpenEntry]);
    if let Some(e) = app.state.entry.as_mut() {
        e.rows[0].time = "0,5".to_owned();
        e.dose_unit = "mg/kg".to_owned();
    }
    app.perform(vec![Action::Select(Selection::Welcome), Action::OpenEntry]);
    assert_eq!(app.state.entry.as_ref().unwrap().rows[0].time, "0,5");
    let text = serde_json::to_string(&app.state).unwrap();
    let back: UiState = serde_json::from_str(&text).unwrap();
    assert_eq!(back.entry, app.state.entry);
    // A state written before the table existed still loads.
    let mut old: Value = serde_json::from_str(&text).unwrap();
    old.as_object_mut().unwrap().remove("entry");
    let back: UiState = serde_json::from_value(old).unwrap();
    assert!(back.entry.is_none());
}

#[test]
fn the_palette_and_the_welcome_screen_open_the_table() {
    let mut app = UiApp::new();
    app.perform(vec![Action::PaletteRun("ui:type_data".to_owned())]);
    assert_eq!(app.state.selection, Selection::Entry);

    let mut h = harness(UiApp::new());
    h.run_steps(2);
    h.get_by_label("Type or paste data…").click();
    h.run_steps(3);
    assert_eq!(h.state().state.selection, Selection::Entry);
}

#[test]
fn ctrl_v_in_a_cell_spreads_the_copied_cells_from_that_cell() {
    let mut app = UiApp::new();
    app.perform(vec![Action::OpenEntry]);
    let mut h = harness(app);
    h.run_steps(2);
    // Text inputs: the name, three units, then the cells row by row (time, concentration, dose).
    let second_row_time = 1 + 3 + 3;
    h.get_all_by_role(egui::accesskit::Role::TextInput)
        .nth(second_row_time)
        .unwrap()
        .focus();
    h.run_steps(2);
    assert_eq!(
        h.state().state.entry.as_ref().unwrap().focus,
        Some((1, Field::Time))
    );
    h.input_mut()
        .events
        .push(egui::Event::Paste("0,5\t2,195\n1\t3,293".to_owned()));
    h.run_steps(2);
    let entry = h.state().state.entry.clone().unwrap();
    assert_eq!(entry.rows[0].time, "");
    assert_eq!(
        (
            entry.rows[1].time.as_str(),
            entry.rows[1].concentration.as_str()
        ),
        ("0,5", "2,195")
    );
    assert_eq!(entry.rows[2].concentration, "3,293");
    assert!(
        entry
            .note
            .as_deref()
            .unwrap()
            .starts_with("Pasted 2 rows from row 2.")
    );
    // One value goes to the cell as usual.
    h.input_mut()
        .events
        .push(egui::Event::Paste("7".to_owned()));
    h.run_steps(2);
    let entry = h.state().state.entry.clone().unwrap();
    assert_eq!(entry.rows[3].time, "");
}

/// A synthetic wide table (T-108): three subjects, times down the first column, French decimal
/// commas, a value below the limit of quantification, a sample missing, and a Dose line.
const WIDE: &str = "Temps (h)\tA\tB\tC\r\n\
Dose (mg/kg)\t30\t30\t45\r\n\
0,5\t9,1\t12,4\t7,8\r\n\
1\t8,4\t11\t7,5\r\n\
2\t7,3\t9,2\t\r\n\
\r\n\
4\t5,6\t6,5\t5,9\r\n\
8\t3,2\t3,4\t4,3\r\n\
12\t1,9\t1,7\t3,1\r\n\
24\t0,6\t< LQ\t1,6\r\n";

/// The same samples as a long CSV file, one row per sample, subject by subject, the dose on each
/// subject's first row.
fn wide_as_csv(dose_on_every_row: bool) -> String {
    let times = ["0.5", "1", "2", "4", "8", "12", "24"];
    let subjects = [
        ("A", "30", ["9.1", "8.4", "7.3", "5.6", "3.2", "1.9", "0.6"]),
        (
            "B",
            "30",
            ["12.4", "11", "9.2", "6.5", "3.4", "1.7", "< LQ"],
        ),
        ("C", "45", ["7.8", "7.5", "", "5.9", "4.3", "3.1", "1.6"]),
    ];
    let mut out = "Subject,Time (h),Concentration (mg/L),Dose (mg/kg)\n".to_owned();
    for (s, dose, concs) in subjects {
        let mut first = true;
        for (t, c) in times.iter().zip(concs) {
            if c.is_empty() {
                continue;
            }
            let d = if first || dose_on_every_row { dose } else { "" };
            out.push_str(&format!("{s},{t},{c},{d}\n"));
            first = false;
        }
    }
    out
}

fn imported_csv(csv: &str) -> UiApp {
    let mut app = UiApp::new();
    app.load_csv("Typed data.csv", csv.as_bytes());
    app.perform(vec![Action::ImportConfirm]);
    assert_eq!(app.engine().project().worksheets().len(), 1, "{csv}");
    app
}

/// The NCA of every subject: the "New analysis" one (the first subject), then `nca.run` on each
/// subject with the units of the worksheet.
fn nca_all(app: &mut UiApp) -> String {
    let mut all = nca(app);
    for subject in ["A", "B", "C"] {
        let v = app
            .engine_mut()
            .execute(
                "nca.run",
                json!({ "worksheet": 1, "subject": subject, "route": "extravascular",
                        "options": { "units": { "time": "h", "concentration": "mg/L", "dose": "mg/kg" } } }),
            )
            .unwrap();
        let result = &v["result"];
        assert!(result["subjects"][0]["outcome"].get("ok").is_some(), "{v}");
        all.push_str(&serde_json::to_string(result).unwrap());
    }
    all
}

#[test]
fn a_wide_table_pasted_gives_the_worksheet_and_the_nca_of_the_same_long_table() {
    let mut file = imported_csv(&wide_as_csv(false));
    let reference = nca_all(&mut file);

    // Pasted with "Several subjects" off: switched on, one row per sample, units taken.
    let mut note = String::new();
    let mut pasted = typed(|e| {
        assert!(!e.subjects);
        note = e.paste(None, WIDE);
        assert!(e.subjects);
        assert_eq!(e.filled().len(), 20);
        assert_eq!((e.time_unit.as_str(), e.dose_unit.as_str()), ("h", "mg/kg"));
    });
    assert!(
        note.starts_with("Pasted 3 subjects × 7 times as 20 rows from row 1"),
        "{note}"
    );
    assert!(note.contains("\"Several subjects\" is now on."), "{note}");
    assert!(note.contains("1 empty cell was left out."), "{note}");
    assert!(note.contains("from the Dose line"), "{note}");
    assert_eq!(sheet(&mut pasted), sheet(&mut file));
    assert_eq!(described(&mut pasted), described(&mut file));
    assert_eq!(nca_all(&mut pasted), reference);

    // The same long table typed by hand.
    let csv = wide_as_csv(false);
    let mut by_hand = typed(|e| {
        e.subjects = true;
        e.dose_unit = "mg/kg".to_owned();
        e.rows = csv
            .lines()
            .skip(1)
            .map(|line| {
                let c: Vec<&str> = line.split(',').collect();
                EntryRow {
                    subject: c[0].to_owned(),
                    time: c[1].to_owned(),
                    concentration: c[2].to_owned(),
                    dose: c[3].to_owned(),
                }
            })
            .collect();
    });
    assert_eq!(nca_all(&mut by_hand), reference);
}

#[test]
fn without_a_dose_line_one_dose_is_given_to_every_subject() {
    let file_csv = wide_as_csv(true).replace(",45\n", ",30\n");
    let mut file = imported_csv(&file_csv);
    let reference = nca_all(&mut file);
    let no_dose: String = WIDE
        .lines()
        .filter(|l| !l.starts_with("Dose"))
        .map(|l| format!("{l}\n"))
        .collect();
    let mut app = typed(|e| {
        e.dose_unit = "mg/kg".to_owned();
        let note = e.paste(None, &no_dose);
        assert!(note.contains("No Dose line"), "{note}");
        assert!(e.rows.iter().all(|r| r.dose.is_empty()));
        assert_eq!(e.fill_dose(), "Type the dose to give first.");
        e.common_dose = "30".to_owned();
        assert_eq!(e.fill_dose(), "Dose 30 given to 3 subjects that had none.");
        let firsts: Vec<usize> = e
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.dose.is_empty())
            .map(|(i, _)| i)
            .collect();
        assert_eq!(firsts, [0, 7, 14]);
        // A subject that has a dose keeps it.
        e.rows[7].dose.clear();
        e.rows[8].dose = "30".to_owned();
        e.rows[0].dose.clear();
        assert_eq!(e.fill_dose(), "Dose 30 given to 1 subject that had none.");
        assert_eq!(e.rows[7].dose, "");
        e.rows[8].dose.clear();
        e.rows[7].dose = "30".to_owned();
        assert_eq!(
            e.fill_dose(),
            "Every subject already has a dose: nothing changed."
        );
    });
    assert_eq!(nca_all(&mut app), reference);
}

#[test]
fn a_paste_that_is_not_a_wide_table_is_spread_as_before() {
    // No time header: the header line has digits and is read as data, cell for cell.
    let mut e = DataEntry::new();
    let said = e.paste(None, "Visit\t1\t2\n0,5\t9\t8\n1\t7\t6");
    assert!(said.starts_with("Pasted 3 rows from row 1."), "{said}");
    assert!(!e.subjects);
    assert_eq!(e.rows[0].time, "Visit");
    // One subject column: the time and its concentration, the header left out.
    let mut e = DataEntry::new();
    let said = e.paste(None, "t (h)\tA\n0,5\t9\n1\t7");
    assert!(said.contains("header"), "{said}");
    assert_eq!(
        (e.rows[1].time.as_str(), e.rows[1].concentration.as_str()),
        ("1", "7")
    );
    assert!(!e.subjects);
}

#[test]
fn a_spreadsheet_cell_in_quotes_with_a_line_break_stays_one_cell() {
    // A spreadsheet quotes a cell holding a line break; the wide header is then still found.
    let mut e = DataEntry::new();
    let said = e.paste(None, "\"Temps\n(h)\"\tA\tB\n0\t1\t2\n1\t3\t4\n");
    assert!(said.starts_with("Pasted 2 subjects × 2 times"), "{said}");
    assert_eq!(e.time_unit, "h");
    assert_eq!(e.rows[2].subject, "B");
    // In a long paste, a quoted cell with a tab stays one cell and a doubled quote is a quote.
    let mut e = DataEntry::new();
    e.paste(None, "0\t\"a\tb \"\"x\"\"\"\t5\n1\t2\t");
    assert_eq!(e.rows[0].concentration, "a b \"x\"");
    assert_eq!(e.rows[0].dose, "5");
    assert_eq!(e.rows[1].concentration, "2");
}

#[test]
fn the_unit_cells_offer_the_engine_units_and_say_at_once_when_one_is_not_recognised() {
    let mut app = UiApp::new();
    app.perform(vec![Action::OpenEntry]);
    let mut h = harness(app);
    h.run_steps(2);
    // The list comes from the engine, mg/kg among the doses; the default units pass.
    let units = h.state().units.clone();
    assert!(units.dose.contains(&"mg/kg".to_owned()), "{units:?}");
    assert_eq!(units.time, ["s", "min", "h", "d"]);
    let checks = h.state().state.entry.as_ref().unwrap().unit_checks.clone();
    assert_eq!(checks.map(|(_, m)| m), Some(Vec::new()));
    // A unit the engine does not know: said under the name line, once the cell is left.
    if let Some(e) = h.state_mut().state.entry.as_mut() {
        e.dose_unit = "mg per kg".to_owned();
    }
    h.run_steps(2);
    let messages = h
        .state()
        .state
        .entry
        .as_ref()
        .unwrap()
        .unit_checks
        .clone()
        .unwrap()
        .1;
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].contains("`mgperkg` is not recognised"),
        "{messages:?}"
    );
    // The menu next to the dose unit puts a listed unit in the cell.
    h.get_all_by_label("\u{23f7}").nth(2).unwrap().click();
    h.run_steps(2);
    h.get_by_label("mg/kg").click();
    h.run_steps(3);
    let entry = h.state().state.entry.clone().unwrap();
    assert_eq!(entry.dose_unit, "mg/kg");
    assert_eq!(entry.unit_checks.map(|(_, m)| m), Some(Vec::new()));
}

#[test]
fn the_worksheet_unit_cells_offer_the_same_list() {
    // An imported file: the menu next to the time unit sets the column's unit with data.set_column.
    let mut h = harness(imported());
    h.run_steps(2);
    h.get_all_by_label("\u{23f7}").next().unwrap().click();
    h.run_steps(2);
    h.get_by_label("min").click();
    h.run_steps(3);
    let unit = described(h.state_mut())["worksheet"]["columns"][0]["unit"].clone();
    assert_eq!(unit, "min");
}
