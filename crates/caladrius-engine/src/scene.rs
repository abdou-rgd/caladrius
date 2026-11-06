//! The plot scene (T-133): what a plot is, once, as data, for any front that draws it: the SVG
//! export (`svg.rs`), the egui painter, or SVG in a web page.
//!
//! A scene holds panels with their axes, ticks, series, guides and hidden points, and one legend.
//! It holds **no pixel and no colour**: a role names what a series is (`subject`, `observed`,
//! `lambda_z_used`...), and the theme of whoever draws decides how it looks. A logarithmic axis is
//! already in log10 (its points, its range and its ticks), so a front applies one affine map from
//! scene units to pixels and nothing else. Nothing is estimated here: the ranges and the ticks are
//! the rules `export.plot` has always followed (nice steps of 1, 2 or 5, whole decades), and the
//! points are the stored values, placed.
//!
//! Two builders: [`from_run`] for the panels of a plot analysis (`plot.xy`), and [`nca`] for the
//! profile of one subject of an NCA analysis, with the roles of its points.

use caladrius_project::{
    PlotAxis, PlotGuide, PlotPanel, PlotPoint, PlotRun, PlotSeries, SeriesRole, SeriesStyle,
};
use serde::{Deserialize, Serialize};

mod nca;
pub(crate) use nca::{NcaDrawing, nca};

/// What a series is. The theme of the front decides its colour and its marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum SceneRole {
    /// The subject at `slot` among the subjects of the worksheet (the slot keeps a colour).
    Subject {
        /// Position of the subject in the worksheet, from 0.
        slot: usize,
    },
    /// Observed concentrations of a single profile.
    Observed,
    /// Predicted values of a fit.
    Predicted,
    /// The line through the samples of a profile (NCA).
    Profile,
    /// A value that a policy put in place of a missing or BLQ one (NCA).
    Replaced,
    /// A point inside the terminal phase that the options leave out of the fit (NCA).
    Excluded,
    /// A point used for λz (NCA).
    LambdaZUsed,
    /// The first point of the λz range (NCA).
    LambdaZStart,
    /// The λz line, from the intercept and λz of the answer (NCA).
    LambdaZFit,
}

impl From<SeriesRole> for SceneRole {
    fn from(role: SeriesRole) -> Self {
        match role {
            SeriesRole::Subject { slot } => SceneRole::Subject { slot },
            SeriesRole::Observed => SceneRole::Observed,
            SeriesRole::Predicted => SceneRole::Predicted,
        }
    }
}

/// One tick: where it is (scene units) and what it says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneTick {
    pub value: f64,
    pub label: String,
}

/// One axis of a panel. On a logarithmic axis `min`, `max` and the tick values are decades
/// (log10), like the points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneAxis {
    /// The title with its unit: `Time (h)`.
    pub title: String,
    /// Logarithmic (base 10).
    pub log: bool,
    /// The lowest value of the axis, margin included, in scene units.
    pub min: f64,
    /// The highest value of the axis, margin included, in scene units.
    pub max: f64,
    pub ticks: Vec<SceneTick>,
}

impl SceneAxis {
    /// Where `t` (scene units) falls between 0 (`min`) and 1 (`max`); 0.5 for an empty range.
    pub fn fraction(&self, t: f64) -> f64 {
        if self.max > self.min {
            (t - self.min) / (self.max - self.min)
        } else {
            0.5
        }
    }
}

/// One point, in scene units. A point below the limit of quantification (`blq`) carries the
/// position where its marker sits on the vertical axis (0 on a linear axis, in the margin under
/// the lowest decade on a logarithmic one).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScenePoint {
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub blq: bool,
}

/// One series of a panel, in the order it is drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneSeries {
    /// The name in the legend.
    pub name: String,
    #[serde(flatten)]
    pub role: SceneRole,
    pub style: SeriesStyle,
    /// Only the points the axes can show, in the order of the source.
    pub points: Vec<ScenePoint>,
}

/// A reference line, in scene units: `y = x` or `y = 0`. A logarithmic vertical axis has no zero
/// line, so the scene holds none there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SceneGuide {
    pub kind: PlotGuide,
    pub from: [f64; 2],
    pub to: [f64; 2],
}

/// A point that the axes of a panel cannot show (zero or below on a logarithmic axis), in the
/// units of the data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HiddenPoint {
    pub series: String,
    pub x: f64,
    pub y: f64,
}

