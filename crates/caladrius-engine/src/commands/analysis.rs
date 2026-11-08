//! `nca.run`, `fit.run`, `fit.initial_estimates`, `model.simulate`, `analysis.run`,
//! `analysis.get`, `export.table`.

use std::collections::BTreeMap;

use caladrius_models::{ModelId, ModelInput};
use caladrius_nca::{NcaOptions, Route};
use caladrius_project::{
    AnalysisId, AnalysisResult, AnalysisSpec, AnalysisStatus, FitSpec, NcaSpec, Project,
    SimulationSpec, WorksheetId,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{CommandDef, parse, respond};
use crate::Engine;
use crate::error::CommandError;
use crate::export;
use crate::run;
use crate::schema::{
    array_of, boolean, integer, nullable, number, object, one_of_strings, reference, root, string,
};

/// The analysis as a machine and a person read it.
pub(crate) fn view(project: &Project, id: AnalysisId) -> Result<Value, CommandError> {
    let a = project.analysis(id)?;
    let mut value = respond(&json!({
        "id": a.id(),
        "label": project.label_of(id)?,
        "name": a.name(),
        "kind": a.spec().kind(),
        "spec": a.spec(),
        "status": project.status(id)?,
        "result": a.result(),
    }))?;
    add_flag_messages(a.result(), &mut value);
    Ok(value)
}

/// Writes the sentence of each quality flag next to the flags (`flag_messages`, in the same
/// order) and the sentence of each parameter that was not calculated (`not_calculated_messages`,
/// by name), so a client shows what to check without holding the wording itself.
fn add_flag_messages(result: Option<&AnalysisResult>, view: &mut Value) {
    match result {
        Some(AnalysisResult::Nca { subjects }) => {
            for (i, s) in subjects.iter().enumerate() {
                let Some(ok) = s.outcome.ok() else { continue };
                let messages: Vec<String> = ok.flags().iter().map(ToString::to_string).collect();
                // And the sentence of each parameter that was not calculated.
                let reasons: std::collections::BTreeMap<&str, String> = ok
                    .parameters()
                    .iter()
                    .filter_map(|p| p.value.reason().map(|r| (p.name.as_str(), r.to_string())))
                    .collect();
                if let Some(slot) = view.pointer_mut(&format!("/result/subjects/{i}")) {
                    slot["flag_messages"] = json!(messages);
                    slot["not_calculated_messages"] = json!(reasons);
                }
            }
        }
        Some(AnalysisResult::Fit(run)) => {
            if let Some(ok) = run.outcome.ok() {
                let messages: Vec<String> = ok.flags().iter().map(ToString::to_string).collect();
                if let Some(slot) = view.pointer_mut("/result") {
                    slot["flag_messages"] = json!(messages);
                    slot["status_message"] = json!(ok.status().message());
                }
            }
        }
        Some(AnalysisResult::Fits { subjects }) => {
            for (i, run) in subjects.iter().enumerate() {
                let Some(ok) = run.outcome.ok() else { continue };
                let messages: Vec<String> = ok.flags().iter().map(ToString::to_string).collect();
                if let Some(slot) = view.pointer_mut(&format!("/result/subjects/{i}")) {
                    slot["flag_messages"] = json!(messages);
                    slot["status_message"] = json!(ok.status().message());
                }
            }
        }
        _ => {}
    }
}

/// Fails early when `analysis` does not exist or is of another kind.
pub(crate) fn check_target(
    project: &Project,
    analysis: Option<AnalysisId>,
    kind: &str,
) -> Result<(), CommandError> {
    if let Some(id) = analysis {
        let existing = project.analysis(id)?.spec().kind();
        if existing != kind {
            return Err(CommandError::new(
                "analysis_kind_changed",
                format!(
                    "analysis {id} is a {existing} analysis; use its own command or create a new analysis"
                ),
            ));
        }
    }
    Ok(())
}

/// Stores a finished run: updates the analysis `analysis`, or creates one.
pub(crate) fn store(
    project: &mut Project,
    analysis: Option<AnalysisId>,
    name: Option<&str>,
    spec: AnalysisSpec,
    result: AnalysisResult,
) -> Result<AnalysisId, CommandError> {
    let id = match analysis {
        Some(id) => {
            project.update_spec(id, spec)?;
            if name.is_some() {
                project.rename_analysis(id, name)?;
            }
            id
        }
        None => project.add_analysis(spec, name)?,
    };
    project.set_result(id, result)?;
    Ok(id)
}

fn spec_properties() -> Vec<(&'static str, Value)> {
    vec![
        (
            "analysis",
            crate::schema::described(
                reference("Id"),
                "Update and re-run this analysis instead of creating a new one.",
            ),
        ),
        ("name", string()),
        ("worksheet", reference("Id")),
        (
            "subject",
            crate::schema::described(
                crate::schema::subject(),
                "One subject, a label as text or number. Omitted or null: every subject (NCA), or the only subject (fit; with several subjects the fit is refused unless one is named or `all` is given). For `fit.run`, the text `all` fits every subject on its own (one independent fit per subject, same model and options), unless a subject is labelled `all`.",
            ),
        ),
    ]
}

fn view_schema() -> Value {
    reference("AnalysisView")
}

// ---- nca.run -----------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NcaRunParams {
    #[serde(default)]
    analysis: Option<AnalysisId>,
    #[serde(default)]
    name: Option<String>,
    worksheet: WorksheetId,
    #[serde(default, deserialize_with = "super::label")]
    subject: Option<String>,
    #[serde(default)]
    route: Option<Route>,
    #[serde(default)]
    dose: Option<f64>,
    #[serde(default)]
    options: NcaOptions,
}

