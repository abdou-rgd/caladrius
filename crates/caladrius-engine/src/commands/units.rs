//! `units.check`: the units Caladrius recognises, and the checks on three of them.

use caladrius_project::{accepted_units, unit_warnings};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{CommandDef, parse, respond};
use crate::Engine;
use crate::error::CommandError;
use crate::schema::{array_of, described, nullable, object, reference, root, string};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckParams {
    #[serde(default)]
    time: Option<String>,
    #[serde(default)]
    concentration: Option<String>,
    #[serde(default)]
    dose: Option<String>,
}

fn check(_engine: &mut Engine, params: Value) -> Result<Value, CommandError> {
    let p: CheckParams = parse("units.check", params)?;
    let given = p.time.is_some() || p.concentration.is_some() || p.dose.is_some();
    let warnings = if given {
        unit_warnings(
            p.time.as_deref(),
            p.concentration.as_deref(),
            p.dose.as_deref(),
        )
    } else {
        Vec::new()
    };
    respond(&json!({ "accepted": accepted_units(), "warnings": warnings }))
}

pub(crate) const CHECK: CommandDef = CommandDef {
    id: "units.check",
    title: "List the recognised units and check three of them",
    description: "The units Caladrius recognises for the time, concentration and dose columns (other text is accepted as a unit but cannot be checked), and, when units are given, the checks a worksheet with these units would show: a unit missing or not recognised, a dose per body size other than kg, a dose mass that differs from the concentration's. Reads nothing from the project.",
    mutates: false,
    params: || {
        let unit = |what: &str| {
            described(
                nullable(string()),
                &format!("The unit of the {what} column; absent or null means none."),
            )
        };
        root(
            "units.check parameters",
            object(
                vec![
                    ("time", unit("time")),
                    ("concentration", unit("concentration")),
                    ("dose", unit("dose")),
                ],
                &[],
            ),
        )
    },
    result: || {
        root(
            "units.check result",
            object(
                vec![
                    (
                        "accepted",
                        object(
                            vec![
                                ("time", array_of(string())),
                                ("concentration", array_of(string())),
                                ("dose", array_of(string())),
                                (
                                    "other_spellings",
                                    json!({ "type": "object", "additionalProperties": array_of(string()) }),
                                ),
                            ],
                            &["time", "concentration", "dose", "other_spellings"],
                        ),
                    ),
                    (
                        "warnings",
                        described(
                            array_of(reference("UnitWarning")),
                            "Empty when no unit is given.",
                        ),
                    ),
                ],
                &["accepted", "warnings"],
            ),
        )
    },
    example: || json!({ "time": "h", "concentration": "mg/L", "dose": "mg/kg" }),
    run: check,
};