/// The points left out of a panel, and the sentence that says so.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HiddenPoints {
    pub count: usize,
    pub note: String,
    pub points: Vec<HiddenPoint>,
}

/// One panel: axes, series and guide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenePanel {
    pub title: String,
    pub x: SceneAxis,
    pub y: SceneAxis,
    pub series: Vec<SceneSeries>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<SceneGuide>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<HiddenPoints>,
    /// Set when nothing is left to draw (a logarithmic axis and no value above zero): the
    /// sentence to show instead of the panel. The axes then hold no tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<String>,
}

/// One entry of the legend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegendEntry {
    pub name: String,
    #[serde(flatten)]
    pub role: SceneRole,
    pub style: SeriesStyle,
}

/// The legend of the figure: one entry per distinct series name, in order of appearance (none
/// for a single series), then the entry for the points below the limit when there are some.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SceneLegend {
    pub entries: Vec<LegendEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below_limit: Option<String>,
}

/// A plot as data: panels in reading order, one legend, the sentences a person should read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotScene {
    pub title: String,
    pub panels: Vec<ScenePanel>,
    pub legend: SceneLegend,
    #[serde(default)]
    pub notes: Vec<String>,
}

/// The sentence of the legend for the points below the limit of quantification.
const BELOW_LIMIT: &str = "below the limit of quantification";

// ---- the input of the layout -----------------------------------------------------------------

/// A series in the units of the data, before the axes place it.
pub(crate) struct RawSeries {
    pub(crate) name: String,
    pub(crate) role: SceneRole,
    pub(crate) style: SeriesStyle,
    pub(crate) points: Vec<PlotPoint>,
    /// The series only joins points that other series already hold (the line through the samples
    /// of a profile): the points it cannot place are not counted as hidden a second time.
    pub(crate) repeats: bool,
}

/// A panel in the units of the data.
pub(crate) struct RawPanel {
    pub(crate) title: String,
    pub(crate) x: PlotAxis,
    pub(crate) y: PlotAxis,
    pub(crate) series: Vec<RawSeries>,
    pub(crate) guide: Option<PlotGuide>,
    /// Keep 0 inside a linear vertical axis even without a zero line or a BLQ point.
    pub(crate) include_zero: bool,
}

fn raw_series(s: &PlotSeries) -> RawSeries {
    RawSeries {
        name: s.name.clone(),
        role: s.role.into(),
        style: s.style,
        points: s.points.clone(),
        repeats: false,
    }
}

fn raw_panel(p: &PlotPanel) -> RawPanel {
    RawPanel {
        title: p.title.clone(),
        x: p.x.clone(),
        y: p.y.clone(),
        series: p.series.iter().map(raw_series).collect(),
        guide: p.guide,
        include_zero: false,
    }
}

/// The scene of the panels a plot analysis stored.
pub(crate) fn from_run(run: &PlotRun) -> PlotScene {
    let raw: Vec<RawPanel> = run.panels.iter().map(raw_panel).collect();
    assemble(run.title.clone(), &raw, run.notes.clone())
}

/// The scene of these panels: each laid out, and one legend for all.
pub(crate) fn assemble(title: String, raw: &[RawPanel], notes: Vec<String>) -> PlotScene {
    let mut entries: Vec<&RawSeries> = Vec::new();
    for s in raw.iter().flat_map(|p| &p.series) {
        if !entries.iter().any(|e| e.name == s.name) {
            entries.push(s);
        }
    }
    let has_blq = raw
        .iter()
        .flat_map(|p| &p.series)
        .any(|s| s.points.iter().any(|q| q.blq));
    let shown = entries.len() > 1 || has_blq;
    let legend = SceneLegend {
        entries: if shown {
            entries
                .iter()
                .map(|e| LegendEntry {
                    name: e.name.clone(),
                    role: e.role,
                    style: e.style,
                })
                .collect()
        } else {
            Vec::new()
        },
        below_limit: has_blq.then(|| BELOW_LIMIT.to_owned()),
    };
    PlotScene {
        title,
        panels: raw.iter().map(lay_out).collect(),
        legend,
        notes,
    }
}

// ---- the layout of one panel ----------------------------------------------------------------

/// One axis while it is built: the scene axis, and where a BLQ marker sits on it.
struct Scale {
    axis: SceneAxis,
    blq_at: f64,
}

impl Scale {
    /// The position of a data value, `None` when the axis cannot show it.
    fn value(&self, v: f64) -> Option<f64> {
        if self.axis.log {
            (v > 0.0 && v.is_finite()).then(|| libm::log10(v))
        } else {
            v.is_finite().then_some(v)
        }
    }
}