fn nca_run(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: NcaRunParams = parse("nca.run", params)?;
    check_target(&engine.project, p.analysis, "nca")?;
    let spec = AnalysisSpec::Nca(NcaSpec {
        worksheet: p.worksheet,
        subject: p.subject,
        route: p.route,
        dose: p.dose,
        options: p.options,
    });
    let result = run::run_spec(&engine.project, &spec)?;
    let id = store(
        &mut engine.project,
        p.analysis,
        p.name.as_deref(),
        spec,
        result,
    )?;
    view(&engine.project, id)
}

pub(crate) const NCA_RUN: CommandDef = CommandDef {
    id: "nca.run",
    title: "Run a non-compartmental analysis",
    description: "Runs the NCA on the profiles of a worksheet (one subject, or every subject when `subject` is omitted) and stores the result in an analysis object (new, or `analysis` updated). Each subject gives its parameters by PKNCA name, each a value or `not_calculated` with a reason, or an error saying what to fix; a subject that the data check refused carries the sentence of its check, and the others run. The route and dose come from the arguments or from the route and dose columns. With an event column (a dose row and observation rows per subject) a subject uses its observation rows and its first dose row: amount, time (the time axis starts at the dose, and observation rows before the dose are left out with the reason `before_dose`), route, and for an infusion the duration from `dur` or `rate`; a `route` in the arguments wins over the file, with a note. A subject with more than one dose row is refused for that subject only. Each result carries the declared covariates of its subject, copied and used nowhere else.",
    mutates: true,
    params: || {
        let mut props = spec_properties();
        props.extend([
            ("route", nullable(reference("Route"))),
            ("dose", nullable(number())),
            ("options", reference("NcaOptions")),
        ]);
        root("nca.run parameters", object(props, &["worksheet"]))
    },
    result: || root("nca.run result", view_schema()),
    example: || json!({ "worksheet": 1, "route": "extravascular" }),
    run: nca_run,
};

// ---- fit.run -----------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FitRunParams {
    #[serde(default)]
    analysis: Option<AnalysisId>,
    #[serde(default)]
    name: Option<String>,
    worksheet: WorksheetId,
    #[serde(default, deserialize_with = "super::label")]
    subject: Option<String>,
    model: ModelId,
    #[serde(default)]
    dose: Option<f64>,
    #[serde(default)]
    weighting: caladrius_fit::Weighting,
    #[serde(default)]
    initial: BTreeMap<String, f64>,
    #[serde(default)]
    options: caladrius_fit::FitOptions,
}

/// What `fit.run` keeps from its parameters between `begin` and `finish`.
pub(crate) struct FitJobData {
    analysis: Option<AnalysisId>,
    name: Option<String>,
    spec: AnalysisSpec,
    meta: run::FitMeta,
}

