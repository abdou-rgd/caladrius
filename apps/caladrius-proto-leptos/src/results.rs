//! The cells of the results table. Every number goes through `caladrius-format` (the same code the
//! egui page runs): nothing here rounds, pads or switches to scientific notation. What this file
//! decides is which parameters are the headline, in which order, and which of them carry a flag.

use std::collections::{BTreeMap, BTreeSet};

use caladrius_format as fmt;

use crate::types::{Candidate, Flag, NcaOk, PValue, SubjectResult};

/// The headline parameters, in the order of the egui page.
pub const SUMMARY: [&str; 12] = [
    "cmax",
    "tmax",
    "auclast",
    "aucinf.obs",
    "aucpext.obs",
    "lambda.z",
    "half.life",
    "adj.r.squared",
    "span.ratio",
    "cl.obs",
    "vz.obs",
    "mrt.obs",
];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Units {
    pub time: Option<String>,
    pub conc: Option<String>,
    pub derived: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub key: String,
    pub label: String,
    /// What is shown in the value cell.
    pub text: String,
    pub unit: String,
    pub flagged: bool,
    pub calculated: bool,
    /// For a parameter that is not calculated: the sentence of the engine.
    pub sentence: String,
}

/// A number, as the formatter writes it (the only way this crate shows one).
pub fn number(x: f64) -> String {
    fmt::number(x)
}

/// A count, as the formatter writes it.
pub fn count(n: usize) -> String {
    fmt::number(n as f64)
}

/// A fit quality (adjusted R squared...), as the formatter writes it.
pub fn fit_quality(x: f64) -> String {
    fmt::fit_quality(x)
}

/// The text of a cell of the candidate list for the adjusted R squared.
pub fn adj_text(candidate: &Candidate) -> String {
    candidate
        .adj_r_squared
        .map_or_else(|| "-".to_owned(), fit_quality)
}

/// The parameter of the summary a flag is about (the rule of the egui page).
pub fn concerns(flag: &Flag) -> String {
    match flag.code.as_str() {
        "low_adjusted_r_squared" => "adj.r.squared".to_owned(),
        "short_span" => "span.ratio".to_owned(),
        "few_points" => "lambda.z.n.points".to_owned(),
        "high_extrapolation" => flag
            .parameter
            .clone()
            .unwrap_or_else(|| "aucpext.obs".to_owned()),
        "replaced_points_in_lambda_z" => "lambda.z".to_owned(),
        "area_past_tlast" => "auclast".to_owned(),
        _ => String::new(),
    }
}

fn cell(
    name: &str,
    value: &PValue,
    subject: &SubjectResult,
    flagged: &BTreeSet<String>,
    units: &Units,
    extravascular: bool,
) -> Cell {
    let label = fmt::parameter_label(name, extravascular);
    if let Some(v) = value.value {
        let shown = if name.contains("r.squared") {
            fmt::fit_quality(v)
        } else {
            fmt::number(v)
        };
        let unit = fmt::parameter_unit(
            name,
            units.time.as_deref(),
            units.conc.as_deref(),
            &units.derived,
        )
        .unwrap_or_default();
        return Cell {
            key: name.to_owned(),
            label,
            text: shown,
            unit,
            flagged: flagged.contains(name),
            calculated: true,
            sentence: String::new(),
        };
    }
    let reason = fmt::words(value.not_calculated.as_deref().unwrap_or(""));
    let sentence = subject
        .not_calculated_messages
        .get(name)
        .cloned()
        .unwrap_or_else(|| reason.clone());
    Cell {
        key: name.to_owned(),
        label,
        text: "not calculated".to_owned(),
        unit: reason,
        flagged: false,
        calculated: false,
        sentence,
    }
}

fn flagged_set(ok: &NcaOk) -> BTreeSet<String> {
    ok.flags
        .iter()
        .map(concerns)
        .filter(|s| !s.is_empty())
        .collect()
}

/// The summary: the headline parameters that the answer holds, each with its flag.
pub fn headline_cells(
    subject: &SubjectResult,
    ok: &NcaOk,
    units: &Units,
    extravascular: bool,
) -> Vec<Cell> {
    let flagged = flagged_set(ok);
    SUMMARY
        .iter()
        .filter_map(|name| ok.parameters.iter().find(|p| p.name == *name))
        .map(|p| cell(&p.name, &p.value, subject, &flagged, units, extravascular))
        .collect()
}

/// "All parameters": every parameter of the answer, under its engine name.
pub fn all_cells(
    subject: &SubjectResult,
    ok: &NcaOk,
    units: &Units,
    extravascular: bool,
) -> Vec<Cell> {
    let flagged = flagged_set(ok);
    ok.parameters
        .iter()
        .map(|p| {
            let mut c = cell(&p.name, &p.value, subject, &flagged, units, extravascular);
            c.label = p.name.clone();
            c
        })
        .collect()
}

/// The terminal-phase sentence of the results (numbers from the formatter).
pub fn terminal_phase_sentence(
    selected: Option<&Candidate>,
    manual: bool,
    time_unit: Option<&str>,
) -> String {
    let Some(c) = selected else {
        return "No terminal phase could be chosen: λz and what depends on it are not calculated."
            .to_owned();
    };
    let unit = time_unit.map(|u| format!(" {u}")).unwrap_or_default();
    format!(
        "Terminal phase: {} points from t = {} to {}{unit}, {}.",
        count(c.n_points),
        number(c.time_first),
        number(c.time_last),
        if manual {
            "chosen by hand"
        } else {
            "chosen automatically"
        }
    )
}

/// Tab-separated text with a header row; the digits are those on screen.
pub fn results_tsv(cells: &[Cell]) -> String {
    let mut lines = vec!["Parameter\tValue\tUnit".to_owned()];
    lines.extend(
        cells
            .iter()
            .map(|c| format!("{}\t{}\t{}", c.label, c.text, c.unit)),
    );
    lines.join("\n")
}
