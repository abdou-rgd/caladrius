//! The units Caladrius recognises, offered next to a unit cell (T-108): the typed table and the
//! worksheet show a small menu with the engine's list (`units.check`), so the person sees what is
//! checked instead of guessing. The cell stays free text: another unit is kept, only not checked,
//! and the engine's checks say so.

use std::collections::BTreeMap;

use egui::{Response, RichText, Ui};
use serde::Deserialize;

use crate::theme::Tokens;

/// The `accepted` part of the answer of `units.check`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Accepted {
    #[serde(default)]
    pub time: Vec<String>,
    #[serde(default)]
    pub concentration: Vec<String>,
    #[serde(default)]
    pub dose: Vec<String>,
    #[serde(default)]
    pub other_spellings: BTreeMap<String, Vec<String>>,
}

impl Accepted {
    /// The units offered for a column of this role (`time`, `concentration`, `dose`); none for the
    /// other roles.
    pub fn for_role(&self, role: &str) -> &[String] {
        match role {
            "time" => &self.time,
            "concentration" => &self.concentration,
            "dose" => &self.dose,
            _ => &[],
        }
    }

    /// The other spellings read like the units of a role, for the hover text: `hr, hour, hours`.
    fn spellings(&self, role: &str) -> Vec<String> {
        let units = self.for_role(role);
        self.other_spellings
            .iter()
            .filter(|(unit, _)| {
                units
                    .iter()
                    .any(|u| u == *unit || u.split('/').any(|part| part == unit.as_str()))
            })
            .flat_map(|(_, other)| other.iter().cloned())
            .collect()
    }

    /// What the menu button says on hover.
    pub fn hint(&self, role: &str) -> String {
        let spellings = self.spellings(role);
        let also = if spellings.is_empty() {
            String::new()
        } else {
            format!(" Also read: {}.", spellings.join(", "))
        };
        format!(
            "The {role} units Caladrius recognises.{also} Other text is kept as the unit but cannot be checked."
        )
    }
}

/// A unit cell: the text box and, when the role has recognised units, a menu button listing them.
/// `width` is the whole cell's. Returns the text box's response, and the unit picked in the menu
/// this frame.
pub fn field(
    ui: &mut Ui,
    tokens: &Tokens,
    id: egui::Id,
    text: &mut String,
    role: &str,
    accepted: &Accepted,
    width: f32,
) -> (Response, Option<String>) {
    let choices = accepted.for_role(role);
    let height = tokens.size.row_height;
    let mut picked = None;
    let response = ui
        .horizontal(|ui| {
            let button = if choices.is_empty() { 0.0 } else { height };
            let response = ui.add_sized(
                [(width - button).max(height), height],
                egui::TextEdit::singleline(text).id(id),
            );
            if !choices.is_empty() {
                let menu = ui.menu_button(RichText::new("\u{23f7}").small(), |ui| {
                    ui.label(
                        RichText::new(format!("Recognised {role} units"))
                            .small()
                            .strong(),
                    );
                    egui::ScrollArea::vertical()
                        .max_height(tokens.size.palette_list_height)
                        .show(ui, |ui| {
                            for unit in choices {
                                if ui.selectable_label(unit == text.trim(), unit).clicked() {
                                    picked = Some(unit.clone());
                                    ui.close();
                                }
                            }
                        });
                });
                menu.response.on_hover_text(accepted.hint(role));
            }
            response
        })
        .inner;
    if let Some(unit) = &picked {
        text.clone_from(unit);
    }
    (response, picked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_names_the_other_spellings_of_the_role_only() {
        let a: Accepted = serde_json::from_value(serde_json::json!({
            "time": ["s", "h"],
            "concentration": ["\u{b5}g/mL", "mg/L"],
            "dose": ["mg", "mg/kg"],
            "other_spellings": { "h": ["hr"], "\u{b5}g": ["ug"], "mL": ["ml"] }
        }))
        .unwrap();
        assert_eq!(a.for_role("dose"), ["mg", "mg/kg"]);
        assert!(a.for_role("subject").is_empty());
        assert!(a.hint("time").contains("Also read: hr."));
        assert!(a.hint("concentration").contains("Also read: ml, ug."));
        assert!(!a.hint("dose").contains("Also read"));
    }
}