/// The first half of `fit.run`: the parameters, the target analysis, the observations and the
/// initial estimates. Nothing is stored and the iterations have not begun.
pub(crate) fn begin_fit(
    engine: &Engine,
    params: Value,
) -> Result<(caladrius_fit::FitRun<ModelId>, FitJobData), CommandError> {
    let p: FitRunParams = parse("fit.run", params)?;
    check_target(&engine.project, p.analysis, "fit")?;
    if run::wants_all_subjects(&engine.project, p.worksheet, p.subject.as_deref()) {
        return Err(CommandError::new(
            "subject_all_not_a_job",
            "`subject` = all runs one fit per subject in a single step and cannot run as a job (no progress, no cancel); use `execute`",
        ));
    }
    let spec = FitSpec {
        worksheet: p.worksheet,
        subject: p.subject,
        model: p.model,
        dose: p.dose,
        weighting: p.weighting,
        initial: p.initial,
        options: p.options,
    };
    let (run, meta) = run::start_fit(&engine.project, &spec)?;
    Ok((
        run,
        FitJobData {
            analysis: p.analysis,
            name: p.name,
            spec: AnalysisSpec::Fit(spec),
            meta,
        },
    ))
}

/// The second half of `fit.run`: stores the finished fit and answers with the analysis.
pub(crate) fn finish_fit(
    engine: &mut Engine,
    data: FitJobData,
    result: caladrius_fit::FitResult,
) -> Result<Value, CommandError> {
    let id = store(
        &mut engine.project,
        data.analysis,
        data.name.as_deref(),
        data.spec,
        run::fit_analysis(data.meta, result),
    )?;
    view(&engine.project, id)
}

/// `execute("fit.run")`: begin, run with nobody watching, finish. `execute` records the entry.
///
/// With `subject` = all (DAT-USE-02) the fits are independent and run one after the other in this
/// call, without the job machinery: one analysis is stored with a list of per-subject results.
fn fit_run(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: FitRunParams = parse("fit.run", params.clone())?;
    if run::wants_all_subjects(&engine.project, p.worksheet, p.subject.as_deref()) {
        check_target(&engine.project, p.analysis, "fit")?;
        let spec = FitSpec {
            worksheet: p.worksheet,
            subject: p.subject,
            model: p.model,
            dose: p.dose,
            weighting: p.weighting,
            initial: p.initial,
            options: p.options,
        };
        let result = run::run_fit_all(&engine.project, &spec)?;
        let id = store(
            &mut engine.project,
            p.analysis,
            p.name.as_deref(),
            AnalysisSpec::Fit(spec),
            result,
        )?;
        return view(&engine.project, id);
    }
    let job = engine.begin_job(crate::job::FIT_RUN, params)?;
    let completed = job.run(&mut crate::job::NeverCancel);
    match engine.finish_job(completed) {
        crate::job::Settled::Recorded(outcome) => outcome,
        crate::job::Settled::Unrecorded(e) => Err(e),
    }
}

fn initial_property() -> (&'static str, Value) {
    (
        "initial",
        crate::schema::described(
            json!({ "type": "object", "additionalProperties": number() }),
            "Initial estimates by parameter name; the fitted parameters are exactly those named. Empty: generated from the data (not for pk2: give every parameter).",
        ),
    )
}

pub(crate) const FIT_RUN: CommandDef = CommandDef {
    id: "fit.run",
    title: "Fit a model to a profile",
    description: "Weighted least-squares fit (Gauss-Newton with the Levenberg and Hartley modification) of a one- or two-compartment model to the profile of one subject; stores the result in an analysis object. The result has the status (converged or why not), estimates with standard error, CV% and confidence intervals, diagnostics, observed and predicted values, the smooth curve, the trace, and quality flags. A fit that cannot start is an error and stores nothing. A worksheet with several subjects needs `subject` (one label); without it the fit is refused with `subject_required`. `subject` = all fits every subject on its own, with the same model and options and the initial estimates generated per subject (`initial` and `dose`, when given, apply to each): the result is a list in `subjects`, one fit per subject in order of appearance, and a subject that cannot be fitted is that subject's result with its error (a subject that the data check refused carries the sentence of its check) while the others run. `all` is not a job: it has no progress and no cancel. For an infusion model (pk1.iv_infusion, pk2.iv_infusion) on an event table, `dur` is held at the duration of the subject's dose row unless `options.fixed.dur` or `initial.dur` says otherwise. Each result carries the declared covariates of its subject, copied and used nowhere else.",
    mutates: true,
    params: || {
        let mut props = spec_properties();
        props.extend([
            ("model", reference("ModelId")),
            ("dose", nullable(number())),
            ("weighting", reference("Weighting")),
            initial_property(),
            ("options", reference("FitOptions")),
        ]);
        root("fit.run parameters", object(props, &["worksheet", "model"]))
    },
    result: || root("fit.run result", view_schema()),
    example: || json!({ "worksheet": 1, "model": "pk1.oral_1" }),
    run: fit_run,
};

