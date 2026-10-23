//! The plot page (T-110): an XY plot is an object of the project, like an NCA or a fit. The page
//! shows its few options (source, subjects ticked or all, linear, semi-log or both, overlaid or
//! one panel per subject, title), and the panels the engine drew (`plot.xy`), with a legend, the
//! notes and the SVG export (`export.plot`). Every change redraws through the engine; the points
//! come from its answer as they are, and nothing is computed here.

use caladrius_engine::project::{
    FitView, PlotGuide, PlotPanel, PlotRun, PlotSeries, SeriesRole, SeriesStyle,
};
use egui::{Color32, RichText, Ui};
use egui_plot::{HLine, Line, MarkerShape, PlotPoints, Points};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::{Action, Notice, NoticeKind, Request, Selection, UiApp};
use crate::fmt;
use crate::model::Status;
use crate::plot::{self, Axes, Dot, Weight};
use crate::plotdata;
use crate::theme::Tokens;
use crate::widgets::{self, combo};

/// Where the plot's points come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// Concentration against time from a worksheet.
    Worksheet { worksheet: u64 },
    /// A plot of a fit result (`view`: `profile`, `observed_predicted`...).
    Fit { fit: u64, view: String },
}

/// The plot page being worked on: the options as the person set them, and the last drawing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlotPage {
    /// The plot object of the project.
    pub analysis: Option<u64>,
    pub source: Source,
    /// Every subject of the worksheet (the ticks in `subjects` are then kept for later).
    pub all_subjects: bool,
    /// The subjects ticked, in the worksheet's order.
    pub subjects: Vec<String>,
    /// `linear`, `log` or `both`.
    pub scale: String,
    /// `overlay` or `per_subject`.
    pub layout: String,
    /// The title typed; empty takes the label of the plot.
    pub title: String,
    /// The title of the last drawing's options, to redraw only when the typed one differs.
    #[serde(skip)]
    pub drawn_title: String,
    /// The label of the plot object.
    #[serde(skip)]
    pub label: String,
    #[serde(skip)]
    pub status: Status,
    /// The last drawing.
    #[serde(skip)]
    pub result: Option<PlotRun>,
    /// Why the last change could not be drawn.
    #[serde(skip)]
    pub error: Option<String>,
}

/// Every scale, with its label.
const SCALES: [(&str, &str); 3] = [
    ("linear", "Linear"),
    ("log", "Semi-log"),
    ("both", "Linear and semi-log"),
];

const LOG_LOG_SCALES: [(&str, &str); 3] = [
    ("linear", "Linear"),
    ("log", "Log-log"),
    ("both", "Linear and log-log"),
];

const LAYOUTS: [(&str, &str); 2] = [
    ("overlay", "Every subject on the same axes"),
    ("per_subject", "One panel per subject"),
];

impl PlotPage {
    fn with_source(source: Source, scale: &str) -> PlotPage {
        PlotPage {
            analysis: None,
            source,
            all_subjects: true,
            subjects: Vec::new(),
            scale: scale.to_owned(),
            layout: "overlay".to_owned(),
            title: String::new(),
            drawn_title: String::new(),
            label: String::new(),
            status: Status::NoResult,
            result: None,
            error: None,
        }
    }

    /// A plot of the data of a worksheet: every subject, linear and semi-log, overlaid.
    pub fn for_worksheet(worksheet: u64) -> PlotPage {
        PlotPage::with_source(Source::Worksheet { worksheet }, "both")
    }

    /// A plot of a fit: linear and log where a log axis makes sense, linear for residuals.
    pub fn for_fit(fit: u64, view: FitView) -> PlotPage {
        let scale = if view.allows_log() { "both" } else { "linear" };
        PlotPage::with_source(
            Source::Fit {
                fit,
                view: view.id().to_owned(),
            },
            scale,
        )
    }

    /// The fit view of the page, when it plots a fit.
    pub fn fit_view(&self) -> Option<FitView> {
        match &self.source {
            Source::Fit { view, .. } => FitView::ALL.into_iter().find(|v| v.id() == view),
            Source::Worksheet { .. } => None,
        }
    }

    /// The worksheet of the page, when it plots one.
    pub fn worksheet(&self) -> Option<u64> {
        match self.source {
            Source::Worksheet { worksheet } => Some(worksheet),
            Source::Fit { .. } => None,
        }
    }

