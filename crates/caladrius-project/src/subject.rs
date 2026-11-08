//! The data of one subject for an analysis, read from the dataset (`specs/data.md`, DAT-USE-01 to
//! 03 and DAT-ERR-08).
//!
//! A worksheet in the profile layout keeps its old path: [`Worksheet::profile`] gives the data, with
//! its own refusals, so that every file that loaded before gives the same result (DAT-LEG-02). A
//! worksheet in the event layout is read from its dataset: the observation rows of the subject, and
//! its first dose row. In both layouts the counts of the observations and the declared covariates
//! come from the dataset.

use std::collections::BTreeMap;

use crate::csv::ImportedTable;
use crate::dataset::{self, CovariateValue, Dataset, DoseKind, Layout, ObservationStatus};
use crate::error::{ProjectError, Result};
use crate::worksheet::{ColumnData, ColumnRole, Profile, Worksheet};

/// What the data of a subject is read for. It only changes a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A non-compartmental analysis (DAT-USE-01).
    Nca,
    /// A model fit (DAT-USE-02).
    Fit,
}

impl Purpose {
    fn name(self) -> &'static str {
        match self {
            Purpose::Nca => "NCA",
            Purpose::Fit => "fit",
        }
    }
}

/// How many observation rows of a subject an analysis used and left out (FIT-ML-20). The three add
/// up to the observation rows of the subject, less the rows before the dose, which a note counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationCounts {
    /// Measured values that reach the analysis (`n_used`).
    pub n_used: usize,
    /// Rows without a value, or marked not to be used (`n_missing`).
    pub n_missing: usize,
    /// Rows below the limit of quantification, or censored (`n_blq`).
    pub n_blq: usize,
}

/// A subject, ready for an analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct SubjectData {
    /// Times measured from the dose, concentrations (NaN when missing), the BLQ flags, the dose and
    /// the route text of the subject.
    pub profile: Profile,
    /// The duration of the dose, when the dose row of the event layout is an infusion.
    pub infusion_duration: Option<f64>,
    /// The counts of the dataset.
    pub counts: ObservationCounts,
    /// The declared covariates of the subject, to be copied into its result and used nowhere else
    /// (DAT-USE-03).
    pub covariates: BTreeMap<String, Option<CovariateValue>>,
    /// What was decided for the subject that the person should know (rows left out before the
    /// dose).
    pub notes: Vec<String>,
}

impl Worksheet {
    /// The records of the worksheet as they are now: doses and observations of each subject, and
    /// which subjects a problem touches (DAT-ERR-08). A pure function of the worksheet.
    pub fn dataset(&self) -> Dataset {
        let table = ImportedTable {
            columns: self.columns().to_vec(),
            delimiter: ',',
            decimal_comma: false,
            notes: Vec::new(),
            blq_limits: Vec::new(),
        };
        dataset::build(&table).dataset
    }

    /// The subjects an analysis runs over, in order of first appearance: those of the dataset in
    /// the event layout, [`Worksheet::subjects`] in the profile layout.
    pub fn analysis_subjects(&self, dataset: &Dataset) -> Vec<String> {
        match dataset.layout {
            Layout::Event => dataset.subjects.iter().map(|s| s.id.clone()).collect(),
            Layout::Profile => self.subjects(),
        }
    }

    /// The data of one subject, for `purpose`. `None` is accepted when there is a single subject.
    ///
    /// In the profile layout this is [`Worksheet::profile`] with the counts and the covariates of
    /// the dataset. In the event layout the subject is refused, with the sentence of its check, when
    /// it carries a `refusal` (DAT-ERR-08) or has more than one dose row (DAT-USE-01); otherwise its
    /// observation rows are measured from the time of its dose row, and the rows before the dose are
    /// left out and counted in a note.
    pub fn subject_data(
        &self,
        dataset: &Dataset,
        subject: Option<&str>,
        purpose: Purpose,
    ) -> Result<SubjectData> {
        match dataset.layout {
            Layout::Profile => self.profile_data(dataset, subject),
            Layout::Event => self.event_data(dataset, subject, purpose),
        }
    }

    fn profile_data(&self, dataset: &Dataset, subject: Option<&str>) -> Result<SubjectData> {
        let profile = self.profile(subject)?;
        let record = dataset
            .subjects
            .iter()
            .find(|s| s.id == profile.subject)
            .ok_or_else(|| {
                ProjectError::new(
                    "dataset_mismatch",
                    format!(
                        "subject {} is in the worksheet but not in its dataset; read the file again",
                        profile.subject
                    ),
                )
            })?;
        Ok(SubjectData {
            counts: ObservationCounts {
                n_used: record.n_observed,
                n_missing: record.n_missing,
                n_blq: record.n_blq,
            },
            covariates: record.covariates.clone(),
            infusion_duration: None,
            notes: Vec::new(),
            profile,
        })
    }

