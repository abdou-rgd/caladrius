//! Drawing a plot object (T-110): from a plot spec and the project to the panels of points. The
//! points are the source's values as they are: the cells of a worksheet, or the observed,
//! predicted and residual values of a stored fit result. Nothing is computed, fitted or
//! interpolated here; a fit is never run again to plot it.

use caladrius_fit::FitResult;
use caladrius_project::{
    AnalysisResult, AnalysisSpec, ColumnRole, FitView, Outcome, PlotAxis, PlotGuide, PlotLayout,
    PlotPanel, PlotPoint, PlotRun, PlotSeries, PlotSource, PlotSpec, Project, SeriesRole,
    SeriesStyle, Worksheet,
};

use crate::error::CommandError;

/// The panels of the plot described by `spec`, from the current state of `project`.
pub(crate) fn draw(project: &Project, spec: &PlotSpec) -> Result<PlotRun, CommandError> {
    let title = match spec.title.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => t.to_owned(),
        _ => project.describe_spec(&AnalysisSpec::Plot(spec.clone())),
    };
    let (panels, notes) = match &spec.source {
        PlotSource::Worksheet { worksheet } => {
            worksheet_panels(project.worksheet(*worksheet)?, spec)?
        }
        PlotSource::Fit { analysis, view } => {
            let a = project.analysis(*analysis)?;
            let AnalysisSpec::Fit(fit_spec) = a.spec() else {
                return Err(CommandError::new(
                    "wrong_plot_source",
                    format!(
                        "analysis {analysis} is a {} analysis; a plot reads a worksheet or a fit",
                        a.spec().kind()
                    ),
                ));
            };
            let run = match a.result() {
                Some(AnalysisResult::Fit(run)) => run,
                _ => {
                    return Err(CommandError::new(
                        "no_result",
                        format!(
                            "fit {analysis} has not been run; run it (fit.run or analysis.run) before plotting it"
                        ),
                    ));
                }
            };
            let fit = match &run.outcome {
                Outcome::Ok(fit) => fit,
                Outcome::Error(message) => {
                    return Err(CommandError::new(
                        "no_result",
                        format!("fit {analysis} has no result to plot: {message}"),
                    ));
                }
            };
            // The units of the worksheet the fit read, when it is still there.
            let ws = project.worksheet(fit_spec.worksheet).ok();
            (fit_panels(fit, *view, spec, ws)?, Vec::new())
        }
    };
    Ok(PlotRun {
        title,
        panels,
        notes,
    })
}

fn axis(label: &str, unit: Option<&str>, log: bool) -> PlotAxis {
    PlotAxis {
        label: label.to_owned(),
        unit: unit.map(str::to_owned),
        log,
    }
}

fn scale_name(log: bool) -> &'static str {
    if log { "semi-log" } else { "linear" }
}

/// Capitalises the first letter (`semi-log` → `Semi-log`).
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The subjects to draw, checked against the worksheet, in the order asked.
fn chosen_subjects(ws: &Worksheet, spec: &PlotSpec) -> Result<Vec<String>, CommandError> {
    let all = ws.subjects();
    let chosen = match &spec.subjects {
        None => all.clone(),
        Some(list) => {
            if let Some(unknown) = list.iter().find(|s| !all.contains(s)) {
                return Err(CommandError::new(
                    "unknown_subject",
                    format!(
                        "worksheet `{}` has no subject `{unknown}`; its subjects are: {}",
                        ws.name(),
                        all.join(", ")
                    ),
                ));
            }
            let mut unique: Vec<String> = Vec::new();
            for s in list {
                if !unique.contains(s) {
                    unique.push(s.clone());
                }
            }
            unique
        }
    };
    if chosen.is_empty() {
        return Err(CommandError::new(
            "no_subjects",
            format!(
                "nothing to plot: choose at least one subject of worksheet `{}` (or leave the list out for all)",
                ws.name()
            ),
        ));
    }
    Ok(chosen)
}