// ---- fit.initial_estimates ---------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialParams {
    worksheet: WorksheetId,
    #[serde(default, deserialize_with = "super::label")]
    subject: Option<String>,
    model: ModelId,
    #[serde(default)]
    dose: Option<f64>,
    #[serde(default)]
    fixed: BTreeMap<String, f64>,
}

fn fit_initial_estimates(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: InitialParams = parse("fit.initial_estimates", params)?;
    let ws = engine.project.worksheet(p.worksheet)?;
    let data = run::fit_data(ws, p.subject.as_deref(), p.dose)?;
    let mut fixed = p.fixed.clone();
    let mut notes = Vec::new();
    run::file_duration(
        p.model,
        data.duration,
        &data.subject,
        &mut fixed,
        &BTreeMap::new(),
        &mut notes,
    );
    let initial = run::initial_estimates(&data, p.model, &fixed)?;
    respond(&json!({
        "subject": data.subject,
        "dose": data.dose,
        "n_observations": data.time.len(),
        "initial": initial,
    }))
}

pub(crate) const FIT_INITIAL_ESTIMATES: CommandDef = CommandDef {
    id: "fit.initial_estimates",
    title: "Initial estimates from the data",
    description: "Estimates the starting values of a model's parameters from one subject's profile (log-linear regression, curve stripping, terminal phase). Reads only: nothing is stored. A zero-order input model needs its duration among `fixed`. Refused for pk2: give the initial values.",
    mutates: false,
    params: || {
        root(
            "fit.initial_estimates parameters",
            object(
                vec![
                    ("worksheet", reference("Id")),
                    ("subject", crate::schema::subject()),
                    ("model", reference("ModelId")),
                    ("dose", nullable(number())),
                    (
                        "fixed",
                        json!({ "type": "object", "additionalProperties": number() }),
                    ),
                ],
                &["worksheet", "model"],
            ),
        )
    },
    result: || {
        root(
            "fit.initial_estimates result",
            object(
                vec![
                    ("subject", string()),
                    ("dose", number()),
                    ("n_observations", integer()),
                    (
                        "initial",
                        json!({ "type": "object", "additionalProperties": number() }),
                    ),
                ],
                &["subject", "dose", "n_observations", "initial"],
            ),
        )
    },
    example: || json!({ "worksheet": 1, "model": "pk1.oral_1" }),
    run: fit_initial_estimates,
};

// ---- model.simulate ----------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Grid {
    start: f64,
    end: f64,
    points: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SimulateParams {
    model: ModelId,
    dose: f64,
    params: BTreeMap<String, f64>,
    #[serde(default)]
    times: Vec<f64>,
    #[serde(default)]
    grid: Option<Grid>,
    #[serde(default)]
    store: bool,
    #[serde(default)]
    name: Option<String>,
    /// With `store`: update this simulation instead of creating another one.
    #[serde(default)]
    analysis: Option<AnalysisId>,
}

fn grid_times(g: &Grid) -> Result<Vec<f64>, CommandError> {
    if !(g.start.is_finite() && g.end.is_finite() && g.start < g.end) {
        return Err(CommandError::invalid(
            "model.simulate",
            "`grid` needs finite `start` < `end`",
        ));
    }
    if !(2..=100_000).contains(&g.points) {
        return Err(CommandError::invalid(
            "model.simulate",
            "`grid.points` must be between 2 and 100000",
        ));
    }
    let step = (g.end - g.start) / (g.points - 1) as f64;
    Ok((0..g.points)
        .map(|i| {
            if i + 1 == g.points {
                g.end
            } else {
                g.start + step * i as f64
            }
        })
        .collect())
}

