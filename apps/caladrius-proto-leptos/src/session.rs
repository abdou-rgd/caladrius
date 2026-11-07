//! The state of the screen and everything that changes it. No DOM and no reactive runtime here: a
//! component reads the state and calls a method; the methods call the engine through [`Engine`]
//! (in the page: `caladrius_bridge::Bridge`, linked into the same WebAssembly module, in process),
//! so a test drives the screen with the real engine or with a recording one.
//!
//! Every user action is a command of the registry. A drag of the λz range, a click on a point, a
//! candidate row and "Back to automatic" all send `nca.run` with the parameters the egui page sends
//! (`crates/caladrius-ui/src/nca.rs`, `NcaPage::params`): worksheet, subject, route, dose, the
//! analysis once it exists, and the options as the engine echoed them, with
//! `lambda_z_selection.manual` set to `{ "range": { "start", "end" } }` (a range, a candidate row),
//! `{ "times": [...] }` (a click) or null (automatic).

use caladrius_bridge::Bridge;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::plotmap::selectable;
use crate::results::Units;
use crate::types::{
    AnalysisStatus, AnalysisView, Candidate, DataRow, NcaOk, Outcome, Scene, SceneAnswer, Seal,
    SubjectResult, WorksheetInfo,
};

/// An error of the engine: its code and its sentence.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineError {
    pub code: String,
    pub message: String,
}

/// The answer of the bridge, with the keys the screen reads.
#[derive(Debug, Clone)]
pub struct Answer {
    pub result: Option<Value>,
    pub error: Option<EngineError>,
    pub seal: Option<Seal>,
    pub command: Option<String>,
}

/// What the session needs of the engine: one call.
pub trait Engine {
    fn call(&mut self, command: &str, params: Value) -> Answer;
}

impl Engine for Bridge {
    fn call(&mut self, command: &str, params: Value) -> Answer {
        let request = json!({ "command": command, "params": params }).to_string();
        let answer = self.answer(&request);
        let error = answer
            .get("error")
            .filter(|e| !e.is_null())
            .map(|e| EngineError {
                code: e
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                message: e
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("The engine refused the request.")
                    .to_owned(),
            });
        let ok = answer.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let result = if ok {
            answer.get("result").cloned()
        } else {
            None
        };
        let seal = answer
            .get("seal")
            .filter(|s| !s.is_null())
            .and_then(|s| serde_json::from_value(s.clone()).ok());
        let command = answer
            .get("command")
            .and_then(Value::as_str)
            .map(str::to_owned);
        Answer {
            result,
            error,
            seal,
            command,
        }
    }
}

#[derive(Debug, Clone)]
pub struct State {
    /// The worksheet is loaded and the first run is done (or failed).
    pub ready: bool,
    pub dataset_name: String,
    pub info: Option<WorksheetInfo>,
    pub rows: Vec<DataRow>,
    pub subject: String,
    pub analysis: Option<u64>,
    pub options: Option<Value>,
    pub view: Option<AnalysisView>,
    /// Seal and command id of the last `nca.run`.
    pub seal: Option<Seal>,
    pub command: Option<String>,
    pub scene: Option<Scene>,
    pub log: bool,
    /// The sentence of an engine error or of a refusal, shown in place.
    pub error: Option<String>,
    /// Why the result is stale, as codes of the engine.
    pub stale_reasons: Vec<String>,
}