fn lay_out(p: &RawPanel) -> ScenePanel {
    let all: Vec<&PlotPoint> = p.series.iter().flat_map(|s| &s.points).collect();
    let xs: Vec<f64> = all.iter().map(|q| q.x).collect();
    let ys: Vec<f64> = all.iter().filter(|q| !q.blq).map(|q| q.y).collect();
    // Observed vs predicted shares one range on both axes, so that the identity line is the diagonal.
    let shared: Vec<f64> = xs.iter().chain(&ys).copied().collect();
    let (xv, yv): (&[f64], &[f64]) = match p.guide {
        Some(PlotGuide::Identity) => (&shared, &shared),
        _ => (&xs, &ys),
    };
    let zero =
        p.include_zero || matches!(p.guide, Some(PlotGuide::Zero)) || all.iter().any(|q| q.blq);
    let (Some(sx), Some(sy)) = (scale_for(&p.x, xv, false), scale_for(&p.y, yv, zero)) else {
        return ScenePanel {
            title: p.title.clone(),
            x: blank_axis(&p.x),
            y: blank_axis(&p.y),
            series: Vec::new(),
            guide: None,
            hidden: None,
            empty: Some(NOTHING_TO_SHOW.to_owned()),
        };
    };
    let mut hidden = Vec::new();
    let mut series = Vec::new();
    for s in &p.series {
        let mut points = Vec::new();
        for q in &s.points {
            let placed = if q.blq {
                sx.value(q.x).map(|x| ScenePoint {
                    x,
                    y: sy.blq_at,
                    blq: true,
                })
            } else {
                match (sx.value(q.x), sy.value(q.y)) {
                    (Some(x), Some(y)) => Some(ScenePoint { x, y, blq: false }),
                    _ => None,
                }
            };
            match placed {
                Some(point) => points.push(point),
                None if s.repeats => {}
                None => hidden.push(HiddenPoint {
                    series: s.name.clone(),
                    x: q.x,
                    y: q.y,
                }),
            }
        }
        series.push(SceneSeries {
            name: s.name.clone(),
            role: s.role,
            style: s.style,
            points,
        });
    }
    let guide = match p.guide {
        Some(PlotGuide::Zero) if !sy.axis.log => Some(SceneGuide {
            kind: PlotGuide::Zero,
            from: [sx.axis.min, 0.0],
            to: [sx.axis.max, 0.0],
        }),
        // The diagonal of the shared range (the same on both axes).
        Some(PlotGuide::Identity) => Some(SceneGuide {
            kind: PlotGuide::Identity,
            from: [sx.axis.min, sx.axis.min],
            to: [sx.axis.max, sx.axis.max],
        }),
        _ => None,
    };
    let hidden = (!hidden.is_empty()).then(|| HiddenPoints {
        count: hidden.len(),
        note: hidden_note(hidden.len(), sx.axis.log || sy.axis.log),
        points: hidden,
    });
    ScenePanel {
        title: p.title.clone(),
        x: sx.axis,
        y: sy.axis,
        series,
        guide,
        hidden,
        empty: None,
    }
}

/// What a panel says when no value can be shown on its logarithmic axis.
const NOTHING_TO_SHOW: &str = "No value above zero: nothing to show on a logarithmic axis.";

fn hidden_note(count: usize, log: bool) -> String {
    if log {
        format!(
            "{count} point(s) at zero or below have no logarithm and are not shown on the logarithmic axis."
        )
    } else {
        format!("{count} point(s) without a finite value are not shown.")
    }
}

/// The axis of a panel with nothing to draw.
fn blank_axis(axis: &PlotAxis) -> SceneAxis {
    SceneAxis {
        title: axis.title(),
        log: axis.log,
        min: 0.0,
        max: 1.0,
        ticks: Vec::new(),
    }
}

fn scale_for(axis: &PlotAxis, values: &[f64], zero: bool) -> Option<Scale> {
    if axis.log {
        logarithmic(axis, values)
    } else {
        Some(linear(axis, values, zero))
    }
}

