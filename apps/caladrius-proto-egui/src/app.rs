//! The window: a top bar, three tiled panes (data table, plot with the λz candidates, results) and
//! the seal line. The same `App::ui` runs in the real window and in the tests.

use std::collections::HashMap;

use egui::{Button, Context, Id, Key, Modifiers, RichText, ScrollArea, TextEdit, WidgetInfo, vec2};
use egui_tiles::{Behavior, SimplificationOptions, TileId, Tiles, Tree, UiResponse};

use crate::measure::Measure;
use crate::model::Status;
use crate::plot;
use crate::state::{Cell, Handle, Role, Screen};
use crate::theme::{Rgba, Tokens};

/// The three panes of the tile tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Table,
    Plot,
    Results,
}

/// The data of edge_blq, embedded so the window opens on it from anywhere.
pub const EDGE_BLQ: &str = include_str!("../../../oracle/data/edge_blq.csv");

pub struct App {
    pub screen: Screen,
    pub dark: bool,
    tokens: Tokens,
    theme_error: Option<String>,
    applied: Option<bool>,
    tree: Tree<Pane>,
    /// Text being typed in a cell, by (worksheet row, column).
    buffers: HashMap<(usize, &'static str), String>,
    pub measure: Option<Measure>,
}

fn build_tree() -> Tree<Pane> {
    let mut tiles = Tiles::default();
    let table = tiles.insert_pane(Pane::Table);
    let plot = tiles.insert_pane(Pane::Plot);
    let results = tiles.insert_pane(Pane::Results);
    let right = tiles.insert_vertical_tile(vec![plot, results]);
    let root = tiles.insert_horizontal_tile(vec![table, right]);
    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(l))) = tiles.get_mut(root)
    {
        l.shares.set_share(table, 0.9);
        l.shares.set_share(right, 2.1);
    }
    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(l))) =
        tiles.get_mut(right)
    {
        l.shares.set_share(plot, 1.3);
        l.shares.set_share(results, 1.0);
    }
    Tree::new(Id::unique("caladrius-egui-tree"), root, tiles)
}

impl App {
    /// The screen on edge_blq, subject 3, extravascular.
    pub fn new(dark: bool) -> App {
        let mut screen = Screen::new();
        if let Err(message) = screen.load_csv("edge_blq", EDGE_BLQ, "3") {
            screen.error = Some(message);
        }
        App::with_screen(screen, dark)
    }

    pub fn with_screen(screen: Screen, dark: bool) -> App {
        let (tokens, theme_error) = Tokens::load(dark);
        App {
            screen,
            dark,
            tokens,
            theme_error,
            applied: None,
            tree: build_tree(),
            buffers: HashMap::new(),
            measure: None,
        }
    }

    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    fn apply_theme(&mut self, ctx: &Context) {
        if self.applied != Some(self.dark) {
            let (tokens, error) = Tokens::load(self.dark);
            tokens.apply(ctx, self.dark);
            self.tokens = tokens;
            self.theme_error = error;
            self.applied = Some(self.dark);
        }
    }