    /// The parameters of `plot.xy` for the page.
    pub fn params(&self) -> Value {
        let mut p = json!({ "scale": self.scale });
        match &self.source {
            Source::Worksheet { worksheet } => {
                p["worksheet"] = json!(worksheet);
                p["layout"] = json!(self.layout);
                if !self.all_subjects {
                    p["subjects"] = json!(self.subjects);
                }
            }
            Source::Fit { fit, view } => {
                p["fit"] = json!(fit);
                p["view"] = json!(view);
            }
        }
        if let Some(id) = self.analysis {
            p["analysis"] = json!(id);
        }
        if !self.title.trim().is_empty() {
            p["title"] = json!(self.title.trim());
        }
        p
    }

    /// A page for a plot object, from the engine's view of it (`analysis.get`, `plot.xy`).
    pub fn from_view(view: &Value) -> Option<PlotPage> {
        if view.get("kind").and_then(Value::as_str) != Some("plot") {
            return None;
        }
        let spec = view.get("spec")?;
        let source = spec.get("source")?;
        let text = |v: &Value, key: &str, default: &str| {
            v.get(key)
                .and_then(Value::as_str)
                .unwrap_or(default)
                .to_owned()
        };
        let mut page = match source.get("kind").and_then(Value::as_str)? {
            "worksheet" => PlotPage::for_worksheet(source.get("worksheet")?.as_u64()?),
            "fit" => {
                let view_id = text(source, "view", "profile");
                let fit_view = FitView::ALL.into_iter().find(|v| v.id() == view_id)?;
                PlotPage::for_fit(source.get("analysis")?.as_u64()?, fit_view)
            }
            _ => return None,
        };
        page.scale = text(spec, "scale", "both");
        page.layout = text(spec, "layout", "overlay");
        match spec.get("subjects").and_then(Value::as_array) {
            Some(list) => {
                page.all_subjects = false;
                page.subjects = list
                    .iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect();
            }
            None => page.all_subjects = true,
        }
        page.title = text(spec, "title", "");
        page.adopt(view);
        Some(page)
    }

