//! The population dataset: the records of a table, one list of doses and one of observations per
//! subject (`specs/data.md`, DAT-*).
//!
//! A table has two layouts. The profile layout is today's (no event column, every row an
//! observation, the dose column holding the dose of the subject); the event layout has an event
//! column and each row is a dose or an observation. Both give the same thing here: for each subject
//! its dose records and its observation records, counted from row 1 over the data rows.
//!
//! Nothing is corrected and nothing is dropped silently. A problem the spec refuses (DAT-ERR-01 to
//! 06) is reported as a check of the reading, one per code, with the first problem in full and the
//! count of the others; the subjects it touches are marked (`refusal`), and the other subjects keep
//! their records, so that they still analyse (OD-02). No row is sorted.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::csv::{ImportedTable, ReadingCheck, key};
use crate::worksheet::{Column, ColumnData, ColumnRole, SINGLE_SUBJECT};

/// How a table is laid out (DAT-LAY-01): decided by the presence of an event column and by nothing
/// else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// Today's layout: no event column, every row is an observation.
    #[default]
    Profile,
    /// One row per dose or observation, told apart by the event column.
    Event,
}

/// Whether a dose is instantaneous or given over a time (DAT-TYP-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoseKind {
    /// An instantaneous input.
    Bolus,
    /// An input over a duration, at a constant rate.
    Infusion,
}

/// One dose of a subject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoseRecord {
    /// Data row, counted from 1; `None` for the dose that the profile layout implies (DAT-LEG-01).
    pub row: Option<usize>,
    /// Time of the dose.
    pub time: f64,
    /// Amount, above 0.
    pub amount: f64,
    /// Rate of an infusion (amount per time); `None` for a bolus.
    pub rate: Option<f64>,
    /// Duration of an infusion; `None` for a bolus.
    pub duration: Option<f64>,
    /// Bolus or infusion.
    pub kind: DoseKind,
}

/// What an observation row holds (DAT-CEN-01, 02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    /// A measured value.
    Observed,
    /// No value, or a row marked not to be used (`mdv` = 1).
    Missing,
    /// Below the limit of quantification, or left-censored; the value is not kept.
    Blq,
}

/// One observation of a subject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationRecord {
    /// Data row, counted from 1.
    pub row: usize,
    /// Time of the observation; `None` when the row has no time (only in the profile layout, where
    /// this is not refused: the event layout refuses it).
    pub time: Option<f64>,
    /// Observed, missing or below the limit.
    pub status: ObservationStatus,
    /// The measured value when `status` is `observed`, else `None`.
    pub value: Option<f64>,
    /// The limit of quantification of a row below it, when the file gives it: the number after the
    /// `<` of the cell (`<0.5`), or the value cell of a row with `cens` = 1, which holds the limit
    /// (DAT-CEN-02). Absent otherwise. The value of such a row stays `None`: the row is left out of
    /// the analyses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// The `limit` column of a row with `cens` = 1: the other, lower bound of the censoring
    /// interval (DAT-CEN-02). Kept and unused. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_limit: Option<f64>,
}

/// The value of a declared covariate for one subject (DAT-COV-01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CovariateValue {
    /// A numeric covariate.
    Number(f64),
    /// A categorical covariate, as written.
    Text(String),
}

/// Everything the table says about one subject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubjectRecords {
    /// The subject label, as written (`1` when the table has no subject column).
    pub id: String,
    /// The number of data rows of the subject.
    pub n_rows: usize,
    /// The dose records, in row order.
    pub doses: Vec<DoseRecord>,
    /// The observation records, in row order. A row that is refused is not listed.
    pub observations: Vec<ObservationRecord>,
    /// Number of observation records.
    pub n_obs: usize,
    /// Of them, measured values.
    pub n_observed: usize,
    /// Of them, missing.
    pub n_missing: usize,
    /// Of them, below the limit of quantification or censored.
    pub n_blq: usize,
    /// The declared covariates: the first non-missing value of the subject, `null` when all its
    /// cells are missing.
    pub covariates: BTreeMap<String, Option<CovariateValue>>,
    /// The code of the first problem found for this subject: an analysis of it is refused with that
    /// reading check while the other subjects run. Absent when nothing is wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// The sentence of that problem for this subject, the one the reading's check gives for the
    /// first subject it names (DAT-ERR-08). Present exactly when `refusal` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_message: Option<String>,
}

