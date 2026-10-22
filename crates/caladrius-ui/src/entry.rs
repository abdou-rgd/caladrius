//! Typing or pasting a small data set instead of opening a CSV file. The table holds the text as
//! typed; "Create worksheet" turns it into tab-separated text and sends it through the same
//! commands as a file (`data.preview`, then `data.import`), so the checks, the messages and the
//! worksheet are those of an import. Nothing here reads a number.

use egui::{Id, RichText, Ui};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::Action;
use crate::import::{PendingImport, ReadOptions};
use crate::theme::Tokens;

/// The name a worksheet gets when the name field is left empty.
pub const DEFAULT_NAME: &str = "Typed data";

/// The empty rows a new table starts with (a small profile fits without adding any).
const START_ROWS: usize = 12;

/// One column of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Subject,
    Time,
    Concentration,
    Dose,
}

impl Field {
    pub const ALL: [Field; 4] = [
        Field::Subject,
        Field::Time,
        Field::Concentration,
        Field::Dose,
    ];

    /// The column name, also the name of the worksheet column.
    pub fn title(self) -> &'static str {
        match self {
            Field::Subject => "Subject",
            Field::Time => "Time",
            Field::Concentration => "Concentration",
            Field::Dose => "Dose",
        }
    }

    /// The role of the worksheet column (`data.import`).
    pub fn role(self) -> &'static str {
        match self {
            Field::Subject => "subject",
            Field::Time => "time",
            Field::Concentration => "concentration",
            Field::Dose => "dose",
        }
    }
}

/// One row as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryRow {
    pub subject: String,
    pub time: String,
    pub concentration: String,
    pub dose: String,
}

impl EntryRow {
    pub fn get(&self, field: Field) -> &str {
        match field {
            Field::Subject => &self.subject,
            Field::Time => &self.time,
            Field::Concentration => &self.concentration,
            Field::Dose => &self.dose,
        }
    }

    pub fn get_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Subject => &mut self.subject,
            Field::Time => &mut self.time,
            Field::Concentration => &mut self.concentration,
            Field::Dose => &mut self.dose,
        }
    }

    fn is_blank(&self, fields: &[Field]) -> bool {
        fields.iter().all(|f| self.get(*f).trim().is_empty())
    }
}

/// The table being typed, its units and its name: serializable (golden rule 7). The engine's
/// answer to the last "Create worksheet" is kept beside it while the table is unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataEntry {
    pub name: String,
    /// Whether the subject column is shown (several subjects).
    pub subjects: bool,
    pub time_unit: String,
    pub concentration_unit: String,
    pub dose_unit: String,
    pub rows: Vec<EntryRow>,
    /// The cell that had the keyboard focus at the last frame, where a paste lands.
    #[serde(skip)]
    pub focus: Option<(usize, Field)>,
    /// The checks of the last "Create worksheet", for the table as it was then.
    #[serde(skip)]
    pub review: Option<PendingImport>,
    /// What the last paste did.
    #[serde(skip)]
    pub note: Option<String>,
}

impl Default for DataEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl DataEntry {
    pub fn new() -> DataEntry {
        DataEntry {
            name: DEFAULT_NAME.to_owned(),
            subjects: false,
            time_unit: "h".to_owned(),
            concentration_unit: "mg/L".to_owned(),
            dose_unit: "mg".to_owned(),
            rows: vec![EntryRow::default(); START_ROWS],
            focus: None,
            review: None,
            note: None,
        }
    }

    /// The columns on screen, in order.
    pub fn fields(&self) -> Vec<Field> {
        Field::ALL
            .into_iter()
            .filter(|f| self.subjects || *f != Field::Subject)
            .collect()
    }

    pub fn unit(&self, field: Field) -> &str {
        match field {
            Field::Subject => "",
            Field::Time => &self.time_unit,
            Field::Concentration => &self.concentration_unit,
            Field::Dose => &self.dose_unit,
        }
    }

