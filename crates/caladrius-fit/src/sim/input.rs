//! The inputs of one simulation (SIM-MOD-05): plain data, checked by [`super::Study::new`].

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::error::SimError;
use super::omega::OmegaShape;
use super::rng::Seed;

/// The largest integer a double (a JSON number, a JavaScript number) holds exactly.
const MAX_SAFE_SEED: f64 = 9_007_199_254_740_991.0;

impl Seed {
    /// A seed given as a decimal string: digits only, 0 <= s < 2^64, whatever its size (SIM-RNG-02).
    pub fn from_text(text: &str) -> Result<Seed, SimError> {
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(SimError::invalid(format!(
                "seed \"{text}\" is not a decimal integer from 0 to 18446744073709551615"
            )));
        }
        text.parse::<u64>().map(Seed).map_err(|_| {
            SimError::invalid(format!(
                "seed \"{text}\" is not below 18446744073709551616 (2^64): give an integer from 0 to 18446744073709551615"
            ))
        })
    }

    /// A seed given as a number: an integer from 0 to 2^53 - 1; anything larger must be a decimal string.
    pub fn from_number(x: f64) -> Result<Seed, SimError> {
        if !x.is_finite() || x.fract() != 0.0 {
            return Err(SimError::invalid(format!("seed {x} is not an integer")));
        }
        if x < 0.0 {
            return Err(SimError::invalid(format!(
                "seed {x} is negative: give an integer from 0"
            )));
        }
        if x > MAX_SAFE_SEED {
            return Err(SimError::invalid(format!(
                "seed {x} is above 9007199254740991: write it as a decimal string"
            )));
        }
        Ok(Seed(x as u64))
    }

    /// The seed as a decimal string, the form in which it is sealed.
    pub fn text(self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Seed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text())
    }
}

impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Seed, D::Error> {
        struct SeedVisitor;
        impl Visitor<'_> for SeedVisitor {
            type Value = Seed;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a decimal string, or an integer below 2^53")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Seed, E> {
                Seed::from_text(v).map_err(|e| E::custom(e.message))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Seed, E> {
                Seed::from_number(v as f64).map_err(|e| E::custom(e.message))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Seed, E> {
                Seed::from_number(v as f64).map_err(|e| E::custom(e.message))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Seed, E> {
                Seed::from_number(v).map_err(|e| E::custom(e.message))
            }
        }
        deserializer.deserialize_any(SeedVisitor)
    }
}

/// The four error models of POP-ERR-01, evaluated on the individual prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// sigma = a.
    Additive,
    /// sigma = b |IPRED|.
    Proportional,
    /// sigma = sqrt(a^2 + b^2 IPRED^2).
    Combined,
    /// sigma = a + b |IPRED|.
    CombinedSd,
}

impl ErrorKind {
    /// The error model of this name.
    pub fn from_name(name: &str) -> Result<ErrorKind, SimError> {
        match name {
            "additive" => Ok(ErrorKind::Additive),
            "proportional" => Ok(ErrorKind::Proportional),
            "combined" => Ok(ErrorKind::Combined),
            "combined_sd" => Ok(ErrorKind::CombinedSd),
            other => Err(SimError::invalid(format!(
                "error_model {other} is not known: choose additive, proportional, combined or combined_sd"
            ))),
        }
    }

    /// The name in a request.
    pub fn name(self) -> &'static str {
        match self {
            ErrorKind::Additive => "additive",
            ErrorKind::Proportional => "proportional",
            ErrorKind::Combined => "combined",
            ErrorKind::CombinedSd => "combined_sd",
        }
    }

    /// The error parameters this model needs.
    pub(super) fn needs(self) -> &'static [&'static str] {
        match self {
            ErrorKind::Additive => &["a"],
            ErrorKind::Proportional => &["b"],
            ErrorKind::Combined | ErrorKind::CombinedSd => &["a", "b"],
        }
    }
}

/// What to do with a simulated value below 0 (SIM-ORD-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NegativeRule {
    /// Leave y as drawn (the default).
    #[default]
    Keep,
    /// y = max(y, 0).
    Zero,
}

