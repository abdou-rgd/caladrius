//! A plot scene as an SVG document (T-110, T-133): the panels in a grid, each with its axes, ticks,
//! grid lines, series and reference line, a title above and one legend below. It draws the
//! [`PlotScene`] and nothing else: the ranges, the ticks, the labels, the hidden points and the
//! legend are the scene's, and the only computation here is the affine map from scene units to
//! the page. Plain text out: the caller writes the file.

use std::fmt::Write as _;

use caladrius_project::{PlotGuide, SeriesStyle};

use crate::scene::{PlotScene, ScenePanel, SceneRole, SceneSeries};

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
    /// The points used for λz (NCA).
    pub selected: &'static str,
    /// The first point of the λz range (NCA).
    pub start: &'static str,
    /// A value a policy put in place of a missing or BLQ one (NCA).
    pub replaced: &'static str,
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
    selected: "#2255c4",
    start: "#00897b",
    replaced: "#9a4d00",
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
fn columns(panels: &[ScenePanel]) -> usize {
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

fn color(role: SceneRole) -> &'static str {
    match role {
        SceneRole::Subject { slot } => COLORS
            .subjects
            .get(slot % COLORS.subjects.len())
            .copied()
            .unwrap_or(COLORS.observed),
        SceneRole::Observed => COLORS.observed,
        SceneRole::Predicted | SceneRole::LambdaZFit => COLORS.predicted,
        SceneRole::Profile | SceneRole::Excluded => COLORS.other,
        SceneRole::Replaced => COLORS.replaced,
        SceneRole::LambdaZUsed => COLORS.selected,
        SceneRole::LambdaZStart => COLORS.start,
    }
}