fn range(values: &[f64]) -> Option<(f64, f64)> {
    let finite = || values.iter().copied().filter(|v| v.is_finite());
    let lo = finite().fold(f64::INFINITY, f64::min);
    let hi = finite().fold(f64::NEG_INFINITY, f64::max);
    (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}

/// A linear axis over `values`, with a margin of 5 %, and 0 inside when `zero` asks for it.
fn linear(axis: &PlotAxis, values: &[f64], zero: bool) -> Scale {
    let (mut lo, mut hi) = range(values).unwrap_or((0.0, 1.0));
    if zero {
        lo = lo.min(0.0);
        hi = hi.max(0.0);
    }
    if hi <= lo {
        let d = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
        lo -= d;
        hi += d;
    }
    let pad = 0.05 * (hi - lo);
    let (lo, hi) = (lo - pad, hi + pad);
    let (ticks, step) = nice_ticks(lo, hi);
    Scale {
        axis: SceneAxis {
            title: axis.title(),
            log: false,
            min: lo,
            max: hi,
            ticks: ticks
                .iter()
                .map(|v| SceneTick {
                    value: *v,
                    label: tick_label(*v, step),
                })
                .collect(),
        },
        blq_at: 0.0,
    }
}

/// A log axis over the positive `values`, rounded out to whole decades, with a margin below
/// for BLQ markers; `None` when no value is positive.
fn logarithmic(axis: &PlotAxis, values: &[f64]) -> Option<Scale> {
    let logs: Vec<f64> = values
        .iter()
        .filter(|v| **v > 0.0 && v.is_finite())
        .map(|v| libm::log10(*v))
        .collect();
    let (lo, hi) = range(&logs)?;
    let mut d_lo = lo.floor();
    let mut d_hi = hi.ceil();
    if d_hi <= d_lo {
        d_lo -= 0.5;
        d_hi += 0.5;
    }
    let ticks = ((d_lo.ceil() as i32)..=(d_hi.floor() as i32))
        .take(40)
        .map(|k| SceneTick {
            value: f64::from(k),
            label: decade_label(k),
        })
        .collect();
    Some(Scale {
        axis: SceneAxis {
            title: axis.title(),
            log: true,
            min: d_lo - 0.12,
            max: d_hi + 0.04,
            ticks,
        },
        blq_at: d_lo - 0.06,
    })
}

/// Tick values from `lo` to `hi` at a step of 1, 2 or 5 times a power of ten (about five).
fn nice_ticks(lo: f64, hi: f64) -> (Vec<f64>, f64) {
    let span = hi - lo;
    if !(span.is_finite() && span > 0.0) {
        return (vec![lo], 1.0);
    }
    let raw = span / 5.0;
    let power = pow10(libm::log10(raw).floor() as i32);
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * power)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * power);
    let first = (lo / step).ceil();
    let last = (hi / step).floor();
    let mut ticks = Vec::new();
    let mut k = first;
    // At most a few dozen ticks; the bound also stops a loop on a degenerate range.
    while k <= last && ticks.len() < 50 {
        let v = k * step;
        ticks.push(if v.abs() < step * 1e-9 { 0.0 } else { v });
        k += 1.0;
    }
    (ticks, step)
}

/// A tick label for a linear axis: as many decimals as the step needs.
fn tick_label(v: f64, step: f64) -> String {
    if v == 0.0 {
        return "0".to_owned();
    }
    if v.abs() >= 1e6 || v.abs() < 1e-4 {
        return format!("{v:e}");
    }
    let decimals = (-libm::log10(step).floor()).clamp(0.0, 8.0) as usize;
    format!("{v:.decimals$}")
}

/// A tick label for a decade: `0.01`, `1`, `1000`, `1e-6`.
fn decade_label(k: i32) -> String {
    match k {
        -4..=-1 => format!("{:.*}", k.unsigned_abs() as usize, pow10(k)),
        0..=5 => format!("{}", pow10(k)),
        _ => format!("1e{k}"),
    }
}