fn model_simulate(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: SimulateParams = parse("model.simulate", params)?;
    let times = match (&p.grid, p.times.is_empty()) {
        (Some(_), false) => {
            return Err(CommandError::invalid(
                "model.simulate",
                "give `times` or `grid`, not both",
            ));
        }
        (Some(g), true) => grid_times(g)?,
        (None, false) => p.times.clone(),
        (None, true) => {
            return Err(CommandError::invalid(
                "model.simulate",
                "give `times` (a list) or `grid` ({start, end, points})",
            ));
        }
    };
    let input = ModelInput {
        model: p.model,
        dose: p.dose,
        params: p.params,
        times: times.clone(),
    };
    let output = run::simulate(&input)?;
    if p.analysis.is_some() && !p.store {
        return Err(CommandError::invalid(
            "model.simulate",
            "`analysis` names the simulation to update, so it needs `store: true`",
        ));
    }
    let analysis = if p.store {
        check_target(&engine.project, p.analysis, "simulation")?;
        let spec = AnalysisSpec::Simulation(SimulationSpec {
            input: input.clone(),
        });
        let result = run::run_spec(&engine.project, &spec)?;
        Some(store(
            &mut engine.project,
            p.analysis,
            p.name.as_deref(),
            spec,
            result,
        )?)
    } else {
        None
    };
    respond(&json!({
        "analysis": analysis,
        "times": times,
        "conc": output.conc(),
        "auc": output.auc(),
        "secondary": output.secondary(),
    }))
}

pub(crate) const MODEL_SIMULATE: CommandDef = CommandDef {
    id: "model.simulate",
    title: "Evaluate a model on a time grid",
    description: "Concentration and AUC of a one- or two-compartment model at the given times (a list, or a regular grid), and its secondary parameters (half-life, clearance, AUC to infinity, MRT, predicted Cmax and Tmax...). Nothing is stored unless `store` is true (with `analysis`, that simulation is updated instead of a new one being created), so it can be called at every change of a parameter for a live curve.",
    mutates: false,
    params: || {
        root(
            "model.simulate parameters",
            object(
                vec![
                    ("model", reference("ModelId")),
                    ("dose", json!({ "type": "number", "minimum": 0 })),
                    (
                        "params",
                        crate::schema::described(
                            json!({ "type": "object", "additionalProperties": number() }),
                            "Parameters by name: pk1 `v` and one of `cl`, `k`; pk2 one set (see model); `ka`, `dur`, `tlag` as needed. Optional regimen: `tau` (steady state, times since the last dose), `tau` and `n_doses` (times since the first dose), or a schedule `dose_time[i]`, `dose_amount[i]`, `dose_dur[i]`.",
                        ),
                    ),
                    ("times", array_of(number())),
                    (
                        "grid",
                        object(
                            vec![
                                ("start", number()),
                                ("end", number()),
                                (
                                    "points",
                                    json!({ "type": "integer", "minimum": 2, "maximum": 100_000 }),
                                ),
                            ],
                            &["start", "end", "points"],
                        ),
                    ),
                    ("store", boolean()),
                    ("name", string()),
                    (
                        "analysis",
                        crate::schema::described(
                            reference("Id"),
                            "With `store`: update and re-run this simulation instead of creating a new one.",
                        ),
                    ),
                ],
                &["model", "dose", "params"],
            ),
        )
    },
    result: || {
        root(
            "model.simulate result",
            object(
                vec![
                    ("analysis", nullable(reference("Id"))),
                    ("times", array_of(number())),
                    ("conc", array_of(number())),
                    ("auc", array_of(number())),
                    (
                        "secondary",
                        json!({ "type": "object", "additionalProperties": number() }),
                    ),
                ],
                &["analysis", "times", "conc", "auc", "secondary"],
            ),
        )
    },
    example: || {
        json!({
            "model": "pk1.oral_1",
            "dose": 100,
            "params": { "v": 20, "k": 0.1, "ka": 1.2 },
            "grid": { "start": 0, "end": 24, "points": 25 },
        })
    },
    run: model_simulate,
};

// ---- analysis.run, analysis.get ----------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalysisParams {
    analysis: AnalysisId,
}

