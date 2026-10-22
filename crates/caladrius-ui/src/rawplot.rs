//! The raw-data plot of a worksheet (T-109): the concentrations as they were entered, against
//! time, on a linear and a semi-log axis side by side, before any analysis. One line and one
//! colour per subject, the points below the limit of quantification marked apart, a legend that
//! hides or shows a subject, and the subject, time and concentration of a point under the pointer.
//! Read only: the points are the cells of the worksheet table; nothing is computed, fitted or
//! interpolated, and the data are never changed.

use std::collections::BTreeSet;

use egui::{Color32, Id, Pos2, RichText, Sense, Shape, Stroke, Ui, Vec2};
use egui_plot::{Line, MarkerShape, PlotPoints, Points};
use serde_json::Value;

use crate::fmt;
use crate::model::{Table, WorksheetInfo};
use crate::plot::{self, Axes, Dot, Weight};
use crate::plotdata::{self, Pt};
use crate::theme::Tokens;

/// The cell text the engine writes for a value below the limit of quantification
/// (`export.table`, NCA-DAT-05c).
const BLQ_CELL: &str = "BLQ";

/// What the hover label adds to the name of the series of BLQ markers.
const BLQ_SUFFIX: &str = " (BLQ)";

/// The marker shapes of the subjects, in order; the BLQ marker (a hollow triangle pointing down)
/// is not among them.
const SHAPES: [MarkerShape; 5] = [
    MarkerShape::Circle,
    MarkerShape::Square,
    MarkerShape::Diamond,
    MarkerShape::Up,
    MarkerShape::Asterisk,
];

/// One row of the worksheet, as drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPoint {
    /// The row of the worksheet (from 0).
    pub row: usize,
    pub time: f64,
    /// The concentration as entered; 0 for a BLQ cell, which the engine stores as 0.
    pub conc: f64,
    pub blq: bool,
}

/// The points of one subject, in time order (stable: rows with the same time keep their order).
#[derive(Debug, Clone, PartialEq)]
pub struct RawSeries {
    /// The subject as written in the subject column; `None` without a subject column.
    pub subject: Option<String>,
    pub points: Vec<RawPoint>,
}

impl RawSeries {
    /// The name in the legend and the hover label.
    pub fn label(&self) -> String {
        self.subject.clone().unwrap_or_else(|| "data".to_owned())
    }

    /// The points with a measured concentration (not BLQ).
    pub fn measured(&self) -> impl Iterator<Item = &RawPoint> {
        self.points.iter().filter(|p| !p.blq)
    }

    pub fn blq(&self) -> impl Iterator<Item = &RawPoint> {
        self.points.iter().filter(|p| p.blq)
    }
}

/// Everything the raw-data plot draws, read from the worksheet table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawData {
    /// One series per subject, in the order the subjects first appear in the table.
    pub series: Vec<RawSeries>,
    pub time_unit: Option<String>,
    pub conc_unit: Option<String>,
    /// Rows left out because their time or concentration is empty or not a number.
    pub skipped: usize,
    /// The role without a column, when the time or the concentration column is not chosen.
    pub missing_role: Option<&'static str>,
}

impl RawData {
    /// The axis titles, with the units of the worksheet when it has them.
    pub fn axis_labels(&self) -> (String, String) {
        let with = |name: &str, unit: &Option<String>| match unit.as_deref() {
            Some(u) if !u.trim().is_empty() => format!("{name} ({u})"),
            _ => name.to_owned(),
        };
        (
            with("Time", &self.time_unit),
            with("Concentration", &self.conc_unit),
        )
    }

    pub fn n_points(&self) -> usize {
        self.series.iter().map(|s| s.points.len()).sum()
    }

    pub fn n_blq(&self) -> usize {
        self.series.iter().map(|s| s.blq().count()).sum()
    }
}

fn number(cell: Option<&Value>) -> Option<f64> {
    cell.and_then(Value::as_f64).filter(|v| v.is_finite())
}

