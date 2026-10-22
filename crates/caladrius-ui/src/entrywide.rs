//! A "wide" table (T-108): the times down the first column, one column per subject with the
//! subject's name in the header, a concentration in each cell, as a course handout or a report
//! prints a profile of several subjects. It reaches the typed table two ways: pasted (detected by
//! [`read`]) or typed in the grid "Subjects in columns". Either way [`WideGrid::long_rows`] turns
//! it into one row per sample before anything reaches the engine, so `data.preview` and
//! `data.import` see the same long table as if it had been typed. Like the rest of the table,
//! nothing here reads a number: a cell is copied as written ("0,5", "< LQ"), and the import says
//! what it makes of it.

use serde::{Deserialize, Serialize};

use crate::entry::EntryRow;

/// One time of the grid and the concentration of each subject at that time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridRow {
    pub time: String,
    /// One cell per subject, in the order of [`WideGrid::subjects`].
    pub values: Vec<String>,
}

/// Times in rows, subjects in columns, an optional dose per subject.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WideGrid {
    /// The subjects' names, the column headers.
    pub subjects: Vec<String>,
    /// The dose of each subject, empty when not given.
    pub doses: Vec<String>,
    pub rows: Vec<GridRow>,
}

impl WideGrid {
    /// `subjects` subjects named 1, 2, 3… and `times` empty times.
    pub fn new(subjects: usize, times: usize) -> WideGrid {
        let mut g = WideGrid::default();
        g.set_subject_count(subjects);
        for _ in 0..times {
            g.add_row();
        }
        g
    }

    /// The name of subject `j`: its header, or its column number when the header is empty.
    pub fn subject_name(&self, j: usize) -> String {
        match self.subjects.get(j).map(|s| s.trim()) {
            Some(name) if !name.is_empty() => name.to_owned(),
            _ => (j + 1).to_string(),
        }
    }

    /// Adds subjects named by their column number, or removes the last ones and their cells.
    pub fn set_subject_count(&mut self, n: usize) {
        while self.subjects.len() < n {
            let next = self.subjects.len() + 1;
            self.subjects.push(next.to_string());
        }
        self.subjects.truncate(n);
        self.doses.resize(n, String::new());
        for r in &mut self.rows {
            r.values.resize(n, String::new());
        }
    }

    pub fn add_row(&mut self) {
        self.rows.push(GridRow {
            time: String::new(),
            values: vec![String::new(); self.subjects.len()],
        });
    }

    pub fn remove_row(&mut self, index: usize) {
        if index < self.rows.len() {
            self.rows.remove(index);
        }
        if self.rows.is_empty() {
            self.add_row();
        }
    }

    /// The cell at time row `i`, column `c`: 0 is the time, `j + 1` the subject `j`.
    pub fn cell_mut(&mut self, i: usize, c: usize) -> Option<&mut String> {
        let row = self.rows.get_mut(i)?;
        match c.checked_sub(1) {
            None => Some(&mut row.time),
            Some(j) => row.values.get_mut(j),
        }
    }

    fn row_is_blank(row: &GridRow) -> bool {
        row.time.trim().is_empty() && row.values.iter().all(|v| v.trim().is_empty())
    }

    /// The row after the last one with something typed.
    pub fn first_free_row(&self) -> usize {
        self.rows
            .iter()
            .rposition(|r| !Self::row_is_blank(r))
            .map_or(0, |last| last + 1)
    }

    /// The number of time rows with something typed.
    pub fn filled_times(&self) -> usize {
        self.rows.iter().filter(|r| !Self::row_is_blank(r)).count()
    }

    /// One row per filled cell, subject by subject, times in the order of the grid; a subject's
    /// dose on its first row, where the worksheet takes it. This is the only place a wide table
    /// becomes a long one.
    pub fn long_rows(&self) -> Vec<EntryRow> {
        let mut out = Vec::new();
        for j in 0..self.subjects.len() {
            let subject = self.subject_name(j);
            let mut first = true;
            for row in &self.rows {
                let concentration = row.values.get(j).map_or("", |v| v.trim());
                if concentration.is_empty() {
                    continue;
                }
                let dose = if first {
                    self.doses.get(j).map_or("", |d| d.trim()).to_owned()
                } else {
                    String::new()
                };
                out.push(EntryRow {
                    subject: subject.clone(),
                    time: row.time.trim().to_owned(),
                    concentration: concentration.to_owned(),
                    dose,
                });
                first = false;
            }
        }
        out
    }

