//! Typing or pasting a small data set instead of opening a CSV file. The table holds the text as
//! typed; "Create worksheet" turns it into tab-separated text and sends it through the same
//! commands as a file (`data.preview`, then `data.import`), so the checks, the messages and the
//! worksheet are those of an import. Nothing here reads a number.

use egui::{Id, RichText, Ui};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::Action;
use crate::entrywide::{self, Wide, WideGrid};
use crate::import::{PendingImport, ReadOptions};
use crate::theme::Tokens;
use crate::unitpick::{self, Accepted};

/// The name a worksheet gets when the name field is left empty.
pub const DEFAULT_NAME: &str = "Typed data";

/// The empty rows a new table starts with (a small profile fits without adding any).
const START_ROWS: usize = 12;

/// The subjects a grid starts with when "Several subjects" is ticked on an empty table.
const START_SUBJECTS: usize = 3;

/// The most subjects the grid offers.
const MAX_SUBJECTS: usize = 200;

/// How the table of several subjects is laid out on screen. Both give the same long table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// One row per sample: subject, time, concentration, dose.
    #[default]
    Long,
    /// Times in rows, subjects in columns ([`WideGrid`]).
    SubjectsInColumns,
}

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
    /// The dose given to every subject without one ("Same dose for every subject").
    #[serde(default)]
    pub common_dose: String,
    /// With several subjects: one row per sample, or subjects in columns.
    #[serde(default)]
    pub layout: Layout,
    /// The table with subjects in columns, used when [`DataEntry::in_columns`].
    #[serde(default)]
    pub grid: WideGrid,
    /// The grid cell that had the keyboard focus (time row, column; column 0 is the time).
    #[serde(skip)]
    pub grid_focus: Option<(usize, usize)>,
    /// The cell that had the keyboard focus at the last frame, where a paste lands.
    #[serde(skip)]
    pub focus: Option<(usize, Field)>,
    /// The checks of the last "Create worksheet", for the table as it was then.
    #[serde(skip)]
    pub review: Option<PendingImport>,
    /// What the last paste did.
    #[serde(skip)]
    pub note: Option<String>,
    /// The engine's checks of the units (`units.check`): the time, concentration and dose units
    /// they were made on, and their messages.
    #[serde(skip)]
    pub unit_checks: Option<([String; 3], Vec<String>)>,
    /// Whether a unit cell had the keyboard focus at the last frame: the units are checked once
    /// the person leaves the cell.
    #[serde(skip)]
    pub unit_focus: bool,
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
            common_dose: String::new(),
            layout: Layout::Long,
            grid: WideGrid::default(),
            grid_focus: None,
            focus: None,
            review: None,
            note: None,
            unit_checks: None,
            unit_focus: false,
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

    /// Whether the table on screen is the grid with subjects in columns.
    pub fn in_columns(&self) -> bool {
        self.subjects && self.layout == Layout::SubjectsInColumns
    }

    /// The rows with something typed in a column on screen, one per sample: the grid's turned
    /// into one row per sample when the subjects are in columns.
    pub fn filled(&self) -> Vec<EntryRow> {
        if self.in_columns() {
            return self.grid.long_rows();
        }
        let fields = self.fields();
        self.rows
            .iter()
            .filter(|r| !r.is_blank(&fields))
            .cloned()
            .collect()
    }

    /// Ticks or unticks "Several subjects". Ticked on an empty table, the subjects go in columns;
    /// unticked, a grid goes back to one row per sample.
    pub fn set_subjects(&mut self, on: bool) {
        if on && !self.subjects && self.filled().is_empty() {
            self.layout = Layout::SubjectsInColumns;
            if self.grid.subjects.is_empty() {
                self.grid = WideGrid::new(START_SUBJECTS, START_ROWS);
            }
        }
        if !on && self.in_columns() {
            self.set_layout(Layout::Long);
        }
        self.subjects = on;
    }

    /// Changes the layout, carrying what is typed over; a long table the grid cannot hold without
    /// losing a cell stays as it is, and the note says why.
    pub fn set_layout(&mut self, layout: Layout) {
        if layout == self.layout {
            return;
        }
        match layout {
            Layout::Long => {
                let mut rows = self.grid.long_rows();
                while rows.len() < START_ROWS {
                    rows.push(EntryRow::default());
                }
                self.rows = rows;
                self.layout = Layout::Long;
            }
            Layout::SubjectsInColumns => {
                let filled = self.filled();
                let grid = if filled.is_empty() {
                    Ok(WideGrid::new(START_SUBJECTS, START_ROWS))
                } else {
                    WideGrid::from_long(&filled)
                };
                match grid {
                    Ok(mut grid) => {
                        while grid.rows.len() < START_ROWS {
                            grid.add_row();
                        }
                        self.grid = grid;
                        self.layout = Layout::SubjectsInColumns;
                    }
                    Err(why) => self.note = Some(why),
                }
            }
        }
        self.focus = None;
        self.grid_focus = None;
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
        if self.in_columns() {
            self.grid.add_row();
        } else {
            self.rows.push(EntryRow::default());
        }
    }

    pub fn remove_row(&mut self, index: usize) {
        if self.in_columns() {
            self.grid.remove_row(index);
            self.grid_focus = None;
            return;
        }
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
        if self.in_columns() {
            self.grid = WideGrid::new(self.grid.subjects.len().max(1), START_ROWS);
        }
        self.grid_focus = None;
        self.focus = None;
        self.review = None;
        self.note = None;
    }

    /// Spreads a paste of several cells (rows on lines, cells separated by tabs, as a spreadsheet
    /// copies them) over the table from `at`, or from the first empty row when no cell has the
    /// focus. Rows are added as needed. A first line without a digit is a header and is left out.
    /// A wide table (times down, one column per subject) is turned into one row per sample first
    /// ([`entrywide`]). Returns what was done, in one sentence or a few.
    pub fn paste(&mut self, at: Option<(usize, Field)>, text: &str) -> String {
        let mut lines = split_paste(text);
        if let Some(wide) = entrywide::read(&lines) {
            return self.paste_wide(at.map(|(row, _)| row), wide);
        }
        let fields = self.fields();
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
        if self.in_columns() {
            return self.paste_in_grid(&lines, header);
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

    /// Spreads pasted cells over the grid from the cell in focus, or from the time column of the
    /// first empty time row: lines go down the times, cells across the subjects.
    fn paste_in_grid(&mut self, lines: &[Vec<String>], header: bool) -> String {
        let (start_row, start_col) = self
            .grid_focus
            .unwrap_or_else(|| (self.grid.first_free_row(), 0));
        let mut dropped = 0;
        for (i, cells) in lines.iter().enumerate() {
            while self.grid.rows.len() <= start_row + i {
                self.grid.add_row();
            }
            for (j, cell) in cells.iter().enumerate() {
                match self.grid.cell_mut(start_row + i, start_col + j) {
                    Some(target) => target.clone_from(cell),
                    None => dropped += 1,
                }
            }
        }
        let n = lines.len();
        let mut said = format!(
            "Pasted {n} line{} from time row {}.",
            if n == 1 { "" } else { "s" },
            start_row + 1
        );
        if header {
            said.push_str(" The first line had no number and was read as a header, left out.");
        }
        if dropped > 0 {
            said.push_str(&format!(
                " {dropped} cell{} beyond the last subject {} left out.",
                if dropped == 1 { "" } else { "s" },
                if dropped == 1 { "was" } else { "were" }
            ));
        }
        said
    }

    /// A pasted wide table: it becomes the grid (subjects in columns), unless one row per sample
    /// is on screen with rows already typed, where its rows go under them (from row `at`, or under
    /// the last filled row). Shows the subjects, and takes the units written in the time header
    /// and the dose line.
    fn paste_wide(&mut self, at: Option<usize>, wide: Wide) -> String {
        let switched = !self.subjects;
        let into_rows = self.subjects && !self.in_columns() && !self.filled().is_empty();
        self.subjects = true;
        let subjects = wide.grid.subjects.len();
        let times = wide.grid.rows.len();
        let rows = wide.grid.long_rows();
        let n = rows.len();
        let mut said = format!(
            "Pasted {subjects} subjects × {times} time{}",
            if times == 1 { "" } else { "s" }
        );
        if into_rows {
            let fields = self.fields();
            let start = at.unwrap_or_else(|| {
                self.rows
                    .iter()
                    .rposition(|r| !r.is_blank(&fields))
                    .map_or(0, |last| last + 1)
            });
            for (i, row) in rows.into_iter().enumerate() {
                while self.rows.len() <= start + i {
                    self.rows.push(EntryRow::default());
                }
                if let Some(r) = self.rows.get_mut(start + i) {
                    *r = row;
                }
            }
            said.push_str(&format!(
                " as {n} rows from row {}, one per sample.",
                start + 1
            ));
        } else {
            let mut grid = wide.grid;
            while grid.rows.len() < START_ROWS {
                grid.add_row();
            }
            self.grid = grid;
            self.layout = Layout::SubjectsInColumns;
            self.grid_focus = None;
            said.push_str(&format!(", subjects in columns ({n} samples)."));
        }
        if let Some(unit) = wide.time_unit {
            self.time_unit = unit;
        }
        if let Some(unit) = wide.dose_unit {
            self.dose_unit = unit;
        }
        if switched {
            said.push_str(" \"Several subjects\" is now on.");
        }
        let empty = subjects * times - n;
        if empty > 0 {
            said.push_str(&format!(
                " {empty} empty cell{} left out.",
                if empty == 1 { " was" } else { "s were" }
            ));
        }
        if wide.titles > 0 {
            said.push_str(&format!(
                " {} line{} above the table left out.",
                wide.titles,
                if wide.titles == 1 { " was" } else { "s were" }
            ));
        }
        if wide.dose_row {
            said.push_str(" Each subject's dose comes from the Dose line.");
        } else {
            said.push_str(" No Dose line: type each subject's dose, or give the same dose to every subject below.");
        }
        said
    }

    /// Gives [`DataEntry::common_dose`] to every subject that has no dose yet, on the subject's
    /// first filled row (the worksheet takes a subject's dose from there). A subject with a dose
    /// keeps it.
    pub fn fill_dose(&mut self) -> String {
        let dose = self.common_dose.trim().to_owned();
        if dose.is_empty() {
            return "Type the dose to give first.".to_owned();
        }
        if self.in_columns() {
            let mut given = 0;
            for j in 0..self.grid.subjects.len() {
                let sampled = self
                    .grid
                    .rows
                    .iter()
                    .any(|r| r.values.get(j).is_some_and(|v| !v.trim().is_empty()));
                if let Some(d) = self.grid.doses.get_mut(j)
                    && sampled
                    && d.trim().is_empty()
                {
                    d.clone_from(&dose);
                    given += 1;
                }
            }
            return dose_note(&dose, given);
        }
        let fields = self.fields();
        let subject = |r: &EntryRow| {
            if self.subjects {
                r.subject.trim().to_owned()
            } else {
                String::new()
            }
        };
        let mut first_rows: Vec<(String, usize)> = Vec::new();
        let mut dosed: Vec<String> = Vec::new();
        for (i, r) in self.rows.iter().enumerate() {
            if r.is_blank(&fields) {
                continue;
            }
            let s = subject(r);
            if !r.dose.trim().is_empty() {
                dosed.push(s.clone());
            }
            if !first_rows.iter().any(|(k, _)| *k == s) {
                first_rows.push((s, i));
            }
        }
        let mut given = 0;
        for (s, i) in first_rows {
            if dosed.contains(&s) {
                continue;
            }
            if let Some(r) = self.rows.get_mut(i) {
                r.dose.clone_from(&dose);
                given += 1;
            }
        }
        dose_note(&dose, given)
    }

    /// The units to check, as typed.
    pub fn units(&self) -> [String; 3] {
        [
            self.time_unit.trim().to_owned(),
            self.concentration_unit.trim().to_owned(),
            self.dose_unit.trim().to_owned(),
        ]
    }
}

/// What "Same dose for every subject" did.
fn dose_note(dose: &str, given: usize) -> String {
    if given == 0 {
        "Every subject already has a dose: nothing changed.".to_owned()
    } else {
        format!(
            "Dose {dose} given to {given} subject{} that had none.",
            if given == 1 { "" } else { "s" }
        )
    }
}

/// The cells of a paste, line by line. With tabs, as a spreadsheet copies cells: a cell in double
/// quotes may hold a tab or a line break (read as a space), and a blank line has no cell. Without
/// tabs, the cells of a line are separated by spaces.
fn split_paste(text: &str) -> Vec<Vec<String>> {
    if !text.contains('\t') {
        return text
            .lines()
            .map(|line| split_cells(line.trim_end_matches('\r')))
            .collect();
    }
    let mut lines = Vec::new();
    let mut line: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let end_line = |line: &mut Vec<String>, lines: &mut Vec<Vec<String>>| {
        if line.iter().all(|c| c.is_empty()) {
            line.clear();
        }
        lines.push(std::mem::take(line));
    };
    while let Some(ch) = chars.next() {
        if quoted {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    cell.push('"');
                }
                '"' => quoted = false,
                '\r' => {}
                '\n' | '\t' => cell.push(' '),
                _ => cell.push(ch),
            }
            continue;
        }
        match ch {
            '"' if cell.trim().is_empty() => {
                cell.clear();
                quoted = true;
            }
            '\t' => line.push(std::mem::take(&mut cell).trim().to_owned()),
            '\r' => {}
            '\n' => {
                line.push(std::mem::take(&mut cell).trim().to_owned());
                end_line(&mut line, &mut lines);
            }
            _ => cell.push(ch),
        }
    }
    if !cell.is_empty() || !line.is_empty() {
        line.push(cell.trim().to_owned());
        end_line(&mut line, &mut lines);
    }
    lines
}