    fn event_data(
        &self,
        dataset: &Dataset,
        subject: Option<&str>,
        purpose: Purpose,
    ) -> Result<SubjectData> {
        let labels = self.analysis_subjects(dataset);
        let label = match subject {
            Some(s) => {
                if !labels.iter().any(|x| x == s) {
                    return Err(ProjectError::new(
                        "unknown_subject",
                        format!(
                            "worksheet `{}` has no subject `{s}`; its subjects are: {}",
                            self.name(),
                            labels.join(", ")
                        ),
                    ));
                }
                s.to_owned()
            }
            None => match labels.as_slice() {
                [only] => only.clone(),
                _ => {
                    return Err(ProjectError::new(
                        "subject_required",
                        format!(
                            "worksheet `{}` has {} subjects ({}); say which one",
                            self.name(),
                            labels.len(),
                            labels.join(", ")
                        ),
                    ));
                }
            },
        };
        let record = dataset
            .subjects
            .iter()
            .find(|s| s.id == label)
            .ok_or_else(|| {
                ProjectError::new(
                    "dataset_mismatch",
                    format!(
                        "subject {label} is in the worksheet but not in its dataset; read the file again"
                    ),
                )
            })?;
        // DAT-ERR-08: an affected subject is refused with the sentence of its check.
        if let Some(code) = &record.refusal {
            let message = record
                .refusal_message
                .clone()
                .unwrap_or_else(|| format!("subject {label} is refused: {code}"));
            return Err(ProjectError::new(code, message));
        }
        if self.column_by_role(ColumnRole::Time).is_none() {
            return Err(ProjectError::new(
                "no_time_column",
                format!(
                    "worksheet `{}` has no time column; give a column the role time",
                    self.name()
                ),
            ));
        }
        if self.column_by_role(ColumnRole::Concentration).is_none() {
            return Err(ProjectError::new(
                "no_concentration_column",
                format!(
                    "worksheet `{}` has no concentration column; give a column the role concentration",
                    self.name()
                ),
            ));
        }
        let numbers = |role| {
            self.column_by_role(role)
                .is_some_and(|c| matches!(c.data, ColumnData::Number(_)))
        };
        if !(numbers(ColumnRole::Time) && numbers(ColumnRole::Concentration)) {
            return Err(ProjectError::new(
                "role_needs_numbers",
                "the time and concentration columns must hold numbers",
            ));
        }
        // DAT-USE-01: one dose row per profile.
        if record.doses.len() > 1 {
            return Err(ProjectError::new(
                "several_doses",
                format!(
                    "subject {label} has {} dose rows; the {} of this version analyses a single dose per profile",
                    record.doses.len(),
                    purpose.name()
                ),
            ));
        }
        let dose = record.doses.first();
        let dose_time = dose.map_or(0.0, |d| d.time);
        let mut profile = Profile {
            subject: label.clone(),
            time: Vec::new(),
            conc: Vec::new(),
            blq: Vec::new(),
            dose: dose.map(|d| d.amount),
            route: dose
                .and_then(|d| d.row)
                .and_then(|row| self.route_at(row.saturating_sub(1)))
                .or_else(|| self.route_of_subject(&label)),
        };
        let mut counts = ObservationCounts {
            n_used: 0,
            n_missing: 0,
            n_blq: 0,
        };
        let mut before_dose = 0usize;
        for o in &record.observations {
            let Some(t) = o.time else {
                return Err(ProjectError::new(
                    "missing_time",
                    format!(
                        "row {} of subject {label} has no time; fill it or delete the row (rows are counted from 1 here)",
                        o.row
                    ),
                ));
            };
            if t < dose_time {
                before_dose += 1;
                continue;
            }
            profile.time.push(t - dose_time);
            profile.blq.push(o.status == ObservationStatus::Blq);
            match (o.status, o.value) {
                (ObservationStatus::Observed, Some(v)) => {
                    counts.n_used += 1;
                    profile.conc.push(v);
                }
                (ObservationStatus::Blq, _) => {
                    counts.n_blq += 1;
                    profile.conc.push(0.0);
                }
                _ => {
                    counts.n_missing += 1;
                    profile.conc.push(f64::NAN);
                }
            }
        }
        let mut notes = Vec::new();
        if before_dose > 0 {
            notes.push(format!(
                "subject {label}: {before_dose} observation row{} before the dose (time {dose_time}) left out (before_dose)",
                if before_dose == 1 { " is" } else { "s are" }
            ));
        }
        Ok(SubjectData {
            profile,
            infusion_duration: dose
                .filter(|d| d.kind == DoseKind::Infusion)
                .and_then(|d| d.duration),
            counts,
            covariates: record.covariates.clone(),
            notes,
        })
    }

    /// The text of the route cell of data row `row` (from 0), if the cell holds one.
    fn route_at(&self, row: usize) -> Option<String> {
        let column = self.column_by_role(ColumnRole::Route)?;
        let text = column.data.text_at(row);
        (!text.is_empty()).then_some(text)
    }

    /// The first route cell of the subject that holds a text, as the profile layout reads it.
    fn route_of_subject(&self, label: &str) -> Option<String> {
        let column = self.column_by_role(ColumnRole::Route)?;
        let subject = self.column_by_role(ColumnRole::Subject);
        (0..self.n_rows()).find_map(|row| {
            if let Some(sc) = subject {
                if sc.data.text_at(row) != label {
                    return None;
                }
            }
            let text = column.data.text_at(row);
            (!text.is_empty()).then_some(text)
        })
    }
}