    /// One frame of the whole screen.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let started = std::time::Instant::now();
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);
        if let Some(mut m) = self.measure.take() {
            m.before_frame(&mut self.screen, &ctx);
            self.measure = Some(m);
        }
        self.keys(&ctx);
        self.top_bar(ui);
        self.bottom_bar(ui);
        let tokens = self.tokens.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(tokens.colors.background.color()))
            .show(ui, |ui| {
                let mut panes = Panes {
                    screen: &mut self.screen,
                    tokens: &tokens,
                    buffers: &mut self.buffers,
                };
                self.tree.ui(&mut panes, ui);
            });
        if let Some(mut m) = self.measure.take() {
            m.after_frame(&mut self.screen, &ctx, started);
            self.measure = Some(m);
        }
    }

    // ---- keyboard --------------------------------------------------------------------------

    fn keys(&mut self, ctx: &Context) {
        let typing = ctx.text_edit_focused();
        let alt = Modifiers::ALT;
        let alt_shift = Modifiers::ALT | Modifiers::SHIFT;
        let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
        let mut run = false;
        let mut start = 0;
        let mut end = 0;
        let mut subject = 0;
        let (mut log, mut auto, mut copy_results, mut copy_data, mut all) =
            (false, false, false, false, false);
        ctx.input_mut(|i| {
            run = i.consume_key(Modifiers::CTRL, Key::R) || i.consume_key(Modifiers::NONE, Key::F5);
            // Alt+Shift first: a key with Shift also matches the Alt-only binding.
            if i.consume_key(alt_shift, Key::ArrowLeft) {
                end = -1;
            }
            if i.consume_key(alt_shift, Key::ArrowRight) {
                end = 1;
            }
            if i.consume_key(alt, Key::ArrowLeft) {
                start = -1;
            }
            if i.consume_key(alt, Key::ArrowRight) {
                start = 1;
            }
            if i.consume_key(alt, Key::ArrowUp) {
                subject = -1;
            }
            if i.consume_key(alt, Key::ArrowDown) {
                subject = 1;
            }
            copy_results = i.consume_key(ctrl_shift, Key::C);
            copy_data = i.consume_key(ctrl_shift, Key::D);
            if !typing {
                log = i.consume_key(Modifiers::NONE, Key::L);
                auto = i.consume_key(Modifiers::NONE, Key::A);
                all = i.consume_key(Modifiers::NONE, Key::P);
            }
        });
        if run {
            self.screen.run();
        }
        if start != 0 {
            self.screen.step_handle(Handle::Start, start);
        }
        if end != 0 {
            self.screen.step_handle(Handle::End, end);
        }
        if subject != 0 {
            self.screen.step_subject(subject);
        }
        if log {
            let next = !self.screen.log;
            self.screen.set_log(next);
        }
        if auto {
            self.screen.back_to_automatic();
        }
        if all {
            self.screen.all_open = !self.screen.all_open;
        }
        if copy_results {
            copy_results_text(&mut self.screen, &ctx.clone());
        }
        if copy_data {
            copy_data_text(&mut self.screen, &ctx.clone());
        }
    }

    // ---- bars ------------------------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens.clone();
        egui::Panel::top("top-bar")
            .frame(tokens.panel_frame(tokens.colors.panel))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(&self.screen.dataset)
                            .heading()
                            .color(tokens.colors.text.color()),
                    );
                    let label = self
                        .screen
                        .run
                        .as_ref()
                        .map(|r| r.label.clone())
                        .unwrap_or_default();
                    ui.label(RichText::new(label).color(tokens.colors.text_muted.color()));
                    ui.separator();
                    if ui
                        .add_enabled(true, Button::new("Previous"))
                        .on_hover_text("Previous subject (Alt+Up)")
                        .clicked()
                    {
                        self.screen.step_subject(-1);
                    }
                    let mut chosen: Option<String> = None;
                    egui::ComboBox::new("subject-selector", "Subject")
                        .selected_text(self.screen.subject.clone())
                        .show_ui(ui, |ui| {
                            for s in self.screen.subjects.clone() {
                                if ui.selectable_label(s == self.screen.subject, &s).clicked() {
                                    chosen = Some(s);
                                }
                            }
                        });
                    if let Some(s) = chosen {
                        self.screen.select_subject(&s, true);
                    }
                    if ui
                        .button("Next")
                        .on_hover_text("Next subject (Alt+Down)")
                        .clicked()
                    {
                        self.screen.step_subject(1);
                    }
                    ui.separator();
                    let run = ui.add(
                        Button::new(RichText::new("Run").color(tokens.colors.accent_text.color()))
                            .fill(tokens.colors.accent.color()),
                    );
                    if run
                        .on_hover_text("Run the analysis (Ctrl+R or F5)")
                        .clicked()
                    {
                        self.screen.run();
                    }
                    badge(ui, &tokens, self.screen.status);
                });
                if let Some(error) = self.theme_error.clone() {
                    ui.colored_label(tokens.colors.error.color(), error);
                }
            });
    }

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens.clone();
        egui::Panel::bottom("seal-line")
            .frame(tokens.panel_frame(tokens.colors.panel))
            .show(ui, |ui| {
                let text = self
                    .screen
                    .seal
                    .as_ref()
                    .map(|s| s.text())
                    .unwrap_or_else(|| "no nca.run yet".to_owned());
                let r = ui.label(
                    RichText::new(text)
                        .monospace()
                        .color(tokens.colors.text_muted.color()),
                );
                r.widget_info(|| {
                    WidgetInfo::labeled(egui::Role::Label, true, "Seal of the last nca.run")
                });
            });
    }
}

