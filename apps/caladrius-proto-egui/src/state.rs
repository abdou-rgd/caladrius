//! The state of the screen and everything that changes it. No egui here: every action is a method,
//! so the window, the tests and the measure mode call the very same code (the handler that a
//! pointer drag uses is [`Screen::drag_handle`]).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use caladrius_bridge::Bridge;
use caladrius_engine::scene::PlotScene;
use caladrius_format as fmt;
use serde_json::{Value, json};

use crate::model::{Candidate, NcaOk, Outcome, Run, SealLine, Status};

/// The two ends of the λz range on the plot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    Start,
    End,
}

/// What a point of the table is, as the scene of `plot.scene` says it (never worked out here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Observed,
    Blq,
    Replaced,
    Excluded,
    Used,
    Start,
    Tmax,
    LeftOut(String),
}

impl Role {
    pub fn label(&self) -> String {
        match self {
            Role::Observed => "observed".to_owned(),
            Role::Blq => "BLQ".to_owned(),
            Role::Replaced => "replaced".to_owned(),
            Role::Excluded => "excluded".to_owned(),
            Role::Used => "λz".to_owned(),
            Role::Start => "λz start".to_owned(),
            Role::Tmax => "Tmax".to_owned(),
            Role::LeftOut(reason) => reason.clone(),
        }
    }
}

/// One row of the worksheet, whole.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetRow {
    pub subject: String,
    pub time: f64,
    pub conc: f64,
}

/// One row of the table on screen: a row of the worksheet for the current subject.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    /// The row in the worksheet, from 0.
    pub sheet_row: usize,
    pub time: f64,
    pub conc: f64,
    pub time_text: String,
    pub conc_text: String,
    pub roles: Vec<Role>,
}

/// One result cell as shown: a parameter with its text, unit and flag.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub name: String,
    pub label: String,
    pub text: String,
    pub unit: String,
    pub flagged: bool,
    /// Set for a parameter that was not calculated: the engine's sentence.
    pub why: Option<String>,
}

/// The headline parameters, in the order of the current NCA page.
pub const SUMMARY: [&str; 12] = [
    "cmax",
    "tmax",
    "auclast",
    "aucinf.obs",
    "aucpext.obs",
    "lambda.z",
    "half.life",
    "adj.r.squared",
    "span.ratio",
    "cl.obs",
    "vz.obs",
    "mrt.obs",
];

/// The text of a number for a parameter, as the screen shows it.
pub fn shown_number(name: &str, x: f64) -> String {
    if name.contains("r.squared") {
        fmt::fit_quality(x)
    } else {
        fmt::number(x)
    }
}

/// The whole state of the NCA screen.
pub struct Screen {
    bridge: Bridge,
    pub dataset: String,
    worksheet: u64,
    pub subjects: Vec<String>,
    pub subject: String,
    analysis: Option<u64>,
    /// The options of the analysis, as the engine echoed them after the last run.
    pub options: Value,
    pub status: Status,
    pub run: Option<Run>,
    pub scene: Option<PlotScene>,
    pub log: bool,
    sheet: Vec<SheetRow>,
    pub table: Vec<TableRow>,
    pub time_unit: Option<String>,
    pub conc_unit: Option<String>,
    derived_units: std::collections::BTreeMap<String, String>,
    /// The engine's sentence for the last thing it refused, shown in place.
    pub error: Option<String>,
    /// A short message that is not an error (a click the screen ignored, for instance).
    pub notice: Option<String>,
    pub seal: Option<SealLine>,
    /// How long the last `nca.run` took, with the scene and the table after it.
    pub last_run: Duration,
    /// The number of `nca.run` calls made since the start.
    pub runs: u64,
    /// The text last put on the clipboard.
    pub copied: Option<String>,
    /// What the results pane drew: (key, text) of each cell, for the check of the measure mode.
    pub shown: Vec<(String, String)>,
    /// Whether the "All parameters" section is open.
    pub all_open: bool,
}

impl Default for Screen {
    fn default() -> Self {
        Screen::new()
    }
}

