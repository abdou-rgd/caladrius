//! `crates/caladrius-ui/src/theme.json` (the tokens of the egui interface) turned into CSS variables.
//! The file is embedded at build time, so a change of it changes the next build; nothing is copied by
//! hand. Light is the default, dark follows the system, and `data-theme` on `<html>` forces one (the
//! screenshots).

use serde_json::{Map, Value};

pub const THEME_JSON: &str = include_str!("../../../crates/caladrius-ui/src/theme.json");

fn px(v: &Value) -> String {
    format!("{}px", v.as_f64().unwrap_or(0.0))
}

/// The declarations of one theme, as `--name: value;` lines.
pub fn declarations(theme: &Value) -> String {
    let mut out: Vec<(String, String)> = Vec::new();
    let section = |name: &str| -> Map<String, Value> {
        theme
            .get(name)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    for (name, value) in section("colors") {
        let name = name.replace('_', "-");
        match value {
            Value::Array(list) => {
                for (i, c) in list.iter().enumerate() {
                    out.push((
                        format!("--c-{name}-{i}"),
                        c.as_str().unwrap_or("").to_owned(),
                    ));
                }
            }
            other => out.push((
                format!("--c-{name}"),
                other.as_str().unwrap_or("").to_owned(),
            )),
        }
    }
    for (group, prefix) in [
        ("radius", "radius"),
        ("spacing", "space"),
        ("size", "size"),
        ("stroke", "stroke"),
        ("font", "font"),
    ] {
        for (name, value) in section(group) {
            out.push((format!("--{prefix}-{}", name.replace('_', "-")), px(&value)));
        }
    }
    out.iter()
        .map(|(k, v)| format!("  {k}: {v};"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The style sheet of the variables: light, then dark by preference, then the explicit choices.
pub fn theme_css(tokens: &Value) -> String {
    let light = declarations(tokens.get("light").unwrap_or(&Value::Null));
    let dark = declarations(tokens.get("dark").unwrap_or(&Value::Null));
    let indented: String = dark.lines().map(|l| format!("  {l}\n")).collect();
    [
        format!(":root {{\n  color-scheme: light dark;\n{light}\n}}"),
        format!("@media (prefers-color-scheme: dark) {{\n  :root:not([data-theme=\"light\"]) {{\n    color-scheme: dark;\n{indented}  }}\n}}"),
        format!(":root[data-theme=\"light\"] {{\n  color-scheme: light;\n{light}\n}}"),
        format!(":root[data-theme=\"dark\"] {{\n  color-scheme: dark;\n{dark}\n}}"),
    ]
    .join("\n")
}

/// The CSS of the variables made from the embedded `theme.json`.
pub fn css() -> String {
    match serde_json::from_str::<Value>(THEME_JSON) {
        Ok(tokens) => theme_css(&tokens),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_of_the_light_and_dark_theme_becomes_a_variable() {
        let css = css();
        assert!(css.contains("--c-background: #f6f7f2;"));
        assert!(css.contains("--c-series-subjects-0: #2a78d6;"));
        assert!(css.contains("--radius-medium: 4px;"));
        assert!(css.contains("--font-body: 14px;"));
        assert!(css.contains(":root[data-theme=\"dark\"]"));
        assert!(css.contains("--c-background: #0e1310;"));
    }
}