fn analysis_run(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: AnalysisParams = parse("analysis.run", params)?;
    let spec = engine.project.analysis(p.analysis)?.spec().clone();
    let result = run::run_spec(&engine.project, &spec)?;
    engine.project.set_result(p.analysis, result)?;
    view(&engine.project, p.analysis)
}

pub(crate) const ANALYSIS_RUN: CommandDef = CommandDef {
    id: "analysis.run",
    title: "Run an analysis again",
    description: "Re-runs an analysis with its stored options on the current data; use it when its status is stale.",
    mutates: true,
    params: || {
        root(
            "analysis.run parameters",
            object(vec![("analysis", reference("Id"))], &["analysis"]),
        )
    },
    result: || root("analysis.run result", view_schema()),
    example: || json!({ "analysis": 2 }),
    run: analysis_run,
};

fn analysis_get(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: AnalysisParams = parse("analysis.get", params)?;
    view(&engine.project, p.analysis)
}

pub(crate) const ANALYSIS_GET: CommandDef = CommandDef {
    id: "analysis.get",
    title: "Get an analysis",
    description: "The options, the last result and the status of an analysis: `no_result`, `fresh`, or `stale` with the reasons (the data or the options changed since the run).",
    mutates: false,
    params: || {
        root(
            "analysis.get parameters",
            object(vec![("analysis", reference("Id"))], &["analysis"]),
        )
    },
    result: || root("analysis.get result", view_schema()),
    example: || json!({ "analysis": 2 }),
    run: analysis_get,
};

// ---- export.table ------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportParams {
    table: String,
    #[serde(default)]
    analysis: Option<AnalysisId>,
    #[serde(default)]
    worksheet: Option<WorksheetId>,
}

fn export_table(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: ExportParams = parse("export.table", params)?;
    let project = &engine.project;
    let (table, label, status) = if p.table == "worksheet" {
        let id = p.worksheet.ok_or_else(|| {
            CommandError::invalid("export.table", "the table `worksheet` needs `worksheet`")
        })?;
        let ws = project.worksheet(id)?;
        (export::worksheet_table(ws), ws.name().to_owned(), None)
    } else {
        let id = p.analysis.ok_or_else(|| {
            CommandError::invalid(
                "export.table",
                format!("the table `{}` needs `analysis`", p.table),
            )
        })?;
        let analysis = project.analysis(id)?;
        let table = export::analysis_table(&p.table, analysis)?;
        let status = match project.status(id)? {
            AnalysisStatus::Fresh => "fresh",
            AnalysisStatus::Stale { .. } => "stale",
            AnalysisStatus::NoResult => "no_result",
        };
        (table, project.label_of(id)?, Some(status))
    };
    respond(&json!({
        "table": p.table,
        "file_name": format!("{}-{}.csv", export::slug(&label), export::slug(&p.table)),
        "status": status,
        "columns": table.columns,
        "rows": table.json_rows(),
        "csv": table.to_csv(),
    }))
}

pub(crate) const EXPORT_TABLE: CommandDef = CommandDef {
    id: "export.table",
    title: "Export a result table",
    description: "A table of a worksheet or of the last result of an analysis, as columns and rows and as CSV text (the caller writes the file). `status` says whether the analysis result is fresh or stale. Tables: worksheet; nca.parameters (one row per subject and parameter); fit.parameters, fit.values, fit.observations, fit.curve, fit.trace; simulation; plot.points (one row per point of a plot: panel, series, x, y, blq).",
    mutates: false,
    params: || {
        root(
            "export.table parameters",
            object(
                vec![
                    ("table", one_of_strings(&export::TABLES)),
                    ("analysis", reference("Id")),
                    ("worksheet", reference("Id")),
                ],
                &["table"],
            ),
        )
    },
    result: || {
        root(
            "export.table result",
            object(
                vec![
                    ("table", string()),
                    ("file_name", string()),
                    (
                        "status",
                        nullable(one_of_strings(&["fresh", "stale", "no_result"])),
                    ),
                    ("columns", array_of(string())),
                    ("rows", array_of(json!({ "type": "array" }))),
                    ("csv", string()),
                ],
                &["table", "file_name", "status", "columns", "rows", "csv"],
            ),
        )
    },
    example: || json!({ "table": "nca.parameters", "analysis": 2 }),
    run: export_table,
};

// ---- analysis.remove ---------------------------------------------------------------------