/// The records of a table.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Dataset {
    /// Profile or event layout.
    pub layout: Layout,
    /// The subjects, in order of first appearance.
    pub subjects: Vec<SubjectRecords>,
}

/// What [`build`] finds in a table.
pub(crate) struct Built {
    /// The records.
    pub dataset: Dataset,
    /// The refusals of DAT-ERR-02 to 06, one per code, in order of appearance.
    pub checks: Vec<ReadingCheck>,
    /// In the event layout, for each row, whether it is an observation row; `None` in the profile
    /// layout, where every row is one. The time checks use it (a dose and an observation may share
    /// a time, DAT-ORD-01).
    pub observation_rows: Option<Vec<bool>>,
}

struct Problem {
    code: &'static str,
    subject: Option<usize>,
    message: String,
}

/// What is known of one subject while the rows are read.
#[derive(Default)]
struct Acc {
    n_rows: usize,
    doses: Vec<DoseRecord>,
    observations: Vec<ObservationRecord>,
    /// Rows classified as observations / doses by the event column.
    n_observation_rows: usize,
    n_dose_rows: usize,
    /// The distinct compartments of the observation rows, in order of appearance.
    compartments: Vec<String>,
    /// Profile layout: the first dose seen (row, amount) and the row of the first different one.
    first_dose: Option<(usize, f64)>,
    dose_conflict: Option<usize>,
    /// Per declared covariate: the first non-missing value and its row; whether it was reported.
    covariates: Vec<Option<(usize, CovariateValue)>>,
    covariate_reported: Vec<bool>,
    /// The previous time of any row and of any observation row.
    last_time: Option<f64>,
    last_observation_time: Option<f64>,
    /// The first time that goes back: (data row from 1, the time, the time before it).
    time_goes_back: Option<(usize, f64, f64)>,
    /// The first observation time that repeats: (data row from 1, the time).
    time_repeats: Option<(usize, f64)>,
    refusal: Option<String>,
    refusal_message: Option<String>,
}

/// The columns of the table that the records use.
struct Cols<'a> {
    time: Option<&'a Column>,
    conc: Option<&'a Column>,
    dose: Option<&'a Column>,
    event: Option<&'a Column>,
    /// A column whose name says it is the amount but that holds text (so it has no role).
    amount_text: Option<&'a Column>,
    rate: Option<&'a Column>,
    dur: Option<&'a Column>,
    cmt: Option<&'a Column>,
    mdv: Option<&'a Column>,
    cens: Option<&'a Column>,
    limit: Option<&'a Column>,
    /// The limits written after a `<` in the concentration column, by row.
    blq_limits: &'a [(usize, f64)],
    covariates: Vec<&'a Column>,
    /// Columns named `ii`, `addl` or `ss`, which have no role (DAT-REC-02).
    history: Vec<&'a Column>,
}

