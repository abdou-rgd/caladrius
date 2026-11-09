//! `plot.xy`, `plot.scene` and `export.plot`: XY plots as objects of the workflow (T-110), and
//! the scene that the screen and the export both draw (T-133).

use caladrius_project::{
    AnalysisId, AnalysisResult, AnalysisSpec, AnalysisStatus, ColumnRole, FitView, Outcome,
    PlotLayout, PlotRun, PlotScale, PlotSource, PlotSpec, WorksheetId,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::analysis::{check_target, store, view};
use super::{CommandDef, parse, respond};
use crate::Engine;
use crate::error::CommandError;
use crate::export;
use crate::run;
use crate::scene::{self, NcaDrawing, PlotScene};
use crate::schema::{
    array_of, boolean, described, integer, nullable, object, one_of_strings, reference, root,
    string,
};
use crate::svg;

// ---- plot.xy -----------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlotParams {
    #[serde(default)]
    analysis: Option<AnalysisId>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    worksheet: Option<WorksheetId>,
    #[serde(default)]
    fit: Option<AnalysisId>,
    #[serde(default)]
    view: Option<FitView>,
    #[serde(default, deserialize_with = "super::labels")]
    subjects: Option<Vec<String>>,
    #[serde(default)]
    scale: PlotScale,
    #[serde(default)]
    layout: PlotLayout,
    #[serde(default)]
    title: Option<String>,
}

fn source(p: &PlotParams) -> Result<PlotSource, CommandError> {
    match (p.worksheet, p.fit) {
        (Some(worksheet), None) => {
            if p.view.is_some() {
                return Err(CommandError::invalid(
                    "plot.xy",
                    "`view` chooses a plot of a fit; a worksheet plot has none",
                ));
            }
            Ok(PlotSource::Worksheet { worksheet })
        }
        (None, Some(analysis)) => Ok(PlotSource::Fit {
            analysis,
            view: p.view.unwrap_or_default(),
        }),
        (Some(_), Some(_)) => Err(CommandError::invalid(
            "plot.xy",
            "give `worksheet` or `fit`, not both",
        )),
        (None, None) => Err(CommandError::invalid(
            "plot.xy",
            "give `worksheet` (concentration against time) or `fit` (a plot of a fit result)",
        )),
    }
}