    /// Takes the engine's answer: the object, its status and its drawing.
    pub fn adopt(&mut self, view: &Value) {
        self.analysis = view.get("id").and_then(Value::as_u64).or(self.analysis);
        self.label = view
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        self.status = crate::model::read(view.get("status").unwrap_or(&Value::Null));
        self.result = view
            .get("result")
            .filter(|r| !r.is_null())
            .and_then(|r| serde_json::from_value(r.clone()).ok());
        self.drawn_title = view
            .pointer("/spec/title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        self.error = None;
    }
}

// ---- drawing -------------------------------------------------------------------------------

/// The colour and marker of a series in the theme: a subject keeps the colour and shape of its
/// place in the worksheet (as on the raw-data plot), observed and predicted take theirs.
pub fn series_style(tokens: &Tokens, role: SeriesRole) -> (Color32, MarkerShape) {
    let c = &tokens.colors;
    match role {
        SeriesRole::Subject { slot } => crate::rawplot::style(tokens, slot, usize::MAX),
        SeriesRole::Observed => (c.series_observed.color(), MarkerShape::Circle),
        SeriesRole::Predicted => (c.series_fit.color(), MarkerShape::Circle),
    }
}

/// The range of `values`, padded; for a time or concentration axis that has no negative value,
/// from 0. A residual axis (`symmetric`) is centred on 0.
pub fn linear_range(values: &[f64], symmetric: bool) -> (f64, f64) {
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !(lo.is_finite() && hi.is_finite()) {
        return (0.0, 1.0);
    }
    if symmetric {
        let reach = lo.abs().max(hi.abs());
        let reach = if reach > 0.0 { reach * 1.15 } else { 1.0 };
        return (-reach, reach);
    }
    let lo = lo.min(0.0);
    let span = if hi > lo { hi - lo } else { 1.0 };
    (lo - 0.03 * span, hi + 0.05 * span)
}

/// The decades that hold the positive `values`, with room under the lowest one for BLQ markers;
/// and where those markers sit. `None` when no value is positive.
pub fn log_range(values: &[f64]) -> Option<((f64, f64), f64)> {
    let positive: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect();
    let lo = positive.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = positive.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    let (d0, d1) = plotdata::decade_range(lo, hi);
    Some(((d0 - 0.12, d1 + 0.04), d0 - 0.06))
}

/// The axes of a panel, and where its BLQ markers sit; `None` when a log axis has nothing to show.
pub fn panel_axes(p: &PlotPanel) -> Option<(Axes, f64)> {
    let xs: Vec<f64> = p
        .series
        .iter()
        .flat_map(|s| &s.points)
        .map(|q| q.x)
        .collect();
    let ys: Vec<f64> = p
        .series
        .iter()
        .flat_map(|s| &s.points)
        .filter(|q| !q.blq)
        .map(|q| q.y)
        .collect();
    let identity = p.guide == Some(PlotGuide::Identity);
    // Observed against predicted: one range on both axes, so that y = x is the diagonal.
    let shared: Vec<f64> = xs.iter().chain(&ys).copied().collect();
    let (xv, yv) = if identity {
        (&shared, &shared)
    } else {
        (&xs, &ys)
    };
    let symmetric = p.guide == Some(PlotGuide::Zero);
    if p.x.log {
        let (x, _) = log_range(xv)?;
        let (y, blq) = log_range(yv)?;
        return Some((Axes::log_log(x, y), blq));
    }
    let x = linear_range(xv, false);
    if p.y.log {
        let (y, blq) = log_range(yv)?;
        Some((Axes::new(x, y, true), blq))
    } else {
        let y = if identity {
            x
        } else {
            linear_range(yv, symmetric)
        };
        Some((Axes::new(x, y, false), 0.0))
    }
}

/// The hover label of a point: the series, then both values with the axis names.
fn hover(name: &str, x: f64, y: f64, labels: &(String, String, bool, bool)) -> String {
    let back = |v: f64, log: bool| if log { 10f64.powf(v) } else { v };
    let (xl, yl, x_log, y_log) = labels;
    let head = if name.is_empty() {
        String::new()
    } else {
        format!("{name}\n")
    };
    format!(
        "{head}{xl} {}\n{yl} {}",
        fmt::number(back(x, *x_log)),
        fmt::number(back(y, *y_log))
    )
}

/// One panel: its title, its axes and its series.
fn panel(ui: &mut Ui, tokens: &Tokens, p: &PlotPanel, id: &str, height: f32) {
    let c = &tokens.colors;
    ui.label(RichText::new(&p.title).small().color(c.text_muted.color()));
    let Some((axes, blq_at)) = panel_axes(p) else {
        ui.add_space(tokens.spacing.large);
        ui.label(
            RichText::new("No value above zero: nothing to show on a logarithmic axis.")
                .color(c.warning.color()),
        );
        return;
    };
    let labels = (p.x.title(), p.y.title(), p.x.log, p.y.log);
    let plot = axes
        .plot(id, height, p.x.title(), p.y.title())
        .label_formatter(move |name, v| hover(name, v.x, v.y, &labels));
    plot.show(ui, |plot_ui| {
        axes.paint_grid(plot_ui, tokens);
        match p.guide {
            Some(PlotGuide::Zero) => plot_ui.hline(
                HLine::new("", 0.0)
                    .color(c.series_other.color())
                    .width(tokens.stroke.medium),
            ),
            Some(PlotGuide::Identity) => plot_ui.line(
                Line::new(
                    "",
                    PlotPoints::from(vec![[axes.x.0, axes.x.0], [axes.x.1, axes.x.1]]),
                )
                .color(c.series_other.color())
                .width(tokens.stroke.medium)
                .style(egui_plot::LineStyle::dashed_loose()),
            ),
            None => {}
        }
        for s in &p.series {
            draw_series(plot_ui, tokens, s, &axes, blq_at);
        }
    });
}

fn draw_series(
    plot_ui: &mut egui_plot::PlotUi<'_>,
    tokens: &Tokens,
    s: &PlotSeries,
    axes: &Axes,
    blq_at: f64,
) {
    let (color, shape) = series_style(tokens, s.role);
    let keep = |v: f64, log: bool| v.is_finite() && (!log || v > 0.0);
    let drawn: Vec<[f64; 2]> = s
        .points
        .iter()
        .filter(|q| !q.blq && keep(q.x, axes.x_log) && keep(q.y, axes.log))
        .map(|q| [axes.x_of(q.x), axes.y_of(q.y)])
        .collect();
    if s.style != SeriesStyle::Points {
        let weight = if s.style == SeriesStyle::Line {
            Weight::Thick
        } else {
            Weight::Medium
        };
        plot_ui.line(
            Line::new(s.name.clone(), PlotPoints::from(drawn.clone()))
                .color(color)
                .width(plot::weight(tokens, weight)),
        );
    }
    if s.style != SeriesStyle::Line {
        plot_ui.points(
            Points::new(s.name.clone(), PlotPoints::from(drawn))
                .shape(shape)
                .radius(plot::dot_size(tokens, Dot::Medium))
                .color(color)
                .filled(true),
        );
    }
    let blq: Vec<[f64; 2]> = s
        .points
        .iter()
        .filter(|q| q.blq && q.x.is_finite())
        .map(|q| [axes.x_of(q.x), blq_at])
        .collect();
    if !blq.is_empty() {
        plot_ui.points(
            Points::new(format!("{} (BLQ)", s.name), PlotPoints::from(blq))
                .shape(MarkerShape::Down)
                .radius(plot::dot_size(tokens, Dot::Large))
                .color(color)
                .filled(false),
        );
    }
}

/// The legend of the plot: one entry per series name, and the BLQ marker when there is one.
fn legend(ui: &mut Ui, tokens: &Tokens, run: &PlotRun) {
    let c = &tokens.colors;
    let mut seen: Vec<&PlotSeries> = Vec::new();
    for s in run.panels.iter().flat_map(|p| &p.series) {
        if !seen.iter().any(|e| e.name == s.name) {
            seen.push(s);
        }
    }
    let blq = run
        .panels
        .iter()
        .flat_map(|p| &p.series)
        .any(|s| s.points.iter().any(|q| q.blq));
    if seen.len() < 2 && !blq {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for s in &seen {
            let (color, shape) = series_style(tokens, s.role);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = tokens.spacing.small;
                crate::rawplot::swatch(ui, tokens, color, shape, true);
                ui.label(RichText::new(&s.name).color(c.text.color()));
            });
        }
        if blq {
            ui.add_space(tokens.spacing.medium);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = tokens.spacing.small;
                crate::rawplot::swatch(ui, tokens, c.text_muted.color(), MarkerShape::Down, false);
                ui.label(
                    RichText::new("below the limit of quantification")
                        .small()
                        .color(c.text_muted.color()),
                );
            });
        }
    });
}

