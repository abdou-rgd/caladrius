//! The map between the plot scene and the pixels, and back. The scene holds a logarithmic axis already
//! in log10 (its points, its range and its ticks), so one affine map is all a front applies: no
//! logarithm is taken here, and a click is turned into a sample by the same map in reverse.

use crate::types::{Role, SceneAxis, ScenePanel, ScenePoint, SceneSeries};

/// The drawing area inside an SVG of `width` by `height` pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub width: f64,
    pub height: f64,
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

/// Where `t` (scene units) falls between 0 (`min`) and 1 (`max`); 0.5 for an empty range, as the engine's own `fraction`.
pub fn fraction(axis: &SceneAxis, t: f64) -> f64 {
    if axis.max > axis.min {
        (t - axis.min) / (axis.max - axis.min)
    } else {
        0.5
    }
}

/// The inverse of [`fraction`].
#[cfg(test)]
pub fn unfraction(axis: &SceneAxis, f: f64) -> f64 {
    axis.min + f * (axis.max - axis.min)
}

pub fn plot_rect(frame: &Frame) -> Rect {
    Rect {
        x0: frame.left,
        y0: frame.top,
        x1: (frame.left + 1.0).max(frame.width - frame.right),
        y1: (frame.top + 1.0).max(frame.height - frame.bottom),
    }
}

pub fn px_of_x(frame: &Frame, panel: &ScenePanel, x: f64) -> f64 {
    let r = plot_rect(frame);
    r.x0 + fraction(&panel.x, x) * (r.x1 - r.x0)
}

pub fn py_of_y(frame: &Frame, panel: &ScenePanel, y: f64) -> f64 {
    let r = plot_rect(frame);
    r.y1 - fraction(&panel.y, y) * (r.y1 - r.y0)
}

#[cfg(test)]
pub fn x_of_px(frame: &Frame, panel: &ScenePanel, px: f64) -> f64 {
    let r = plot_rect(frame);
    unfraction(&panel.x, (px - r.x0) / (r.x1 - r.x0))
}

#[cfg(test)]
pub fn y_of_py(frame: &Frame, panel: &ScenePanel, py: f64) -> f64 {
    let r = plot_rect(frame);
    unfraction(&panel.y, (r.y1 - py) / (r.y1 - r.y0))
}

pub fn find(panel: &ScenePanel, role: Role) -> Option<&SceneSeries> {
    panel.series.iter().find(|s| s.role == role)
}

/// The samples a person can pick: the points of the profile line that the axes can show and that
/// hold a positive concentration. On a logarithmic axis the scene has already left out the points it
/// cannot show (zero or below); on a linear axis a zero is a point of the profile but not a sample
/// the terminal phase can use. A marker below the limit (`blq`) is never one.
pub fn selectable(panel: &ScenePanel) -> Vec<ScenePoint> {
    match find(panel, Role::Profile) {
        Some(profile) => profile
            .points
            .iter()
            .filter(|p| !p.blq && (panel.y.log || p.y > 0.0))
            .copied()
            .collect(),
        None => Vec::new(),
    }
}

/// The sample nearest to a pixel, within `radius` pixels (Euclidean), or none.
pub fn pick_point(
    frame: &Frame,
    panel: &ScenePanel,
    px: f64,
    py: f64,
    radius: f64,
) -> Option<ScenePoint> {
    let mut best = None;
    let mut best_d = radius * radius;
    for p in selectable(panel) {
        let dx = px_of_x(frame, panel, p.x) - px;
        let dy = py_of_y(frame, panel, p.y) - py;
        let d = dx * dx + dy * dy;
        if d <= best_d {
            best_d = d;
            best = Some(p);
        }
    }
    best
}

/// The sample whose column is nearest to a pixel: where a handle lands while it is dragged.
pub fn snap_sample(frame: &Frame, panel: &ScenePanel, px: f64) -> Option<ScenePoint> {
    let mut best = None;
    let mut best_d = f64::INFINITY;
    for p in selectable(panel) {
        let d = (px_of_x(frame, panel, p.x) - px).abs();
        if d < best_d {
            best_d = d;
            best = Some(p);
        }
    }
    best
}

/// The time of the sample `steps` places after `time` (before, if negative), kept inside the list.
pub fn step_sample(panel: &ScenePanel, time: f64, steps: i32) -> Option<f64> {
    let samples = selectable(panel);
    let at = samples.iter().position(|p| p.x == time)?;
    let last = samples.len().saturating_sub(1);
    let next = (at as i64 + i64::from(steps)).clamp(0, last as i64) as usize;
    samples.get(next).map(|p| p.x)
}

/// A coordinate as text for an SVG attribute: two decimals at most. Not a displayed number.
pub fn coord(n: f64) -> String {
    let rounded = (n * 100.0).round() / 100.0;
    // `{}` of an f64 is the shortest text that reads back as the same double.
    format!("{rounded}")
}

/// The points of a line as an SVG `points` attribute.
pub fn polyline(frame: &Frame, panel: &ScenePanel, series: &SceneSeries) -> String {
    series
        .points
        .iter()
        .map(|p| {
            format!(
                "{},{}",
                coord(px_of_x(frame, panel, p.x)),
                coord(py_of_y(frame, panel, p.y))
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}
