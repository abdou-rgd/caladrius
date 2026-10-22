//! Units as data (golden rule 5, `AGENTS.md` section 7 friction 5). A unit is free text on a
//! column; this module recognises the common ones to check dose, concentration and time together
//! and to name the derived units (AUC, CL, V). It never converts a value.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A check that failed or could not be made; shown before running an analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitWarning {
    /// Stable code: `missing_unit`, `unrecognised_unit`, `mass_mismatch`, `dose_per_body_size`.
    pub code: String,
    /// What to check or fix.
    pub message: String,
}

fn warning(code: &str, message: String) -> UnitWarning {
    UnitWarning {
        code: code.to_owned(),
        message,
    }
}

fn normalise(unit: &str) -> String {
    unit.trim()
        .replace(['\u{b5}', '\u{3bc}'], "u")
        .replace(' ', "")
}

/// A unit a column may carry: the spelling shown, the other spellings read the same, and its size
/// in the base unit of its kind (grams, litres, seconds). These tables are the units Caladrius
/// recognises; [`accepted_units`] lists them for the person and the agents.
type Known = (&'static str, &'static [&'static str], f64);

/// Units of mass, grams in one unit.
const MASSES: [Known; 6] = [
    ("pg", &[], 1e-12),
    ("ng", &[], 1e-9),
    ("\u{b5}g", &["ug", "mcg"], 1e-6),
    ("mg", &[], 1e-3),
    ("g", &[], 1.0),
    ("kg", &[], 1e3),
];

/// Units of volume, litres in one unit.
const VOLUMES: [Known; 4] = [
    ("\u{b5}L", &["uL", "ul"], 1e-6),
    ("mL", &["ml"], 1e-3),
    ("dL", &["dl"], 1e-1),
    ("L", &["l"], 1.0),
];

/// Units of time, seconds in one unit.
const TIMES: [Known; 4] = [
    ("s", &["sec"], 1.0),
    ("min", &[], 60.0),
    ("h", &["hr", "hour", "hours"], 3600.0),
    ("d", &["day", "days"], 86400.0),
];

/// The size of a normalised unit in a table, when the table has it.
fn size_in(table: &[Known], unit: &str) -> Option<f64> {
    table
        .iter()
        .find(|(shown, other, _)| normalise(shown) == unit || other.contains(&unit))
        .map(|(_, _, size)| *size)
}

/// Grams in one unit of mass, for the units written as a prefix and `g`.
fn mass_in_grams(unit: &str) -> Option<f64> {
    size_in(&MASSES, unit)
}

/// Litres in one unit of volume.
fn volume_in_litres(unit: &str) -> Option<f64> {
    size_in(&VOLUMES, unit)
}

/// Seconds in one unit of time.
fn time_in_seconds(unit: &str) -> Option<f64> {
    size_in(&TIMES, unit)
}

/// The units recognised for each kind of column, as shown to the person (`units.check`). Any
/// other text is still accepted as a unit; it is only not checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedUnits {
    /// The units of time.
    pub time: Vec<String>,
    /// Every mass per volume, by mass then volume: `pg/µL` … `kg/L`.
    pub concentration: Vec<String>,
    /// Every mass, then every mass per kg of body weight (`mg/kg`).
    pub dose: Vec<String>,
    /// The other spellings read the same as a unit shown, such as `ug` for `µg` or `hr` for `h`;
    /// spaces are ignored and the Greek mu is read as the micro sign.
    pub other_spellings: BTreeMap<String, Vec<String>>,
}

/// The units recognised, from the same tables the checks use.
pub fn accepted_units() -> AcceptedUnits {
    let shown = |table: &[Known]| -> Vec<String> {
        table.iter().map(|(s, _, _)| (*s).to_owned()).collect()
    };
    let masses = shown(&MASSES);
    let concentration = masses
        .iter()
        .flat_map(|m| VOLUMES.iter().map(move |(v, _, _)| format!("{m}/{v}")))
        .collect();
    let dose = masses
        .iter()
        .cloned()
        .chain(masses.iter().map(|m| format!("{m}/kg")))
        .collect();
    let other_spellings = MASSES
        .iter()
        .chain(&VOLUMES)
        .chain(&TIMES)
        .filter(|(_, other, _)| !other.is_empty())
        .map(|(s, other, _)| {
            (
                (*s).to_owned(),
                other.iter().map(|o| (*o).to_owned()).collect(),
            )
        })
        .collect();
    AcceptedUnits {
        time: shown(&TIMES),
        concentration,
        dose,
        other_spellings,
    }
}

/// The mass of a dose per kilogram of body weight, such as `mg` for `mg/kg`.
fn per_kg_mass(unit: &str) -> Option<&str> {
    unit.strip_suffix("/kg")
        .filter(|m| mass_in_grams(m).is_some())
}

/// (mass unit, volume unit) of a concentration such as `ng/mL`.
fn split_concentration(unit: &str) -> Option<(&str, &str)> {
    let (mass, volume) = unit.split_once('/')?;
    mass_in_grams(mass)?;
    volume_in_litres(volume)?;
    Some((mass, volume))
}

/// The checks on the units of the time, concentration and dose columns of one worksheet.
/// `None` is a column without a unit (or no such column).
pub fn unit_warnings(
    time: Option<&str>,
    concentration: Option<&str>,
    dose: Option<&str>,
) -> Vec<UnitWarning> {
    let mut out = Vec::new();
    let time = time.map(normalise).filter(|u| !u.is_empty());
    let conc = concentration.map(normalise).filter(|u| !u.is_empty());
    let dose = dose.map(normalise).filter(|u| !u.is_empty());
    for (what, unit) in [("time", &time), ("concentration", &conc), ("dose", &dose)] {
        if unit.is_none() {
            out.push(warning(
                "missing_unit",
                format!(
                    "the {what} has no unit; derived units (AUC, clearance, volume) cannot be named"
                ),
            ));
        }
    }
    if let Some(t) = &time {
        if time_in_seconds(t).is_none() {
            out.push(warning(
                "unrecognised_unit",
                format!("the time unit `{t}` is not recognised (s, min, h, d); consistency cannot be checked"),
            ));
        }
    }
    let conc_parts = conc.as_deref().and_then(split_concentration);
    if let Some(c) = &conc {
        if conc_parts.is_none() {
            out.push(warning(
                "unrecognised_unit",
                format!("the concentration unit `{c}` is not recognised (mass/volume such as ng/mL); consistency cannot be checked"),
            ));
        }
    }
    if let Some(d) = &dose {
        // A mass per kg of body weight (NCA-UNIT-01, task T-098) is checked by its mass.
        let d = per_kg_mass(d).unwrap_or(d);
        if d.contains("/m2") || d.contains("/m^2") || d.contains("/m\u{b2}") {
            out.push(warning(
                "dose_per_body_size",
                format!("the dose unit `{d}` is per body size other than body weight; give the absolute dose (for example mg) or a dose per kg (mg/kg) to compute clearance and volumes"),
            ));
        } else if mass_in_grams(d).is_none() {
            out.push(warning(
                "unrecognised_unit",
                format!("the dose unit `{d}` is not recognised (a mass such as mg); consistency cannot be checked"),
            ));
        } else if let Some((mass, _)) = conc_parts {
            if mass != d {
                out.push(warning(
                    "mass_mismatch",
                    format!("the dose is in {d} but the concentration is per {mass}; clearance and volumes will carry a conversion factor, convert one of them first"),
                ));
            }
        }
    }
    out
}

/// Names of the derived units, from the units of the time, concentration and dose columns:
/// `auc`, `aumc`, `mrt`, `half_life`, `lambda_z`, and (when the dose unit is known) `cl` and `v`.
/// A missing unit leaves the corresponding names out; nothing is guessed.
pub fn derived_units(
    time: Option<&str>,
    concentration: Option<&str>,
    dose: Option<&str>,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let time = time.map(normalise).filter(|u| !u.is_empty());
    let conc = concentration.map(normalise).filter(|u| !u.is_empty());
    let dose = dose.map(normalise).filter(|u| !u.is_empty());
    let mut put = |name: &str, value: String| {
        out.insert(name.to_owned(), value);
    };
    if let Some(t) = &time {
        put("mrt", t.clone());
        put("half_life", t.clone());
        put("lambda_z", format!("1/{t}"));
    }
    if let (Some(t), Some(c)) = (&time, &conc) {
        put("auc", format!("{t}*{c}"));
        put("aumc", format!("{t}^2*{c}"));
    }
    if let (Some(t), Some(c), Some(d)) = (&time, &conc, &dose) {
        let (mass_of_dose, per_kg) = match per_kg_mass(d) {
            Some(m) => (m, "/kg"),
            None => (d.as_str(), ""),
        };
        match split_concentration(c) {
            Some((mass, volume)) if mass == mass_of_dose => {
                put("cl", format!("{volume}/{t}{per_kg}"));
                put("v", format!("{volume}{per_kg}"));
            }
            _ => {
                put("cl", format!("{d}/({t}*{c})"));
                put("v", format!("{d}/({c})"));
            }
        }
    }
    out
}