impl Default for State {
    fn default() -> Self {
        State {
            ready: false,
            dataset_name: String::new(),
            info: None,
            rows: Vec::new(),
            subject: String::new(),
            analysis: None,
            options: None,
            view: None,
            seal: None,
            command: None,
            scene: None,
            log: true,
            error: None,
            stale_reasons: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    None,
    Fresh,
    Stale,
}

impl State {
    pub fn status(&self) -> Status {
        match self.view.as_ref().map(|v| &v.status) {
            Some(AnalysisStatus::Fresh) => Status::Fresh,
            Some(AnalysisStatus::Stale { .. }) => Status::Stale,
            _ => Status::None,
        }
    }

    pub fn subject_result(&self) -> Option<&SubjectResult> {
        let subjects = &self.view.as_ref()?.result.as_ref()?.subjects;
        subjects
            .iter()
            .find(|s| s.subject == self.subject)
            .or_else(|| subjects.first())
    }

    pub fn ok(&self) -> Option<&NcaOk> {
        match &self.subject_result()?.outcome {
            Outcome::Ok(ok) => Some(ok),
            Outcome::Error(_) => None,
        }
    }

    pub fn selected_candidate(&self) -> Option<&Candidate> {
        self.ok()?.lambda_z_candidates.iter().find(|c| c.selected)
    }

    pub fn is_manual(&self) -> bool {
        self.options
            .as_ref()
            .and_then(|o| o.pointer("/lambda_z_selection/manual"))
            .is_some_and(|m| !m.is_null())
    }

    /// The units of the worksheet columns and the derived ones the engine names.
    pub fn units(&self) -> Units {
        let cols = self
            .info
            .as_ref()
            .map(|i| i.columns.as_slice())
            .unwrap_or_default();
        let unit = |role: &str| {
            cols.iter()
                .find(|c| c.role == role)
                .and_then(|c| c.unit.clone())
        };
        Units {
            time: unit("time"),
            conc: unit("concentration"),
            derived: self
                .info
                .as_ref()
                .map(|i| i.derived_units.clone())
                .unwrap_or_default(),
        }
    }

    /// The samples of the profile a person can pick, from the scene.
    pub fn samples(&self) -> Vec<f64> {
        self.scene
            .as_ref()
            .and_then(|s| s.panels.first())
            .map(|p| selectable(p).iter().map(|q| q.x).collect())
            .unwrap_or_default()
    }

    /// The times of the points used for λz, from the scene (the series `lambda_z_used`).
    pub fn used_times(&self) -> Vec<f64> {
        self.scene
            .as_ref()
            .and_then(|s| s.panels.first())
            .and_then(|p| {
                p.series
                    .iter()
                    .find(|s| s.role == crate::types::Role::LambdaZUsed)
            })
            .map(|s| s.points.iter().map(|p| p.x).collect())
            .unwrap_or_default()
    }

    /// The two ends of the terminal phase on the plot.
    pub fn range(&self) -> Option<(f64, f64)> {
        let used = self.used_times();
        let first = *used.first()?;
        Some(
            used.iter()
                .fold((first, first), |(lo, hi), t| (lo.min(*t), hi.max(*t))),
        )
    }
}

/// The sentence that says what changed, for a stale result (the wording of the egui page).
pub fn stale_sentence(reasons: &[String]) -> String {
    let parts: Vec<&str> = reasons
        .iter()
        .map(|code| match code.as_str() {
            "input_changed" => "the data changed",
            "options_changed" => "the options changed",
            "worksheet_missing" => "the worksheet is gone",
            _ => "an input changed",
        })
        .collect();
    let what = if parts.is_empty() {
        "an input changed".to_owned()
    } else {
        parts.join(" and ")
    };
    format!(
        "Out of date: {what} since this result was computed. Run the analysis again to refresh it."
    )
}

pub const TWO_POINTS: &str =
    "The terminal phase needs at least 2 points: keep one more selected before removing this one.";

/// The options with `lambda_z_selection.manual` replaced; the other options are kept as the engine echoed them.
pub fn with_manual(options: Option<&Value>, manual: Value) -> Value {
    let mut base = options
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if let Some(obj) = base.as_object_mut() {
        let selection = obj.entry("lambda_z_selection").or_insert_with(|| json!({}));
        if !selection.is_object() {
            *selection = json!({});
        }
        if let Some(sel) = selection.as_object_mut() {
            sel.insert("manual".to_owned(), manual);
        }
    }
    base
}

/// Which column of the worksheet a cell edit is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellRole {
    Time,
    Concentration,
}

fn decode<T: DeserializeOwned>(value: Option<Value>) -> Result<T, String> {
    let value = value.ok_or_else(|| "The engine gave no result.".to_owned())?;
    serde_json::from_value(value)
        .map_err(|e| format!("The answer of the engine could not be read: {e}"))
}

pub struct Session<E: Engine> {
    state: State,
    engine: E,
}

impl<E: Engine> Session<E> {
    pub fn new(engine: E) -> Self {
        Session {
            state: State::default(),
            engine,
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    #[cfg(test)]
    pub fn engine(&self) -> &E {
        &self.engine
    }

    #[cfg(test)]
    pub fn engine_mut_for_test(&mut self) -> &mut E {
        &mut self.engine
    }

    fn call<T: DeserializeOwned>(
        &mut self,
        command: &str,
        params: Value,
        fallback: &str,
    ) -> Result<(T, Answer), String> {
        let mut answer = self.engine.call(command, params);
        if let Some(error) = &answer.error {
            return Err(if error.message.is_empty() {
                fallback.to_owned()
            } else {
                error.message.clone()
            });
        }
        let value: T = decode(answer.result.take())?;
        Ok((value, answer))
    }

    // ---- data -------------------------------------------------------------------------------

    /// Imports a CSV (columns subject, time, conc, dose), reads the worksheet and runs the first subject.
    pub fn load_dataset(&mut self, name: &str, csv: &str, subject: Option<&str>) {
        #[derive(serde::Deserialize)]
        struct Imported {
            worksheet: WorksheetInfo,
        }
        #[derive(serde::Deserialize)]
        struct Table {
            columns: Vec<String>,
            rows: Vec<Vec<Value>>,
        }
        let info = match self.call::<Imported>(
            "data.import",
            json!({ "name": name, "csv": csv }),
            "The engine could not read the data.",
        ) {
            Ok((imported, _)) => imported.worksheet,
            Err(e) => {
                self.state.error = Some(e);
                self.state.ready = true;
                return;
            }
        };
        let table = match self.call::<Table>(
            "export.table",
            json!({ "table": "worksheet", "worksheet": info.id }),
            "The engine could not list the data.",
        ) {
            Ok((table, _)) => table,
            Err(e) => {
                self.state.error = Some(e);
                self.state.ready = true;
                return;
            }
        };
        let rows = read_rows(&info, &table.columns, &table.rows);
        self.state = State {
            dataset_name: name.to_owned(),
            subject: subject
                .map(str::to_owned)
                .or_else(|| info.subjects.first().cloned())
                .unwrap_or_default(),
            info: Some(info),
            rows,
            ..State::default()
        };
        self.run();
        self.state.ready = true;
    }

    /// A cell edit: the worksheet changes, and the analysis that read it is stale until it runs again.
    pub fn edit_cell(&mut self, row_index: usize, role: CellRole, text: &str) {
        #[derive(serde::Deserialize)]
        struct Edited {
            #[serde(default)]
            understood: Option<f64>,
            #[serde(default)]
            stale_analyses: Vec<u64>,
            worksheet: WorksheetInfo,
        }
        let Some(info) = self.state.info.clone() else {
            return;
        };
        let wanted = match role {
            CellRole::Time => "time",
            CellRole::Concentration => "concentration",
        };
        let Some(column) = info
            .columns
            .iter()
            .find(|c| c.role == wanted)
            .map(|c| c.name.clone())
        else {
            return;
        };
        let params =
            json!({ "worksheet": info.id, "column": column, "row": row_index, "value": text });
        let edited =
            match self.call::<Edited>("data.set_cell", params, "The engine refused the value.") {
                Ok((edited, _)) => edited,
                Err(e) => {
                    self.state.error = Some(e);
                    return;
                }
            };
        for row in self.state.rows.iter_mut().filter(|r| r.index == row_index) {
            match role {
                CellRole::Time => row.time = edited.understood.unwrap_or(row.time),
                CellRole::Concentration => row.conc = edited.understood,
            }
        }
        self.state.info = Some(edited.worksheet);
        self.state.error = None;
        if let Some(analysis) = self.state.analysis
            && edited.stale_analyses.contains(&analysis)
            && let Ok((view, _)) = self.call::<AnalysisView>(
                "analysis.get",
                json!({ "analysis": analysis }),
                "The engine could not read the analysis.",
            )
        {
            self.state.stale_reasons = reasons_of(&view);
            self.state.view = Some(view);
        }
    }

    // ---- the analysis -----------------------------------------------------------------------

    /// The parameters of `nca.run` for what the screen holds (the egui page's `NcaPage::params`).
    pub fn run_params(&self) -> Value {
        let s = &self.state;
        let mut params = json!({
            "worksheet": s.info.as_ref().map_or(0, |i| i.id),
            "subject": s.subject,
            "route": "extravascular",
            "dose": Value::Null,
        });
        if let Some(analysis) = s.analysis {
            params["analysis"] = json!(analysis);
        }
        if let Some(options) = s.options.as_ref().filter(|o| o.is_object()) {
            params["options"] = options.clone();
        }
        params
    }

    /// Runs `nca.run` and draws its scene.
    pub fn run(&mut self) {
        let params = self.run_params();
        let (view, answer) =
            match self.call::<AnalysisView>("nca.run", params, "The engine gave no result.") {
                Ok(done) => done,
                Err(e) => {
                    self.state.error = Some(e);
                    return;
                }
            };
        let options = view
            .spec
            .get("options")
            .cloned()
            .or_else(|| self.state.options.clone());
        let (scene, error) = self.fetch_scene(view.id, &view);
        self.state.analysis = Some(view.id);
        self.state.options = options;
        self.state.seal = answer.seal;
        self.state.command = answer.command;
        self.state.scene = scene;
        self.state.error = error;
        self.state.stale_reasons = reasons_of(&view);
        self.state.view = Some(view);
    }

    fn fetch_scene(
        &mut self,
        analysis: u64,
        view: &AnalysisView,
    ) -> (Option<Scene>, Option<String>) {
        let subjects = view
            .result
            .as_ref()
            .map(|r| r.subjects.as_slice())
            .unwrap_or_default();
        let Some(subject) = subjects
            .iter()
            .find(|x| x.subject == self.state.subject)
            .or_else(|| subjects.first())
        else {
            return (None, None);
        };
        if let Outcome::Error(sentence) = &subject.outcome {
            return (None, Some(sentence.clone()));
        }
        let params =
            json!({ "analysis": analysis, "subject": self.state.subject, "log": self.state.log });
        match self.call::<SceneAnswer>("plot.scene", params, "The engine could not draw the plot.")
        {
            Ok((answer, _)) => (Some(answer.scene), None),
            Err(e) => (None, Some(e)),
        }
    }

    /// Semi-log or linear: the scene is asked again; the analysis does not run.
    pub fn set_log(&mut self, log: bool) {
        self.state.log = log;
        let (Some(analysis), Some(view)) = (self.state.analysis, self.state.view.clone()) else {
            return;
        };
        let (scene, error) = self.fetch_scene(analysis, &view);
        self.state.scene = scene;
        self.state.error = error;
    }

    /// A new subject: the hand-picked terminal phase belongs to one subject, so it is forgotten.
    pub fn select_subject(&mut self, subject: &str) {
        self.state.subject = subject.to_owned();
        self.state.options = Some(with_manual(self.state.options.as_ref(), Value::Null));
        self.run();
    }

    // ---- the terminal phase -----------------------------------------------------------------

    /// A range of the terminal phase, chosen by hand. Refused with a sentence when it holds fewer than 2 samples.
    pub fn set_range(&mut self, start: f64, end: f64) {
        let (a, b) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        if self
            .state
            .samples()
            .iter()
            .filter(|t| **t >= a && **t <= b)
            .count()
            < 2
        {
            self.state.error = Some(TWO_POINTS.to_owned());
            return;
        }
        self.state.options = Some(with_manual(
            self.state.options.as_ref(),
            json!({ "range": { "start": a, "end": b } }),
        ));
        self.run();
    }

    /// The handler of the start handle of the plot (a pointer drag, a key, the measure).
    pub fn set_range_start(&mut self, time: f64) {
        let end = self.state.range().map_or(time, |r| r.1);
        self.set_range(time, end);
    }

    /// The handler of the end handle.
    pub fn set_range_end(&mut self, time: f64) {
        let start = self.state.range().map_or(time, |r| r.0);
        self.set_range(start, time);
    }

    /// A click on the sample at `time`: it joins the hand-picked terminal phase or leaves it.
    pub fn toggle_point(&mut self, time: f64) {
        let used = self.state.used_times();
        let mut next: Vec<f64> = if used.contains(&time) {
            used.iter().copied().filter(|t| *t != time).collect()
        } else {
            used.into_iter().chain([time]).collect()
        };
        next.sort_by(f64::total_cmp);
        if next.len() < 2 {
            self.state.error = Some(TWO_POINTS.to_owned());
            return;
        }
        self.state.options = Some(with_manual(
            self.state.options.as_ref(),
            json!({ "times": next }),
        ));
        self.run();
    }

    /// A row of the candidate list.
    pub fn pick_candidate(&mut self, candidate: &Candidate) {
        let manual =
            json!({ "range": { "start": candidate.time_first, "end": candidate.time_last } });
        self.state.options = Some(with_manual(self.state.options.as_ref(), manual));
        self.run();
    }

    pub fn back_to_automatic(&mut self) {
        self.state.options = Some(with_manual(self.state.options.as_ref(), Value::Null));
        self.run();
    }
}

fn reasons_of(view: &AnalysisView) -> Vec<String> {
    match &view.status {
        AnalysisStatus::Stale { reasons } => reasons.iter().map(|r| r.code.clone()).collect(),
        _ => Vec::new(),
    }
}

/// A subject given as a number in the table (the engine writes 3 as 3.0): the name the worksheet lists.
fn subject_id(n: &serde_json::Number) -> String {
    match n.as_i64() {
        Some(i) => i.to_string(),
        None => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => (f as i64).to_string(),
            _ => n.to_string(),
        },
    }
}

/// Reads the rows of `export.table` into the rows the table shows.
pub fn read_rows(info: &WorksheetInfo, columns: &[String], rows: &[Vec<Value>]) -> Vec<DataRow> {
    let at = |role: &str| {
        let name = info
            .columns
            .iter()
            .find(|c| c.role == role)
            .map(|c| c.name.as_str());
        name.and_then(|n| columns.iter().position(|c| c == n))
    };
    let (subject, time, conc) = (at("subject"), at("time"), at("concentration"));
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let cell = |i: Option<usize>| i.and_then(|i| row.get(i));
            DataRow {
                index,
                subject: match cell(subject) {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Number(n)) => subject_id(n),
                    Some(v) => v.to_string(),
                    None => info.subjects.first().cloned().unwrap_or_default(),
                },
                time: cell(time).and_then(Value::as_f64).unwrap_or(f64::NAN),
                conc: cell(conc).and_then(Value::as_f64),
            }
        })
        .collect()
}