fn worksheet_panels(
    ws: &Worksheet,
    spec: &PlotSpec,
) -> Result<(Vec<PlotPanel>, Vec<String>), CommandError> {
    for role in [ColumnRole::Time, ColumnRole::Concentration] {
        if ws.column_by_role(role).is_none() {
            return Err(CommandError::new(
                "missing_column",
                format!(
                    "worksheet `{}` has no {} column; give a column that role first (data.set_column)",
                    ws.name(),
                    role.id()
                ),
            ));
        }
    }
    let all = ws.subjects();
    let chosen = chosen_subjects(ws, spec)?;
    // One subject in the whole worksheet: the colour of observed points, as on the other plots.
    let single = all.len() <= 1;
    let mut skipped = 0usize;
    let mut series: Vec<PlotSeries> = Vec::new();
    for subject in &chosen {
        let profile = ws.profile(Some(subject))?;
        let mut points: Vec<PlotPoint> = Vec::new();
        for ((x, y), blq) in profile.time.iter().zip(&profile.conc).zip(&profile.blq) {
            if x.is_finite() && (y.is_finite() || *blq) {
                points.push(PlotPoint {
                    x: *x,
                    y: if *blq { 0.0 } else { *y },
                    blq: *blq,
                });
            } else {
                skipped += 1;
            }
        }
        // Drawn in time order (rows with the same time keep their order); values unchanged.
        points.sort_by(|a, b| a.x.total_cmp(&b.x));
        let slot = all.iter().position(|s| s == subject).unwrap_or(0);
        series.push(PlotSeries {
            name: if single && subject == caladrius_project::SINGLE_SUBJECT {
                "observed".to_owned()
            } else {
                subject.clone()
            },
            role: if single {
                SeriesRole::Observed
            } else {
                SeriesRole::Subject { slot }
            },
            style: SeriesStyle::LinePoints,
            points,
        });
    }
    let x_unit = ws.unit_of(ColumnRole::Time);
    let y_unit = ws.unit_of(ColumnRole::Concentration);
    let panel = |title: String, log: bool, series: Vec<PlotSeries>| PlotPanel {
        title,
        x: axis("Time", x_unit, false),
        y: axis("Concentration", y_unit, log),
        series,
        guide: None,
    };
    let mut panels = Vec::new();
    match spec.layout {
        PlotLayout::Overlay => {
            for log in spec.scale.panels() {
                panels.push(panel(capital(scale_name(*log)), *log, series.clone()));
            }
        }
        PlotLayout::PerSubject => {
            for s in &series {
                for log in spec.scale.panels() {
                    let title = if spec.scale.panels().len() > 1 {
                        format!("{}, {}", s.name, scale_name(*log))
                    } else {
                        s.name.clone()
                    };
                    panels.push(panel(title, *log, vec![s.clone()]));
                }
            }
        }
    }
    let mut notes = Vec::new();
    if skipped > 0 {
        notes.push(format!(
            "{skipped} row(s) without a time or a concentration are not drawn."
        ));
    }
    let blq = series
        .iter()
        .flat_map(|s| &s.points)
        .filter(|p| p.blq)
        .count();
    if blq > 0 {
        notes.push(format!(
            "{blq} point(s) below the limit of quantification are marked apart: at 0 on a linear axis, under the lowest decade on a logarithmic one."
        ));
    }
    let not_positive = series
        .iter()
        .flat_map(|s| &s.points)
        .filter(|p| !p.blq && p.y <= 0.0)
        .count();
    if not_positive > 0 && spec.scale.panels().contains(&true) {
        notes.push(format!(
            "{not_positive} point(s) at zero or below have no logarithm: they are left out of the logarithmic panels only."
        ));
    }
    Ok((panels, notes))
}

fn points(pairs: impl Iterator<Item = (f64, f64)>) -> Vec<PlotPoint> {
    pairs
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| PlotPoint { x, y, blq: false })
        .collect()
}

fn fit_panels(
    fit: &FitResult,
    view: FitView,
    spec: &PlotSpec,
    ws: Option<&Worksheet>,
) -> Result<Vec<PlotPanel>, CommandError> {
    if !view.allows_log() && spec.scale.panels().contains(&true) {
        return Err(CommandError::new(
            "log_scale_not_possible",
            format!(
                "{} can be negative, so they have no logarithmic axis; use the linear scale",
                view.title().to_lowercase().replace(" vs time", "")
            ),
        ));
    }
    let t_unit = ws.and_then(|w| w.unit_of(ColumnRole::Time));
    let c_unit = ws.and_then(|w| w.unit_of(ColumnRole::Concentration));
    let obs = fit.observations();
    let observed = |pairs: Vec<PlotPoint>| PlotSeries {
        name: "observed".to_owned(),
        role: SeriesRole::Observed,
        style: SeriesStyle::Points,
        points: pairs,
    };
    let mut panels = Vec::new();
    for log in spec.scale.panels() {
        let log = *log;
        let name = capital(scale_name(log));
        let panel = match view {
            FitView::Profile => {
                // The smooth curve of the fit; without one, the predictions at the observed times.
                let curve: Vec<PlotPoint> = if fit.curve().is_empty() {
                    points(obs.iter().map(|o| (o.time, o.predicted)))
                } else {
                    points(fit.curve().iter().map(|p| (p.time, p.conc)))
                };
                PlotPanel {
                    title: name,
                    x: axis("Time", t_unit, false),
                    y: axis("Concentration", c_unit, log),
                    series: vec![
                        observed(points(obs.iter().map(|o| (o.time, o.observed)))),
                        PlotSeries {
                            name: "predicted".to_owned(),
                            role: SeriesRole::Predicted,
                            style: SeriesStyle::Line,
                            points: curve,
                        },
                    ],
                    guide: None,
                }
            }
            FitView::ObservedPredicted => PlotPanel {
                title: if log { "Log-log".to_owned() } else { name },
                x: axis("Predicted concentration", c_unit, log),
                y: axis("Observed concentration", c_unit, log),
                series: vec![observed(points(
                    obs.iter().map(|o| (o.predicted, o.observed)),
                ))],
                guide: Some(PlotGuide::Identity),
            },
            FitView::ResidualsTime => PlotPanel {
                title: name,
                x: axis("Time", t_unit, false),
                y: axis("Residual", c_unit, false),
                series: vec![observed(points(obs.iter().map(|o| (o.time, o.residual))))],
                guide: Some(PlotGuide::Zero),
            },
            FitView::WeightedResidualsTime => PlotPanel {
                title: name,
                x: axis("Time", t_unit, false),
                y: axis("Weighted residual", None, false),
                series: vec![observed(points(
                    obs.iter().map(|o| (o.time, o.weighted_residual)),
                ))],
                guide: Some(PlotGuide::Zero),
            },
        };
        panels.push(panel);
    }
    Ok(panels)
}
