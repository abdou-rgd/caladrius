//! A plot result as an SVG document (T-110): the panels in a grid, each with its axes, ticks,
//! grid lines, series and reference line, a title above and one legend below. The points are
//! placed where the stored values say; nothing is computed but their position on the page.
//! Plain text out: the caller writes the file.

use std::fmt::Write as _;

use caladrius_project::{
    PlotAxis, PlotGuide, PlotPanel, PlotPoint, PlotRun, PlotSeries, SeriesRole, SeriesStyle,
};

/// The colours of an exported plot: the light theme of the interface (a document is printed on
/// white). `caladrius-ui` checks that its light theme tokens give the same values.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgColors {
    pub background: &'static str,
    pub text: &'static str,
    pub text_muted: &'static str,
    pub grid: &'static str,
    pub observed: &'static str,
    pub predicted: &'static str,
    pub other: &'static str,
    pub subjects: [&'static str; 8],
}

/// The colours used by `export.plot`.
pub const COLORS: SvgColors = SvgColors {
    background: "#ffffff",
    text: "#132017",
    text_muted: "#5b675e",
    grid: "#e3e8df",
    observed: "#33415c",
    predicted: "#c62828",
    other: "#8b95a7",
    subjects: [
        "#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#6250d6", "#e34948",
    ],
};

/// The size of one panel, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Size {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

/// The default size of one panel.
pub(crate) const PANEL: Size = Size {
    width: 420.0,
    height: 300.0,
};

const MARGIN_LEFT: f64 = 64.0;
const MARGIN_RIGHT: f64 = 16.0;
const MARGIN_TOP: f64 = 26.0;
const MARGIN_BOTTOM: f64 = 46.0;
const TITLE_BAND: f64 = 34.0;
const LEGEND_ROW: f64 = 20.0;
const FONT: &str = "font-family=\"sans-serif\"";
const MARKER: f64 = 4.0;

