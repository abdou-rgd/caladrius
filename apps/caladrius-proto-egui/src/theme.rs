//! The theme tokens: colours, radii, spacing and sizes of `crates/caladrius-ui/src/theme.json`,
//! embedded at build time from that very file (no value is copied into this crate). No visual style
//! of its own: the tokens decide, light and dark.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle};
use serde::{Deserialize, Deserializer};

/// The token file of the current interface, embedded.
const TOKEN_FILE: &str = include_str!("../../../crates/caladrius-ui/src/theme.json");

/// A colour written `#rrggbb` or `#rrggbbaa`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba(pub Color32);

fn parse_hex(text: &str) -> Option<Color32> {
    let hex = text.strip_prefix('#')?;
    if !(hex.len() == 6 || hex.len() == 8) || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
    };
    let alpha = if hex.len() == 8 { byte(6)? } else { 255 };
    Some(Color32::from_rgba_unmultiplied(
        byte(0)?,
        byte(2)?,
        byte(4)?,
        alpha,
    ))
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        parse_hex(&text)
            .map(Rgba)
            .ok_or_else(|| serde::de::Error::custom(format!("`{text}` is not a colour")))
    }
}

impl Rgba {
    pub fn color(self) -> Color32 {
        self.0
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Colors {
    pub background: Rgba,
    pub panel: Rgba,
    pub panel_alt: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    pub accent_text: Rgba,
    pub border: Rgba,
    pub hover: Rgba,
    pub selected: Rgba,
    pub ok: Rgba,
    pub warning: Rgba,
    pub error: Rgba,
    pub stale: Rgba,
    pub stale_background: Rgba,
    pub plot_background: Rgba,
    pub plot_grid: Rgba,
    pub series_observed: Rgba,
    pub series_selected: Rgba,
    pub series_start: Rgba,
    pub series_fit: Rgba,
    pub series_replaced: Rgba,
    pub series_other: Rgba,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Radius {
    pub small: f32,
    pub medium: f32,
    pub large: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Spacing {
    pub small: f32,
    pub medium: f32,
    pub large: f32,
    pub panel_margin: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Sizes {
    pub row_height: f32,
    pub list_max_height: f32,
    pub plot_min_height: f32,
    pub marker_small: f32,
    pub marker_medium: f32,
    pub marker_large: f32,
    pub hit_radius: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Strokes {
    pub thin: f32,
    pub medium: f32,
    pub thick: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Fonts {
    pub small: f32,
    pub body: f32,
    pub heading: f32,
    pub monospace: f32,
}

/// The tokens of one theme.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Tokens {
    pub colors: Colors,
    pub radius: Radius,
    pub spacing: Spacing,
    pub size: Sizes,
    pub stroke: Strokes,
    pub font: Fonts,
}

#[derive(Deserialize)]
struct TokenFile {
    light: Tokens,
    dark: Tokens,
}

impl Tokens {
    /// The tokens of the light or the dark theme, from the embedded file. A broken file gives the
    /// default (all zero) tokens and the reason, never a panic.
    pub fn load(dark: bool) -> (Tokens, Option<String>) {
        match serde_json::from_str::<TokenFile>(TOKEN_FILE) {
            Ok(file) => (if dark { file.dark } else { file.light }, None),
            Err(e) => (Tokens::default(), Some(format!("theme.json: {e}"))),
        }
    }

    /// Sets the visuals, spacing and text sizes of egui from the tokens.
    pub fn apply(&self, ctx: &egui::Context, dark: bool) {
        let c = &self.colors;
        let mut style = (*ctx.global_style()).clone();
        let mut v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.override_text_color = Some(c.text.color());
        v.panel_fill = c.background.color();
        v.window_fill = c.panel.color();
        v.extreme_bg_color = c.plot_background.color();
        v.faint_bg_color = c.panel_alt.color();
        v.code_bg_color = c.panel_alt.color();
        v.hyperlink_color = c.accent.color();
        v.warn_fg_color = c.warning.color();
        v.error_fg_color = c.error.color();
        v.selection.bg_fill = c.selected.color();
        v.selection.stroke = Stroke::new(self.stroke.thin, c.accent.color());
        let radius = CornerRadius::same(self.radius.medium.round() as u8);
        v.window_corner_radius = CornerRadius::same(self.radius.large.round() as u8);
        v.menu_corner_radius = radius;
        let border = Stroke::new(self.stroke.thin, c.border.color());
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = radius;
            w.fg_stroke.color = c.text.color();
        }
        v.widgets.noninteractive.bg_stroke = border;
        v.widgets.noninteractive.bg_fill = c.panel.color();
        v.widgets.inactive.bg_fill = c.panel_alt.color();
        v.widgets.inactive.weak_bg_fill = c.panel_alt.color();
        v.widgets.inactive.bg_stroke = border;
        v.widgets.hovered.bg_fill = c.hover.color();
        v.widgets.hovered.weak_bg_fill = c.hover.color();
        v.widgets.hovered.bg_stroke = Stroke::new(self.stroke.thin, c.accent.color());
        v.widgets.active.bg_fill = c.hover.color();
        v.widgets.active.weak_bg_fill = c.hover.color();
        v.widgets.open.bg_fill = c.hover.color();
        style.visuals = v;
        style.spacing.item_spacing = egui::vec2(self.spacing.medium, self.spacing.small + 2.0);
        style.spacing.button_padding = egui::vec2(self.spacing.medium, self.spacing.small);
        for (name, size, family) in [
            (TextStyle::Body, self.font.body, FontFamily::Proportional),
            (TextStyle::Button, self.font.body, FontFamily::Proportional),
            (TextStyle::Small, self.font.small, FontFamily::Proportional),
            (
                TextStyle::Heading,
                self.font.heading,
                FontFamily::Proportional,
            ),
            (
                TextStyle::Monospace,
                self.font.monospace,
                FontFamily::Monospace,
            ),
        ] {
            style.text_styles.insert(name, FontId::new(size, family));
        }
        ctx.set_global_style(style);
    }

    /// The frame of a pane body.
    pub fn panel_frame(&self, fill: Rgba) -> egui::Frame {
        egui::Frame::new()
            .fill(fill.color())
            .inner_margin(Margin::same(self.spacing.panel_margin.round() as i8))
    }

    /// A box with a coloured edge (a warning, an error).
    pub fn banner_frame(&self, fill: Rgba, edge: Rgba) -> egui::Frame {
        egui::Frame::new()
            .fill(fill.color())
            .stroke(Stroke::new(self.stroke.medium, edge.color()))
            .corner_radius(CornerRadius::same(self.radius.medium.round() as u8))
            .inner_margin(Margin::same(self.spacing.medium.round() as i8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_token_file_reads_in_both_modes() {
        for dark in [false, true] {
            let (tokens, error) = Tokens::load(dark);
            assert_eq!(error, None);
            assert!(tokens.font.body > 0.0);
        }
        let (light, _) = Tokens::load(false);
        let (dark, _) = Tokens::load(true);
        assert_ne!(light.colors.background, dark.colors.background);
    }
}