/// Reads the plot's points from the worksheet table: the columns with the roles time,
/// concentration and subject; a `BLQ` cell of the concentration column is a BLQ point at 0.
pub fn raw_data(info: &WorksheetInfo, table: &Table) -> RawData {
    let mut data = RawData {
        time_unit: info.unit_of("time").map(str::to_owned),
        conc_unit: info.unit_of("concentration").map(str::to_owned),
        ..RawData::default()
    };
    let index = |role: &str| info.column_of(role).and_then(|n| table.column_index(n));
    let (Some(time_col), Some(conc_col)) = (index("time"), index("concentration")) else {
        data.missing_role = Some(if index("time").is_none() {
            "time"
        } else {
            "concentration"
        });
        return data;
    };
    let subject_col = index("subject");
    for (row, cells) in table.rows.iter().enumerate() {
        let time = number(cells.get(time_col));
        let conc_cell = cells.get(conc_col);
        let blq = conc_cell.and_then(Value::as_str) == Some(BLQ_CELL);
        let conc = if blq { Some(0.0) } else { number(conc_cell) };
        let (Some(time), Some(conc)) = (time, conc) else {
            data.skipped += 1;
            continue;
        };
        let subject = subject_col
            .and_then(|c| cells.get(c))
            .map(crate::sheet::cell_text);
        let point = RawPoint {
            row,
            time,
            conc,
            blq,
        };
        match data.series.iter_mut().find(|s| s.subject == subject) {
            Some(s) => s.points.push(point),
            None => data.series.push(RawSeries {
                subject,
                points: vec![point],
            }),
        }
    }
    for s in &mut data.series {
        s.points.sort_by(|a, b| a.time.total_cmp(&b.time));
    }
    data
}

/// The colour and marker of the series at `index` among `count`: a single series takes the
/// colour of observed points; several take the subject colours in order, and past the last
/// colour the next marker shape, so that colour is never the only way to tell two subjects apart.
pub fn style(tokens: &Tokens, index: usize, count: usize) -> (Color32, MarkerShape) {
    let c = &tokens.colors;
    let palette = &c.series_subjects;
    let shape = SHAPES
        .get(index % SHAPES.len())
        .copied()
        .unwrap_or(MarkerShape::Circle);
    if count <= 1 || palette.is_empty() {
        return (c.series_observed.color(), MarkerShape::Circle);
    }
    let color = palette
        .get(index % palette.len())
        .map_or(c.series_observed.color(), |r| r.color());
    (color, shape)
}

/// The axes of one plot, with a margin below and on the left so that markers on the edge (time
/// 0, BLQ points) are drawn whole, and where a BLQ marker sits on the vertical axis: at 0 on the
/// linear plot, in the margin under the lowest decade on the semi-log plot (0 has no logarithm).
pub fn raw_axes(x: (f64, f64), y: (f64, f64), log: bool) -> (Axes, f64) {
    let x = (x.0 - 0.02 * (x.1 - x.0), x.1);
    if log {
        (Axes::new(x, (y.0 - 0.12, y.1 + 0.04), true), y.0 - 0.06)
    } else {
        (Axes::new(x, (y.0 - 0.05 * (y.1 - y.0), y.1), false), 0.0)
    }
}

/// The hover label of a point: the subject, the time and the concentration with their units.
pub fn hover_text(name: &str, x: f64, y: f64, log: bool, units: &(String, String)) -> String {
    let unit = |u: &str| {
        if u.is_empty() {
            String::new()
        } else {
            format!(" {u}")
        }
    };
    let time = format!("time {}{}", fmt::number(x), unit(&units.0));
    if let Some(subject) = name.strip_suffix(BLQ_SUFFIX) {
        return format!("{subject}\n{time}\nbelow the limit of quantification");
    }
    let conc = if log { 10f64.powf(y) } else { y };
    let conc = format!("concentration {}{}", fmt::number(conc), unit(&units.1));
    if name.is_empty() {
        format!("{time}\n{conc}")
    } else {
        format!("{name}\n{time}\n{conc}")
    }
}