/// The `fresh` or `stale` badge (UX-FR-12): the words, never the colour alone.
pub fn badge(ui: &mut egui::Ui, tokens: &Tokens, status: Status) {
    let (fg, bg) = match status {
        Status::Fresh => (tokens.colors.ok, tokens.colors.panel_alt),
        Status::Stale => (tokens.colors.stale, tokens.colors.stale_background),
        Status::NoResult => (tokens.colors.text_muted, tokens.colors.panel_alt),
    };
    let r = egui::Frame::new()
        .fill(bg.color())
        .stroke(egui::Stroke::new(tokens.stroke.medium, fg.color()))
        .corner_radius(tokens.radius.large.round() as u8)
        .inner_margin(egui::Margin::symmetric(
            tokens.spacing.medium.round() as i8,
            2,
        ))
        .show(ui, |ui| {
            ui.label(RichText::new(status.label()).color(fg.color()).strong())
        });
    r.response.widget_info(|| {
        WidgetInfo::labeled(
            egui::Role::Label,
            true,
            format!("Analysis is {}", status.label()),
        )
    });
}

fn copy_results_text(screen: &mut Screen, ctx: &Context) {
    let mut cells = screen.headline();
    cells.extend(screen.all_parameters());
    let text = Screen::tsv(&cells);
    ctx.copy_text(text.clone());
    screen.copied = Some(text);
}

fn copy_data_text(screen: &mut Screen, ctx: &Context) {
    let mut text = String::from("Time\tConcentration\tRole\n");
    for r in &screen.table {
        let roles: Vec<String> = r.roles.iter().map(Role::label).collect();
        text.push_str(&format!(
            "{}\t{}\t{}\n",
            r.time_text,
            r.conc_text,
            roles.join(", ")
        ));
    }
    ctx.copy_text(text.clone());
    screen.copied = Some(text);
}

fn role_color(role: &Role, tokens: &Tokens) -> Rgba {
    let c = &tokens.colors;
    match role {
        Role::Observed => c.text_muted,
        Role::Blq => c.warning,
        Role::Replaced => c.series_replaced,
        Role::Excluded | Role::LeftOut(_) => c.series_other,
        Role::Used => c.series_selected,
        Role::Start => c.series_start,
        Role::Tmax => c.accent,
    }
}

// ---- the panes ---------------------------------------------------------------------------------

struct Panes<'a> {
    screen: &'a mut Screen,
    tokens: &'a Tokens,
    buffers: &'a mut HashMap<(usize, &'static str), String>,
}