/// The panels per row: a linear and a semi-log panel side by side, else up to three, fewer when
/// the page is narrow.
pub fn columns(run: &PlotRun, width: f32, min_width: f32) -> usize {
    let n = run.panels.len().max(1);
    let mixed = run.panels.iter().any(|p| p.y.log) && run.panels.iter().any(|p| !p.y.log);
    let wanted = if mixed { 2.min(n) } else { 3.min(n) };
    let fits = if min_width > 0.0 {
        (width / min_width).floor().max(1.0) as usize
    } else {
        wanted
    };
    wanted.min(fits).max(1)
}

/// The drawing of the plot: title, legend, panels in a grid, notes.
pub fn drawing(ui: &mut Ui, tokens: &Tokens, run: &PlotRun, id: u64) {
    let c = &tokens.colors;
    ui.label(RichText::new(&run.title).size(tokens.font.heading).strong());
    legend(ui, tokens, run);
    let cols = columns(run, ui.available_width(), tokens.size.plot_panel_min_width);
    let height = tokens.size.plot_min_height;
    for (row, chunk) in run.panels.chunks(cols).enumerate() {
        ui.columns(cols, |uis| {
            for (k, (p, col)) in chunk.iter().zip(uis.iter_mut()).enumerate() {
                panel(col, tokens, p, &format!("plot-{id}-{row}-{k}"), height);
            }
        });
    }
    for note in &run.notes {
        ui.label(RichText::new(note).small().color(c.text_muted.color()));
    }
}