/// The cells of one pasted line without tabs: runs of spaces separate them, except after a lone
/// `<`, which belongs to the next word (`< LQ`, `< 0,45`, as a copied column writes them).
fn split_cells(line: &str) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    for word in line.split_whitespace() {
        match cells.last_mut() {
            Some(last) if last == "<" => {
                last.push(' ');
                last.push_str(word);
            }
            _ => cells.push(word.to_owned()),
        }
    }
    cells
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
pub fn screen(
    ui: &mut Ui,
    tokens: &Tokens,
    entry: &mut DataEntry,
    accepted: &Accepted,
    actions: &mut Vec<Action>,
) {
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
        RichText::new("Copy the cells in a spreadsheet and press Ctrl+V in the table to fill it at once. Several subjects go in columns, the times shared by all, or one row per sample; a table copied with one column per subject is recognised.")
            .color(c.text_muted.color()),
    );
    ui.add_space(tokens.spacing.medium);
    ui.horizontal(|ui| {
        ui.label("Worksheet name");
        ui.add(
            egui::TextEdit::singleline(&mut entry.name).desired_width(tokens.size.text_box_width),
        );
        ui.add_space(tokens.spacing.large);
        let mut several = entry.subjects;
        if ui.checkbox(&mut several, "Several subjects").changed() {
            entry.set_subjects(several);
        }
        if entry.subjects {
            ui.add_space(tokens.spacing.medium);
            let mut layout = entry.layout;
            ui.selectable_value(
                &mut layout,
                Layout::SubjectsInColumns,
                "Subjects in columns",
            )
            .on_hover_text("Times in rows, one column per subject: the times are typed once.");
            ui.selectable_value(&mut layout, Layout::Long, "One row per sample")
                .on_hover_text("Subject, time, concentration and dose on every row.");
            entry.set_layout(layout);
        }
        if entry.in_columns() {
            ui.add_space(tokens.spacing.medium);
            ui.label("Number of subjects");
            let mut n = entry.grid.subjects.len();
            let changed = ui
                .add(
                    egui::DragValue::new(&mut n)
                        .range(1..=MAX_SUBJECTS)
                        .update_while_editing(false),
                )
                .changed();
            if changed {
                entry.grid.set_subject_count(n);
            }
        }
    });
    ui.add_space(tokens.spacing.medium);
    // The engine's checks of the units, made once the person left the unit cell.
    if let Some((_, messages)) = &entry.unit_checks {
        for m in messages {
            ui.label(RichText::new(format!("Check the units: {m}.")).color(c.warning.color()));
        }
        if !messages.is_empty() {
            ui.label(
                RichText::new(
                    "The \u{23f7} button next to each unit lists the units Caladrius recognises.",
                )
                .small()
                .color(c.text_muted.color()),
            );
        }
    }

    if entry.in_columns() {
        entry.unit_focus = crate::entrygrid::table(ui, tokens, entry, accepted);
    } else {
        long_table(ui, tokens, entry, accepted);
    }
    controls(ui, tokens, entry, actions);
}