impl Behavior<Pane> for Panes<'_> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile: TileId, pane: &mut Pane) -> UiResponse {
        let tokens = self.tokens.clone();
        egui::Frame::new()
            .fill(tokens.colors.panel.color())
            .stroke(egui::Stroke::new(
                tokens.stroke.thin,
                tokens.colors.border.color(),
            ))
            .corner_radius(tokens.radius.medium.round() as u8)
            .inner_margin(egui::Margin::same(tokens.spacing.medium.round() as i8))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                match pane {
                    Pane::Table => self.table_pane(ui),
                    Pane::Plot => self.plot_pane(ui),
                    Pane::Results => self.results_pane(ui),
                }
            });
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::Table => "Data",
            Pane::Plot => "Plot",
            Pane::Results => "Results",
        }
        .into()
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        self.tokens.spacing.medium
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            all_panes_must_have_tabs: false,
            ..SimplificationOptions::default()
        }
    }

    fn is_tile_draggable(&self, _tiles: &Tiles<Pane>, _tile_id: TileId) -> bool {
        false
    }
}

impl Panes<'_> {
    fn title(&self, ui: &mut egui::Ui, text: &str) {
        ui.label(
            RichText::new(text)
                .heading()
                .color(self.tokens.colors.text.color()),
        );
    }

    // ---- data table ----------------------------------------------------------------------

    fn table_pane(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens.clone();
        ui.horizontal(|ui| {
            self.title(ui, "Data");
            if ui
                .button("Copy data")
                .on_hover_text("Ctrl+Shift+D")
                .clicked()
            {
                let ctx = ui.ctx().clone();
                copy_data_text(self.screen, &ctx);
            }
        });
        let unit = |u: &Option<String>| u.clone().map(|u| format!(" ({u})")).unwrap_or_default();
        ui.label(
            RichText::new(format!(
                "Subject {}: time{}, concentration{}. Edit a cell, then run again.",
                self.screen.subject,
                unit(&self.screen.time_unit),
                unit(&self.screen.conc_unit)
            ))
            .size(tokens.font.small)
            .color(tokens.colors.text_muted.color()),
        );
        let rows = self.screen.table.clone();
        let used = self.screen.used_times();
        let mut edits: Vec<(usize, &'static str, String)> = Vec::new();
        let mut toggle: Option<f64> = None;
        ScrollArea::vertical().id_salt("data-rows").show(ui, |ui| {
            egui::Grid::new("data-grid")
                .num_columns(4)
                .spacing(vec2(tokens.spacing.medium, tokens.spacing.small))
                .show(ui, |ui| {
                    for h in ["Time", "Concentration", "Role", "λz"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for row in &rows {
                        for (column, original) in
                            [("time", &row.time_text), ("conc", &row.conc_text)]
                        {
                            let id = Id::unique(("cell", row.sheet_row, column));
                            let focused = ui.memory(|m| m.has_focus(id));
                            let key = (row.sheet_row, column);
                            if !focused {
                                self.buffers.insert(key, original.clone());
                            }
                            let buf = self.buffers.entry(key).or_insert_with(|| original.clone());
                            let r = ui.add(
                                TextEdit::singleline(buf)
                                    .id(id)
                                    .desired_width(88.0)
                                    .font(egui::TextStyle::Monospace),
                            );
                            r.widget_info(|| {
                                WidgetInfo::labeled(
                                    egui::Role::TextInput,
                                    true,
                                    format!(
                                        "{} at row {}",
                                        if column == "time" {
                                            "Time"
                                        } else {
                                            "Concentration"
                                        },
                                        row.sheet_row + 1
                                    ),
                                )
                            });
                            if r.lost_focus() && buf != original {
                                edits.push((row.sheet_row, column, buf.clone()));
                            }
                        }
                        ui.horizontal(|ui| {
                            for role in &row.roles {
                                chip(ui, &tokens, role);
                            }
                        });
                        let in_range = used.contains(&row.time);
                        let label = if in_range { "Leave out" } else { "Use for λz" };
                        let b = ui.add_enabled(row.conc > 0.0, Button::new(label).small());
                        b.widget_info(|| {
                            WidgetInfo::labeled(
                                egui::Role::Button,
                                row.conc > 0.0,
                                format!("{label} the point at time {}", row.time_text),
                            )
                        });
                        if b.clicked() {
                            toggle = Some(row.time);
                        }
                        ui.end_row();
                    }
                });
        });
        for (row, column, text) in edits {
            self.screen.edit_cell(row, column, &text);
        }
        if let Some(t) = toggle {
            self.screen.toggle_point(t);
        }
    }

    // ---- plot and candidates -------------------------------------------------------------

    fn plot_pane(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens.clone();
        ui.horizontal_wrapped(|ui| {
            self.title(ui, "Plot");
            let log = self.screen.log;
            if ui
                .selectable_label(log, "Semi-log")
                .on_hover_text("Press L to toggle")
                .clicked()
            {
                self.screen.set_log(true);
            }
            if ui
                .selectable_label(!log, "Linear")
                .on_hover_text("Press L to toggle")
                .clicked()
            {
                self.screen.set_log(false);
            }
            ui.separator();
            let manual = self.screen.manual();
            ui.label(if manual {
                "λz range: chosen by hand"
            } else {
                "λz range: automatic"
            });
            if ui
                .add_enabled(manual, Button::new("Back to automatic"))
                .on_hover_text("Press A")
                .clicked()
            {
                self.screen.back_to_automatic();
            }
        });
        if let Some(error) = self.screen.error.clone() {
            tokens
                .banner_frame(tokens.colors.panel_alt, tokens.colors.error)
                .show(ui, |ui| {
                    ui.colored_label(tokens.colors.error.color(), error);
                });
        }
        if let Some(note) = self.screen.notice.clone() {
            ui.colored_label(tokens.colors.warning.color(), note);
        }
        let cand_w = 330.0_f32.min(ui.available_width() * 0.4);
        let total = ui.available_size();
        let gap = tokens.spacing.medium;
        let plot_w = (total.x - cand_w - gap).max(200.0);
        let mut action = plot::PlotAction::default();
        ui.horizontal_top(|ui| {
            let down = egui::Layout::top_down(egui::Align::Min);
            ui.allocate_ui_with_layout(vec2(plot_w, total.y), down, |ui| {
                action = plot::show(ui, &tokens, self.screen);
            });
            ui.allocate_ui_with_layout(vec2(cand_w, total.y), down, |ui| {
                self.candidates(ui);
            });
        });
        if let Some((handle, time)) = action.drag {
            self.screen.drag_handle(handle, time);
        }
        if let Some((handle, n)) = action.step {
            self.screen.step_handle(handle, n);
        }
        if let Some(time) = action.toggle {
            self.screen.toggle_point(time);
        }
        if action.linear {
            self.screen.set_log(false);
        }
    }

    fn candidates(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens.clone();
        ui.label(RichText::new("λz candidates").strong());
        let cands = self.screen.candidates();
        let mut picked: Option<usize> = None;
        ScrollArea::vertical()
            .id_salt("candidates")
            .max_height(ui.available_height())
            .show(ui, |ui| {
                if cands.is_empty() {
                    ui.label("No candidate range for this subject.");
                }
                for (i, c) in cands.iter().enumerate() {
                    let r2 = c
                        .adj_r_squared
                        .map(caladrius_format::fit_quality)
                        .unwrap_or_else(|| "n/a".to_owned());
                    let text = format!(
                        "{} pts  {} to {}  λz {}  adj R² {}{}",
                        c.n_points,
                        caladrius_format::exact(c.time_first),
                        caladrius_format::exact(c.time_last),
                        caladrius_format::number(c.lambda_z),
                        r2,
                        if c.valid { "" } else { "  (not valid)" },
                    );
                    let r = ui.selectable_label(c.selected, text.clone());
                    r.widget_info(|| {
                        WidgetInfo::selected(
                            egui::Role::Button,
                            true,
                            c.selected,
                            format!("Candidate {}: {text}", i + 1),
                        )
                    });
                    if r.clicked() {
                        picked = Some(i);
                    }
                }
            });
        let _ = &tokens;
        if let Some(i) = picked {
            self.screen.pick_candidate(i);
        }
    }

    // ---- results -------------------------------------------------------------------------

    fn results_pane(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens.clone();
        self.screen.shown.clear();
        ui.horizontal(|ui| {
            self.title(ui, "Results");
            if ui
                .button("Copy results")
                .on_hover_text("Ctrl+Shift+C")
                .clicked()
            {
                let ctx = ui.ctx().clone();
                copy_results_text(self.screen, &ctx);
            }
            if self.screen.status == Status::Stale {
                ui.colored_label(
                    tokens.colors.stale.color(),
                    "The data changed since the last run: run again.",
                );
            }
        });
        if let Some(error) = self.screen.error.clone() {
            if self.screen.headline().is_empty() {
                ui.colored_label(tokens.colors.error.color(), error);
                return;
            }
        }
        let messages = self
            .screen
            .run
            .as_ref()
            .map(|r| r.subject.flag_messages.clone())
            .unwrap_or_default();
        for m in &messages {
            tokens
                .banner_frame(tokens.colors.panel_alt, tokens.colors.warning)
                .show(ui, |ui| {
                    ui.colored_label(tokens.colors.warning.color(), caladrius_format::plain(m));
                });
        }
        let headline = self.screen.headline();
        ScrollArea::vertical().id_salt("results").show(ui, |ui| {
            egui::Grid::new("headline-grid")
                .num_columns(6)
                .spacing(vec2(tokens.spacing.large, tokens.spacing.small))
                .show(ui, |ui| {
                    for (i, c) in headline.iter().enumerate() {
                        self.param_cell(ui, &tokens, c, "headline");
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            ui.add_space(tokens.spacing.medium);
            let open = self.screen.all_open;
            let h = egui::CollapsingHeader::new("All parameters")
                .open(Some(open))
                .show(ui, |ui| {
                    egui::Grid::new("all-grid")
                        .num_columns(3)
                        .spacing(vec2(tokens.spacing.large, tokens.spacing.small))
                        .show(ui, |ui| {
                            for c in self.screen.all_parameters() {
                                self.param_cell(ui, &tokens, &c, "all");
                                ui.end_row();
                            }
                        });
                });
            if h.header_response.clicked() {
                self.screen.all_open = !open;
            }
        });
    }

    /// Label, value and unit of one parameter; records the value text for the checks.
    fn param_cell(&mut self, ui: &mut egui::Ui, tokens: &Tokens, c: &Cell, section: &str) {
        let flag = if c.flagged { "  flag" } else { "" };
        ui.label(
            RichText::new(format!("{}{}", c.label, flag)).color(if c.flagged {
                tokens.colors.warning.color()
            } else {
                tokens.colors.text_muted.color()
            }),
        );
        let shown = if section == "all" && c.why.is_some() {
            c.unit.clone()
        } else {
            c.text.clone()
        };
        let value = ui.label(RichText::new(&shown).monospace().strong());
        value.widget_info(|| {
            WidgetInfo::labeled(
                egui::Role::Label,
                true,
                format!("{}: {} {}", c.label, shown, c.unit),
            )
        });
        self.screen
            .shown
            .push((format!("{section}:{}", c.name), shown));
        if section == "headline" || c.why.is_none() {
            ui.label(RichText::new(&c.unit).color(tokens.colors.text_muted.color()));
        }
    }
}

fn chip(ui: &mut egui::Ui, tokens: &Tokens, role: &Role) {
    let color = role_color(role, tokens).color();
    egui::Frame::new()
        .fill(tokens.colors.panel_alt.color())
        .stroke(egui::Stroke::new(tokens.stroke.thin, color))
        .corner_radius(tokens.radius.large.round() as u8)
        .inner_margin(egui::Margin::symmetric(
            tokens.spacing.small.round() as i8,
            0,
        ))
        .show(ui, |ui| {
            ui.label(
                RichText::new(role.label())
                    .size(tokens.font.small)
                    .color(color),
            );
        });
}