    /// The grid of a long table, when nothing would be lost: every row names its subject, no
    /// subject has two samples at the same time or two different doses. Times keep the order in
    /// which they first appear.
    pub fn from_long(rows: &[EntryRow]) -> Result<WideGrid, String> {
        let mut g = WideGrid::default();
        for r in rows {
            let subject = r.subject.trim();
            if subject.is_empty() {
                return Err(
                    "A row has no subject: give every row its subject before putting the subjects in columns."
                        .to_owned(),
                );
            }
            let j = match g.subjects.iter().position(|s| s == subject) {
                Some(j) => j,
                None => {
                    g.subjects.push(subject.to_owned());
                    g.set_subject_count(g.subjects.len());
                    g.subjects.len() - 1
                }
            };
            let time = r.time.trim();
            let i = match g.rows.iter().position(|row| row.time == time) {
                Some(i) => i,
                None => {
                    g.add_row();
                    let last = g.rows.len() - 1;
                    if let Some(t) = g.cell_mut(last, 0) {
                        *t = time.to_owned();
                    }
                    last
                }
            };
            match g.cell_mut(i, j + 1) {
                Some(cell) if cell.is_empty() => *cell = r.concentration.trim().to_owned(),
                _ => {
                    return Err(format!(
                        "Subject {subject} has two samples at time {time}: the table stays one row per sample."
                    ));
                }
            }
            let dose = r.dose.trim();
            if let Some(d) = g.doses.get_mut(j)
                && !dose.is_empty()
            {
                if d.is_empty() {
                    *d = dose.to_owned();
                } else if d != dose {
                    return Err(format!(
                        "Subject {subject} has two doses: the table stays one row per sample."
                    ));
                }
            }
        }
        Ok(g)
    }
}

/// What a wide paste gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wide {
    pub grid: WideGrid,
    /// The unit written in the time header, `Temps (h)`.
    pub time_unit: Option<String>,
    /// Whether the paste had a dose row, and the unit written in its first cell, `Dose (mg/kg)`.
    pub dose_row: bool,
    pub dose_unit: Option<String>,
    /// The lines above the header with at most one cell (a title copied with the table), left out.
    pub titles: usize,
}

/// The names a time column goes by, compared in lower case once the unit is taken off. An empty
/// header cell, the corner of the table, also says time.
const TIME_NAMES: [&str; 4] = ["", "t", "time", "temps"];

/// The names of the other columns of a long table: a header holding one of them is a long table
/// (`Time  Concentration  Dose`), not a row of subject names. Single letters are left out: `A`,
/// `B`, `C` name subjects more often than columns.
const ROLE_NAMES: [&str; 11] = [
    "time",
    "temps",
    "subject",
    "sujet",
    "id",
    "cp",
    "conc",
    "concentration",
    "dose",
    "amount",
    "quantité",
];

/// A header cell split into its name and the unit written after it: `Temps (h)` is `temps` and
/// `h`, `Dose [mg/kg]` is `dose` and `mg/kg`, `(h)` alone is an empty name and `h`.
fn name_and_unit(cell: &str) -> (String, Option<String>) {
    let t = cell.trim();
    for (open, close) in [('(', ')'), ('[', ']')] {
        if let Some(inner) = t.strip_suffix(close)
            && let Some(at) = inner.rfind(open)
        {
            let name = inner.get(..at).unwrap_or_default().trim().to_lowercase();
            let unit = inner.get(at + open.len_utf8()..).unwrap_or_default().trim();
            return (name, (!unit.is_empty()).then(|| unit.to_owned()));
        }
    }
    (t.to_lowercase(), None)
}

fn filled_cells(line: &[String]) -> usize {
    line.iter().filter(|c| !c.trim().is_empty()).count()
}

/// The cell of a line, empty past its end.
fn cell(line: &[String], j: usize) -> &str {
    line.get(j).map_or("", |c| c.trim())
}