/// The subjects hidden from the plot by a click on the legend (view state only).
fn hidden_id(info: &WorksheetInfo) -> Id {
    Id::new(("raw-plot-hidden", info.id))
}

/// The raw-data card of the worksheet screen.
pub fn card(ui: &mut Ui, tokens: &Tokens, info: &WorksheetInfo, table: &Table) {
    let c = &tokens.colors;
    let data = raw_data(info, table);
    tokens.card_frame().show(ui, |ui| {
        ui.label(RichText::new("Raw data").strong());
        ui.label(
            RichText::new(
                "The concentrations as entered, before any analysis: look at the shape of the curves first. Nothing is computed here.",
            )
            .small()
            .color(c.text_muted.color()),
        );
        if let Some(role) = data.missing_role {
            ui.add_space(tokens.spacing.small);
            ui.label(
                RichText::new(format!(
                    "Choose the {role} column in Columns below to see the plot."
                ))
                .color(c.warning.color()),
            );
            return;
        }
        if data.n_points() == 0 {
            ui.add_space(tokens.spacing.small);
            ui.label(
                RichText::new("No row has both a time and a concentration yet.")
                    .color(c.warning.color()),
            );
            return;
        }
        let mut hidden: BTreeSet<String> = ui
            .data(|d| d.get_temp(hidden_id(info)))
            .unwrap_or_default();
        legend(ui, tokens, &data, &mut hidden);
        ui.data_mut(|d| d.insert_temp(hidden_id(info), hidden.clone()));
        let shown: Vec<(usize, &RawSeries)> = data
            .series
            .iter()
            .enumerate()
            .filter(|(_, s)| !hidden.contains(&s.label()))
            .collect();
        if shown.is_empty() {
            ui.label(
                RichText::new("Every subject is hidden: click a name above to show it.")
                    .color(c.text_muted.color()),
            );
            return;
        }
        let count = data.series.len();
        let height = tokens.size.plot_min_height;
        let side_by_side = ui.available_width() >= 2.0 * tokens.size.plot_panel_min_width;
        if side_by_side {
            ui.columns(2, |cols| {
                if let [left, right] = cols {
                    one_plot(left, tokens, info, &data, &shown, count, false, height);
                    one_plot(right, tokens, info, &data, &shown, count, true, height);
                }
            });
        } else {
            one_plot(ui, tokens, info, &data, &shown, count, false, height);
            one_plot(ui, tokens, info, &data, &shown, count, true, height);
        }
        let mut notes: Vec<String> = Vec::new();
        if data.n_blq() > 0 {
            notes.push(format!(
                "{} point(s) below the limit of quantification, drawn as hollow triangles: at 0 on the linear plot, under the lowest decade on the semi-log plot.",
                data.n_blq()
            ));
        }
        let measured: Vec<Pt> = shown
            .iter()
            .flat_map(|(_, s)| s.measured())
            .map(|p| Pt {
                time: p.time,
                conc: p.conc,
                replaced: false,
            })
            .collect();
        let hidden_on_log = plotdata::log_view(&measured).hidden.len();
        if let Some(note) = plotdata::hidden_note(hidden_on_log, measured.len()) {
            notes.push(format!("Semi-log plot: {note}."));
        }
        if data.skipped > 0 {
            notes.push(format!(
                "{} row(s) without a time or a concentration are not drawn.",
                data.skipped
            ));
        }
        for note in notes {
            ui.label(RichText::new(note).small().color(c.text_muted.color()));
        }
    });
}

