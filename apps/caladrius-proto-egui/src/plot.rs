//! The plot, painted by hand from the `plot.scene` command and nothing else. The axes arrive in
//! scene units (a log axis is already in log10), so the only arithmetic here is an affine map to
//! pixels. Colours, sizes and strokes come from the theme tokens.

use caladrius_engine::scene::{PlotScene, SceneAxis, ScenePanel, SceneRole};
use egui::{
    Align2, Color32, FontId, Id, Key, Pos2, Rect, Role, Sense, Shape, Stroke, StrokeKind, Vec2,
    WidgetInfo, pos2, vec2,
};

use crate::state::{Handle, Screen};
use crate::theme::Tokens;

/// What the user asked for on the plot during one frame.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PlotAction {
    /// A handle was dragged to this time.
    pub drag: Option<(Handle, f64)>,
    /// A handle was moved by this many points with the keyboard.
    pub step: Option<(Handle, i32)>,
    /// A point was clicked.
    pub toggle: Option<f64>,
    /// The "switch to linear" button of the hidden-points note was pressed.
    pub linear: bool,
}

/// The id of a handle (stable, so the tests and the keyboard find it).
pub fn handle_id(handle: Handle) -> Id {
    Id::unique(("lambda-z-handle", matches!(handle, Handle::Start)))
}

/// The label a screen reader reads for a handle.
pub fn handle_label(handle: Handle, time: f64) -> String {
    let which = match handle {
        Handle::Start => "start",
        Handle::End => "end",
    };
    format!(
        "λz range {which} at time {}. Left and right arrows move it one point.",
        caladrius_format::exact(time)
    )
}

/// The drawing area inside the room the plot takes (the margins hold the ticks and the titles).
pub fn inner_rect(outer: Rect) -> Rect {
    let (left, bottom, top, right) = (64.0, 40.0, 26.0, 16.0);
    Rect::from_min_max(
        pos2(outer.left() + left, outer.top() + top),
        pos2(outer.right() - right, outer.bottom() - bottom),
    )
}

/// Where a point of the scene (scene units) falls in the room `outer` that the plot took.
pub fn pixel(outer: Rect, panel: &ScenePanel, x: f64, y: f64) -> Pos2 {
    Frame {
        rect: inner_rect(outer),
        x: panel.x.clone(),
        y: panel.y.clone(),
    }
    .px(x, y)
}

struct Frame {
    rect: Rect,
    x: SceneAxis,
    y: SceneAxis,
}

impl Frame {
    fn px(&self, x: f64, y: f64) -> Pos2 {
        pos2(
            self.rect.left() + self.x.fraction(x) as f32 * self.rect.width(),
            self.rect.bottom() - self.y.fraction(y) as f32 * self.rect.height(),
        )
    }

    fn time_at(&self, px: f32) -> f64 {
        let frac = f64::from((px - self.rect.left()) / self.rect.width().max(1.0));
        self.x.min + frac * (self.x.max - self.x.min)
    }
}

fn series_color(role: SceneRole, tokens: &Tokens) -> Color32 {
    let c = &tokens.colors;
    match role {
        SceneRole::Observed => c.series_observed.color(),
        SceneRole::Profile => c.text_muted.color(),
        SceneRole::Replaced => c.series_replaced.color(),
        SceneRole::Excluded => c.series_other.color(),
        SceneRole::LambdaZUsed => c.series_selected.color(),
        SceneRole::LambdaZStart => c.series_start.color(),
        SceneRole::LambdaZFit => c.series_fit.color(),
        SceneRole::Subject { .. } | SceneRole::Predicted => c.series_other.color(),
    }
}

fn marker_radius(role: SceneRole, tokens: &Tokens) -> f32 {
    match role {
        SceneRole::LambdaZStart => tokens.size.marker_large,
        SceneRole::LambdaZUsed => tokens.size.marker_medium,
        _ => tokens.size.marker_small,
    }
}

fn is_line(role: SceneRole) -> bool {
    matches!(role, SceneRole::Profile | SceneRole::LambdaZFit)
}