/// The panels per row: a linear and a logarithmic panel of the same thing side by side, else up
/// to three per row.
fn columns(panels: &[PlotPanel]) -> usize {
    let n = panels.len().max(1);
    let mixed = panels.iter().any(|p| p.y.log) && panels.iter().any(|p| !p.y.log);
    if mixed { 2.min(n) } else { 3.min(n) }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// A coordinate on the page, with two decimals at most.
fn px(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = format!("{r:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// Tick values from `lo` to `hi` at a step of 1, 2 or 5 times a power of ten (about five).
fn nice_ticks(lo: f64, hi: f64) -> (Vec<f64>, f64) {
    let span = hi - lo;
    if !(span.is_finite() && span > 0.0) {
        return (vec![lo], 1.0);
    }
    let raw = span / 5.0;
    let power = 10f64.powf(raw.log10().floor());
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
    let decimals = (-step.log10().floor()).clamp(0.0, 8.0) as usize;
    format!("{v:.decimals$}")
}

/// A tick label for a decade: `0.01`, `1`, `1000`, `1e-6`.
fn decade_label(k: i32) -> String {
    match k {
        -4..=-1 => format!("{:.*}", k.unsigned_abs() as usize, 10f64.powi(k)),
        0..=5 => format!("{}", 10f64.powi(k)),
        _ => format!("1e{k}"),
    }
}

/// One axis on the page: its range in data units (decades on a log axis) and its ticks.
#[derive(Debug, Clone)]
struct Scale {
    lo: f64,
    hi: f64,
    log: bool,
    ticks: Vec<(f64, String)>,
    /// Where a BLQ point sits on this axis (a vertical axis only).
    blq_at: f64,
}

impl Scale {
    /// The position of a data value, `None` when a log axis cannot show it.
    fn value(&self, v: f64) -> Option<f64> {
        if self.log {
            (v > 0.0 && v.is_finite()).then(|| v.log10())
        } else {
            v.is_finite().then_some(v)
        }
    }

    fn fraction(&self, t: f64) -> f64 {
        if self.hi > self.lo {
            (t - self.lo) / (self.hi - self.lo)
        } else {
            0.5
        }
    }
}

fn range(values: &[f64]) -> Option<(f64, f64)> {
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}

/// A linear axis over `values`, with a margin, and 0 inside when `zero` asks for it.
fn linear(values: &[f64], zero: bool) -> Scale {
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
        lo,
        hi,
        log: false,
        ticks: ticks.iter().map(|v| (*v, tick_label(*v, step))).collect(),
        blq_at: 0.0,
    }
}

/// A log axis over the positive `values`, rounded out to whole decades, with a margin below
/// for BLQ markers; `None` when no value is positive.
fn logarithmic(values: &[f64]) -> Option<Scale> {
    let logs: Vec<f64> = values
        .iter()
        .filter(|v| **v > 0.0 && v.is_finite())
        .map(|v| v.log10())
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
        .map(|k| (f64::from(k), decade_label(k)))
        .collect();
    Some(Scale {
        lo: d_lo - 0.12,
        hi: d_hi + 0.04,
        log: true,
        ticks,
        blq_at: d_lo - 0.06,
    })
}

fn scale_for(axis: &PlotAxis, values: &[f64], zero: bool) -> Option<Scale> {
    if axis.log {
        logarithmic(values)
    } else {
        Some(linear(values, zero))
    }
}

fn color(role: SeriesRole) -> &'static str {
    match role {
        SeriesRole::Subject { slot } => COLORS
            .subjects
            .get(slot % COLORS.subjects.len())
            .copied()
            .unwrap_or(COLORS.observed),
        SeriesRole::Observed => COLORS.observed,
        SeriesRole::Predicted => COLORS.predicted,
    }
}

/// The marker of a series: subjects take the next shape after each round of colours, so colour
/// is never the only way to tell two subjects apart.
fn shape(role: SeriesRole) -> usize {
    match role {
        SeriesRole::Subject { slot } => slot % 5,
        _ => 0,
    }
}

fn marker(out: &mut String, shape: usize, x: f64, y: f64, fill: &str) {
    let r = MARKER;
    let poly = |pts: &[(f64, f64)]| {
        pts.iter()
            .map(|(a, b)| format!("{},{}", px(x + a), px(y + b)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let _ = match shape {
        1 => writeln!(
            out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\"/>",
            px(x - r * 0.85),
            px(y - r * 0.85),
            px(r * 1.7),
            px(r * 1.7)
        ),
        2 => writeln!(
            out,
            "<polygon points=\"{}\" fill=\"{fill}\"/>",
            poly(&[
                (0.0, -r * 1.2),
                (r * 1.2, 0.0),
                (0.0, r * 1.2),
                (-r * 1.2, 0.0)
            ])
        ),
        3 => writeln!(
            out,
            "<polygon points=\"{}\" fill=\"{fill}\"/>",
            poly(&[(0.0, -r * 1.1), (r * 1.1, r * 0.9), (-r * 1.1, r * 0.9)])
        ),
        4 => writeln!(
            out,
            "<path d=\"M{} {}L{} {}M{} {}L{} {}\" stroke=\"{fill}\" stroke-width=\"2\"/>",
            px(x - r),
            px(y - r),
            px(x + r),
            px(y + r),
            px(x - r),
            px(y + r),
            px(x + r),
            px(y - r)
        ),
        _ => writeln!(
            out,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{fill}\"/>",
            px(x),
            px(y),
            px(r)
        ),
    };
}

/// The hollow triangle pointing down of a point below the limit of quantification.
fn blq_marker(out: &mut String, x: f64, y: f64, stroke: &str) {
    let r = MARKER * 1.3;
    let _ = writeln!(
        out,
        "<polygon points=\"{},{} {},{} {},{}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\"/>",
        px(x - r),
        px(y - r * 0.8),
        px(x + r),
        px(y - r * 0.8),
        px(x),
        px(y + r * 0.8)
    );
}

/// Draws one panel with its top-left corner at (`left`, `top`).
fn panel(out: &mut String, p: &PlotPanel, index: usize, left: f64, top: f64, size: Size) {
    let c = &COLORS;
    let x0 = left + MARGIN_LEFT;
    let x1 = left + size.width - MARGIN_RIGHT;
    let y0 = top + MARGIN_TOP;
    let y1 = top + size.height - MARGIN_BOTTOM;
    let _ = writeln!(
        out,
        "<text x=\"{}\" y=\"{}\" {FONT} font-size=\"13\" font-weight=\"bold\" fill=\"{}\">{}</text>",
        px(x0),
        px(top + 17.0),
        c.text,
        escape(&p.title)
    );
    let all: Vec<&PlotPoint> = p.series.iter().flat_map(|s| &s.points).collect();
    let xs: Vec<f64> = all.iter().map(|q| q.x).collect();
    let ys: Vec<f64> = all.iter().filter(|q| !q.blq).map(|q| q.y).collect();
    // Observed vs predicted shares one range on both axes, so that the identity line is the diagonal.
    let shared: Vec<f64> = xs.iter().chain(&ys).copied().collect();
    let (xv, yv): (&[f64], &[f64]) = match p.guide {
        Some(PlotGuide::Identity) => (&shared, &shared),
        _ => (&xs, &ys),
    };
    let zero = matches!(p.guide, Some(PlotGuide::Zero)) || all.iter().any(|q| q.blq);
    let (Some(sx), Some(sy)) = (scale_for(&p.x, xv, false), scale_for(&p.y, yv, zero)) else {
        let _ = writeln!(
            out,
            "<text x=\"{}\" y=\"{}\" {FONT} font-size=\"12\" fill=\"{}\">No value above zero: nothing to show on a logarithmic axis.</text>",
            px(x0),
            px((y0 + y1) / 2.0),
            c.text_muted
        );
        return;
    };
    let to_x = |v: f64| x0 + sx.fraction(v) * (x1 - x0);
    let to_y = |v: f64| y1 - sy.fraction(v) * (y1 - y0);
    // Frame, grid and ticks.
    let _ = writeln!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\"/>",
        px(x0),
        px(y0),
        px(x1 - x0),
        px(y1 - y0),
        c.background,
        c.grid
    );
    for (v, label) in &sx.ticks {
        let x = to_x(*v);
        let _ = writeln!(
            out,
            "<line x1=\"{0}\" y1=\"{1}\" x2=\"{0}\" y2=\"{2}\" stroke=\"{3}\"/>\n<text x=\"{0}\" y=\"{4}\" {FONT} font-size=\"11\" text-anchor=\"middle\" fill=\"{5}\">{6}</text>",
            px(x),
            px(y0),
            px(y1),
            c.grid,
            px(y1 + 15.0),
            c.text_muted,
            escape(label)
        );
    }
    for (v, label) in &sy.ticks {
        let y = to_y(*v);
        let _ = writeln!(
            out,
            "<line x1=\"{0}\" y1=\"{1}\" x2=\"{2}\" y2=\"{1}\" stroke=\"{3}\"/>\n<text x=\"{4}\" y=\"{5}\" {FONT} font-size=\"11\" text-anchor=\"end\" fill=\"{6}\">{7}</text>",
            px(x0),
            px(y),
            px(x1),
            c.grid,
            px(x0 - 6.0),
            px(y + 4.0),
            c.text_muted,
            escape(label)
        );
    }
    let _ = writeln!(
        out,
        "<text x=\"{}\" y=\"{}\" {FONT} font-size=\"12\" text-anchor=\"middle\" fill=\"{}\">{}</text>",
        px((x0 + x1) / 2.0),
        px(y1 + 34.0),
        c.text,
        escape(&p.x.title())
    );
    let (lx, ly) = (left + 16.0, (y0 + y1) / 2.0);
    let _ = writeln!(
        out,
        "<text x=\"{0}\" y=\"{1}\" {FONT} font-size=\"12\" text-anchor=\"middle\" fill=\"{2}\" transform=\"rotate(-90 {0} {1})\">{3}</text>",
        px(lx),
        px(ly),
        c.text,
        escape(&p.y.title())
    );
    // Everything else stays inside the frame.
    let clip = format!("clip-{index}");
    let _ = writeln!(
        out,
        "<clipPath id=\"{clip}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>\n<g clip-path=\"url(#{clip})\">",
        px(x0),
        px(y0),
        px(x1 - x0),
        px(y1 - y0)
    );
    match p.guide {
        Some(PlotGuide::Zero) if !sy.log => {
            let _ = writeln!(
                out,
                "<line x1=\"{}\" y1=\"{2}\" x2=\"{}\" y2=\"{2}\" stroke=\"{3}\" stroke-width=\"1.5\"/>",
                px(x0),
                px(x1),
                px(to_y(0.0)),
                c.other
            );
        }
        Some(PlotGuide::Identity) => {
            // The diagonal of the shared range (the same on both axes).
            let _ = writeln!(
                out,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\"/>",
                px(to_x(sx.lo)),
                px(to_y(sx.lo)),
                px(to_x(sx.hi)),
                px(to_y(sx.hi)),
                c.other
            );
        }
        _ => {}
    }
    for s in &p.series {
        series(out, s, &sx, &sy, &to_x, &to_y);
    }
    let _ = writeln!(out, "</g>");
}

fn series(
    out: &mut String,
    s: &PlotSeries,
    sx: &Scale,
    sy: &Scale,
    to_x: &dyn Fn(f64) -> f64,
    to_y: &dyn Fn(f64) -> f64,
) {
    let fill = color(s.role);
    let placed: Vec<(f64, f64)> = s
        .points
        .iter()
        .filter(|q| !q.blq)
        .filter_map(|q| Some((to_x(sx.value(q.x)?), to_y(sy.value(q.y)?))))
        .collect();
    if matches!(s.style, SeriesStyle::Line | SeriesStyle::LinePoints) && placed.len() > 1 {
        let path: Vec<String> = placed
            .iter()
            .map(|(x, y)| format!("{},{}", px(*x), px(*y)))
            .collect();
        let width = if s.style == SeriesStyle::Line {
            "2"
        } else {
            "1.5"
        };
        let _ = writeln!(
            out,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{fill}\" stroke-width=\"{width}\"/>",
            path.join(" ")
        );
    }
    if matches!(s.style, SeriesStyle::Points | SeriesStyle::LinePoints) {
        for (x, y) in &placed {
            marker(out, shape(s.role), *x, *y, fill);
        }
    }
    let at = to_y(sy.blq_at);
    for q in s.points.iter().filter(|q| q.blq) {
        if let Some(x) = sx.value(q.x) {
            blq_marker(out, to_x(x), at, fill);
        }
    }
}

/// The entries of the legend: one per distinct series name, in order of appearance.
fn legend_entries(run: &PlotRun) -> Vec<&PlotSeries> {
    let mut seen: Vec<&PlotSeries> = Vec::new();
    for s in run.panels.iter().flat_map(|p| &p.series) {
        if !seen.iter().any(|e| e.name == s.name) {
            seen.push(s);
        }
    }
    seen
}

/// The SVG document of a plot result. `size` is the size of one panel.
pub(crate) fn render(run: &PlotRun, size: Size) -> String {
    let c = &COLORS;
    let cols = columns(&run.panels);
    let rows = run.panels.len().div_ceil(cols).max(1);
    let width = size.width * cols as f64;
    let entries = legend_entries(run);
    let has_blq = run
        .panels
        .iter()
        .flat_map(|p| &p.series)
        .any(|s| s.points.iter().any(|q| q.blq));
    // The legend wraps at the width of the figure; about 7 px per character at 12 px.
    let item_width = |name: &str| 34.0 + 7.0 * name.chars().count() as f64;
    let mut legend_rows = 0usize;
    let mut cursor = f64::INFINITY;
    let mut placed: Vec<(f64, usize)> = Vec::new();
    let show_legend = entries.len() > 1 || has_blq;
    let mut items: Vec<String> = if show_legend {
        entries.iter().map(|e| e.name.clone()).collect()
    } else {
        Vec::new()
    };
    if has_blq {
        items.push("below the limit of quantification".to_owned());
    }
    for name in &items {
        let w = item_width(name);
        if cursor + w > width - 16.0 {
            legend_rows += 1;
            cursor = 16.0;
        }
        placed.push((cursor, legend_rows.saturating_sub(1)));
        cursor += w + 12.0;
    }
    let grid_top = TITLE_BAND;
    let height = grid_top + size.height * rows as f64 + LEGEND_ROW * legend_rows as f64 + 8.0;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{0}\" height=\"{1}\" viewBox=\"0 0 {0} {1}\">",
        px(width),
        px(height)
    );
    let _ = writeln!(out, "<title>{}</title>", escape(&run.title));
    let _ = writeln!(
        out,
        "<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>",
        c.background
    );
    let _ = writeln!(
        out,
        "<text x=\"16\" y=\"23\" {FONT} font-size=\"16\" font-weight=\"bold\" fill=\"{}\">{}</text>",
        c.text,
        escape(&run.title)
    );
    for (i, p) in run.panels.iter().enumerate() {
        let left = size.width * (i % cols) as f64;
        let top = grid_top + size.height * (i / cols) as f64;
        panel(&mut out, p, i, left, top, size);
    }
    let legend_top = grid_top + size.height * rows as f64;
    for (k, name) in items.iter().enumerate() {
        let Some((x, row)) = placed.get(k).copied() else {
            continue;
        };
        let y = legend_top + LEGEND_ROW * row as f64 + 10.0;
        match entries.get(k).filter(|_| show_legend) {
            Some(e) => {
                let fill = color(e.role);
                if e.style != SeriesStyle::Points {
                    let _ = writeln!(
                        out,
                        "<line x1=\"{}\" y1=\"{2}\" x2=\"{}\" y2=\"{2}\" stroke=\"{fill}\" stroke-width=\"2\"/>",
                        px(x),
                        px(x + 22.0),
                        px(y)
                    );
                }
                if e.style != SeriesStyle::Line {
                    marker(&mut out, shape(e.role), x + 11.0, y, fill);
                }
            }
            None => blq_marker(&mut out, x + 11.0, y, c.text_muted),
        }
        let _ = writeln!(
            out,
            "<text x=\"{}\" y=\"{}\" {FONT} font-size=\"12\" fill=\"{}\">{}</text>",
            px(x + 28.0),
            px(y + 4.0),
            c.text,
            escape(name)
        );
    }
    let _ = writeln!(out, "</svg>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use caladrius_project::PlotAxis;

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

    fn run(log: bool) -> PlotRun {
        PlotRun {
            title: "A & B <invented>".to_owned(),
            panels: vec![PlotPanel {
                title: "Linear".to_owned(),
                x: axis("Time", false),
                y: axis("Concentration", log),
                series: vec![
                    PlotSeries {
                        name: "A".to_owned(),
                        role: SeriesRole::Subject { slot: 0 },
                        style: SeriesStyle::LinePoints,
                        points: vec![
                            PlotPoint {
                                x: 0.0,
                                y: 0.0,
                                blq: true,
                            },
                            pt(1.0, 8.0),
                            pt(2.0, 4.0),
                        ],
                    },
                    PlotSeries {
                        name: "B".to_owned(),
                        role: SeriesRole::Subject { slot: 6 },
                        style: SeriesStyle::LinePoints,
                        points: vec![pt(1.0, 6.0), pt(2.0, 0.0)],
                    },
                ],
                guide: None,
            }],
            notes: Vec::new(),
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
    fn a_log_axis_covers_whole_decades_and_leaves_room_for_blq() {
        let s = logarithmic(&[0.0, 0.5, 30.0]).unwrap();
        assert_eq!(
            s.ticks.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(),
            vec!["0.1", "1", "10", "100"]
        );
        assert!(s.lo < s.blq_at && s.blq_at < -1.0);
        assert_eq!(s.value(0.0), None);
        assert!(logarithmic(&[0.0, -1.0]).is_none());
        // One value: a decade around it.
        let one = logarithmic(&[5.0]).unwrap();
        assert!(one.lo < 5f64.log10() && 5f64.log10() < one.hi);
    }

    #[test]
    fn the_document_is_escaped_and_holds_every_point() {
        let svg = render(&run(false), PANEL);
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("<title>A &amp; B &lt;invented&gt;</title>"));
        assert!(!svg.contains("<invented>"));
        // Two lines, 4 measured markers (A: circles, B slot 6: squares), one BLQ triangle.
        assert_eq!(svg.matches("<polyline").count(), 2);
        assert_eq!(svg.matches("<circle").count(), 2 + 1);
        // B: its line, two squares, and the line and square of its legend entry.
        assert_eq!(svg.matches(COLORS.subjects[6]).count(), 1 + 2 + 2);
        // Squares of B and its legend entry, the frame of the panel and its clip.
        assert_eq!(svg.matches("<rect x=").count(), 2 + 1 + 2);
        // Two polylines, the BLQ triangle and its legend entry are drawn hollow.
        assert_eq!(svg.matches("fill=\"none\" stroke=").count(), 2 + 1 + 1);
        assert!(svg.contains("below the limit of quantification"));
        assert!(svg.contains("Time (h)"));
        // Every opened element is closed.
        assert_eq!(svg.matches("<g ").count(), svg.matches("</g>").count());
        assert_eq!(svg.matches("<text").count(), svg.matches("</text>").count());
    }

    #[test]
    fn a_log_panel_leaves_out_zero_and_says_when_nothing_is_left() {
        let svg = render(&run(true), PANEL);
        // B's zero is not on the line: B has one placed point, so no polyline for it.
        assert_eq!(svg.matches("<polyline").count(), 1);
        let mut empty = run(true);
        if let Some(p) = empty.panels.first_mut() {
            p.series.retain(|s| s.name == "B");
            if let Some(s) = p.series.first_mut() {
                s.points = vec![pt(1.0, 0.0)];
            }
        }
        assert!(render(&empty, PANEL).contains("nothing to show on a logarithmic axis"));
    }

    #[test]
    fn panels_go_two_per_row_for_linear_and_log_else_three() {
        let mut r = run(false);
        let mut log = r.panels[0].clone();
        log.y.log = true;
        r.panels = vec![r.panels[0].clone(), log.clone(), r.panels[0].clone(), log];
        assert_eq!(columns(&r.panels), 2);
        let svg = render(&r, PANEL);
        assert!(svg.contains(&format!("width=\"{}\"", px(2.0 * PANEL.width))));
        r.panels.retain(|p| !p.y.log);
        assert_eq!(columns(&r.panels), 2);
        r.panels.push(r.panels[0].clone());
        r.panels.push(r.panels[0].clone());
        assert_eq!(columns(&r.panels), 3);
    }
}