impl Screen {
    pub fn new() -> Screen {
        Screen {
            bridge: Bridge::new(),
            dataset: String::new(),
            worksheet: 0,
            subjects: Vec::new(),
            subject: String::new(),
            analysis: None,
            options: Value::Null,
            status: Status::NoResult,
            run: None,
            scene: None,
            log: true,
            sheet: Vec::new(),
            table: Vec::new(),
            time_unit: None,
            conc_unit: None,
            derived_units: std::collections::BTreeMap::new(),
            error: None,
            notice: None,
            seal: None,
            last_run: Duration::ZERO,
            runs: 0,
            copied: None,
            shown: Vec::new(),
            all_open: false,
        }
    }

    // ---- the engine -----------------------------------------------------------------------

    /// One command. The answer's `result` on success, the engine's sentence on failure.
    fn call(&mut self, command: &str, params: Value) -> Result<Value, String> {
        let request = json!({ "command": command, "params": params }).to_string();
        let answer = self.bridge.answer(&request);
        if answer.get("ok").and_then(Value::as_bool) == Some(true) {
            if command == "nca.run" {
                self.seal = Some(SealLine {
                    command: command.to_owned(),
                    sha256: answer
                        .pointer("/seal/sha256")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    engine: answer
                        .pointer("/seal/engine")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                });
            }
            Ok(answer.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(answer
                .pointer("/error/message")
                .and_then(Value::as_str)
                .map(fmt::plain)
                .unwrap_or_else(|| "the engine refused the command".to_owned()))
        }
    }

    // ---- loading --------------------------------------------------------------------------

    /// Imports a CSV (columns subject, time, concentration, dose), picks `prefer` as the subject
    /// when it is there (else the first), and runs the analysis. The engine's sentence on failure.
    pub fn load_csv(&mut self, name: &str, csv: &str, prefer: &str) -> Result<(), String> {
        let done = self.call(
            "data.import",
            json!({ "name": name, "csv": csv, "delimiter": null, "decimal_comma": false }),
        )?;
        let sheet = done.get("worksheet").cloned().unwrap_or(Value::Null);
        self.dataset = name.to_owned();
        self.worksheet = sheet.get("id").and_then(Value::as_u64).unwrap_or(0);
        self.subjects = sheet
            .get("subjects")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let unit_of = |role: &str| {
            sheet
                .get("columns")
                .and_then(Value::as_array)
                .and_then(|cols| {
                    cols.iter()
                        .find(|c| c.get("role").and_then(Value::as_str) == Some(role))
                })
                .and_then(|c| c.get("unit"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        self.time_unit = unit_of("time");
        self.conc_unit = unit_of("concentration");
        self.derived_units = sheet
            .get("derived_units")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        self.analysis = None;
        self.options = Value::Null;
        self.run = None;
        self.scene = None;
        self.refresh_sheet()?;
        self.subject = if self.subjects.iter().any(|s| s == prefer) {
            prefer.to_owned()
        } else {
            self.subjects.first().cloned().unwrap_or_default()
        };
        self.rebuild_table();
        self.run();
        Ok(())
    }

    /// Reads the whole worksheet again.
    fn refresh_sheet(&mut self) -> Result<(), String> {
        let table = self.call(
            "export.table",
            json!({ "table": "worksheet", "worksheet": self.worksheet }),
        )?;
        let names: Vec<String> = table
            .get("columns")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let at = |n: &str| names.iter().position(|c| c == n);
        let (Some(si), Some(ti), Some(ci)) = (at("subject"), at("time"), at("conc")) else {
            return Err("the worksheet has no subject, time and conc columns".to_owned());
        };
        let label = |v: &Value| match v {
            Value::String(s) => s.clone(),
            other => other
                .as_f64()
                .map(|x| format!("{x}"))
                .unwrap_or_else(|| other.to_string()),
        };
        self.sheet = table
            .get("rows")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|r| SheetRow {
                        subject: r.get(si).map(label).unwrap_or_default(),
                        time: r.get(ti).and_then(Value::as_f64).unwrap_or(f64::NAN),
                        conc: r.get(ci).and_then(Value::as_f64).unwrap_or(f64::NAN),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(())
    }

    /// The rows of the current subject, with their texts; the roles come later, from the scene.
    fn rebuild_table(&mut self) {
        self.table = self
            .sheet
            .iter()
            .enumerate()
            .filter(|(_, r)| r.subject == self.subject)
            .map(|(i, r)| TableRow {
                sheet_row: i,
                time: r.time,
                conc: r.conc,
                time_text: fmt::exact(r.time),
                conc_text: fmt::exact(r.conc),
                roles: Vec::new(),
            })
            .collect();
        self.refresh_roles();
    }

    // ---- running ---------------------------------------------------------------------------

    /// The parameters of `nca.run` for what the screen holds (the shape the egui page sends).
    pub fn params(&self) -> Value {
        let mut params = json!({
            "worksheet": self.worksheet,
            "subject": self.subject,
            "route": "extravascular",
            "dose": null,
        });
        if let Some(id) = self.analysis {
            params["analysis"] = json!(id);
        }
        if self.options.is_object() {
            params["options"] = self.options.clone();
        }
        params
    }

    /// Runs the analysis for the current subject and options, then reads the scene.
    pub fn run(&mut self) {
        let started = Instant::now();
        match self.call("nca.run", self.params()) {
            Ok(view) => {
                self.runs += 1;
                if let Some(run) = Run::from_value(&view) {
                    self.analysis = Some(run.id);
                    if run.options.is_object() {
                        self.options = run.options.clone();
                    }
                    self.error = match &run.subject.outcome {
                        Outcome::Error(message) => Some(fmt::plain(message)),
                        _ => None,
                    };
                    self.run = Some(run);
                    self.notice = None;
                }
                self.refresh_scene();
            }
            Err(message) => {
                self.runs += 1;
                self.error = Some(message);
            }
        }
        self.last_run = started.elapsed();
    }

    /// Reads `plot.scene` for the analysis (the linear or semi-log axis) and the status.
    pub fn refresh_scene(&mut self) {
        let Some(id) = self.analysis else { return };
        let answer = self.call(
            "plot.scene",
            json!({ "analysis": id, "subject": self.subject, "log": self.log }),
        );
        match answer {
            Ok(v) => {
                self.status =
                    Status::from_text(v.get("status").and_then(Value::as_str).unwrap_or_default());
                self.scene = v
                    .get("scene")
                    .and_then(|s| serde_json::from_value::<PlotScene>(s.clone()).ok());
            }
            Err(message) => {
                self.scene = None;
                // A subject whose analysis failed has no scene: the run's error says why.
                if self.error.is_none() {
                    self.error = Some(message);
                }
            }
        }
        self.refresh_roles();
    }

    /// The axis of the plot: semi-log (true) or linear (false).
    pub fn set_log(&mut self, log: bool) {
        if self.log != log {
            self.log = log;
            self.refresh_scene();
        }
    }

    // ---- roles ----------------------------------------------------------------------------

    /// The role of each point of the table, read from the scene and the answer.
    fn refresh_roles(&mut self) {
        let mut roles: HashMap<u64, Vec<Role>> = HashMap::new();
        let mut add = |time: f64, role: Role| {
            let list = roles.entry(time.to_bits()).or_default();
            if !list.contains(&role) {
                list.push(role);
            }
        };
        if let Some(scene) = &self.scene {
            if let Some(panel) = scene.panels.first() {
                let names: HashMap<&str, Role> = [
                    ("Observed", Role::Observed),
                    ("Replaced value", Role::Replaced),
                    ("Left out of λz", Role::Excluded),
                    ("Used for λz", Role::Used),
                    ("Start of the λz range", Role::Start),
                ]
                .into_iter()
                .collect();
                for series in &panel.series {
                    let Some(role) = names.get(series.name.as_str()) else {
                        continue;
                    };
                    for p in &series.points {
                        // Scene x is the time itself (the time axis is linear).
                        add(
                            p.x,
                            if p.blq && *role == Role::Observed {
                                Role::Blq
                            } else {
                                role.clone()
                            },
                        );
                    }
                }
                if let Some(hidden) = &panel.hidden {
                    for h in &hidden.points {
                        if let Some(role) = names.get(h.series.as_str()) {
                            add(h.x, role.clone());
                        }
                    }
                }
            }
        }
        if let Some(ok) = self.run.as_ref().and_then(Run::ok) {
            if let Some(tmax) = ok.number("tmax") {
                add(tmax, Role::Tmax);
            }
            for r in &ok.removed {
                add(r.time, Role::LeftOut(fmt::words(&r.reason)));
            }
        }
        for row in &mut self.table {
            let mut list = roles.remove(&row.time.to_bits()).unwrap_or_default();
            // The start of the range is also used for λz: say only the more precise.
            if list.contains(&Role::Start) {
                list.retain(|r| *r != Role::Used);
            }
            // A row that is observed and plays another part is not just "observed".
            if list.len() > 1 {
                list.retain(|r| *r != Role::Observed);
            }
            row.roles = list;
        }
    }

    // ---- the lambda z range ---------------------------------------------------------------

    /// The times of the current subject that can end a range: samples above zero.
    pub fn eligible_times(&self) -> Vec<f64> {
        let mut times: Vec<f64> = self
            .table
            .iter()
            .filter(|r| r.conc > 0.0 && r.time.is_finite())
            .map(|r| r.time)
            .collect();
        times.sort_by(f64::total_cmp);
        times.dedup();
        times
    }

    /// The times used for λz as the scene shows them.
    pub fn used_times(&self) -> Vec<f64> {
        let Some(panel) = self.scene.as_ref().and_then(|s| s.panels.first()) else {
            return Vec::new();
        };
        let mut used: Vec<f64> = panel
            .series
            .iter()
            .filter(|s| s.name == "Used for λz")
            .flat_map(|s| s.points.iter().map(|p| p.x))
            .collect();
        if let Some(hidden) = &panel.hidden {
            used.extend(
                hidden
                    .points
                    .iter()
                    .filter(|h| h.series == "Used for λz")
                    .map(|h| h.x),
            );
        }
        used.sort_by(f64::total_cmp);
        used
    }

    /// True when the terminal phase was chosen by hand.
    pub fn manual(&self) -> bool {
        self.options
            .pointer("/lambda_z_selection/manual")
            .is_some_and(|m| !m.is_null())
    }

    /// The range as it stands: the one chosen by hand, else the engine's selected candidate.
    pub fn range(&self) -> Option<(f64, f64)> {
        if let Some(r) = self.options.pointer("/lambda_z_selection/manual/range") {
            let start = r.get("start").and_then(Value::as_f64)?;
            let end = r.get("end").and_then(Value::as_f64)?;
            return Some((start, end));
        }
        if let Some(times) = self
            .options
            .pointer("/lambda_z_selection/manual/times")
            .and_then(Value::as_array)
        {
            let ts: Vec<f64> = times.iter().filter_map(Value::as_f64).collect();
            let lo = ts.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = ts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if lo.is_finite() && hi.is_finite() {
                return Some((lo, hi));
            }
        }
        self.run
            .as_ref()
            .and_then(Run::ok)
            .and_then(NcaOk::selected)
            .map(|c| (c.time_first, c.time_last))
    }

    fn set_manual(&mut self, manual: Value) {
        if !self.options.is_object() {
            self.options = json!({});
        }
        if !self
            .options
            .get("lambda_z_selection")
            .is_some_and(Value::is_object)
        {
            self.options["lambda_z_selection"] = json!({});
        }
        self.options["lambda_z_selection"]["manual"] = manual;
    }

    /// The eligible time nearest to `time`.
    fn snap(&self, time: f64) -> Option<f64> {
        self.eligible_times()
            .into_iter()
            .min_by(|a, b| (a - time).abs().total_cmp(&(b - time).abs()))
    }

    /// The handler of a drag of one end of the range to `time`: it snaps to the nearest sample
    /// above zero and, when that changes the range, asks the engine again. True when it did.
    pub fn drag_handle(&mut self, handle: Handle, time: f64) -> bool {
        let Some(snapped) = self.snap(time) else {
            return false;
        };
        let Some((start, end)) = self.range() else {
            return false;
        };
        let next = match handle {
            Handle::Start => (snapped, end),
            Handle::End => (start, snapped),
        };
        if next == (start, end) && self.manual() {
            return false;
        }
        if next.0 >= next.1 {
            self.notice = Some(
                "The start of the range must stay before its end: move the other handle first."
                    .to_owned(),
            );
            return false;
        }
        self.set_manual(json!({ "range": { "start": next.0, "end": next.1 } }));
        self.run();
        true
    }

    /// Moves a handle by `points` samples (negative: earlier). True when the engine was asked.
    pub fn step_handle(&mut self, handle: Handle, points: i32) -> bool {
        let times = self.eligible_times();
        let Some((start, end)) = self.range() else {
            return false;
        };
        let from = match handle {
            Handle::Start => start,
            Handle::End => end,
        };
        let Some(at) = times.iter().position(|t| *t == from) else {
            return false;
        };
        let target = at as i64 + i64::from(points);
        let Some(time) = usize::try_from(target)
            .ok()
            .and_then(|i| times.get(i).copied())
        else {
            self.notice = Some("That handle is already at the end of the profile.".to_owned());
            return false;
        };
        self.drag_handle(handle, time)
    }

    /// A click on a point: it joins the hand-picked terminal phase or leaves it. The phase keeps
    /// at least two points.
    pub fn toggle_point(&mut self, time: f64) {
        let mut times = self.used_times();
        if let Some(i) = times.iter().position(|t| *t == time) {
            times.remove(i);
        } else {
            times.push(time);
            times.sort_by(f64::total_cmp);
        }
        if times.len() < 2 {
            self.notice = Some(
                "The terminal phase needs at least 2 points: keep one more selected before removing this one."
                    .to_owned(),
            );
            return;
        }
        self.set_manual(json!({ "times": times }));
        self.run();
    }

    /// A row of the candidates list: its range becomes the chosen one.
    pub fn pick_candidate(&mut self, index: usize) {
        let Some(c) = self
            .run
            .as_ref()
            .and_then(Run::ok)
            .and_then(|ok| ok.lambda_z_candidates.get(index))
            .cloned()
        else {
            return;
        };
        self.set_manual(json!({ "range": { "start": c.time_first, "end": c.time_last } }));
        self.run();
    }

    /// Gives the choice of the range back to the engine.
    pub fn back_to_automatic(&mut self) {
        if self.manual() {
            self.set_manual(Value::Null);
            self.run();
        }
    }

    // ---- subject and data -----------------------------------------------------------------

    /// Changes the subject; the hand-picked phase of the previous one is forgotten. `run`: ask the
    /// engine now (the measure mode runs once, with the drag).
    pub fn select_subject(&mut self, subject: &str, run: bool) {
        if !self.subjects.iter().any(|s| s == subject) {
            return;
        }
        self.subject = subject.to_owned();
        if self.manual() {
            self.set_manual(Value::Null);
        }
        self.rebuild_table();
        if run {
            self.run();
        }
    }

    /// The next (`1`) or previous (`-1`) subject.
    pub fn step_subject(&mut self, by: i32) {
        let Some(at) = self.subjects.iter().position(|s| *s == self.subject) else {
            return;
        };
        let target = at as i64 + i64::from(by);
        if let Some(next) = usize::try_from(target)
            .ok()
            .and_then(|i| self.subjects.get(i).cloned())
        {
            self.select_subject(&next, true);
        }
    }

    /// Edits a cell (`time` or `conc`) of a row of the worksheet. The analysis then shows `stale`
    /// until it runs again (UX-FR-12).
    pub fn edit_cell(&mut self, sheet_row: usize, column: &str, text: &str) {
        let answer = self.call(
            "data.set_cell",
            json!({ "worksheet": self.worksheet, "column": column, "row": sheet_row, "value": text }),
        );
        match answer {
            Ok(_) => {
                self.error = None;
                let _ = self.refresh_sheet();
                self.rebuild_table();
                self.refresh_scene();
            }
            Err(message) => {
                self.error = Some(message);
                self.rebuild_table();
            }
        }
    }

    // ---- results --------------------------------------------------------------------------

    fn unit_of(&self, name: &str) -> String {
        fmt::parameter_unit(
            name,
            self.time_unit.as_deref(),
            self.conc_unit.as_deref(),
            &self.derived_units,
        )
        .unwrap_or_default()
    }

    fn cell(&self, name: &str, label: String, ok: &NcaOk, flagged: &[&str]) -> Option<Cell> {
        let value = ok.get(name)?;
        let not_calculated = self
            .run
            .as_ref()
            .and_then(|r| r.subject.not_calculated_messages.get(name).cloned());
        Some(match (value.value, &value.not_calculated) {
            (Some(x), _) => Cell {
                name: name.to_owned(),
                label,
                text: shown_number(name, x),
                unit: self.unit_of(name),
                flagged: flagged.contains(&name),
                why: None,
            },
            (None, reason) => {
                let reason = reason.clone().unwrap_or_default();
                Cell {
                    name: name.to_owned(),
                    label,
                    text: "not calculated".to_owned(),
                    unit: fmt::words(&reason),
                    flagged: false,
                    why: Some(not_calculated.unwrap_or_else(|| fmt::words(&reason))),
                }
            }
        })
    }

    /// The headline parameters, each with its label, text, unit and flag.
    pub fn headline(&self) -> Vec<Cell> {
        let Some(ok) = self.run.as_ref().and_then(Run::ok) else {
            return Vec::new();
        };
        let flagged: Vec<&str> = ok.flags.iter().map(|f| f.concerns()).collect();
        SUMMARY
            .iter()
            .filter_map(|n| self.cell(n, fmt::parameter_label(n, true), ok, &flagged))
            .collect()
    }

    /// Every parameter of the answer.
    pub fn all_parameters(&self) -> Vec<Cell> {
        let Some(ok) = self.run.as_ref().and_then(Run::ok) else {
            return Vec::new();
        };
        ok.parameters
            .iter()
            .filter_map(|p| self.cell(&p.name, p.name.clone(), ok, &[]))
            .collect()
    }

    /// The cells as tab-separated text with a header row; the digits are those shown.
    pub fn tsv(cells: &[Cell]) -> String {
        let mut out = String::from("Parameter\tValue\tUnit\n");
        for c in cells {
            out.push_str(&format!("{}\t{}\t{}\n", c.label, c.text, c.unit));
        }
        out
    }

    /// The texts the results should show, worked out from the raw JSON of the answer alone (a way
    /// that shares no code with the screen but the formatting crate): `(key, text)`.
    pub fn expected_cells(&self) -> Vec<(String, String)> {
        let Some(run) = &self.run else {
            return Vec::new();
        };
        let Some(params) = run.raw_parameters.as_array() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for p in params {
            let name = p.get("name").and_then(Value::as_str).unwrap_or_default();
            let value = p.pointer("/value/value").and_then(Value::as_f64);
            let reason = p
                .pointer("/value/not_calculated")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if SUMMARY.contains(&name) {
                out.push((
                    format!("headline:{name}"),
                    value.map_or_else(|| "not calculated".to_owned(), |x| shown_number(name, x)),
                ));
            }
            out.push((
                format!("all:{name}"),
                value.map_or_else(|| fmt::words(reason), |x| shown_number(name, x)),
            ));
        }
        out
    }

    /// The terminal-phase candidates of the answer.
    pub fn candidates(&self) -> Vec<Candidate> {
        self.run
            .as_ref()
            .and_then(Run::ok)
            .map(|ok| ok.lambda_z_candidates.clone())
            .unwrap_or_default()
    }

    /// The worksheet rows of the whole dataset (the measure mode reads the times of a subject).
    pub fn sheet_rows(&self) -> &[SheetRow] {
        &self.sheet
    }

    pub fn derived_units(&self) -> &std::collections::BTreeMap<String, String> {
        &self.derived_units
    }
}