fn analysis_remove(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: AnalysisParams = parse("analysis.remove", params)?;
    engine.project.remove_analysis(p.analysis)?;
    let mut out = super::project::overview(engine)?;
    if let Some(map) = out.as_object_mut() {
        map.insert("removed".to_owned(), json!({ "analyses": [p.analysis] }));
    }
    Ok(out)
}

pub(crate) const ANALYSIS_REMOVE: CommandDef = CommandDef {
    id: "analysis.remove",
    title: "Remove an analysis",
    description: "Removes an analysis object with its options and result. Returns the project overview and what was removed.",
    mutates: true,
    params: || {
        root(
            "analysis.remove parameters",
            object(vec![("analysis", reference("Id"))], &["analysis"]),
        )
    },
    result: || {
        let mut schema = super::project::overview_schema();
        super::data::removed_schema(&mut schema, false);
        root("analysis.remove result", schema)
    },
    example: || json!({ "analysis": 3 }),
    run: analysis_remove,
};

// ---- fit.evaluate ------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluateParams {
    worksheet: WorksheetId,
    #[serde(default, deserialize_with = "super::label")]
    subject: Option<String>,
    model: ModelId,
    #[serde(default)]
    dose: Option<f64>,
    #[serde(default)]
    weighting: caladrius_fit::Weighting,
    #[serde(default)]
    initial: BTreeMap<String, f64>,
    #[serde(default)]
    options: caladrius_fit::FitOptions,
}

fn fit_evaluate(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: EvaluateParams = parse("fit.evaluate", params)?;
    let ws = engine.project.worksheet(p.worksheet)?;
    let data = run::fit_data(ws, p.subject.as_deref(), p.dose)?;
    // The weighted sum of squares at the starting values is the first row of the trace of a fit
    // that is allowed one iteration (the fit crate computes it; nothing is computed here).
    let mut options = p.options;
    options.max_iterations = 1;
    options.n_curve = 0;
    let input = caladrius_fit::FitInput {
        model: p.model,
        dose: data.dose,
        time: data.time.clone(),
        conc: data.conc.clone(),
        weighting: p.weighting,
        initial: p.initial,
        options,
    };
    let result =
        caladrius_fit::run(&input).map_err(|e| CommandError::new("fit_error", e.to_string()))?;
    let Some(start) = result.trace().first() else {
        return Err(CommandError::new(
            "fit_error",
            "the fit gave no trace, so the objective at the starting values is not known",
        ));
    };
    let starting: BTreeMap<&String, f64> = result
        .parameters()
        .iter()
        .zip(start.estimates.iter().copied())
        .collect();
    respond(&json!({
        "subject": data.subject,
        "dose": data.dose,
        "n_observations": data.time.len(),
        "wrss": start.wrss,
        "starting_values": starting,
    }))
}

pub(crate) const FIT_EVALUATE: CommandDef = CommandDef {
    id: "fit.evaluate",
    title: "Objective at the starting values",
    description: "The weighted sum of squares of one subject's observations for the given starting values (or the generated ones when none are given), as the first row of a fit's trace. Reads only: nothing is stored. A page uses it to show how far the starting curve is from the data while the person edits the estimates.",
    mutates: false,
    params: || {
        root(
            "fit.evaluate parameters",
            object(
                vec![
                    ("worksheet", reference("Id")),
                    ("subject", crate::schema::subject()),
                    ("model", reference("ModelId")),
                    ("dose", nullable(number())),
                    ("weighting", reference("Weighting")),
                    initial_property(),
                    ("options", reference("FitOptions")),
                ],
                &["worksheet", "model"],
            ),
        )
    },
    result: || {
        root(
            "fit.evaluate result",
            object(
                vec![
                    ("subject", string()),
                    ("dose", number()),
                    ("n_observations", integer()),
                    ("wrss", number()),
                    (
                        "starting_values",
                        json!({ "type": "object", "additionalProperties": number() }),
                    ),
                ],
                &[
                    "subject",
                    "dose",
                    "n_observations",
                    "wrss",
                    "starting_values",
                ],
            ),
        )
    },
    example: || json!({ "worksheet": 1, "model": "pk1.oral_1", "initial": { "v": 20, "k": 0.1, "ka": 1.2 } }),
    run: fit_evaluate,
};