/// Reads `lines` (the cells of the pasted lines) as a wide table, or says it is not one with
/// `None`, and the paste is then spread over the table as it comes.
///
/// A wide table is: a header line whose first cell is empty or names the time (`Temps (h)`,
/// `Time`, `t`), followed by at least two subject names that are not the name of a long table's
/// column; then lines whose first cell holds a digit (the time) or names the dose. Blank lines are
/// skipped, and so are lines with at most one cell above the header (a title) or below it (a time
/// with no value, a footnote).
pub fn read(lines: &[Vec<String>]) -> Option<Wide> {
    let mut lines = lines.iter().filter(|l| filled_cells(l) > 0).peekable();
    let mut titles = 0;
    while lines.peek().is_some_and(|l| filled_cells(l) <= 1) {
        lines.next();
        titles += 1;
    }
    let header = lines.next()?;
    let width = header
        .iter()
        .rposition(|c| !c.trim().is_empty())
        .map_or(0, |last| last + 1);
    let (time_name, time_unit) = name_and_unit(cell(header, 0));
    if !TIME_NAMES.contains(&time_name.as_str()) {
        return None;
    }
    let subjects: Vec<String> = header
        .get(1..width)
        .unwrap_or_default()
        .iter()
        .map(|c| c.trim().to_owned())
        .collect();
    let named = |s: &String| !s.is_empty() && !ROLE_NAMES.contains(&name_and_unit(s).0.as_str());
    if subjects.len() < 2 || !subjects.iter().all(named) {
        return None;
    }

    let mut times: Vec<&Vec<String>> = Vec::new();
    let mut dose: Option<(&Vec<String>, Option<String>)> = None;
    for line in lines.filter(|l| filled_cells(l) > 1) {
        if line.iter().skip(width).any(|c| !c.trim().is_empty()) {
            return None;
        }
        let first = cell(line, 0);
        let (name, unit) = name_and_unit(first);
        if name == "dose" && dose.is_none() {
            dose = Some((line, unit));
        } else if first.chars().any(|ch| ch.is_ascii_digit()) {
            times.push(line);
        } else {
            return None;
        }
    }
    if times.is_empty() {
        return None;
    }

    let grid = WideGrid {
        doses: (0..subjects.len())
            .map(|j| dose.as_ref().map_or("", |(d, _)| cell(d, j + 1)).to_owned())
            .collect(),
        rows: times
            .iter()
            .map(|line| GridRow {
                time: cell(line, 0).to_owned(),
                values: (0..subjects.len())
                    .map(|j| cell(line, j + 1).to_owned())
                    .collect(),
            })
            .collect(),
        subjects,
    };
    Some(Wide {
        grid,
        time_unit,
        dose_row: dose.is_some(),
        dose_unit: dose.and_then(|(_, unit)| unit),
        titles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(|l| l.split('\t').map(|c| c.trim().to_owned()).collect())
            .collect()
    }

    /// The long rows as `subject|time|concentration|dose`.
    fn triples(g: &WideGrid) -> Vec<String> {
        g.long_rows()
            .into_iter()
            .map(|r| format!("{}|{}|{}|{}", r.subject, r.time, r.concentration, r.dose))
            .collect()
    }

    #[test]
    fn a_wide_table_turns_into_one_row_per_sample_subject_by_subject() {
        let w = read(&lines(
            "Temps (h)\tA\tB\tC\n0,5\t4,1\t3,9\t5,2\n\n2\t2,6\t\t3,3\n8\t< LQ\t0,8\tBLQ\n",
        ))
        .unwrap();
        assert_eq!(w.grid.subjects, ["A", "B", "C"]);
        assert_eq!(w.grid.rows.len(), 3);
        assert_eq!(w.time_unit.as_deref(), Some("h"));
        assert!(!w.dose_row);
        assert_eq!(
            triples(&w.grid),
            [
                "A|0,5|4,1|",
                "A|2|2,6|",
                "A|8|< LQ|",
                "B|0,5|3,9|",
                "B|8|0,8|",
                "C|0,5|5,2|",
                "C|2|3,3|",
                "C|8|BLQ|",
            ]
        );
    }

    #[test]
    fn the_time_header_may_be_time_t_empty_or_a_unit_alone_under_a_title() {
        for header in ["Time", "t (min)", "TEMPS [h]", "", "  "] {
            let w = read(&lines(&format!("{header}\t1\t2\n0\t1\t2\n1\t3\t4")));
            assert_eq!(w.map(|w| w.grid.long_rows().len()), Some(4), "{header:?}");
        }
        // A title copied above the table, and a header cell broken over two lines by a word
        // processor: "Temps" is a title line, "(h)" a header with the unit only.
        let w = read(&lines(
            "Plasma concentrations (mg/L)\nTemps\n(h)\t1\t2\n0,5\t9\t8\n1\t7\t6\nValues below the limit are excluded.",
        ))
        .unwrap();
        assert_eq!((w.titles, w.grid.rows.len()), (2, 2));
        assert_eq!(w.time_unit.as_deref(), Some("h"));
        assert_eq!(w.grid.subjects, ["1", "2"]);
    }

    #[test]
    fn a_dose_row_gives_each_subject_its_dose_on_its_first_row() {
        let w = read(&lines(
            "Time (h)\tS1\tS2\nDose (mg/kg)\t30\t35\n0\t\t0\n1\t5\t6\n4\t3\t4",
        ))
        .unwrap();
        assert!(w.dose_row);
        assert_eq!(w.dose_unit.as_deref(), Some("mg/kg"));
        assert_eq!(
            triples(&w.grid),
            ["S1|1|5|30", "S1|4|3|", "S2|0|0|35", "S2|1|6|", "S2|4|4|",]
        );
    }

    #[test]
    fn a_long_table_becomes_a_grid_only_when_nothing_is_lost() {
        let row = |s: &str, t: &str, c: &str, d: &str| EntryRow {
            subject: s.to_owned(),
            time: t.to_owned(),
            concentration: c.to_owned(),
            dose: d.to_owned(),
        };
        let long = [
            row("A", "0", "0", "30"),
            row("A", "1", "5", ""),
            row("B", "1", "6", "35"),
            row("B", "2", "4", ""),
        ];
        let g = WideGrid::from_long(&long).unwrap();
        assert_eq!(g.subjects, ["A", "B"]);
        assert_eq!(g.doses, ["30", "35"]);
        let times: Vec<&str> = g.rows.iter().map(|r| r.time.as_str()).collect();
        assert_eq!(times, ["0", "1", "2"]);
        // Back to one row per sample: the same rows.
        assert_eq!(g.long_rows(), long);
        // Two samples at one time, two doses, a row without subject: refused.
        let twice = [row("A", "1", "5", ""), row("A", "1", "6", "")];
        assert!(
            WideGrid::from_long(&twice)
                .unwrap_err()
                .contains("two samples")
        );
        let doses = [row("A", "0", "1", "30"), row("A", "1", "2", "40")];
        assert!(
            WideGrid::from_long(&doses)
                .unwrap_err()
                .contains("two doses")
        );
        assert!(WideGrid::from_long(&[row("", "0", "1", "")]).is_err());
    }

    #[test]
    fn the_subject_count_adds_and_removes_columns() {
        let mut g = WideGrid::new(3, 2);
        assert_eq!(g.subjects, ["1", "2", "3"]);
        *g.cell_mut(0, 0).unwrap() = "0,5".to_owned();
        *g.cell_mut(0, 3).unwrap() = "7".to_owned();
        g.subjects[1].clear();
        assert_eq!(g.subject_name(1), "2");
        g.set_subject_count(5);
        assert_eq!(g.rows[1].values.len(), 5);
        assert_eq!(g.subjects[4], "5");
        g.set_subject_count(2);
        assert_eq!((g.doses.len(), g.rows[0].values.len()), (2, 2));
        assert!(g.long_rows().is_empty());
        assert_eq!(g.first_free_row(), 1);
        assert!(g.cell_mut(0, 3).is_none());
    }

    #[test]
    fn other_tables_are_not_wide() {
        for text in [
            // No time header: the first cell names something else, or the header is missing.
            "Visit\tS1\tS2\n0\t1\t2",
            "0,5\t85\t87\n1\t75\t81",
            // One subject column only.
            "Time\tS1\n0\t1\n1\t2",
            // A long table: its header names the other columns.
            "Time (h)\tConcentration (mg/L)\tDose (mg)\n0\t0\t100\n1\t3\t",
            "Temps\tConc\tSujet\n0\t1\tA",
            "t\tSubject\tConc\n0\tA\t1",
            // A gap in the header, a cell past the header, a line that is neither time nor dose.
            "Time\tS1\t\tS3\n0\t1\t2\t3",
            "Time\tS1\tS2\n0\t1\t2\t3",
            "Time\tS1\tS2\nweight\t3\t4\n0\t1\t2",
            // A header and nothing under it.
            "Time\tS1\tS2\n",
            "",
        ] {
            assert_eq!(read(&lines(text)), None, "{text:?}");
        }
    }
}