/// Draws the plot into the space left and answers what the user did on it.
pub fn show(ui: &mut egui::Ui, tokens: &Tokens, screen: &Screen) -> PlotAction {
    let mut action = PlotAction::default();
    let Some(scene) = screen.scene.as_ref() else {
        ui.label(
            screen
                .error
                .clone()
                .unwrap_or_else(|| "No scene yet: run the analysis.".to_owned()),
        );
        return action;
    };
    let Some(panel) = scene.panels.first() else {
        ui.label("The scene has no panel.");
        return action;
    };
    if let Some(empty) = &panel.empty {
        ui.label(empty.clone());
        return action;
    }
    // Room for the notes under the plot.
    let notes_h = 44.0;
    let avail = ui.available_size();
    let size = vec2(
        avail.x,
        (avail.y - notes_h).max(tokens.size.plot_min_height),
    );
    let (outer, area) = {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        (rect, response)
    };
    let painter = ui.painter_at(outer);
    painter.rect_filled(
        outer,
        tokens.radius.medium,
        tokens.colors.plot_background.color(),
    );
    let inner = inner_rect(outer);
    let frame = Frame {
        rect: inner,
        x: panel.x.clone(),
        y: panel.y.clone(),
    };
    draw_axes(&painter, &frame, panel, tokens);
    painter.text(
        pos2(outer.right() - 8.0, outer.top() + 6.0),
        Align2::RIGHT_TOP,
        &panel.title,
        FontId::proportional(tokens.font.small),
        tokens.colors.text_muted.color(),
    );
    area.widget_info(|| {
        WidgetInfo::labeled(
            Role::Image,
            true,
            format!(
                "Concentration against time, {}. {} series.",
                panel.title,
                panel.series.len()
            ),
        )
    });
    let plot_clip = ui.painter_at(inner);
    for s in &panel.series {
        let color = series_color(s.role, tokens);
        let pts: Vec<Pos2> = s.points.iter().map(|p| frame.px(p.x, p.y)).collect();
        if is_line(s.role) {
            plot_clip.add(Shape::line(pts, Stroke::new(tokens.stroke.medium, color)));
        } else {
            for (p, pt) in pts.iter().zip(&s.points) {
                let r = marker_radius(s.role, tokens);
                if pt.blq {
                    // A point below the limit: a hollow marker in the margin.
                    painter.circle_stroke(*p, r, Stroke::new(tokens.stroke.medium, color));
                } else if matches!(s.role, SceneRole::Excluded) {
                    let d = r;
                    let st = Stroke::new(tokens.stroke.medium, color);
                    painter.line_segment([*p + vec2(-d, -d), *p + vec2(d, d)], st);
                    painter.line_segment([*p + vec2(-d, d), *p + vec2(d, -d)], st);
                } else {
                    painter.circle_filled(*p, r, color);
                }
            }
        }
    }
    // A click on a point toggles it.
    if area.clicked() {
        if let Some(at) = area.interact_pointer_pos() {
            if let Some(time) = nearest_point(panel, &frame, at, tokens.size.hit_radius + 6.0) {
                action.toggle = Some(time);
            }
        }
    }
    // The two handles, on top of the points.
    if let Some((start, end)) = screen.range() {
        for (handle, time) in [(Handle::Start, start), (Handle::End, end)] {
            if let Some(next) = handle_ui(ui, &frame, tokens, handle, time) {
                match next {
                    HandleEvent::Drag(t) => action.drag = Some((handle, t)),
                    HandleEvent::Step(n) => action.step = Some((handle, n)),
                }
            }
        }
    }
    legend_and_notes(ui, tokens, scene, panel, &mut action);
    action
}

enum HandleEvent {
    Drag(f64),
    Step(i32),
}