/// The type of a row of the event layout (DAT-TYP-01).
enum Kind {
    Observation,
    Dose,
    /// An event type of `rxode2` that is not supported: its text and its meaning.
    Unsupported(String, &'static str),
    /// Empty or not an event type: its text.
    Unknown(String),
}

/// True when the event cell of `row` says it is an observation (DAT-TYP-01); the notes of the
/// importer use it to find the cells that are ignored on observation rows.
pub(crate) fn is_observation_row(event: &Column, row: usize) -> bool {
    matches!(event_kind(event, row), Kind::Observation)
}

fn first_with(table: &ImportedTable, role: ColumnRole) -> Option<&Column> {
    table.columns.iter().find(|c| c.role == role)
}

/// The number written after the `<` of the concentration cell of `row` (`<0.5`), if any.
fn text_limit(cols: &Cols, row: usize) -> Option<f64> {
    // Sorted by row: they are collected in order.
    let at = cols
        .blq_limits
        .binary_search_by_key(&row, |(r, _)| *r)
        .ok()?;
    cols.blq_limits.get(at).map(|(_, limit)| *limit)
}

fn number_at(column: Option<&Column>, row: usize) -> Option<f64> {
    match column.map(|c| &c.data) {
        Some(ColumnData::Number(v)) => v.get(row).copied().flatten(),
        _ => None,
    }
}

fn name_of(column: Option<&Column>, default: &str) -> String {
    column.map_or_else(|| default.to_owned(), |c| c.name.clone())
}

fn event_kind(column: &Column, row: usize) -> Kind {
    let shown = column.data.text_at(row);
    let word = shown.trim().to_lowercase();
    let number = match &column.data {
        ColumnData::Number(v) => v.get(row).copied().flatten(),
        ColumnData::Text(_) => word.parse::<f64>().ok().filter(|x| x.is_finite()),
    };
    if let Some(x) = number {
        return if x == 0.0 {
            Kind::Observation
        } else if x == 1.0 {
            Kind::Dose
        } else if x == 2.0 {
            Kind::Unsupported(shown, "other")
        } else if x == 3.0 {
            Kind::Unsupported(shown, "reset")
        } else if x == 4.0 {
            Kind::Unsupported(shown, "reset and dose")
        } else {
            Kind::Unknown(shown)
        };
    }
    match word.as_str() {
        "obs" | "observation" => Kind::Observation,
        "dose" => Kind::Dose,
        _ => Kind::Unknown(shown),
    }
}

/// The text of a non-empty, non-zero cell of a history column, if the row has one.
fn history_value(column: &Column, row: usize) -> Option<String> {
    let shown = column.data.text_at(row);
    let t = shown.trim();
    let zero = match &column.data {
        ColumnData::Number(v) => v.get(row).copied().flatten().is_none_or(|x| x == 0.0),
        ColumnData::Text(_) => t.is_empty() || t == "0",
    };
    (!zero).then(|| t.to_owned())
}

fn covariate_cell(column: &Column, row: usize) -> Option<CovariateValue> {
    match &column.data {
        ColumnData::Number(v) => v.get(row).copied().flatten().map(CovariateValue::Number),
        ColumnData::Text(v) => v
            .get(row)
            .filter(|s| !s.trim().is_empty())
            .map(|s| CovariateValue::Text(s.clone())),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Reads the records of `table` and the refusals they raise.
pub(crate) fn build(table: &ImportedTable) -> Built {
    let layout = if first_with(table, ColumnRole::Event).is_some() {
        Layout::Event
    } else {
        Layout::Profile
    };
    let n_rows = table
        .columns
        .iter()
        .map(|c| c.data.len())
        .max()
        .unwrap_or(0);
    let subject_col = first_with(table, ColumnRole::Subject);
    let cols = Cols {
        time: first_with(table, ColumnRole::Time),
        conc: first_with(table, ColumnRole::Concentration),
        dose: first_with(table, ColumnRole::Dose),
        event: first_with(table, ColumnRole::Event),
        amount_text: table.columns.iter().find(|c| {
            c.role == ColumnRole::Other
                && crate::csv::role_of(&c.name, true) == Some(ColumnRole::Dose)
        }),
        rate: first_with(table, ColumnRole::Rate),
        dur: first_with(table, ColumnRole::Dur),
        cmt: first_with(table, ColumnRole::Cmt),
        mdv: first_with(table, ColumnRole::Mdv),
        cens: first_with(table, ColumnRole::Cens),
        limit: first_with(table, ColumnRole::Limit),
        blq_limits: &table.blq_limits,
        covariates: table
            .columns
            .iter()
            .filter(|c| c.role == ColumnRole::Covariate)
            .collect(),
        history: table
            .columns
            .iter()
            .filter(|c| {
                c.role == ColumnRole::Other && matches!(key(&c.name).as_str(), "ii" | "addl" | "ss")
            })
            .collect(),
    };

    // The subjects, in order of first appearance, and the subject of each row.
    let mut ids: Vec<String> = Vec::new();
    let mut row_subject: Vec<Option<usize>> = Vec::with_capacity(n_rows);
    if let Some(c) = subject_col {
        let mut place: HashMap<String, usize> = HashMap::new();
        for row in 0..n_rows {
            let label = c.data.text_at(row);
            if label.is_empty() {
                row_subject.push(None);
                continue;
            }
            let index = match place.get(&label) {
                Some(i) => *i,
                None => {
                    place.insert(label.clone(), ids.len());
                    ids.push(label);
                    ids.len() - 1
                }
            };
            row_subject.push(Some(index));
        }
    } else {
        row_subject.resize(n_rows, None);
    }
    // A table with at most one subject: a blank subject cell is that subject (DAT-ERR-06 refuses a
    // blank cell only when there are several).
    let several = ids.len() > 1;
    // The profile layout takes its subjects as `Worksheet::profile` does: a row whose subject cell
    // is blank belongs to no subject there, even when the table has one subject, and the dose check
    // below must see the same rows. With no label at all there is no profile to ask for.
    let named = !ids.is_empty();
    let profile_by_label = layout == Layout::Profile && subject_col.is_some() && named;
    if !several && !profile_by_label {
        if ids.is_empty() {
            ids.push(SINGLE_SUBJECT.to_owned());
        }
        for slot in &mut row_subject {
            slot.get_or_insert(0);
        }
    }

    let mut accs: Vec<Acc> = ids
        .iter()
        .map(|_| Acc {
            covariates: vec![None; cols.covariates.len()],
            covariate_reported: vec![false; cols.covariates.len()],
            ..Acc::default()
        })
        .collect();
    let mut problems: Vec<Problem> = Vec::new();
    let mut observation_rows = vec![false; n_rows];

    for row in 0..n_rows {
        let Some(si) = row_subject.get(row).copied().flatten() else {
            if layout == Layout::Event {
                problems.push(Problem {
                    code: "empty_subject",
                    subject: None,
                    message: format!(
                        "row {} has no subject; every row of a table with several subjects needs one; fill column `{}`",
                        row + 1,
                        name_of(subject_col, "subject")
                    ),
                });
            }
            continue;
        };
        let (Some(id), Some(acc)) = (ids.get(si), accs.get_mut(si)) else {
            continue;
        };
        acc.n_rows += 1;
        if let Some(t) = number_at(cols.time, row) {
            if let Some(last) = acc.last_time.filter(|last| t < *last) {
                acc.time_goes_back.get_or_insert((row + 1, t, last));
            }
            acc.last_time = Some(t);
        }
        read_covariates(&cols, id, row, si, acc, &mut problems);
        let is_observation = match layout {
            Layout::Event => event_row(&cols, id, row, si, acc, &mut problems),
            Layout::Profile => {
                profile_row(&cols, row, acc);
                true
            }
        };
        if let Some(flag) = observation_rows.get_mut(row) {
            *flag = is_observation;
        }
    }

    // Subject-level findings, after all the rows are read.
    for (si, (id, acc)) in ids.iter().zip(accs.iter_mut()).enumerate() {
        match layout {
            Layout::Profile => {
                if let Some((first_row, amount)) = acc.first_dose {
                    acc.doses.push(DoseRecord {
                        row: None,
                        time: 0.0,
                        amount,
                        rate: None,
                        duration: None,
                        kind: DoseKind::Bolus,
                    });
                    if let Some(other_row) =
                        acc.dose_conflict.filter(|_| subject_col.is_none() || named)
                    {
                        problems.push(Problem {
                            code: "different_doses",
                            subject: Some(si),
                            message: format!(
                                "subject {id} has two different doses ({} and {}) in column `{}`; one dose per profile",
                                cols.dose.map_or_else(String::new, |c| c.data.text_at(first_row)),
                                cols.dose.map_or_else(String::new, |c| c.data.text_at(other_row)),
                                name_of(cols.dose, "dose")
                            ),
                        });
                    }
                }
            }
            Layout::Event => {
                if let [first, second, ..] = acc.compartments.as_slice() {
                    problems.push(Problem {
                        code: "several_endpoints",
                        subject: Some(si),
                        message: format!(
                            "the observation rows of subject {id} are in several compartments ({first} and {second}); one observed compartment is analysed at a time. Split the table by compartment, or give a table with one."
                        ),
                    });
                }
                if acc.n_observation_rows == 0 && acc.n_rows > 0 {
                    let what = if acc.n_dose_rows == 0 {
                        "no observation row".to_owned()
                    } else {
                        format!(
                            "no observation (only {})",
                            plural(acc.n_dose_rows, "dose row", "dose rows")
                        )
                    };
                    problems.push(Problem {
                        code: "subject_without_observation",
                        subject: Some(si),
                        message: format!(
                            "subject {id} has {what}, so there is nothing to analyse or fit for it; add its observations or remove its rows",
                        ),
                    });
                }
            }
        }
    }

    // The subjects each problem touches, then the time problems the other checks report.
    for p in &problems {
        if let Some(acc) = p.subject.and_then(|i| accs.get_mut(i)) {
            if acc.refusal.is_none() {
                acc.refusal = Some(p.code.to_owned());
                acc.refusal_message = Some(p.message.clone());
            }
        }
    }
    // The two time problems have the sentences of the checks of the reading (`check_times`), for
    // this subject; a table without a subject column leaves the subject out of them, as they do.
    for (id, acc) in ids.iter().zip(accs.iter_mut()) {
        if acc.refusal.is_some() {
            continue;
        }
        let of = if subject_col.is_some() {
            format!(" of subject {id}")
        } else {
            String::new()
        };
        if let Some((row, b, a)) = acc.time_goes_back {
            acc.refusal = Some("time_not_increasing".to_owned());
            acc.refusal_message = Some(crate::csv::time_goes_back_message(row, &of, b, a));
        } else if let Some((row, b)) = acc.time_repeats {
            acc.refusal = Some("duplicate_times".to_owned());
            acc.refusal_message = Some(crate::csv::duplicate_time_message(row, &of, b));
        }
    }

    let checks = checks_of(&problems);
    let subjects = ids
        .into_iter()
        .zip(accs)
        .map(|(id, acc)| {
            let count =
                |s: ObservationStatus| acc.observations.iter().filter(|o| o.status == s).count();
            SubjectRecords {
                n_obs: acc.observations.len(),
                n_observed: count(ObservationStatus::Observed),
                n_missing: count(ObservationStatus::Missing),
                n_blq: count(ObservationStatus::Blq),
                covariates: cols
                    .covariates
                    .iter()
                    .enumerate()
                    .map(|(k, c)| {
                        let value = acc
                            .covariates
                            .get(k)
                            .cloned()
                            .flatten()
                            .map(|(_, value)| value);
                        (c.name.clone(), value)
                    })
                    .collect(),
                id,
                n_rows: acc.n_rows,
                doses: acc.doses,
                observations: acc.observations,
                refusal: acc.refusal,
                refusal_message: acc.refusal_message,
            }
        })
        .collect();
    Built {
        dataset: Dataset { layout, subjects },
        checks,
        observation_rows: (layout == Layout::Event).then_some(observation_rows),
    }
}

/// One check per code: the first problem in full and the count of the others (DAT-ERR-*).
fn checks_of(problems: &[Problem]) -> Vec<ReadingCheck> {
    let mut codes: Vec<&'static str> = Vec::new();
    for p in problems {
        if !codes.contains(&p.code) {
            codes.push(p.code);
        }
    }
    codes
        .into_iter()
        .filter_map(|code| {
            let mut mine = problems.iter().filter(|p| p.code == code);
            let first = mine.next()?;
            let others = mine.count();
            let message = if others == 0 {
                first.message.clone()
            } else {
                format!("{} (and {others} more of this kind)", first.message)
            };
            Some(ReadingCheck {
                code: code.to_owned(),
                message,
            })
        })
        .collect()
}

/// DAT-COV-01 and DAT-ERR-05: the value of a declared covariate is the first non-missing value of the
/// subject; a different one later is `covariate_varies`.
fn read_covariates(
    cols: &Cols,
    id: &str,
    row: usize,
    si: usize,
    acc: &mut Acc,
    problems: &mut Vec<Problem>,
) {
    for (k, column) in cols.covariates.iter().enumerate() {
        let Some(value) = covariate_cell(column, row) else {
            continue;
        };
        let Some(slot) = acc.covariates.get_mut(k) else {
            continue;
        };
        match slot {
            None => *slot = Some((row, value)),
            Some((first_row, first)) if *first != value => {
                let reported = acc.covariate_reported.get_mut(k);
                if reported.as_ref().is_some_and(|r| !**r) {
                    if let Some(r) = reported {
                        *r = true;
                    }
                    problems.push(Problem {
                        code: "covariate_varies",
                        subject: Some(si),
                        message: format!(
                            "covariate `{}` takes several values for subject {id} ({} at row {} and {} at row {}); in this version a covariate has one value per subject. Give one value per subject, or declare the column as not a covariate",
                            column.name,
                            column.data.text_at(*first_row),
                            *first_row + 1,
                            column.data.text_at(row),
                            row + 1
                        ),
                    });
                }
            }
            Some(_) => {}
        }
    }
}

/// A row of the profile layout: an observation (DAT-LEG-01); the dose column holds the dose of the
/// subject, one value per subject.
fn profile_row(cols: &Cols, row: usize, acc: &mut Acc) {
    let time = number_at(cols.time, row);
    if let Some(t) = time {
        if acc.last_observation_time == Some(t) {
            acc.time_repeats.get_or_insert((row + 1, t));
        }
        acc.last_observation_time = Some(t);
    }
    acc.n_observation_rows += 1;
    if let Some(amount) = number_at(cols.dose, row) {
        match acc.first_dose {
            None => acc.first_dose = Some((row, amount)),
            Some((_, first)) if first != amount && acc.dose_conflict.is_none() => {
                acc.dose_conflict = Some(row);
            }
            Some(_) => {}
        }
    }
    let value = number_at(cols.conc, row);
    let blq = cols.conc.is_some_and(|c| c.is_blq(row));
    let (status, value) = match (blq, value) {
        (true, _) => (ObservationStatus::Blq, None),
        (false, Some(x)) => (ObservationStatus::Observed, Some(x)),
        (false, None) => (ObservationStatus::Missing, None),
    };
    let limit = (status == ObservationStatus::Blq)
        .then(|| text_limit(cols, row))
        .flatten();
    acc.observations.push(ObservationRecord {
        row: row + 1,
        time,
        status,
        value,
        limit,
        lower_limit: None,
    });
}

/// A row of the event layout. Returns whether it is an observation row (by its event type, whatever
/// else is wrong with it).
fn event_row(
    cols: &Cols,
    id: &str,
    row: usize,
    si: usize,
    acc: &mut Acc,
    problems: &mut Vec<Problem>,
) -> bool {
    let r = row + 1;
    let mut fail = |code: &'static str, message: String| {
        problems.push(Problem {
            code,
            subject: Some(si),
            message,
        });
    };
    let Some(event) = cols.event else {
        return false;
    };
    let kind = event_kind(event, row);
    let time = number_at(cols.time, row);
    let is_observation = matches!(kind, Kind::Observation);
    if is_observation {
        acc.n_observation_rows += 1;
        if let Some(t) = time {
            if acc.last_observation_time == Some(t) {
                acc.time_repeats.get_or_insert((row + 1, t));
            }
            acc.last_observation_time = Some(t);
        }
    }
    match &kind {
        Kind::Unknown(shown) => {
            let what = if shown.trim().is_empty() {
                "no event type".to_owned()
            } else {
                format!("the unknown event type `{}`", shown.trim())
            };
            fail(
                "unknown_event",
                format!(
                    "row {r} of subject {id} has {what} in column `{}`; use 0 or obs for an observation, 1 or dose for a dose",
                    event.name
                ),
            );
            return false;
        }
        Kind::Unsupported(shown, meaning) => {
            fail(
                "unsupported_event",
                format!(
                    "row {r} of subject {id} has event type {} (`{meaning}`), which is not supported in this version; supported types are 0 (observation) and 1 (dose)",
                    shown.trim()
                ),
            );
            return false;
        }
        Kind::Observation | Kind::Dose => {}
    }
    if !is_observation {
        acc.n_dose_rows += 1;
    }
    let Some(t) = time else {
        fail(
            "missing_time",
            format!(
                "row {r} of subject {id} has no time in column `{}`; fill it or remove the row",
                name_of(cols.time, "time")
            ),
        );
        return is_observation;
    };
    if is_observation {
        observation_row(cols, id, row, t, acc, &mut fail);
    } else {
        dose_row(cols, id, row, t, acc, &mut fail);
    }
    is_observation
}

fn dose_row(
    cols: &Cols,
    id: &str,
    row: usize,
    time: f64,
    acc: &mut Acc,
    fail: &mut impl FnMut(&'static str, String),
) {
    let r = row + 1;
    let amt = name_of(cols.dose, "amt");
    let Some(amount) = number_at(cols.dose, row) else {
        let message = match (cols.dose, cols.amount_text) {
            (Some(_), _) => format!(
                "row {r} of subject {id} is a dose but has no amount; give the amount in column `{amt}`, or change the event type if it is an observation"
            ),
            (None, Some(text)) => format!(
                "row {r} of subject {id} is a dose but column `{}` holds text, not numbers, so there is no amount; make the amounts numbers, or change the event type if it is an observation",
                text.name
            ),
            (None, None) => format!(
                "row {r} of subject {id} is a dose but the table has no amount column; add a column named amt (or amount) with the amount of each dose, or change the event type if it is an observation"
            ),
        };
        fail("dose_without_amount", message);
        return;
    };
    if amount <= 0.0 {
        fail(
            "non_positive_amount",
            format!(
                "row {r} of subject {id} is a dose with an amount of {} in column `{amt}`; the amount of a dose must be above 0",
                cols.dose.map_or_else(String::new, |c| c.data.text_at(row))
            ),
        );
        return;
    }
    let rate = number_at(cols.rate, row);
    let dur = number_at(cols.dur, row);
    for (value, what, column) in [(rate, "rate", cols.rate), (dur, "duration", cols.dur)] {
        if value.is_some_and(|x| x < 0.0) {
            fail(
                "unsupported_event",
                format!(
                    "row {r} of subject {id} has a negative {what} ({}) in column `{}` (a modelled {what}), which is not supported in this version; give a {what} of 0 or more, or leave it empty",
                    column.map_or_else(String::new, |c| c.data.text_at(row)),
                    name_of(column, what)
                ),
            );
            return;
        }
    }
    let (rate, duration, kind) = match (rate.filter(|x| *x > 0.0), dur.filter(|x| *x > 0.0)) {
        (None, None) => (None, None, DoseKind::Bolus),
        (Some(q), None) => {
            let d = amount / q;
            if !d.is_finite() {
                not_finite(fail, (r, id), amount, ("rate", q), ("duration", d));
                return;
            }
            (Some(q), Some(d), DoseKind::Infusion)
        }
        (None, Some(d)) => {
            let q = amount / d;
            if !q.is_finite() {
                not_finite(fail, (r, id), amount, ("duration", d), ("rate", q));
                return;
            }
            (Some(q), Some(d), DoseKind::Infusion)
        }
        (Some(q), Some(d)) => {
            if (q * d - amount).abs() > 1e-6 * amount {
                fail(
                    "rate_duration_mismatch",
                    format!(
                        "row {r} of subject {id} gives an amount of {amount}, a rate of {q} (column `{}`) and a duration of {d} (column `{}`), but rate times duration is {}, not the amount; give the rate or the duration, not both, or make them agree",
                        name_of(cols.rate, "rate"),
                        name_of(cols.dur, "dur"),
                        q * d
                    ),
                );
                return;
            }
            (Some(q), Some(d), DoseKind::Infusion)
        }
    };
    for column in &cols.history {
        if history_value(column, row).is_some() {
            fail(
                "unsupported_history",
                format!(
                    "row {r} of subject {id} asks for additional doses or a steady state (`{}`), which are not supported in this version; give each dose on its own row",
                    column.name
                ),
            );
            return;
        }
    }
    acc.doses.push(DoseRecord {
        row: Some(r),
        time,
        amount,
        rate,
        duration,
        kind,
    });
}

/// A rate or duration derived from the amount and the other one is not a finite number (the amount
/// is huge or the other one is tiny). Refused as `rate_duration_mismatch` (DAT-ERR-06): the rate
/// and the duration do not make a usable infusion.
fn not_finite(
    fail: &mut impl FnMut(&'static str, String),
    (r, id): (usize, &str),
    amount: f64,
    (given, given_value): (&str, f64),
    (derived, derived_value): (&str, f64),
) {
    fail(
        "rate_duration_mismatch",
        format!(
            "row {r} of subject {id} gives an amount of {amount} and a {given} of {given_value}, from which the {derived} (the amount divided by the {given}) is {derived_value}, not a finite number; give a {given} of a usual size, or give the {derived} instead"
        ),
    );
}

fn observation_row(
    cols: &Cols,
    id: &str,
    row: usize,
    time: f64,
    acc: &mut Acc,
    fail: &mut impl FnMut(&'static str, String),
) {
    let r = row + 1;
    if number_at(cols.dose, row).is_some() {
        fail(
            "amount_on_observation",
            format!(
                "row {r} of subject {id} is an observation but has an amount ({}); an amount belongs on a dose row. Remove the amount, or change the event type to dose",
                cols.dose.map_or_else(String::new, |c| c.data.text_at(row))
            ),
        );
        return;
    }
    let mdv = number_at(cols.mdv, row);
    if mdv.is_some_and(|m| m != 0.0 && m != 1.0) {
        fail(
            "unknown_mdv",
            format!(
                "row {r} of subject {id} has the value {} in column `{}`; use 0 to use the value, 1 to leave it out, or leave the cell empty",
                cols.mdv.map_or_else(String::new, |c| c.data.text_at(row)),
                name_of(cols.mdv, "mdv")
            ),
        );
        return;
    }
    let skipped = mdv == Some(1.0);
    let value = number_at(cols.conc, row);
    let censored = match number_at(cols.cens, row) {
        None => false,
        Some(0.0) => false,
        Some(1.0) => {
            if value.is_none() && !skipped {
                fail(
                    "unsupported_censoring",
                    format!(
                        "row {r} of subject {id} is marked left-censored (1) but the limit is missing; the value column holds the limit of a censored row"
                    ),
                );
                return;
            }
            true
        }
        Some(_) => {
            fail(
                "unsupported_censoring",
                format!(
                    "row {r} of subject {id} is marked right-censored (or has an unknown censoring code {}); only left-censored values (1) are supported",
                    cols.cens.map_or_else(String::new, |c| c.data.text_at(row))
                ),
            );
            return;
        }
    };
    let blq = cols.conc.is_some_and(|c| c.is_blq(row));
    // The cell as read, before the value of a row below the limit is dropped.
    let cell = value;
    let (status, value) = if skipped {
        (ObservationStatus::Missing, None)
    } else if censored || blq {
        (ObservationStatus::Blq, None)
    } else if let Some(x) = value {
        (ObservationStatus::Observed, Some(x))
    } else {
        (ObservationStatus::Missing, None)
    };
    // The limit of a row below it: the number after the `<` of the cell, else, on a censored row,
    // its value cell (the `rxode2` convention). The `limit` column of a censored row is the other,
    // lower bound. A row left out by `mdv` has neither.
    let below = status == ObservationStatus::Blq;
    let limit = if below {
        text_limit(cols, row).or(if censored && !blq { cell } else { None })
    } else {
        None
    };
    let lower_limit = if below && censored {
        number_at(cols.limit, row)
    } else {
        None
    };
    let compartment = cols.cmt.map_or_else(String::new, |c| c.data.text_at(row));
    if !compartment.is_empty() && !acc.compartments.contains(&compartment) {
        acc.compartments.push(compartment);
    }
    acc.observations.push(ObservationRecord {
        row: r,
        time: Some(time),
        status,
        value,
        limit,
        lower_limit,
    });
}