/// The marker of a series: subjects take the next shape after each round of colours, so colour
/// is never the only way to tell two subjects apart; the roles of an NCA profile each have one.
fn shape(role: SceneRole) -> usize {
    match role {
        SceneRole::Subject { slot } => slot % 5,
        SceneRole::Replaced => 2,
        SceneRole::LambdaZStart => 3,
        SceneRole::Excluded => 4,
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
fn panel(out: &mut String, p: &ScenePanel, index: usize, left: f64, top: f64, size: Size) {
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
    if let Some(sentence) = &p.empty {
        let _ = writeln!(
            out,
            "<text x=\"{}\" y=\"{}\" {FONT} font-size=\"12\" fill=\"{}\">{}</text>",
            px(x0),
            px((y0 + y1) / 2.0),
            c.text_muted,
            escape(sentence)
        );
        return;
    }
    let (sx, sy) = (&p.x, &p.y);
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
    for tick in &sx.ticks {
        let x = to_x(tick.value);
        let _ = writeln!(
            out,
            "<line x1=\"{0}\" y1=\"{1}\" x2=\"{0}\" y2=\"{2}\" stroke=\"{3}\"/>\n<text x=\"{0}\" y=\"{4}\" {FONT} font-size=\"11\" text-anchor=\"middle\" fill=\"{5}\">{6}</text>",
            px(x),
            px(y0),
            px(y1),
            c.grid,
            px(y1 + 15.0),
            c.text_muted,
            escape(&tick.label)
        );
    }
    for tick in &sy.ticks {
        let y = to_y(tick.value);
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
            escape(&tick.label)
        );
    }
    let _ = writeln!(
        out,
        "<text x=\"{}\" y=\"{}\" {FONT} font-size=\"12\" text-anchor=\"middle\" fill=\"{}\">{}</text>",
        px((x0 + x1) / 2.0),
        px(y1 + 34.0),
        c.text,
        escape(&sx.title)
    );
    let (lx, ly) = (left + 16.0, (y0 + y1) / 2.0);
    let _ = writeln!(
        out,
        "<text x=\"{0}\" y=\"{1}\" {FONT} font-size=\"12\" text-anchor=\"middle\" fill=\"{2}\" transform=\"rotate(-90 {0} {1})\">{3}</text>",
        px(lx),
        px(ly),
        c.text,
        escape(&sy.title)
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
    if let Some(g) = &p.guide {
        let (ax, ay) = (px(to_x(g.from[0])), px(to_y(g.from[1])));
        let (bx, by) = (px(to_x(g.to[0])), px(to_y(g.to[1])));
        let dash = match g.kind {
            PlotGuide::Zero => "",
            PlotGuide::Identity => " stroke-dasharray=\"6 4\"",
        };
        let _ = writeln!(
            out,
            "<line x1=\"{ax}\" y1=\"{ay}\" x2=\"{bx}\" y2=\"{by}\" stroke=\"{}\" stroke-width=\"1.5\"{dash}/>",
            c.other
        );
    }
    for s in &p.series {
        series(out, s, &to_x, &to_y);
    }
    let _ = writeln!(out, "</g>");
}

fn series(out: &mut String, s: &SceneSeries, to_x: &dyn Fn(f64) -> f64, to_y: &dyn Fn(f64) -> f64) {
    let fill = color(s.role);
    let placed: Vec<(f64, f64)> = s
        .points
        .iter()
        .filter(|q| !q.blq)
        .map(|q| (to_x(q.x), to_y(q.y)))
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
    for q in s.points.iter().filter(|q| q.blq) {
        blq_marker(out, to_x(q.x), to_y(q.y), fill);
    }
}

/// The SVG document of a plot scene. `size` is the size of one panel.
pub(crate) fn render(scene: &PlotScene, size: Size) -> String {
    let c = &COLORS;
    let cols = columns(&scene.panels);
    let rows = scene.panels.len().div_ceil(cols).max(1);
    let width = size.width * cols as f64;
    // The legend wraps at the width of the figure; about 7 px per character at 12 px.
    let item_width = |name: &str| 34.0 + 7.0 * name.chars().count() as f64;
    let mut legend_rows = 0usize;
    let mut cursor = f64::INFINITY;
    let mut placed: Vec<(f64, usize)> = Vec::new();
    let mut items: Vec<&str> = scene
        .legend
        .entries
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    if let Some(sentence) = &scene.legend.below_limit {
        items.push(sentence);
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
    let _ = writeln!(out, "<title>{}</title>", escape(&scene.title));
    let _ = writeln!(
        out,
        "<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>",
        c.background
    );
    let _ = writeln!(
        out,
        "<text x=\"16\" y=\"23\" {FONT} font-size=\"16\" font-weight=\"bold\" fill=\"{}\">{}</text>",
        c.text,
        escape(&scene.title)
    );
    for (i, p) in scene.panels.iter().enumerate() {
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
        match scene.legend.entries.get(k) {
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
    use crate::scene;
    use caladrius_project::{PlotAxis, PlotPanel, PlotPoint, PlotRun, PlotSeries, SeriesRole};

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

    fn draw(run: &PlotRun) -> String {
        render(&scene::from_run(run), PANEL)
    }

    #[test]
    fn the_document_is_escaped_and_holds_every_point() {
        let svg = draw(&run(false));
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
        let svg = draw(&run(true));
        // B's zero is not on the line: B has one placed point, so no polyline for it.
        assert_eq!(svg.matches("<polyline").count(), 1);
        let mut empty = run(true);
        if let Some(p) = empty.panels.first_mut() {
            p.series.retain(|s| s.name == "B");
            if let Some(s) = p.series.first_mut() {
                s.points = vec![pt(1.0, 0.0)];
            }
        }
        assert!(draw(&empty).contains("nothing to show on a logarithmic axis"));
    }

    #[test]
    fn panels_go_two_per_row_for_linear_and_log_else_three() {
        let mut r = run(false);
        let mut log = r.panels[0].clone();
        log.y.log = true;
        r.panels = vec![r.panels[0].clone(), log.clone(), r.panels[0].clone(), log];
        assert_eq!(columns(&scene::from_run(&r).panels), 2);
        let svg = draw(&r);
        assert!(svg.contains(&format!("width=\"{}\"", px(2.0 * PANEL.width))));
        r.panels.retain(|p| !p.y.log);
        assert_eq!(columns(&scene::from_run(&r).panels), 2);
        r.panels.push(r.panels[0].clone());
        r.panels.push(r.panels[0].clone());
        assert_eq!(columns(&scene::from_run(&r).panels), 3);
    }
}