    pub fn unit_mut(&mut self, field: Field) -> Option<&mut String> {
        match field {
            Field::Subject => None,
            Field::Time => Some(&mut self.time_unit),
            Field::Concentration => Some(&mut self.concentration_unit),
            Field::Dose => Some(&mut self.dose_unit),
        }
    }

    /// The rows with something typed in a column on screen.
    pub fn filled(&self) -> Vec<&EntryRow> {
        let fields = self.fields();
        self.rows.iter().filter(|r| !r.is_blank(&fields)).collect()
    }

    /// The columns written to the worksheet: those on screen, without a subject column that is
    /// empty on every row.
    fn written(&self) -> Vec<Field> {
        let filled = self.filled();
        self.fields()
            .into_iter()
            .filter(|f| *f != Field::Subject || filled.iter().any(|r| !r.subject.trim().is_empty()))
            .collect()
    }

    /// The header of a column as a CSV file would write it: `Time (h)`, or `Time` with no unit.
    fn header(&self, field: Field) -> String {
        let unit = self.unit(field).trim();
        if unit.is_empty() {
            field.title().to_owned()
        } else {
            format!("{} ({unit})", field.title())
        }
    }

    /// The table as tab-separated text, a header line then the filled rows, cells as typed.
    pub fn csv(&self) -> String {
        let fields = self.written();
        let mut out = String::new();
        let header: Vec<String> = fields.iter().map(|f| quoted(&self.header(*f))).collect();
        out.push_str(&header.join("\t"));
        out.push('\n');
        for row in self.filled() {
            let cells: Vec<String> = fields.iter().map(|f| quoted(row.get(*f).trim())).collect();
            out.push_str(&cells.join("\t"));
            out.push('\n');
        }
        out
    }

    /// The worksheet name: the one typed, else [`DEFAULT_NAME`].
    pub fn worksheet_name(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            DEFAULT_NAME.to_owned()
        } else {
            name.to_owned()
        }
    }

    /// The table as an import waiting for the engine's preview: tabs between the cells, and the
    /// decimal comma accepted (a cell with a point is read with the point).
    pub fn pending(&self) -> PendingImport {
        let mut p = PendingImport::new(&self.worksheet_name(), self.csv());
        p.options = ReadOptions {
            delimiter: Some('\t'),
            decimal_comma: Some(true),
        };
        p
    }

    /// The roles and units of the columns, said explicitly to `data.import`.
    pub fn import_columns(&self) -> Value {
        Value::Array(
            self.written()
                .into_iter()
                .map(|f| {
                    let unit = self.unit(f).trim();
                    json!({
                        "name": f.title(),
                        "role": f.role(),
                        "unit": if unit.is_empty() { Value::Null } else { json!(unit) },
                    })
                })
                .collect(),
        )
    }

    /// Whether the checks shown were made on the table as it is now.
    pub fn review_is_current(&self) -> bool {
        self.review
            .as_ref()
            .is_some_and(|r| r.csv == self.csv() && r.name == self.worksheet_name())
    }

    pub fn add_row(&mut self) {
        self.rows.push(EntryRow::default());
    }

    pub fn remove_row(&mut self, index: usize) {
        if index < self.rows.len() {
            self.rows.remove(index);
        }
        if self.rows.is_empty() {
            self.add_row();
        }
        self.focus = None;
    }

    pub fn clear(&mut self) {
        self.rows = vec![EntryRow::default(); START_ROWS];
        self.focus = None;
        self.review = None;
        self.note = None;
    }

    /// Spreads a paste of several cells (rows on lines, cells separated by tabs, as a spreadsheet
    /// copies them) over the table from `at`, or from the first empty row when no cell has the
    /// focus. Rows are added as needed. A first line without a digit is a header and is left out.
    /// Returns what was done, in one sentence.
    pub fn paste(&mut self, at: Option<(usize, Field)>, text: &str) -> String {
        let fields = self.fields();
        let mut lines: Vec<Vec<String>> = text
            .lines()
            .map(|line| split_cells(line.trim_end_matches('\r')))
            .collect();
        while lines.last().is_some_and(|l| l.iter().all(|c| c.is_empty())) {
            lines.pop();
        }
        let header = lines
            .first()
            .is_some_and(|l| !l.iter().any(|c| c.chars().any(|ch| ch.is_ascii_digit())));
        if header {
            lines.remove(0);
        }
        if lines.is_empty() {
            return "Nothing to paste: the clipboard holds no row with a number.".to_owned();
        }
        let (start_row, start_col) = match at {
            Some((row, field)) => (row, fields.iter().position(|f| *f == field).unwrap_or(0)),
            None => (
                self.rows
                    .iter()
                    .rposition(|r| !r.is_blank(&fields))
                    .map_or(0, |last| last + 1),
                0,
            ),
        };
        let mut dropped = 0;
        for (i, cells) in lines.iter().enumerate() {
            let row = start_row + i;
            while self.rows.len() <= row {
                self.add_row();
            }
            for (j, cell) in cells.iter().enumerate() {
                let target = fields.get(start_col + j).copied();
                match (target, self.rows.get_mut(row)) {
                    (Some(field), Some(r)) => *r.get_mut(field) = cell.clone(),
                    _ => dropped += 1,
                }
            }
        }
        let n = lines.len();
        let mut said = format!(
            "Pasted {n} row{} from row {}.",
            if n == 1 { "" } else { "s" },
            start_row + 1
        );
        if header {
            said.push_str(" The first line had no number and was read as a header, left out.");
        }
        if dropped > 0 {
            said.push_str(&format!(
                " {dropped} cell{} beyond the last column {} left out.",
                if dropped == 1 { "" } else { "s" },
                if dropped == 1 { "was" } else { "were" }
            ));
        }
        said
    }
}

