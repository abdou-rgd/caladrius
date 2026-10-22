//! The typed table with the subjects in columns (T-108): times down the first column, one column
//! per subject, a dose line on top. Typing several subjects this way does not repeat the times;
//! "Create worksheet" turns the grid into one row per sample ([`crate::entrywide::WideGrid`]) and
//! sends it through the same commands as the table with one row per sample.

use egui::{Id, RichText, Ui};

use crate::entry::DataEntry;
use crate::theme::Tokens;
use crate::unitpick::{self, Accepted};

/// The grid column of the times; subject `j` is column `j + 1`.
const TIME_COLUMN: usize = 0;

/// The three units on one line, above the grid.
fn units(ui: &mut Ui, tokens: &Tokens, entry: &mut DataEntry, accepted: &Accepted) -> bool {
    let mut focus = false;
    ui.horizontal(|ui| {
        for (label, role) in [
            ("Time unit", "time"),
            ("Concentration unit", "concentration"),
            ("Dose unit", "dose"),
        ] {
            ui.label(label);
            let unit = match role {
                "time" => &mut entry.time_unit,
                "concentration" => &mut entry.concentration_unit,
                _ => &mut entry.dose_unit,
            };
            let (response, _) = unitpick::field(
                ui,
                tokens,
                Id::new(("entry-unit", role)),
                unit,
                role,
                accepted,
                tokens.size.cell_width,
            );
            focus |= response.has_focus();
            ui.add_space(tokens.spacing.medium);
        }
    });
    focus
}

/// The grid; returns whether a unit cell has the focus.
pub fn table(ui: &mut Ui, tokens: &Tokens, entry: &mut DataEntry, accepted: &Accepted) -> bool {
    let c = &tokens.colors;
    let unit_focus = units(ui, tokens, entry, accepted);
    ui.add_space(tokens.spacing.small);
    let mut remove = None;
    let mut focus = None;
    let grid = &mut entry.grid;
    let cell = [tokens.size.cell_width, tokens.size.row_height];
    egui::ScrollArea::horizontal()
        .id_salt("entry-grid-scroll")
        .show(ui, |ui| {
            egui::Grid::new("entry-grid")
                .striped(true)
                .spacing([tokens.spacing.small, tokens.spacing.small])
                .show(ui, |ui| {
                    ui.label(RichText::new("subject").small().color(c.text_muted.color()));
                    ui.add_sized(cell, egui::Label::new(RichText::new("Time").strong()));
                    for (j, name) in grid.subjects.iter_mut().enumerate() {
                        ui.add_sized(
                            cell,
                            egui::TextEdit::singleline(name)
                                .font(egui::TextStyle::Button)
                                .id(Id::new(("entry-grid-subject", j))),
                        )
                        .on_hover_text("The subject's name; click to change it.");
                    }
                    ui.end_row();
                    ui.label(RichText::new("dose").small().color(c.text_muted.color()));
                    ui.label("");
                    for (j, dose) in grid.doses.iter_mut().enumerate() {
                        ui.add_sized(
                            cell,
                            egui::TextEdit::singleline(dose)
                                .hint_text("dose")
                                .id(Id::new(("entry-grid-dose", j))),
                        );
                    }
                    ui.end_row();
                    for (i, row) in grid.rows.iter_mut().enumerate() {
                        ui.label(
                            RichText::new((i + 1).to_string())
                                .small()
                                .color(c.text_muted.color()),
                        );
                        let id = Id::new(("entry-grid-cell", i, TIME_COLUMN));
                        let response =
                            ui.add_sized(cell, egui::TextEdit::singleline(&mut row.time).id(id));
                        if response.has_focus() {
                            focus = Some((i, TIME_COLUMN));
                        }
                        for (j, value) in row.values.iter_mut().enumerate() {
                            let id = Id::new(("entry-grid-cell", i, j + 1));
                            let response =
                                ui.add_sized(cell, egui::TextEdit::singleline(value).id(id));
                            if response.has_focus() {
                                focus = Some((i, j + 1));
                            }
                        }
                        if ui
                            .small_button("Remove")
                            .on_hover_text("Remove this time for every subject")
                            .clicked()
                        {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
        });
    entry.grid_focus = focus;
    if let Some(i) = remove {
        entry.remove_row(i);
    }
    unit_focus
}