/// The legend: one entry per subject (a click hides or shows it), and the BLQ marker.
fn legend(ui: &mut Ui, tokens: &Tokens, data: &RawData, hidden: &mut BTreeSet<String>) {
    let c = &tokens.colors;
    let count = data.series.len();
    // One profile without a subject column and nothing below the limit: no legend to read.
    if count == 1 && data.series.iter().all(|s| s.subject.is_none()) && data.n_blq() == 0 {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        if data.series.iter().any(|s| s.subject.is_some()) {
            ui.label(RichText::new("Subject").small().color(c.text_muted.color()));
        }
        for (i, s) in data.series.iter().enumerate() {
            let label = s.label();
            let off = hidden.contains(&label);
            let (color, shape) = style(tokens, i, count);
            let color = if off { c.series_other.color() } else { color };
            let text = if off {
                RichText::new(&label)
                    .color(c.text_muted.color())
                    .strikethrough()
            } else {
                RichText::new(&label).color(c.text.color())
            };
            let response = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = tokens.spacing.small;
                    swatch(ui, tokens, color, shape, true);
                    ui.add(egui::Label::new(text).sense(Sense::click()))
                })
                .inner
                .on_hover_text(if off {
                    "Click to show this subject"
                } else {
                    "Click to hide this subject"
                });
            if response.clicked() && count > 1 {
                if off {
                    hidden.remove(&label);
                } else {
                    hidden.insert(label);
                }
            }
        }
        if data.n_blq() > 0 {
            ui.add_space(tokens.spacing.medium);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = tokens.spacing.small;
                swatch(ui, tokens, c.text_muted.color(), MarkerShape::Down, false);
                ui.label(
                    RichText::new("below the limit of quantification")
                        .small()
                        .color(c.text_muted.color()),
                );
            });
        }
        if !hidden.is_empty() && ui.small_button("Show all").clicked() {
            hidden.clear();
        }
    });
}

/// A legend swatch: a short line and the marker, filled or hollow.
fn swatch(ui: &mut Ui, tokens: &Tokens, color: Color32, shape: MarkerShape, line: bool) {
    let side = tokens.size.row_height;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(side, side), Sense::hover());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let stroke = Stroke::new(tokens.stroke.medium, color);
    if line {
        painter.line_segment(
            [
                Pos2::new(rect.left(), center.y),
                Pos2::new(rect.right(), center.y),
            ],
            stroke,
        );
    }
    let r = plot::dot_size(tokens, Dot::Medium);
    // Filled for a subject, hollow for the BLQ marker.
    let poly = |pts: Vec<Pos2>| {
        if line {
            Shape::convex_polygon(pts, color, stroke)
        } else {
            Shape::closed_line(pts, stroke)
        }
    };
    let at = |dx: f32, dy: f32| Pos2::new(center.x + dx, center.y + dy);
    let shape = match shape {
        MarkerShape::Square => poly(vec![at(-r, -r), at(r, -r), at(r, r), at(-r, r)]),
        MarkerShape::Diamond => poly(vec![at(0.0, -r), at(r, 0.0), at(0.0, r), at(-r, 0.0)]),
        MarkerShape::Up => poly(vec![at(0.0, -r), at(r, r), at(-r, r)]),
        MarkerShape::Down => poly(vec![at(-r, -r), at(r, -r), at(0.0, r)]),
        MarkerShape::Asterisk => {
            let mut lines = Vec::new();
            for k in 0..3 {
                let a = std::f32::consts::FRAC_PI_3 * k as f32 + std::f32::consts::FRAC_PI_2;
                let (s, c) = a.sin_cos();
                lines.push(Shape::line_segment(
                    [at(-r * c, -r * s), at(r * c, r * s)],
                    stroke,
                ));
            }
            Shape::Vec(lines)
        }
        _ if line => Shape::circle_filled(center, r, color),
        _ => Shape::circle_stroke(center, r, stroke),
    };
    painter.add(shape);
}