fn plot_xy(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: PlotParams = parse("plot.xy", params)?;
    check_target(&engine.project, p.analysis, "plot")?;
    let spec = AnalysisSpec::Plot(PlotSpec {
        source: source(&p)?,
        subjects: p.subjects.clone(),
        scale: p.scale,
        layout: p.layout,
        title: p.title.clone().filter(|t| !t.trim().is_empty()),
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

pub(crate) const PLOT_XY: CommandDef = CommandDef {
    id: "plot.xy",
    title: "Plot data or a fit",
    description: "Draws an XY plot and stores it as an analysis object (new, or `analysis` updated), so it is saved with the project and marked stale when its source changes. Source: `worksheet` (concentration against time, one series per subject, all subjects or `subjects`, overlaid or one panel per subject) or `fit` (a fit analysis already run: `view` profile, observed_predicted, residuals_time or weighted_residuals_time; nothing is fitted again). `scale` linear, log or both (both: a linear and a logarithmic panel; residuals are linear only). The result holds the panels with their axes, units and points as they are in the source (BLQ points flagged); nothing is computed. `export.plot` gives the SVG.",
    mutates: true,
    params: || {
        root(
            "plot.xy parameters",
            object(
                vec![
                    (
                        "analysis",
                        described(
                            reference("Id"),
                            "Update and redraw this plot instead of creating a new one.",
                        ),
                    ),
                    ("name", string()),
                    (
                        "worksheet",
                        described(reference("Id"), "Plot the data of this worksheet."),
                    ),
                    (
                        "fit",
                        described(
                            reference("Id"),
                            "Plot the stored result of this fit analysis.",
                        ),
                    ),
                    ("view", one_of_strings(&FitView::ALL.map(FitView::id))),
                    (
                        "subjects",
                        described(
                            nullable(array_of(crate::schema::subject())),
                            "The subjects drawn, in this order; omitted or null: every subject. Worksheet plots only.",
                        ),
                    ),
                    ("scale", one_of_strings(&["linear", "log", "both"])),
                    ("layout", one_of_strings(&["overlay", "per_subject"])),
                    (
                        "title",
                        described(
                            nullable(string()),
                            "Shown above the plot; omitted: the label of the plot.",
                        ),
                    ),
                ],
                &[],
            ),
        )
    },
    result: || root("plot.xy result", reference("AnalysisView")),
    example: || json!({ "worksheet": 1, "scale": "both", "layout": "per_subject" }),
    run: plot_xy,
};

// ---- the scene of a plot, for plot.scene and export.plot -----------------------------------

/// What `plot.scene` and `export.plot` draw: the scene of a plot analysis, or of one subject of
/// an NCA analysis.
struct Drawing {
    scene: PlotScene,
    /// `plot` or `nca`.
    kind: &'static str,
    /// The subject of an NCA scene.
    subject: Option<String>,
    status: &'static str,
}

/// The panels of `run` on the axis that `log` asks for (`None`: all of them).
fn panels_on(
    run: &PlotRun,
    log: Option<bool>,
    analysis: AnalysisId,
) -> Result<PlotRun, CommandError> {
    let Some(log) = log else {
        return Ok(run.clone());
    };
    let panels: Vec<_> = run
        .panels
        .iter()
        .filter(|p| p.y.log == log)
        .cloned()
        .collect();
    if panels.is_empty() {
        return Err(CommandError::new(
            "no_such_panel",
            format!(
                "plot {analysis} has no {} panel; draw it with scale `{}` or `both` (plot.xy), or leave `log` out",
                if log { "logarithmic" } else { "linear" },
                if log { "log" } else { "linear" },
            ),
        ));
    }
    Ok(PlotRun {
        title: run.title.clone(),
        panels,
        notes: run.notes.clone(),
    })
}

fn drawing(
    engine: &Engine,
    command: &str,
    analysis: AnalysisId,
    subject: Option<&str>,
    log: Option<bool>,
) -> Result<Drawing, CommandError> {
    let project = &engine.project;
    let stored = project.analysis(analysis)?;
    let result = stored.result().ok_or_else(|| {
        CommandError::new(
            "no_result",
            format!(
                "analysis {analysis} has no result to draw; run it (plot.xy, nca.run or analysis.run) before {command}"
            ),
        )
    })?;
    let status = match project.status(analysis)? {
        AnalysisStatus::Fresh => "fresh",
        AnalysisStatus::Stale { .. } => "stale",
        AnalysisStatus::NoResult => "no_result",
    };
    match result {
        AnalysisResult::Plot(run) => {
            if subject.is_some() {
                return Err(CommandError::invalid(
                    command,
                    format!(
                        "analysis {analysis} is a plot; `subject` names a subject of an NCA analysis"
                    ),
                ));
            }
            let run = panels_on(run, log, analysis)?;
            Ok(Drawing {
                scene: scene::from_run(&run),
                kind: "plot",
                subject: None,
                status,
            })
        }
        AnalysisResult::Nca { subjects } => {
            let AnalysisSpec::Nca(spec) = stored.spec() else {
                return Err(CommandError::new(
                    "internal_error",
                    format!("analysis {analysis} holds an NCA result without an NCA spec"),
                ));
            };
            let names = || {
                subjects
                    .iter()
                    .map(|s| s.subject.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let one = match (subject, subjects.as_slice()) {
                (Some(wanted), _) => subjects.iter().find(|s| s.subject == wanted).ok_or_else(|| {
                    CommandError::new(
                        "unknown_subject",
                        format!(
                            "analysis {analysis} has no subject `{wanted}`; the subjects are: {}",
                            names()
                        ),
                    )
                })?,
                (None, [only]) => only,
                (None, _) => {
                    return Err(CommandError::invalid(
                        command,
                        format!(
                            "analysis {analysis} is an NCA of {} subjects; give `subject`, one of: {}",
                            subjects.len(),
                            names()
                        ),
                    ));
                }
            };
            let Outcome::Ok(nca) = &one.outcome else {
                let why = match &one.outcome {
                    Outcome::Error(message) => message.as_str(),
                    Outcome::Ok(_) => "",
                };
                return Err(CommandError::new(
                    "no_result",
                    format!(
                        "subject {} of analysis {analysis} has no result to draw: {why}",
                        one.subject
                    ),
                ));
            };
            // The units the analysis ran with, else those of the worksheet columns.
            let sheet = project.worksheet(spec.worksheet).ok();
            let units = spec.options.units.as_ref();
            let time_unit = units
                .map(|u| u.time.as_str())
                .or_else(|| sheet.and_then(|w| w.unit_of(ColumnRole::Time)));
            let concentration_unit = units
                .map(|u| u.concentration.as_str())
                .or_else(|| sheet.and_then(|w| w.unit_of(ColumnRole::Concentration)));
            Ok(Drawing {
                scene: scene::nca(&NcaDrawing {
                    subject: &one.subject,
                    result: nca,
                    options: &spec.options,
                    time_unit,
                    concentration_unit,
                    log: log.unwrap_or(false),
                }),
                kind: "nca",
                subject: Some(one.subject.clone()),
                status,
            })
        }
        _ => Err(CommandError::new(
            "not_a_plot",
            format!(
                "analysis {analysis} is a {} analysis; {command} draws a plot (make one with plot.xy) or an NCA analysis",
                stored.spec().kind()
            ),
        )),
    }
}

// ---- plot.scene --------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlotSceneParams {
    analysis: AnalysisId,
    #[serde(default, deserialize_with = "super::label")]
    subject: Option<String>,
    #[serde(default)]
    log: Option<bool>,
}

fn plot_scene(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: PlotSceneParams = parse("plot.scene", params)?;
    let d = drawing(
        engine,
        "plot.scene",
        p.analysis,
        p.subject.as_deref(),
        p.log,
    )?;
    let mut answer = json!({
        "analysis": p.analysis,
        "kind": d.kind,
        "status": d.status,
        "scene": respond(&d.scene)?,
    });
    if let (Some(subject), Some(map)) = (d.subject, answer.as_object_mut()) {
        map.insert("subject".to_owned(), json!(subject));
    }
    Ok(answer)
}

pub(crate) const PLOT_SCENE: CommandDef = CommandDef {
    id: "plot.scene",
    title: "The scene of a plot",
    description: "The drawing of a plot analysis, or of one subject of an NCA analysis, as data that any front can draw: panels with their axes, ticks (value and label), range and series, the points a logarithmic axis cannot show with the sentence that says so, the reference lines and the legend. No pixel and no colour: a series has a role (`subject`, `observed`, `predicted`; for an NCA profile `profile`, `lambda_z_used`, `lambda_z_start`, `lambda_z_fit`, `excluded`, `replaced`) and the theme of whoever draws decides how it looks. A logarithmic axis is already in log10 (its range, its ticks and its points), so a front only maps scene units to pixels. For an NCA analysis `subject` names the subject (it may be left out when there is one) and `log` chooses the semi-logarithmic axis (default linear); the lambda z line is drawn from the intercept and lambda z of the answer, with no new estimate. For a plot analysis `log` keeps only the panels of that axis. `status` says whether the analysis is fresh or stale.",
    mutates: false,
    params: || {
        root(
            "plot.scene parameters",
            object(
                vec![
                    ("analysis", reference("Id")),
                    (
                        "subject",
                        described(
                            crate::schema::subject(),
                            "NCA analysis only: the subject to draw (omitted: the only one).",
                        ),
                    ),
                    (
                        "log",
                        described(
                            nullable(boolean()),
                            "NCA analysis: a semi-logarithmic concentration axis (default linear). Plot analysis: keep only the logarithmic (true) or the linear (false) panels.",
                        ),
                    ),
                ],
                &["analysis"],
            ),
        )
    },
    result: || {
        root(
            "plot.scene result",
            object(
                vec![
                    ("analysis", reference("Id")),
                    ("kind", one_of_strings(&["plot", "nca"])),
                    ("status", one_of_strings(&["fresh", "stale", "no_result"])),
                    ("subject", string()),
                    ("scene", reference("PlotScene")),
                ],
                &["analysis", "kind", "status", "scene"],
            ),
        )
    },
    example: || json!({ "analysis": 4 }),
    run: plot_scene,
};

// ---- export.plot -------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportPlotParams {
    analysis: AnalysisId,
    #[serde(default = "svg_format")]
    format: String,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default, deserialize_with = "super::label")]
    subject: Option<String>,
    #[serde(default)]
    log: Option<bool>,
}

fn svg_format() -> String {
    "svg".to_owned()
}

fn side(value: Option<u32>, default: f64, what: &str) -> Result<f64, CommandError> {
    match value {
        None => Ok(default),
        Some(v) if (200..=2000).contains(&v) => Ok(f64::from(v)),
        Some(v) => Err(CommandError::invalid(
            "export.plot",
            format!("`{what}` is {v}; give a panel {what} between 200 and 2000 pixels"),
        )),
    }
}

fn export_plot(engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: ExportPlotParams = parse("export.plot", params)?;
    if p.format != "svg" {
        return Err(CommandError::invalid(
            "export.plot",
            format!(
                "the format `{}` is not available; the formats are: svg",
                p.format
            ),
        ));
    }
    let size = svg::Size {
        width: side(p.width, svg::PANEL.width, "width")?,
        height: side(p.height, svg::PANEL.height, "height")?,
    };
    let d = drawing(
        engine,
        "export.plot",
        p.analysis,
        p.subject.as_deref(),
        p.log,
    )?;
    let label = engine.project.label_of(p.analysis)?;
    // A plot keeps the name it always had; an NCA profile names its subject and its axis.
    let mut name = export::slug(&label);
    if let Some(subject) = &d.subject {
        name = format!("{name}-subject-{}", export::slug(subject));
        match p.log {
            Some(true) => name.push_str("-semi-log"),
            Some(false) => name.push_str("-linear"),
            None => {}
        }
    }
    let mut answer = json!({
        "format": "svg",
        "file_name": format!("{name}.svg"),
        "status": d.status,
        "panels": d.scene.panels.len(),
        "svg": svg::render(&d.scene, size),
    });
    if let (Some(subject), Some(map)) = (d.subject, answer.as_object_mut()) {
        map.insert("subject".to_owned(), json!(subject));
    }
    Ok(answer)
}

pub(crate) const EXPORT_PLOT: CommandDef = CommandDef {
    id: "export.plot",
    title: "Export a plot as SVG",
    description: "The last drawing of a plot analysis, or of one subject of an NCA analysis, as an SVG document (text; the caller writes the file), with a suggested file name. It draws the scene of `plot.scene`, so the file and a screen that draws the scene show the same thing. `status` says whether the analysis is fresh or stale. `width` and `height` are the size of one panel in pixels (default 420 by 300); the panels are laid out in a grid with one legend. Colours are those of the light theme. For an NCA analysis give `subject` (it may be left out when there is one) and `log` for the semi-logarithmic axis; for a plot analysis `log` keeps only the panels of that axis.",
    mutates: false,
    params: || {
        root(
            "export.plot parameters",
            object(
                vec![
                    ("analysis", reference("Id")),
                    ("format", one_of_strings(&["svg"])),
                    (
                        "width",
                        json!({ "type": "integer", "minimum": 200, "maximum": 2000 }),
                    ),
                    (
                        "height",
                        json!({ "type": "integer", "minimum": 200, "maximum": 2000 }),
                    ),
                    (
                        "subject",
                        described(
                            crate::schema::subject(),
                            "NCA analysis only: the subject to draw (omitted: the only one).",
                        ),
                    ),
                    (
                        "log",
                        described(
                            nullable(boolean()),
                            "NCA analysis: a semi-logarithmic concentration axis (default linear). Plot analysis: keep only the logarithmic (true) or the linear (false) panels.",
                        ),
                    ),
                ],
                &["analysis"],
            ),
        )
    },
    result: || {
        root(
            "export.plot result",
            object(
                vec![
                    ("format", one_of_strings(&["svg"])),
                    ("file_name", string()),
                    ("status", one_of_strings(&["fresh", "stale", "no_result"])),
                    ("panels", integer()),
                    ("svg", string()),
                    ("subject", string()),
                ],
                &["format", "file_name", "status", "panels", "svg"],
            ),
        )
    },
    example: || json!({ "analysis": 4 }),
    run: export_plot,
};