/// 10^k, the double nearest to it, for any `k` (zero or infinity beyond about 300 decades): the text
/// `1e{k}` read by the standard library's parser, which rounds correctly and the same on every
/// machine. A power function does not give that for every `k` (`libm::pow(10, -5)` is one bit away
/// from 1e-5, and a platform's `powf` may be too), nor does a loop of products beyond 10^22.
fn pow10(k: i32) -> f64 {
    format!("1e{}", k.clamp(-400, 400))
        .parse()
        .unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> PlotPoint {
        PlotPoint { x, y, blq: false }
    }

    fn axis(label: &str, log: bool) -> PlotAxis {
        PlotAxis {
            label: label.to_owned(),
            unit: Some("h".to_owned()),
            log,
        }
    }

    fn series(name: &str, slot: usize, points: Vec<PlotPoint>) -> RawSeries {
        RawSeries {
            name: name.to_owned(),
            role: SceneRole::Subject { slot },
            style: SeriesStyle::LinePoints,
            points,
            repeats: false,
        }
    }

    fn panel(log: bool, guide: Option<PlotGuide>, series: Vec<RawSeries>) -> RawPanel {
        RawPanel {
            title: "P".to_owned(),
            x: axis("Time", false),
            y: axis("Concentration", log),
            series,
            guide,
            include_zero: false,
        }
    }

    #[test]
    fn ticks_are_round_numbers_inside_the_range() {
        let (t, step) = nice_ticks(-0.3, 9.4);
        assert_eq!(step, 2.0);
        assert_eq!(t, vec![0.0, 2.0, 4.0, 6.0, 8.0]);
        let (t, step) = nice_ticks(0.012, 0.047);
        assert!((step - 0.01).abs() < 1e-15);
        assert_eq!(t.len(), 3);
        assert_eq!(tick_label(0.02, step), "0.02");
        assert_eq!(tick_label(1500.0, 500.0), "1500");
        assert_eq!(nice_ticks(3.0, 3.0).0, vec![3.0]);
        assert_eq!(decade_label(-2), "0.01");
        assert_eq!(decade_label(3), "1000");
        assert_eq!(decade_label(-7), "1e-7");
    }

    #[test]
    fn a_power_of_ten_is_the_nearest_double_whatever_the_exponent() {
        for (k, expected) in [
            (0, 1.0_f64),
            (3, 1e3),
            (-1, 1e-1),
            (-5, 1e-5),
            (22, 1e22),
            (23, 1e23),
            (-30, 1e-30),
            (67, 1e67),
            (308, 1e308),
            (-323, 1e-323),
        ] {
            assert_eq!(pow10(k).to_bits(), expected.to_bits(), "10^{k}");
        }
        assert!(pow10(309).is_infinite() && pow10(i32::MAX).is_infinite());
        assert_eq!(pow10(-400), 0.0);
        assert_eq!(pow10(i32::MIN), 0.0);
    }

    #[test]
    fn a_span_of_a_few_subnormal_numbers_neither_hangs_nor_draws_nan() {
        // raw = span / 5 rounds to 0: log10 is -inf, and the step is 0 (no tick, as it always was).
        let (ticks, step) = nice_ticks(0.0, 5e-324);
        assert!(ticks.is_empty() && step == 0.0);
        // raw is subnormal but not 0: the ticks are drawn.
        let (ticks, step) = nice_ticks(0.0, 1e-307);
        assert!(step > 0.0 && ticks.len() >= 2, "{ticks:?} {step}");
        assert!(ticks.iter().all(|t| t.is_finite()), "{ticks:?}");
    }

    #[test]
    fn a_log_axis_covers_whole_decades_and_leaves_room_for_blq() {
        let a = axis("Concentration", true);
        let s = logarithmic(&a, &[0.0, 0.5, 30.0]).unwrap();
        assert_eq!(
            s.axis
                .ticks
                .iter()
                .map(|t| t.label.as_str())
                .collect::<Vec<_>>(),
            vec!["0.1", "1", "10", "100"]
        );
        // Whole decades -1 to 2, then the margins: 0.12 below, 0.04 above.
        assert!((s.axis.min - -1.12).abs() < 1e-12 && (s.axis.max - 2.04).abs() < 1e-12);
        assert!(s.axis.min < s.blq_at && s.blq_at < -1.0);
        assert_eq!(s.value(0.0), None);
        assert!(logarithmic(&a, &[0.0, -1.0]).is_none());
        // One value: a decade around it.
        let one = logarithmic(&a, &[5.0]).unwrap();
        let l5 = libm::log10(5.0);
        assert!(one.axis.min < l5 && l5 < one.axis.max);
    }

    #[test]
    fn a_log_axis_holds_log10_points_and_says_what_it_hides() {
        let raw = vec![panel(
            true,
            None,
            vec![series(
                "A",
                0,
                vec![
                    PlotPoint {
                        x: 0.0,
                        y: 0.0,
                        blq: true,
                    },
                    pt(1.0, 100.0),
                    pt(2.0, 0.0),
                    pt(4.0, 0.5),
                ],
            )],
        )];
        let scene = assemble("t".to_owned(), &raw, Vec::new());
        let p = &scene.panels[0];
        assert!(p.y.log && !p.x.log && p.empty.is_none());
        let s = &p.series[0];
        // The zero is hidden, the others are log10, the BLQ marker sits in the margin.
        assert_eq!(s.points.len(), 3);
        assert_eq!(
            s.points[1],
            ScenePoint {
                x: 1.0,
                y: 2.0,
                blq: false
            }
        );
        assert!(s.points[0].blq && s.points[0].y < p.y.ticks[0].value);
        let h = p.hidden.as_ref().unwrap();
        assert_eq!(h.count, 1);
        assert_eq!((h.points[0].x, h.points[0].y), (2.0, 0.0));
        assert!(h.note.starts_with("1 point(s) at zero or below"));
        // A BLQ point in the data adds its entry to the legend, even for one series.
        assert_eq!(scene.legend.entries.len(), 1);
        assert_eq!(
            scene.legend.below_limit.as_deref(),
            Some("below the limit of quantification")
        );
    }

    #[test]
    fn a_log_panel_with_no_positive_value_is_empty_and_says_so() {
        let raw = vec![panel(true, None, vec![series("A", 0, vec![pt(1.0, 0.0)])])];
        let p = &assemble("t".to_owned(), &raw, Vec::new()).panels[0];
        assert!(
            p.empty
                .as_deref()
                .is_some_and(|s| s.contains("nothing to show"))
        );
        assert!(p.series.is_empty() && p.y.ticks.is_empty());
    }

    #[test]
    fn guides_follow_the_axes() {
        let zero = vec![panel(
            false,
            Some(PlotGuide::Zero),
            vec![series("A", 0, vec![pt(0.0, -1.0), pt(3.0, 2.0)])],
        )];
        let g = assemble("t".to_owned(), &zero, Vec::new()).panels[0]
            .guide
            .unwrap();
        assert_eq!(g.kind, PlotGuide::Zero);
        assert_eq!((g.from[1], g.to[1]), (0.0, 0.0));
        // No zero line on a logarithmic vertical axis.
        let log = vec![panel(
            true,
            Some(PlotGuide::Zero),
            vec![series("A", 0, vec![pt(1.0, 1.0)])],
        )];
        assert!(
            assemble("t".to_owned(), &log, Vec::new()).panels[0]
                .guide
                .is_none()
        );
        // The identity line is the diagonal of the shared range.
        let id = vec![panel(
            false,
            Some(PlotGuide::Identity),
            vec![series("A", 0, vec![pt(1.0, 2.0), pt(5.0, 4.0)])],
        )];
        let p = &assemble("t".to_owned(), &id, Vec::new()).panels[0];
        let g = p.guide.unwrap();
        assert_eq!(g.from, [p.x.min, p.x.min]);
        assert_eq!(g.to, [p.x.max, p.x.max]);
        assert_eq!((p.x.min, p.x.max), (p.y.min, p.y.max));
    }

    #[test]
    fn the_legend_has_one_entry_per_name_and_none_for_one_series() {
        let one = vec![panel(false, None, vec![series("A", 0, vec![pt(0.0, 1.0)])])];
        let l = assemble("t".to_owned(), &one, Vec::new()).legend;
        assert!(l.entries.is_empty() && l.below_limit.is_none());
        let two = vec![
            panel(
                false,
                None,
                vec![
                    series("A", 0, vec![pt(0.0, 1.0)]),
                    series("B", 1, vec![pt(0.0, 1.0)]),
                ],
            ),
            panel(false, None, vec![series("A", 0, vec![pt(0.0, 1.0)])]),
        ];
        let l = assemble("t".to_owned(), &two, Vec::new()).legend;
        assert_eq!(
            l.entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "B"]
        );
    }

    #[test]
    fn a_scene_has_no_pixel_and_no_colour_and_round_trips_through_json() {
        let raw = vec![panel(
            false,
            None,
            vec![series("A", 3, vec![pt(0.0, 1.0), pt(1.0, 2.0)])],
        )];
        let scene = assemble("t".to_owned(), &raw, vec!["a note".to_owned()]);
        let json = serde_json::to_value(&scene).unwrap();
        assert_eq!(json["panels"][0]["series"][0]["role"], "subject");
        assert_eq!(json["panels"][0]["series"][0]["slot"], 3);
        let text = json.to_string();
        assert!(!text.contains('#') && !text.contains("color") && !text.contains("px"));
        let back: PlotScene = serde_json::from_value(json).unwrap();
        assert_eq!(back, scene);
    }
}
