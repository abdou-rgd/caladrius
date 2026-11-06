//! The scene of the profile of one subject of an NCA analysis (T-133): the samples with the role
//! each one plays in the terminal phase, the points below the limit, and the λz line.
//!
//! The roles come from the answer of `nca.run` and from the options that produced it; nothing is
//! estimated. The λz line is a drawing of two engine values, the intercept and the λz of the
//! selected fit, evaluated at 61 times from the first point of the phase to the last time with
//! `libm` (golden rule 10).

use caladrius_nca::{
    LambdaZCandidate, LambdaZManual, NcaOptions, NcaResult, ProfilePoint, RemovalReason,
};
use caladrius_project::{PlotAxis, PlotPoint, SeriesStyle};

use super::{PlotScene, RawPanel, RawSeries, SceneRole, assemble};

/// The steps of the λz line (the screen draws 60 as well).
const LINE_STEPS: u32 = 60;

/// What the scene of an NCA profile is built from.
pub(crate) struct NcaDrawing<'a> {
    /// The label of the subject.
    pub(crate) subject: &'a str,
    /// The answer of the analysis for this subject.
    pub(crate) result: &'a NcaResult,
    /// The options the analysis ran with.
    pub(crate) options: &'a NcaOptions,
    pub(crate) time_unit: Option<&'a str>,
    pub(crate) concentration_unit: Option<&'a str>,
    /// A logarithmic concentration axis.
    pub(crate) log: bool,
}

/// What a sample is, for the terminal phase.
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Observed,
    Replaced,
    Excluded,
    Used,
}

/// Whether the engine could use `p` in the fit `fit` (its range) under `options`: the same
/// conditions as the selection of the terminal phase (a positive sample, a replaced value only
/// when the options allow it, the samples the user chose or did not leave out).
fn part(p: &ProfilePoint, fit: Option<&LambdaZCandidate>, options: &NcaOptions) -> Part {
    let selection = &options.lambda_z_selection;
    let in_range = fit.is_some_and(|c| p.time >= c.time_first && p.time <= c.time_last);
    let used = in_range
        && p.is_positive_sample()
        && !(selection.exclude_replaced && p.is_replaced())
        && match &selection.manual {
            Some(LambdaZManual::Times(times)) => times.contains(&p.time),
            Some(LambdaZManual::Range { .. }) => true,
            None => !selection.exclude.contains(&p.time),
        };
    if used {
        Part::Used
    } else if in_range && p.is_positive_sample() {
        Part::Excluded
    } else if p.is_replaced() {
        Part::Replaced
    } else {
        Part::Observed
    }
}

fn series(name: &str, role: SceneRole, style: SeriesStyle, points: Vec<PlotPoint>) -> RawSeries {
    RawSeries {
        name: name.to_owned(),
        role,
        style,
        points,
        repeats: role == SceneRole::Profile,
    }
}

/// The scene of the profile of one subject, linear or semi-logarithmic.
pub(crate) fn nca(d: &NcaDrawing) -> PlotScene {
    let samples: Vec<&ProfilePoint> = d
        .result
        .profile()
        .iter()
        .filter(|p| p.is_sample())
        .collect();
    let fit = d.result.lambda_z_candidates().iter().find(|c| c.selected);
    let point = |p: &&ProfilePoint| PlotPoint {
        x: p.time,
        y: p.conc,
        blq: false,
    };
    let with = |wanted: Part| -> Vec<PlotPoint> {
        samples
            .iter()
            .filter(|p| part(p, fit, d.options) == wanted)
            .map(point)
            .collect()
    };
    let mut observed = with(Part::Observed);
    // The samples below the limit that the cleaning left out: a marker, at no value.
    observed.extend(
        d.result
            .removed()
            .iter()
            .filter(|r| r.reason == RemovalReason::Blq)
            .map(|r| PlotPoint {
                x: r.time,
                y: 0.0,
                blq: true,
            }),
    );
    let used = with(Part::Used);
    let start: Vec<PlotPoint> = used.first().copied().into_iter().collect();
    let mut all = vec![series(
        "Profile",
        SceneRole::Profile,
        SeriesStyle::Line,
        samples.iter().map(point).collect(),
    )];
    all.push(series(
        "Observed",
        SceneRole::Observed,
        SeriesStyle::Points,
        observed,
    ));
    all.push(series(
        "Replaced value",
        SceneRole::Replaced,
        SeriesStyle::Points,
        with(Part::Replaced),
    ));
    all.push(series(
        "Left out of λz",
        SceneRole::Excluded,
        SeriesStyle::Points,
        with(Part::Excluded),
    ));
    all.push(series(
        "Used for λz",
        SceneRole::LambdaZUsed,
        SeriesStyle::Points,
        used,
    ));
    all.push(series(
        "Start of the λz range",
        SceneRole::LambdaZStart,
        SeriesStyle::Points,
        start,
    ));
    // The terminal-phase line: the intercept and λz of the answer, from the first point of the
    // phase to the last time.
    if let Some(c) = fit {
        let last = samples.iter().map(|p| p.time).fold(0.0_f64, f64::max);
        let span = (last - c.time_first).max(f64::MIN_POSITIVE);
        let line = (0..=LINE_STEPS)
            .map(|i| {
                let t = c.time_first + span * f64::from(i) / f64::from(LINE_STEPS);
                PlotPoint {
                    x: t,
                    y: libm::exp(c.intercept - c.lambda_z * t),
                    blq: false,
                }
            })
            .collect();
        all.push(series(
            "λz fit",
            SceneRole::LambdaZFit,
            SeriesStyle::Line,
            line,
        ));
    }
    all.retain(|s| !s.points.is_empty());
    let axis = |label: &str, unit: Option<&str>, log| PlotAxis {
        label: label.to_owned(),
        unit: unit.map(str::to_owned),
        log,
    };
    let panel = RawPanel {
        title: format!(
            "Subject {}, {}",
            d.subject,
            if d.log { "semi-log" } else { "linear" }
        ),
        x: axis("Time", d.time_unit, false),
        y: axis("Concentration", d.concentration_unit, d.log),
        series: all,
        guide: None,
        // A concentration profile is read from its baseline.
        include_zero: true,
    };
    assemble(
        format!("Concentration over time, subject {}", d.subject),
        &[panel],
        Vec::new(),
    )
}
