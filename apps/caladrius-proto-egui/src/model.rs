//! What the screen reads from the engine's answers: small typed views of the JSON, tolerant of
//! extra fields. Nothing here computes; a missing field is an empty value, never a panic.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// A value or the reason for its absence.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ParamValue {
    pub value: Option<f64>,
    pub not_calculated: Option<String>,
}

/// One parameter of an NCA result.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Param {
    pub name: String,
    pub value: ParamValue,
}

/// A point of the integrated profile.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProfilePoint {
    pub time: f64,
    pub conc: f64,
    pub origin: String,
}

/// An input point left out of the profile.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Removed {
    pub index: usize,
    pub time: f64,
    pub reason: String,
}

/// A candidate terminal phase.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Candidate {
    pub n_points: usize,
    pub time_first: f64,
    pub time_last: f64,
    pub lambda_z: f64,
    pub adj_r_squared: Option<f64>,
    pub valid: bool,
    pub selected: bool,
}

/// A quality flag: its code and the parameter it concerns.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Flag {
    pub code: String,
    pub parameter: Option<String>,
}

impl Flag {
    /// The parameter of the summary this flag is about.
    pub fn concerns(&self) -> &str {
        match self.code.as_str() {
            "low_adjusted_r_squared" => "adj.r.squared",
            "short_span" => "span.ratio",
            "few_points" => "lambda.z.n.points",
            "high_extrapolation" => self.parameter.as_deref().unwrap_or("aucpext.obs"),
            "replaced_points_in_lambda_z" => "lambda.z",
            "area_past_tlast" => "auclast",
            _ => "",
        }
    }
}

/// The result of one subject.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct NcaOk {
    pub parameters: Vec<Param>,
    pub profile: Vec<ProfilePoint>,
    pub removed: Vec<Removed>,
    pub lambda_z_candidates: Vec<Candidate>,
    pub flags: Vec<Flag>,
}

impl NcaOk {
    pub fn get(&self, name: &str) -> Option<&ParamValue> {
        self.parameters
            .iter()
            .find(|p| p.name == name)
            .map(|p| &p.value)
    }

    pub fn number(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(|v| v.value)
    }

    pub fn selected(&self) -> Option<&Candidate> {
        self.lambda_z_candidates.iter().find(|c| c.selected)
    }
}

/// An outcome: a result or the engine's sentence.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok(Box<NcaOk>),
    Error(String),
    #[default]
    Missing,
}

/// One subject of an NCA result.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct SubjectResult {
    pub subject: String,
    pub dose: Option<f64>,
    pub outcome: Outcome,
    pub flag_messages: Vec<String>,
    pub not_calculated_messages: BTreeMap<String, String>,
}

/// The answer of `nca.run` for one subject.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub id: u64,
    pub label: String,
    pub options: Value,
    pub subject: SubjectResult,
    /// The raw JSON of the subject's parameters, kept to check the cells on screen against.
    pub raw_parameters: Value,
}

impl Run {
    /// Reads the `result` of an `nca.run` answer.
    pub fn from_value(view: &Value) -> Option<Run> {
        if view.get("kind").and_then(Value::as_str) != Some("nca") {
            return None;
        }
        let subject_json = view.pointer("/result/subjects/0")?;
        let subject =
            serde_json::from_value::<SubjectResult>(subject_json.clone()).unwrap_or_else(|_| {
                SubjectResult {
                    outcome: Outcome::Error("the result could not be read".to_owned()),
                    ..SubjectResult::default()
                }
            });
        Some(Run {
            id: view.get("id").and_then(Value::as_u64).unwrap_or(0),
            label: view
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            options: view
                .pointer("/spec/options")
                .cloned()
                .unwrap_or(Value::Null),
            subject,
            raw_parameters: subject_json
                .pointer("/outcome/ok/parameters")
                .cloned()
                .unwrap_or(Value::Null),
        })
    }

    pub fn ok(&self) -> Option<&NcaOk> {
        match &self.subject.outcome {
            Outcome::Ok(ok) => Some(ok),
            _ => None,
        }
    }
}

/// Whether the analysis matches its inputs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    NoResult,
    Fresh,
    Stale,
}

impl Status {
    pub fn from_text(text: &str) -> Status {
        match text {
            "fresh" => Status::Fresh,
            "stale" => Status::Stale,
            _ => Status::NoResult,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Fresh => "fresh",
            Status::Stale => "stale",
            Status::NoResult => "not run",
        }
    }
}

/// The seal line: what the last `nca.run` answered, in short form.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SealLine {
    pub command: String,
    pub sha256: String,
    pub engine: String,
}

impl SealLine {
    /// The text under the results: the command, the first 12 characters of the seal, the engine.
    pub fn text(&self) -> String {
        let short: String = self.sha256.chars().take(12).collect();
        format!(
            "{} · seal {} · engine {}",
            self.command,
            if short.is_empty() { "none" } else { &short },
            if self.engine.is_empty() {
                "unknown"
            } else {
                &self.engine
            }
        )
    }
}
