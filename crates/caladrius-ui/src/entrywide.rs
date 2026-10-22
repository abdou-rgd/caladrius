//! A "wide" table pasted into the typed table (T-108): the times down the first column, one column
//! per subject with the subject's name in the header, a concentration in each cell, as a course
//! handout or a report prints a profile of several subjects. The typed table holds one row per
//! sample, so the paste is turned around here, cell for cell, before anything reaches the engine:
//! `data.preview` and `data.import` see the same long table as if it had been typed. Like the rest
//! of the table, nothing here reads a number: a cell is copied as written ("0,5", "< LQ"), and the
//! import says what it makes of it.

use crate::entry::EntryRow;

/// What a wide paste gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wide {
    /// The subjects, in the order of the header.
    pub subjects: Vec<String>,
    /// The number of time rows.
    pub times: usize,
    /// One row per filled cell, subject by subject, times in the order of the paste; the dose of a
    /// subject, when the paste has a dose row, on the subject's first row.
    pub rows: Vec<EntryRow>,
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

    let mut rows = Vec::new();
    for (j, subject) in subjects.iter().enumerate() {
        let mut first = true;
        for line in &times {
            let concentration = cell(line, j + 1);
            if concentration.is_empty() {
                continue;
            }
            let dose_cell = match (&dose, first) {
                (Some((d, _)), true) => cell(d, j + 1).to_owned(),
                _ => String::new(),
            };
            rows.push(EntryRow {
                subject: subject.clone(),
                time: cell(line, 0).to_owned(),
                concentration: concentration.to_owned(),
                dose: dose_cell,
            });
            first = false;
        }
    }
    Some(Wide {
        subjects,
        times: times.len(),
        rows,
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

    fn triples(w: &Wide) -> Vec<(&str, &str, &str, &str)> {
        w.rows
            .iter()
            .map(|r| {
                (
                    r.subject.as_str(),
                    r.time.as_str(),
                    r.concentration.as_str(),
                    r.dose.as_str(),
                )
            })
            .collect()
    }

    #[test]
    fn a_wide_table_turns_into_one_row_per_sample_subject_by_subject() {
        let w = read(&lines(
            "Temps (h)\tA\tB\tC\n0,5\t4,1\t3,9\t5,2\n\n2\t2,6\t\t3,3\n8\t< LQ\t0,8\tBLQ\n",
        ))
        .unwrap();
        assert_eq!(w.subjects, ["A", "B", "C"]);
        assert_eq!(w.times, 3);
        assert_eq!(w.time_unit.as_deref(), Some("h"));
        assert!(!w.dose_row);
        assert_eq!(
            triples(&w),
            [
                ("A", "0,5", "4,1", ""),
                ("A", "2", "2,6", ""),
                ("A", "8", "< LQ", ""),
                ("B", "0,5", "3,9", ""),
                ("B", "8", "0,8", ""),
                ("C", "0,5", "5,2", ""),
                ("C", "2", "3,3", ""),
                ("C", "8", "BLQ", ""),
            ]
        );
    }

    #[test]
    fn the_time_header_may_be_time_t_empty_or_a_unit_alone_under_a_title() {
        for header in ["Time", "t (min)", "TEMPS [h]", "", "  "] {
            let w = read(&lines(&format!("{header}\t1\t2\n0\t1\t2\n1\t3\t4")));
            assert_eq!(w.map(|w| w.rows.len()), Some(4), "{header:?}");
        }
        // A title copied above the table, and a header cell broken over two lines by a word
        // processor: "Temps" is a title line, "(h)" a header with the unit only.
        let w = read(&lines(
            "Plasma concentrations (mg/L)\nTemps\n(h)\t1\t2\n0,5\t9\t8\n1\t7\t6\nValues below the limit are excluded.",
        ))
        .unwrap();
        assert_eq!((w.titles, w.times), (2, 2));
        assert_eq!(w.time_unit.as_deref(), Some("h"));
        assert_eq!(w.subjects, ["1", "2"]);
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
            triples(&w),
            [
                ("S1", "1", "5", "30"),
                ("S1", "4", "3", ""),
                ("S2", "0", "0", "35"),
                ("S2", "1", "6", ""),
                ("S2", "4", "4", ""),
            ]
        );
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