/// The plot page: options on top, the drawing below.
pub fn central(
    ui: &mut Ui,
    tokens: &Tokens,
    page: &mut PlotPage,
    worksheets: &[(u64, String, Vec<String>)],
    analyses: &[(u64, String)],
    actions: &mut Vec<Action>,
) {
    let c = &tokens.colors;
    let mut changed = false;
    ui.label(
        RichText::new(if page.label.is_empty() {
            "Plot"
        } else {
            &page.label
        })
        .size(tokens.font.display)
        .strong(),
    );
    ui.label(
        RichText::new("A plot is an object of the project: it is saved with it and marked out of date when its data change. The points are drawn as they are; nothing is computed.")
            .small()
            .color(c.text_muted.color()),
    );
    widgets::section(ui, tokens, "Options", |ui| {
        egui::Grid::new("plot-options")
            .num_columns(2)
            .spacing([tokens.spacing.large, tokens.spacing.small])
            .show(ui, |ui| {
                match page.source.clone() {
                    Source::Worksheet { worksheet } => {
                        ui.label("Data");
                        let choices: Vec<(String, String)> = worksheets
                            .iter()
                            .map(|(id, name, _)| (id.to_string(), name.clone()))
                            .collect();
                        let refs: Vec<(&str, &str)> = choices
                            .iter()
                            .map(|(a, b)| (a.as_str(), b.as_str()))
                            .collect();
                        if let Some(id) = combo(ui, "plot-worksheet", &worksheet.to_string(), &refs)
                            .and_then(|v| v.parse::<u64>().ok())
                        {
                            page.source = Source::Worksheet { worksheet: id };
                            page.all_subjects = true;
                            page.subjects.clear();
                            changed = true;
                        }
                        ui.end_row();
                        let subjects = worksheets
                            .iter()
                            .find(|(id, _, _)| *id == worksheet)
                            .map(|(_, _, s)| s.clone())
                            .unwrap_or_default();
                        if subjects.len() > 1 {
                            ui.label("Subjects");
                            ui.vertical(|ui| {
                                if ui
                                    .checkbox(&mut page.all_subjects, "All subjects")
                                    .changed()
                                {
                                    if !page.all_subjects && page.subjects.is_empty() {
                                        page.subjects = subjects.clone();
                                    }
                                    changed = true;
                                }
                                if !page.all_subjects {
                                    ui.horizontal_wrapped(|ui| {
                                        for s in &subjects {
                                            let mut on = page.subjects.contains(s);
                                            if ui.checkbox(&mut on, s).changed() {
                                                page.subjects = subjects
                                                    .iter()
                                                    .filter(|x| {
                                                        if *x == s {
                                                            on
                                                        } else {
                                                            page.subjects.contains(x)
                                                        }
                                                    })
                                                    .cloned()
                                                    .collect();
                                                changed = true;
                                            }
                                        }
                                    });
                                }
                            });
                            ui.end_row();
                            ui.label("Layout");
                            if let Some(v) = combo(ui, "plot-layout", &page.layout, &LAYOUTS) {
                                page.layout = v;
                                changed = true;
                            }
                            ui.end_row();
                        }
                    }
                    Source::Fit { fit, view } => {
                        ui.label("Fit");
                        let name = analyses
                            .iter()
                            .find(|(id, _)| *id == fit)
                            .map_or_else(|| format!("analysis {fit}"), |(_, l)| l.clone());
                        ui.label(RichText::new(name).color(c.text_muted.color()));
                        ui.end_row();
                        ui.label("Plot");
                        let views: Vec<(&str, &str)> =
                            FitView::ALL.iter().map(|v| (v.id(), v.title())).collect();
                        if let Some(v) = combo(ui, "plot-view", &view, &views) {
                            let allows_log = FitView::ALL
                                .iter()
                                .find(|x| x.id() == v)
                                .is_some_and(|x| x.allows_log());
                            if !allows_log {
                                page.scale = "linear".to_owned();
                            }
                            page.source = Source::Fit { fit, view: v };
                            changed = true;
                        }
                        ui.end_row();
                    }
                }
                ui.label("Scale");
                let log_ok = page.fit_view().is_none_or(FitView::allows_log);
                // Observed against predicted puts both axes on the log scale.
                let scales = if page.fit_view() == Some(FitView::ObservedPredicted) {
                    LOG_LOG_SCALES
                } else {
                    SCALES
                };
                ui.horizontal(|ui| {
                    for (value, label) in scales {
                        let enabled = log_ok || value == "linear";
                        let response = ui
                            .add_enabled(
                                enabled,
                                egui::RadioButton::new(page.scale == value, label),
                            )
                            .on_disabled_hover_text(
                                "Residuals can be negative: they have no logarithmic axis",
                            );
                        if response.clicked() && page.scale != value {
                            page.scale = value.to_owned();
                            changed = true;
                        }
                    }
                });
                ui.end_row();
                ui.label("Title");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut page.title)
                        .hint_text("the name of the plot")
                        .desired_width(tokens.size.plot_panel_min_width),
                );
                if response.lost_focus() && page.title.trim() != page.drawn_title.trim() {
                    changed = true;
                }
                ui.end_row();
            });
        ui.add_space(tokens.spacing.small);
        ui.horizontal(|ui| {
            let can_export = page.analysis.is_some() && page.result.is_some();
            if ui
                .add_enabled(can_export, egui::Button::new("Export SVG…"))
                .on_hover_text("Save the drawing as an SVG file (light colours, for documents)")
                .clicked()
            {
                if let Some(id) = page.analysis {
                    actions.push(Action::ExportPlot(id));
                }
            }
        });
    });
    if let Some(e) = &page.error {
        widgets::error_box(ui, tokens, e);
    }
    widgets::stale_banner(ui, tokens, &page.status, page.analysis, actions);
    ui.add_space(tokens.spacing.medium);
    match (&page.result, page.analysis) {
        (Some(run), Some(id)) => {
            tokens
                .card_frame()
                .show(ui, |ui| drawing(ui, tokens, run, id));
        }
        _ => {
            ui.label(
                RichText::new("The plot appears here once it has been drawn.")
                    .color(c.text_muted.color()),
            );
        }
    }
    if changed {
        actions.push(Action::PlotChanged);
    }
}