fn handle_ui(
    ui: &mut egui::Ui,
    frame: &Frame,
    tokens: &Tokens,
    handle: Handle,
    time: f64,
) -> Option<HandleEvent> {
    let x = frame.px(time, frame.y.min).x;
    let strip = Rect::from_min_max(
        pos2(x - 7.0, frame.rect.top() - 6.0),
        pos2(x + 7.0, frame.rect.bottom()),
    );
    let id = handle_id(handle);
    let response = ui.interact(
        strip,
        id,
        Sense::click_and_drag().union(Sense::focusable_noninteractive()),
    );
    response.widget_info(|| WidgetInfo::labeled(Role::Slider, true, handle_label(handle, time)));
    let color = match handle {
        Handle::Start => tokens.colors.series_start.color(),
        Handle::End => tokens.colors.series_selected.color(),
    };
    let painter = ui.painter();
    let width = if response.hovered() || response.dragged() {
        tokens.stroke.thick
    } else {
        tokens.stroke.medium
    };
    painter.line_segment(
        [pos2(x, frame.rect.top()), pos2(x, frame.rect.bottom())],
        Stroke::new(width, color),
    );
    let grip = Rect::from_center_size(pos2(x, frame.rect.top() - 2.0), vec2(12.0, 12.0));
    painter.rect_filled(grip, tokens.radius.small, color);
    if response.has_focus() {
        painter.rect_stroke(
            strip.expand(2.0),
            tokens.radius.small,
            Stroke::new(tokens.stroke.medium, tokens.colors.accent.color()),
            StrokeKind::Outside,
        );
        // Arrows stay with the handle while it has the focus.
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    ..Default::default()
                },
            );
        });
        let step = ui.input(|i| {
            if i.key_pressed(Key::ArrowLeft) {
                -1
            } else if i.key_pressed(Key::ArrowRight) {
                1
            } else {
                0
            }
        });
        if step != 0 {
            return Some(HandleEvent::Step(step));
        }
    }
    if response.dragged() {
        if let Some(p) = response.interact_pointer_pos() {
            return Some(HandleEvent::Drag(frame.time_at(p.x)));
        }
    }
    None
}

fn nearest_point(panel: &ScenePanel, frame: &Frame, at: Pos2, reach: f32) -> Option<f64> {
    let mut best: Option<(f32, f64)> = None;
    for s in &panel.series {
        if is_line(s.role) {
            continue;
        }
        for p in s.points.iter().filter(|p| !p.blq) {
            let d = frame.px(p.x, p.y).distance(at);
            if d <= reach && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, p.x));
            }
        }
    }
    best.map(|(_, t)| t)
}

fn draw_axes(painter: &egui::Painter, frame: &Frame, panel: &ScenePanel, tokens: &Tokens) {
    let grid = Stroke::new(tokens.stroke.thin, tokens.colors.plot_grid.color());
    let text = tokens.colors.text_muted.color();
    let font = FontId::proportional(tokens.font.small);
    for t in &panel.x.ticks {
        let x = frame.px(t.value, frame.y.min).x;
        painter.line_segment(
            [pos2(x, frame.rect.top()), pos2(x, frame.rect.bottom())],
            grid,
        );
        painter.text(
            pos2(x, frame.rect.bottom() + 4.0),
            Align2::CENTER_TOP,
            &t.label,
            font.clone(),
            text,
        );
    }
    for t in &panel.y.ticks {
        let y = frame.px(frame.x.min, t.value).y;
        painter.line_segment(
            [pos2(frame.rect.left(), y), pos2(frame.rect.right(), y)],
            grid,
        );
        painter.text(
            pos2(frame.rect.left() - 6.0, y),
            Align2::RIGHT_CENTER,
            &t.label,
            font.clone(),
            text,
        );
    }
    let border = Stroke::new(tokens.stroke.thin, tokens.colors.border.color());
    painter.rect_stroke(frame.rect, 0.0, border, StrokeKind::Inside);
    painter.text(
        pos2(frame.rect.center().x, frame.rect.bottom() + 22.0),
        Align2::CENTER_TOP,
        &panel.x.title,
        font.clone(),
        text,
    );
    painter.text(
        pos2(frame.rect.left(), frame.rect.top() - 8.0),
        Align2::LEFT_BOTTOM,
        &panel.y.title,
        font,
        text,
    );
}

fn legend_and_notes(
    ui: &mut egui::Ui,
    tokens: &Tokens,
    scene: &PlotScene,
    panel: &ScenePanel,
    action: &mut PlotAction,
) {
    ui.horizontal_wrapped(|ui| {
        for e in &scene.legend.entries {
            let color = series_color(e.role, tokens);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, color);
            ui.label(egui::RichText::new(&e.name).size(tokens.font.small));
        }
        if let Some(text) = &scene.legend.below_limit {
            ui.label(egui::RichText::new(text).size(tokens.font.small));
        }
    });
    if let Some(hidden) = &panel.hidden {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(tokens.colors.warning.color(), &hidden.note);
            if panel.y.log && ui.button("Switch to linear").clicked() {
                action.linear = true;
            }
        });
    }
    for note in &scene.notes {
        ui.label(egui::RichText::new(note).size(tokens.font.small));
    }
}