/// One of the two plots: linear (`log` false) or semi-log.
#[allow(clippy::too_many_arguments)]
fn one_plot(
    ui: &mut Ui,
    tokens: &Tokens,
    info: &WorksheetInfo,
    data: &RawData,
    shown: &[(usize, &RawSeries)],
    count: usize,
    log: bool,
    height: f32,
) {
    let c = &tokens.colors;
    ui.label(
        RichText::new(if log { "Semi-log" } else { "Linear" })
            .small()
            .color(c.text_muted.color()),
    );
    let all: Vec<Pt> = shown
        .iter()
        .flat_map(|(_, s)| s.points.iter())
        .map(|p| Pt {
            time: p.time,
            conc: p.conc,
            replaced: false,
        })
        .collect();
    let measured: Vec<Pt> = shown
        .iter()
        .flat_map(|(_, s)| s.measured())
        .map(|p| Pt {
            time: p.time,
            conc: p.conc,
            replaced: false,
        })
        .collect();
    let (x_range, lin) = plotdata::linear_ranges(&all);
    let y_range = if log {
        match plotdata::log_view(&measured).decades {
            Some(d) => d,
            None => {
                ui.add_space(tokens.spacing.large);
                ui.label(
                    RichText::new(
                        "No measured concentration above zero: nothing to show on a log axis.",
                    )
                    .color(c.warning.color()),
                );
                return;
            }
        }
    } else {
        lin
    };
    let (axes, blq_at) = raw_axes(x_range, y_range, log);
    let labels = data.axis_labels();
    let units = (
        data.time_unit.clone().unwrap_or_default(),
        data.conc_unit.clone().unwrap_or_default(),
    );
    let id = format!("raw-{}-{}", if log { "log" } else { "lin" }, info.id);
    let plot = axes
        .plot(&id, height, labels.0, labels.1)
        .label_formatter(move |name, v| hover_text(name, v.x, v.y, log, &units));
    plot.show(ui, |plot_ui| {
        axes.paint_grid(plot_ui, tokens);
        for (i, s) in shown {
            let (color, shape) = style(tokens, *i, count);
            let name = s.label();
            let drawn: Vec<[f64; 2]> = s
                .measured()
                .filter(|p| !log || p.conc > 0.0)
                .map(|p| [p.time, axes.y_of(p.conc)])
                .collect();
            plot_ui.line(
                Line::new(name.clone(), PlotPoints::from(drawn.clone()))
                    .color(color)
                    .width(plot::weight(tokens, Weight::Medium)),
            );
            plot_ui.points(
                Points::new(name.clone(), PlotPoints::from(drawn))
                    .shape(shape)
                    .radius(plot::dot_size(tokens, Dot::Medium))
                    .color(color)
                    .filled(true),
            );
            let blq: Vec<[f64; 2]> = s.blq().map(|p| [p.time, blq_at]).collect();
            if !blq.is_empty() {
                plot_ui.points(
                    Points::new(format!("{name}{BLQ_SUFFIX}"), PlotPoints::from(blq))
                        .shape(MarkerShape::Down)
                        .radius(plot::dot_size(tokens, Dot::Large))
                        .color(color)
                        .filled(false),
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ColumnInfo;
    use crate::theme::ThemeMode;
    use serde_json::json;

    fn col(name: &str, role: &str, unit: Option<&str>) -> ColumnInfo {
        ColumnInfo {
            name: name.to_owned(),
            role: role.to_owned(),
            unit: unit.map(str::to_owned),
            kind: "number".to_owned(),
            missing: 0,
        }
    }

    fn info(columns: Vec<ColumnInfo>) -> WorksheetInfo {
        WorksheetInfo {
            id: 7,
            name: "w".to_owned(),
            columns,
            ..WorksheetInfo::default()
        }
    }

    /// Two invented subjects, the rows of the second out of time order, a BLQ cell, an empty
    /// concentration and a text time.
    fn table() -> Table {
        Table {
            columns: vec!["id".into(), "t".into(), "c".into(), "dose".into()],
            rows: vec![
                vec![json!("A"), json!(0), json!("BLQ"), json!(50)],
                vec![json!("A"), json!(1), json!(4.5), json!(50)],
                vec![json!("A"), json!(2), json!(3.0), json!(50)],
                vec![json!("B"), json!(2), json!(2.0), json!(50)],
                vec![json!("B"), json!(0.5), json!(5.0), json!(50)],
                vec![json!("B"), json!(4), json!(null), json!(50)],
                vec![json!("B"), json!("late"), json!(1.0), json!(50)],
                vec![json!("A"), json!(8), json!("BLQ"), json!(50)],
            ],
        }
    }

    fn full_info() -> WorksheetInfo {
        info(vec![
            col("id", "subject", None),
            col("t", "time", Some("h")),
            col("c", "concentration", Some("mg/L")),
            col("dose", "dose", Some("mg")),
        ])
    }

    #[test]
    fn one_series_per_subject_in_order_of_appearance_sorted_by_time() {
        let d = raw_data(&full_info(), &table());
        let names: Vec<String> = d.series.iter().map(RawSeries::label).collect();
        assert_eq!(names, vec!["A", "B"]);
        let a: Vec<(f64, f64, bool)> = d.series[0]
            .points
            .iter()
            .map(|p| (p.time, p.conc, p.blq))
            .collect();
        assert_eq!(
            a,
            vec![
                (0.0, 0.0, true),
                (1.0, 4.5, false),
                (2.0, 3.0, false),
                (8.0, 0.0, true)
            ]
        );
        // The rows of B were entered out of order: drawn in time order, values unchanged.
        let b: Vec<(usize, f64, f64)> = d.series[1]
            .points
            .iter()
            .map(|p| (p.row, p.time, p.conc))
            .collect();
        assert_eq!(b, vec![(4, 0.5, 5.0), (3, 2.0, 2.0)]);
        // The empty concentration and the text time are left out and counted.
        assert_eq!(d.skipped, 2);
        assert_eq!(d.n_blq(), 2);
        assert_eq!(d.n_points(), 6);
        assert_eq!(d.series[0].measured().count(), 2);
    }

    #[test]
    fn the_axes_carry_the_units_of_the_worksheet() {
        let d = raw_data(&full_info(), &table());
        assert_eq!(
            d.axis_labels(),
            ("Time (h)".to_owned(), "Concentration (mg/L)".to_owned())
        );
        let bare = info(vec![
            col("t", "time", None),
            col("c", "concentration", Some(" ")),
        ]);
        assert_eq!(
            raw_data(&bare, &table()).axis_labels(),
            ("Time".to_owned(), "Concentration".to_owned())
        );
    }

    #[test]
    fn without_a_subject_column_every_row_is_one_series() {
        let no_subject = info(vec![
            col("t", "time", None),
            col("c", "concentration", None),
        ]);
        let d = raw_data(&no_subject, &table());
        assert_eq!(d.series.len(), 1);
        assert_eq!(d.series[0].subject, None);
        assert_eq!(d.series[0].label(), "data");
        assert_eq!(d.n_points(), 6);
    }

    #[test]
    fn a_missing_role_is_said_and_nothing_is_drawn() {
        let d = raw_data(&info(vec![col("c", "concentration", None)]), &table());
        assert_eq!(d.missing_role, Some("time"));
        assert!(d.series.is_empty());
        let d = raw_data(&info(vec![col("t", "time", None)]), &table());
        assert_eq!(d.missing_role, Some("concentration"));
    }

    #[test]
    fn subjects_get_distinct_colours_and_shapes_from_the_theme() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let t = Tokens::embedded(mode).unwrap();
            let palette = &t.colors.series_subjects;
            assert_eq!(palette.len(), 8);
            // One series: the colour of observed points, as on the analysis plots.
            assert_eq!(
                style(&t, 0, 1),
                (t.colors.series_observed.color(), MarkerShape::Circle)
            );
            // Up to 40 subjects, no two share both a colour and a shape.
            let styles: Vec<String> = (0..40).map(|i| format!("{:?}", style(&t, i, 40))).collect();
            let unique: BTreeSet<&String> = styles.iter().collect();
            assert_eq!(unique.len(), 40, "{mode:?}");
            // Colours follow the fixed order of the theme.
            assert_eq!(style(&t, 1, 3).0, palette[1].color());
            assert_eq!(style(&t, 9, 12).0, palette[1].color());
            assert_ne!(style(&t, 9, 12).1, style(&t, 1, 12).1);
        }
    }

    #[test]
    fn subject_colours_stand_out_from_the_plot_background() {
        // Non-text contrast for graphical marks: at least 2:1 on the plot (the palette is the
        // colour-blind checked set; its lighter slots rely on the legend and the hover label).
        fn luminance(c: Color32) -> f64 {
            let f = |v: u8| {
                let s = f64::from(v) / 255.0;
                if s <= 0.03928 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
        }
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let t = Tokens::embedded(mode).unwrap();
            let bg = luminance(t.colors.plot_background.color());
            for (i, c) in t.colors.series_subjects.iter().enumerate() {
                let l = luminance(c.color());
                let ratio = (l.max(bg) + 0.05) / (l.min(bg) + 0.05);
                assert!(ratio >= 2.0, "{mode:?} subject colour {i}: {ratio:.2}");
            }
        }
    }

    #[test]
    fn blq_points_sit_at_zero_or_under_the_lowest_decade() {
        let (axes, blq) = raw_axes((0.0, 10.0), (0.0, 5.0), false);
        assert_eq!(blq, 0.0);
        // A margin under zero and before time 0, so that their markers are drawn whole.
        assert!(axes.y.0 < 0.0 && axes.x.0 < 0.0 && axes.y.1 == 5.0);
        let (axes, blq) = raw_axes((0.0, 10.0), (-1.0, 1.0), true);
        assert!(axes.y.0 < blq && blq < -1.0, "{blq} in {:?}", axes.y);
        assert!(axes.y.1 > 1.0);
    }

    #[test]
    fn the_hover_label_names_the_subject_time_and_concentration() {
        let units = ("h".to_owned(), "mg/L".to_owned());
        assert_eq!(
            hover_text("A", 2.0, 3.0, false, &units),
            "A\ntime 2 h\nconcentration 3 mg/L"
        );
        // On the log axis the value is read back from its exponent.
        assert_eq!(
            hover_text("A", 2.0, 1.0, true, &units),
            "A\ntime 2 h\nconcentration 10 mg/L"
        );
        assert_eq!(
            hover_text("B (BLQ)", 8.0, -1.0, true, &units),
            "B\ntime 8 h\nbelow the limit of quantification"
        );
        let none = (String::new(), String::new());
        assert_eq!(
            hover_text("", 1.5, 2.0, false, &none),
            "time 1.5\nconcentration 2"
        );
    }

    #[test]
    fn the_worksheet_shows_the_raw_plot_and_its_legend_hides_a_subject() {
        use crate::{Action, UiApp};
        use egui_kittest::kittest::Queryable;
        let mut app = UiApp::new();
        app.load_csv(
            "two.csv",
            b"subject,time,conc\nP,0,BLQ\nP,1,4\nP,2,2\nQ,0,BLQ\nQ,1,5\nQ,2,3\n",
        );
        app.perform(vec![Action::ImportConfirm]);
        let mut h = crate::tests::harness(app);
        h.run_steps(3);
        h.get_by_label("Raw data");
        h.get_by_label("Linear");
        h.get_by_label("Semi-log");
        h.get_by_label("below the limit of quantification");
        // The legend comes before the grid, whose cells also read P and Q.
        h.get_all_by_label("Q").next().unwrap().click();
        h.run_steps(2);
        let id = Id::new(("raw-plot-hidden", 1_u64));
        let hidden: Option<BTreeSet<String>> = h.ctx.data(|d| d.get_temp(id));
        assert_eq!(hidden, Some(BTreeSet::from(["Q".to_owned()])));
        h.get_by_label("Show all").click();
        h.run_steps(2);
        let hidden: Option<BTreeSet<String>> = h.ctx.data(|d| d.get_temp(id));
        assert_eq!(hidden, Some(BTreeSet::new()));
    }
}