// ---- the application side ------------------------------------------------------------------

impl UiApp {
    /// The worksheets as the plot page lists them: id, name, subjects.
    pub(crate) fn plot_worksheets(&self) -> Vec<(u64, String, Vec<String>)> {
        self.overview
            .worksheets
            .iter()
            .map(|w| (w.id, w.name.clone(), w.subjects.clone()))
            .collect()
    }

    /// Draws `page` through `plot.xy` and shows it: a new plot object, or the page's own updated.
    fn draw_plot(&mut self, mut page: PlotPage) {
        match self.call("plot.xy", page.params()) {
            Ok(view) => {
                page.adopt(&view);
                self.notice = None;
            }
            Err(message) => page.error = Some(message),
        }
        let id = page.analysis;
        self.state.nca = None;
        self.state.fit = None;
        self.state.sim = None;
        self.state.plot = Some(page);
        self.refresh_overview();
        if let Some(id) = id {
            self.state.selection = Selection::Analysis(id);
        }
    }

    /// A new plot of the worksheet in view (or the first one).
    pub(crate) fn new_plot(&mut self) {
        let worksheet = self
            .selected_worksheet()
            .or_else(|| self.overview.worksheets.first().map(|w| w.id));
        let Some(id) = worksheet else {
            self.notice = Some(Notice {
                kind: NoticeKind::Info,
                text: "Open a CSV file first: a plot needs a worksheet or a fit.".to_owned(),
            });
            return;
        };
        self.draw_plot(PlotPage::for_worksheet(id));
    }

    /// A new plot of a fit already run.
    pub(crate) fn new_fit_plot(&mut self, fit: u64, view: &str) {
        let Some(view) = FitView::ALL.into_iter().find(|v| v.id() == view) else {
            return;
        };
        self.draw_plot(PlotPage::for_fit(fit, view));
    }

    /// The options of the page changed: draw it again.
    pub(crate) fn plot_changed(&mut self) {
        if let Some(page) = self.state.plot.take() {
            self.draw_plot(page);
        }
    }

    /// Opens the page of a plot object.
    pub(crate) fn open_plot(&mut self, view: &Value) {
        match PlotPage::from_view(view) {
            Some(page) => {
                self.state.nca = None;
                self.state.fit = None;
                self.state.sim = None;
                self.state.plot = Some(page);
            }
            None => {
                self.state.plot = None;
                self.notice = Some(Notice {
                    kind: NoticeKind::Info,
                    text: "This plot cannot be shown here; use caladrius-cli.".to_owned(),
                });
            }
        }
    }

    /// Asks the program around to save the SVG of a plot (`export.plot`).
    pub(crate) fn export_plot(&mut self, id: u64) {
        let Ok(answer) = self.call("export.plot", json!({ "analysis": id })) else {
            return;
        };
        let name = answer
            .get("file_name")
            .and_then(Value::as_str)
            .unwrap_or("plot.svg")
            .to_owned();
        let Some(svg) = answer.get("svg").and_then(Value::as_str) else {
            return;
        };
        // The file says which engine drew it and carries the seal of the command (a comment).
        let svg = match self.stamp_of_last_command() {
            Some(line) => caladrius_engine::seal::svg_with_stamp(svg, &line),
            None => svg.to_owned(),
        };
        self.requests.push(Request::SaveFile {
            suggested: name,
            bytes: svg.into_bytes(),
        });
    }
}