/// The table with one row per sample.
fn long_table(ui: &mut Ui, tokens: &Tokens, entry: &mut DataEntry, accepted: &Accepted) {
    let c = &tokens.colors;
    let fields = entry.fields();
    let mut remove = None;
    let mut focus = None;
    let mut unit_focus = false;
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
                        let (response, _) = unitpick::field(
                            ui,
                            tokens,
                            Id::new(("entry-unit", f.role())),
                            unit,
                            f.role(),
                            accepted,
                            cell[0],
                        );
                        unit_focus |= response.has_focus();
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
    entry.unit_focus = unit_focus;
    if let Some(i) = remove {
        entry.remove_row(i);
    }
}

/// The buttons under the table, the common dose, the notes, the checks and "Create worksheet".
fn controls(ui: &mut Ui, tokens: &Tokens, entry: &mut DataEntry, actions: &mut Vec<Action>) {
    let c = &tokens.colors;
    ui.horizontal(|ui| {
        if ui.button("Add a row").clicked() {
            entry.add_row();
        }
        if ui.button("Clear the table").clicked() {
            entry.clear();
        }
        let counted = if entry.in_columns() {
            format!(
                "{} times, {} samples",
                entry.grid.filled_times(),
                entry.filled().len()
            )
        } else {
            format!("{} filled rows", entry.filled().len())
        };
        ui.label(RichText::new(counted).small().color(c.text_muted.color()));
    });
    if entry.subjects {
        ui.horizontal(|ui| {
            ui.label("Same dose for every subject");
            ui.add(
                egui::TextEdit::singleline(&mut entry.common_dose)
                    .desired_width(tokens.size.cell_width),
            );
            ui.label(RichText::new(entry.dose_unit.trim()).color(c.text_muted.color()));
            if ui
                .button("Give it to the subjects without a dose")
                .on_hover_text(
                    "Writes this dose for each subject that has samples and no dose yet.",
                )
                .clicked()
            {
                entry.note = Some(entry.fill_dose());
            }
        });
    }
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
    fn a_value_below_the_limit_stays_one_cell_in_a_paste_without_tabs() {
        assert_eq!(split_cells("< LQ"), ["< LQ"]);
        assert_eq!(split_cells("24 < 0,45 BLQ"), ["24", "< 0,45", "BLQ"]);
        assert_eq!(split_cells("<LQ 3"), ["<LQ", "3"]);
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
