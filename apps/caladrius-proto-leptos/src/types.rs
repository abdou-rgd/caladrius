//! The shapes of the answers this screen reads. They follow the result schemas of the commands
//! (`describe()`); only the keys the screen uses are named, the others are ignored.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Candidate {
    pub n_points: usize,
    pub time_first: f64,
    pub time_last: f64,
    pub lambda_z: f64,
    #[serde(default)]
    pub adj_r_squared: Option<f64>,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PValue {
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub not_calculated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ParamValue {
    pub name: String,
    pub value: PValue,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Flag {
    pub code: String,
    #[serde(default)]
    pub parameter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Removed {
    pub time: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NcaOk {
    #[serde(default)]
    pub flags: Vec<Flag>,
    #[serde(default)]
    pub lambda_z_candidates: Vec<Candidate>,
    #[serde(default)]
    pub parameters: Vec<ParamValue>,
    #[serde(default)]
    pub removed: Vec<Removed>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok(NcaOk),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SubjectResult {
    pub subject: String,
    #[serde(default)]
    pub flag_messages: Vec<String>,
    #[serde(default)]
    pub not_calculated_messages: BTreeMap<String, String>,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StaleReason {
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AnalysisStatus {
    Fresh,
    NoResult,
    Stale {
        #[serde(default)]
        reasons: Vec<StaleReason>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NcaResult {
    #[serde(default)]
    pub subjects: Vec<SubjectResult>,
}

/// The result of `nca.run` and `analysis.get`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AnalysisView {
    pub id: u64,
    #[serde(default)]
    pub label: String,
    pub status: AnalysisStatus,
    #[serde(default)]
    pub spec: Value,
    #[serde(default)]
    pub result: Option<NcaResult>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Column {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorksheetInfo {
    pub id: u64,
    pub name: String,
    pub rows: usize,
    #[serde(default)]
    pub subjects: Vec<String>,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub derived_units: BTreeMap<String, String>,
}

/// One row of the worksheet, with the row number the engine counts from 0.
#[derive(Debug, Clone, PartialEq)]
pub struct DataRow {
    pub index: usize,
    pub subject: String,
    pub time: f64,
    pub conc: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SceneTick {
    pub value: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SceneAxis {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub log: bool,
    pub min: f64,
    pub max: f64,
    #[serde(default)]
    pub ticks: Vec<SceneTick>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct ScenePoint {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub blq: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Subject,
    Observed,
    Predicted,
    Profile,
    Replaced,
    Excluded,
    LambdaZUsed,
    LambdaZStart,
    LambdaZFit,
    #[serde(other)]
    Other,
}

impl Role {
    /// The name used in CSS classes.
    pub fn css(self) -> &'static str {
        match self {
            Role::Subject => "subject",
            Role::Observed => "observed",
            Role::Predicted => "predicted",
            Role::Profile => "profile",
            Role::Replaced => "replaced",
            Role::Excluded => "excluded",
            Role::LambdaZUsed => "lambda_z_used",
            Role::LambdaZStart => "lambda_z_start",
            Role::LambdaZFit => "lambda_z_fit",
            Role::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Line,
    Points,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SceneSeries {
    pub name: String,
    pub role: Role,
    pub style: Style,
    #[serde(default)]
    pub points: Vec<ScenePoint>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HiddenPoint {
    pub series: String,
    pub x: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Hidden {
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub points: Vec<HiddenPoint>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScenePanel {
    #[serde(default)]
    pub title: String,
    pub x: SceneAxis,
    pub y: SceneAxis,
    #[serde(default)]
    pub series: Vec<SceneSeries>,
    #[serde(default)]
    pub hidden: Option<Hidden>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LegendEntry {
    pub name: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct Legend {
    #[serde(default)]
    pub entries: Vec<LegendEntry>,
    #[serde(default)]
    pub below_limit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Scene {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub panels: Vec<ScenePanel>,
    #[serde(default)]
    pub legend: Legend,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SceneAnswer {
    pub scene: Scene,
}

/// The seal of an answer.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Seal {
    pub sha256: String,
    #[serde(default)]
    pub engine: Option<String>,
}