impl NegativeRule {
    /// The rule of this name.
    pub fn from_name(name: &str) -> Result<NegativeRule, SimError> {
        match name {
            "keep" => Ok(NegativeRule::Keep),
            "zero" => Ok(NegativeRule::Zero),
            other => Err(SimError::invalid(format!(
                "option negative {other} is not known: use `keep` or `zero`"
            ))),
        }
    }

    /// The name in a request.
    pub fn name(self) -> &'static str {
        match self {
            NegativeRule::Keep => "keep",
            NegativeRule::Zero => "zero",
        }
    }
}

impl OmegaShape {
    /// The shape of this name.
    pub fn from_name(name: &str) -> Result<OmegaShape, SimError> {
        match name {
            "diagonal" => Ok(OmegaShape::Diagonal),
            "block" => Ok(OmegaShape::Block),
            other => Err(SimError::invalid(format!(
                "omega_shape {other} is not known: use `diagonal` or `block`"
            ))),
        }
    }
}

/// One row of a design table (the layout of `specs/data.md`, DAT-LAY-01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    /// The subject cell as imported: the label, exactly (SIM-RNG-03).
    pub subject: String,
    /// The time of the row.
    pub time: f64,
    /// A dose row or an observation row.
    pub kind: RowKind,
    /// The amount of a dose row.
    pub amt: Option<f64>,
    /// Whether the value cell of an observation row was filled: it is ignored and counted (SIM-MOD-04 item 1).
    pub dv_given: bool,
}

/// The two kinds of row of a design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    /// A dose.
    Dose,
    /// An observation.
    Observation,
}

/// One subject of a design given subject by subject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignSubject {
    /// The label.
    pub label: String,
    /// The dose.
    pub dose: f64,
    /// The time of the dose; the model is evaluated at `time - dose_time` (DAT-USE-01).
    pub dose_time: f64,
    /// The observation times, in the order of the table.
    pub times: Vec<f64>,
}

/// The design (SIM-MOD-04): subjects, a table, or one list of times with a dose and m subjects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Design {
    /// Subject by subject.
    Subjects(Vec<DesignSubject>),
    /// A table of dose and observation rows; the subjects are taken in order of first appearance.
    Table(Vec<TableRow>),
    /// `m` subjects labelled "1" to "m", the same dose and times (SIM-MOD-04 item 5).
    Grid {
        /// The observation times.
        times: Vec<f64>,
        /// The dose.
        dose: f64,
        /// The number of subjects.
        m: u64,
    },
}

fn one() -> u64 {
    1
}

/// The whole call (SIM-MOD-05). A result repeats it, seed and options included, so that it can be replayed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimInput {
    /// A model id of SIM-MOD-02, e.g. `pk1.iv_bolus`.
    pub model: String,
    /// The typical value of every parameter of the model.
    pub theta: BTreeMap<String, f64>,
    /// The parameters with a random effect, in order: the order is part of the model (SIM-MOD-03).
    pub random_effects: Vec<String>,
    /// The shape of `omega`; `diagonal` when absent.
    #[serde(default)]
    pub omega_shape: OmegaShape,
    /// Omega: the variances, or the lower triangle by rows.
    pub omega: Vec<f64>,
    /// The error model.
    pub error_model: ErrorKind,
    /// The error parameter `a` (additive, combined).
    pub a: Option<f64>,
    /// The error parameter `b` (proportional, combined).
    pub b: Option<f64>,
    /// The design.
    pub design: Design,
    /// The seed; required (`missing_seed` without it).
    pub seed: Option<Seed>,
    /// The number of replicates, 1 to 2^24; 1 when absent.
    #[serde(default = "one")]
    pub replicates: u64,
    /// The negative-value rule; `keep` when absent.
    #[serde(default)]
    pub negative: NegativeRule,
    /// The lower limit of quantification, one number for the run.
    pub lloq: Option<f64>,
    /// The probabilities of the summary; `None` for the default `[0.05, 0.5, 0.95]`.
    pub quantiles: Option<Vec<f64>>,
}