/// The cells of one pasted line: tabs when there are any, else runs of spaces.
fn split_cells(line: &str) -> Vec<String> {
    if line.contains('\t') {
        line.split('\t').map(|c| c.trim().to_owned()).collect()
    } else {
        line.split_whitespace().map(str::to_owned).collect()
    }
}

/// A cell for the tab-separated text: quoted only when it holds a quote.
fn quoted(cell: &str) -> String {
    if cell.contains('"') {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_owned()
    }
}

/// Whether a paste is for the table rather than for one cell: it spans several cells.
pub fn is_table_paste(text: &str) -> bool {
    text.trim_end_matches(['\r', '\n']).contains(['\t', '\n'])
}

/// The screen.
pub fn screen(ui: &mut Ui, tokens: &Tokens, entry: &mut DataEntry, actions: &mut Vec<Action>) {
    let c = &tokens.colors;
    // A paste of several cells fills the table; a single value goes to the cell as usual.
    let pasted = ui.input_mut(|i| {
        let mut taken = None;
        i.events.retain(|e| match e {
            egui::Event::Paste(text) if is_table_paste(text) => {
                taken = Some(text.clone());
                false
            }
            _ => true,
        });
        taken
    });
    if let Some(text) = pasted {
        let at = entry.focus;
        entry.note = Some(entry.paste(at, &text));
    }

    ui.label(
        RichText::new("Type or paste data")
            .size(tokens.font.title)
            .strong(),
    );
    ui.label(
        RichText::new("One row per sample. Copy the cells in a spreadsheet and press Ctrl+V in the table to fill it at once; the dose is needed once per subject.")
            .color(c.text_muted.color()),
    );
    ui.add_space(tokens.spacing.medium);
    ui.horizontal(|ui| {
        ui.label("Worksheet name");
        ui.add(
            egui::TextEdit::singleline(&mut entry.name).desired_width(tokens.size.text_box_width),
        );
        ui.add_space(tokens.spacing.large);
        ui.checkbox(
            &mut entry.subjects,
            "Several subjects (adds a subject column)",
        );
    });
    ui.add_space(tokens.spacing.medium);

    let fields = entry.fields();
    let mut remove = None;
    let mut focus = None;
    egui::Grid::new("entry-table")
        .striped(true)
        .spacing([tokens.spacing.small, tokens.spacing.small])
        .show(ui, |ui| {
            let cell = [tokens.size.cell_width, tokens.size.row_height];
            ui.label("");
            for f in &fields {
                ui.add_sized(cell, egui::Label::new(RichText::new(f.title()).strong()));
            }
            ui.end_row();
            ui.label(RichText::new("unit").small().color(c.text_muted.color()));
            for f in &fields {
                match entry.unit_mut(*f) {
                    Some(unit) => {
                        ui.add_sized(
                            cell,
                            egui::TextEdit::singleline(unit).id(Id::new(("entry-unit", f.role()))),
                        );
                    }
                    None => {
                        ui.label("");
                    }
                }
            }
            ui.end_row();
            for (i, row) in entry.rows.iter_mut().enumerate() {
                ui.label(
                    RichText::new((i + 1).to_string())
                        .small()
                        .color(c.text_muted.color()),
                );
                for f in &fields {
                    let response = ui.add_sized(
                        cell,
                        egui::TextEdit::singleline(row.get_mut(*f)).id(Id::new((
                            "entry-cell",
                            i,
                            f.role(),
                        ))),
                    );
                    if response.has_focus() {
                        focus = Some((i, *f));
                    }
                }
                if ui
                    .small_button("Remove")
                    .on_hover_text("Remove this row")
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    entry.focus = focus;
    if let Some(i) = remove {
        entry.remove_row(i);
    }
    ui.horizontal(|ui| {
        if ui.button("Add a row").clicked() {
            entry.add_row();
        }
        if ui.button("Clear the table").clicked() {
            entry.clear();
        }
        ui.label(
            RichText::new(format!("{} filled rows", entry.filled().len()))
                .small()
                .color(c.text_muted.color()),
        );
    });
    if let Some(note) = &entry.note {
        ui.label(RichText::new(note).small().color(c.text_muted.color()));
    }

    // The engine's checks of the last "Create worksheet", while the table is unchanged.
    if !entry.review_is_current() {
        entry.review = None;
    }
    if let Some(review) = entry.review.as_mut() {
        ui.add_space(tokens.spacing.medium);
        if let Some(error) = &review.error {
            crate::widgets::error_box(ui, tokens, error);
            ui.label(
                RichText::new("The headers and the separator are written by Caladrius here: look for a cell of the time, concentration or dose column that is not a number (a letter, a stray symbol, two decimal marks).")
                    .small()
                    .color(c.text_muted.color()),
            );
        }
        let checks = review
            .chosen_reading()
            .map(|r| r.checks.clone())
            .unwrap_or_default();
        {
            if !checks.is_empty() {
                tokens
                    .banner_frame(c.stale_background, c.stale)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("These data look wrong. Correct the table, or create the worksheet anyway.")
                                .strong()
                                .color(c.stale.color()),
                        );
                        for check in &checks {
                            ui.label(
                                RichText::new(format!("Check: {}", check.message))
                                    .color(c.warning.color()),
                            );
                        }
                        ui.checkbox(
                            &mut review.acknowledged,
                            "I have looked at these checks and want to create the worksheet anyway",
                        );
                    });
            }
        }
    }

    ui.add_space(tokens.spacing.medium);
    ui.horizontal(|ui| {
        let any = !entry.filled().is_empty();
        if ui
            .add_enabled(any, tokens.primary_button("Create worksheet"))
            .on_disabled_hover_text("Type or paste at least one row first.")
            .clicked()
        {
            actions.push(Action::EntryCreate);
        }
        if ui.button("Cancel").clicked() {
            actions.push(Action::EntryCancel);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(rows: &[[&str; 3]]) -> DataEntry {
        let mut e = DataEntry::new();
        e.rows = rows
            .iter()
            .map(|[t, c, d]| EntryRow {
                subject: String::new(),
                time: (*t).to_owned(),
                concentration: (*c).to_owned(),
                dose: (*d).to_owned(),
            })
            .collect();
        e
    }

    #[test]
    fn the_table_becomes_tab_separated_text_with_units_in_the_headers() {
        let mut e = typed(&[["0", "0", "100"], ["0,5", "2.1", ""], ["", "", ""]]);
        assert_eq!(
            e.csv(),
            "Time (h)\tConcentration (mg/L)\tDose (mg)\n0\t0\t100\n0,5\t2.1\t\n"
        );
        // A subject column shown but left empty is not written; one with a value is.
        e.subjects = true;
        assert!(e.csv().starts_with("Time (h)\t"));
        e.rows[0].subject = "S1".to_owned();
        assert!(e.csv().starts_with("Subject\tTime (h)\t"));
        assert!(e.csv().contains("\nS1\t0\t0\t100\n\t0,5\t2.1\t\n"));
        // An empty unit gives a bare name and a null unit for the import.
        e.dose_unit = "  ".to_owned();
        assert!(e.csv().contains("\tDose\n"));
        let cols = e.import_columns();
        assert_eq!(
            cols[3],
            json!({ "name": "Dose", "role": "dose", "unit": null })
        );
        assert_eq!(cols[0]["role"], "subject");
    }

    #[test]
    fn a_spreadsheet_paste_fills_the_table_from_the_cell_in_focus() {
        let mut e = DataEntry::new();
        let said = e.paste(
            None,
            "Temps (h)\tConc (mg/L)\n0\t0\r\n0,5\t2,195\r\n1\t3,293\r\n\r\n",
        );
        assert!(said.starts_with("Pasted 3 rows from row 1."), "{said}");
        assert!(said.contains("header"));
        assert_eq!(e.rows[1].time, "0,5");
        assert_eq!(e.rows[2].concentration, "3,293");
        // Without focus a second paste goes under the last filled row.
        e.paste(None, "2\t3,971\n4\t3,611");
        assert_eq!(e.rows[3].time, "2");
        assert_eq!(e.rows[4].concentration, "3,611");
        // From a cell in focus: the dose column, extra cells left out, rows added.
        let said = e.paste(Some((11, Field::Dose)), "100\t7\n\n100");
        assert_eq!(e.rows.len(), 14);
        assert_eq!(e.rows[11].dose, "100");
        assert_eq!(e.rows[13].dose, "100");
        assert!(
            said.contains("1 cell beyond the last column was left out"),
            "{said}"
        );
        // Spaces between the cells work too; a paste with no number does nothing.
        let mut e = DataEntry::new();
        e.paste(None, "0 0 100\n1 2.5");
        assert_eq!(
            (e.rows[0].dose.as_str(), e.rows[1].concentration.as_str()),
            ("100", "2.5")
        );
        let before = e.clone();
        assert!(
            e.paste(None, "time\tconc\n")
                .starts_with("Nothing to paste")
        );
        assert_eq!(e, before);
    }

    #[test]
    fn rows_are_added_removed_and_cleared_and_one_row_always_stays() {
        let mut e = typed(&[["0", "1", ""]]);
        e.add_row();
        assert_eq!(e.rows.len(), 2);
        e.remove_row(0);
        e.remove_row(0);
        e.remove_row(7);
        assert_eq!(e.rows.len(), 1);
        e.rows[0].time = "3".to_owned();
        e.clear();
        assert_eq!(e.rows.len(), START_ROWS);
        assert!(e.filled().is_empty());
    }

    #[test]
    fn only_a_paste_of_several_cells_is_for_the_table() {
        assert!(is_table_paste("0\t1"));
        assert!(is_table_paste("0\n1"));
        assert!(!is_table_paste("3,25"));
        assert!(!is_table_paste("3,25\r\n"));
    }

    #[test]
    fn the_import_reads_tabs_and_either_decimal_mark() {
        let mut e = typed(&[["0", "1", "5"]]);
        e.name = "  ".to_owned();
        let p = e.pending();
        assert_eq!(p.name, DEFAULT_NAME);
        let params = p.preview_params();
        assert_eq!(params["delimiter"], "\t");
        assert_eq!(params["decimal_comma"], true);
        // A cell with a quote stays one cell.
        assert_eq!(quoted("a\"b"), "\"a\"\"b\"");
    }
}
