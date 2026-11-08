//! Errors of a model evaluation (`specs/models.md` MOD-GEN-04, golden rule 6). Each one names the
//! parameter or value and says what to fix.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Why a model could not be evaluated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "code")]
pub enum ModelError {
    /// A parameter name that no model knows.
    UnknownParameter {
        /// The name given.
        name: String,
        /// The model id.
        model: String,
    },
    /// A known parameter that this model does not have.
    ParameterNotInModel {
        /// The name given.
        name: String,
        /// The model id.
        model: String,
    },
    /// A parameter the model needs is missing.
    MissingParameter {
        /// The missing name (`cl or k` for the elimination parameter).
        name: String,
        /// The model id.
        model: String,
    },
    /// Both `cl` and `k` are given.
    ClearanceAndRateConstant,
    /// A parameter outside its domain.
    ParameterOutOfDomain {
        /// The parameter.
        name: String,
        /// The value given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        value: f64,
        /// The domain, e.g. `a finite number > 0`.
        domain: String,
    },
    /// The dose is negative or not finite.
    InvalidDose {
        /// The dose given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        value: f64,
    },
    /// A time is NaN or infinite.
    NonFiniteTime {
        /// Position in `times`.
        index: usize,
    },
    /// Valid parameters whose result does not fit in a double (e.g. V or CL near 1e-300).
    Overflow {
        /// What could not be represented.
        what: String,
    },
    /// `q`, `vp` or `k12` is 0: the model has one compartment (MOD-2C-05 item 1).
    OneCompartment {
        /// The parameter that is 0.
        name: String,
        /// The model id.
        model: String,
    },
    /// Parameters of two parameterisations of a two-compartment model (MOD-2C-02, 2C-05 item 4).
    MixedParameterSets {
        /// A parameter of the first set found.
        first: String,
        /// A parameter of another set.
        second: String,
        /// The model id.
        model: String,
    },
    /// No parameter of any set of a two-compartment model.
    NoParameterSet {
        /// The model id.
        model: String,
    },
    /// Macro set with `alpha` <= `beta` (MOD-2C-05 item 2): the engine does not sort.
    AlphaNotAboveBeta {
        /// `alpha` given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        alpha: f64,
        /// `beta` given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        beta: f64,
    },
    /// Macro set with a dose of 0: vc = dose / (a + b) is undefined (MOD-2C-05 item 2).
    MacroWithoutDose,
    /// Macro set of a three-compartment model with a dose of 0: vc = dose / (a + b + c) is
    /// undefined (MOD-3C-06). A variant of its own keeps the message (and so the digest
    /// of answers) of the two-compartment refusal as it was.
    MacroWithoutDoseThree,
    /// A quantity derived from valid parameters is not a finite number > 0 (MOD-2C-05 item 3).
    DerivedOutOfRange {
        /// The derived quantity, e.g. `k21`.
        name: String,
        /// How it is derived, e.g. `q / vp`.
        from: String,
        /// Its value.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        value: f64,
    },
    /// The two exponents cannot be told apart in double precision: k12·k21 underflows
    /// (MOD-2C-05 item 3).
    DegenerateExponents {
        /// The model id.
        model: String,
        /// Which quantity and why, for three compartments (MOD-3C-06 items 3 and 5, MOD-3C-08);
        /// absent for two compartments.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// One peripheral compartment of a three-compartment model is missing: `q`, `vp` or `k1x` is 0
    /// (MOD-3C-06 item 1). The model is a two-compartment one, or a one-compartment one when both
    /// peripherals are missing.
    PeripheralMissing {
        /// The parameter that is 0.
        name: String,
        /// The model id.
        model: String,
        /// Both peripheral compartments are missing.
        both: bool,
    },
    /// Macro set of a three-compartment model whose exponents are not in the order alpha > beta >
    /// gamma (MOD-3C-06 item 2): the engine does not sort.
    ExponentsNotDecreasing {
        /// `alpha` given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        alpha: f64,
        /// `beta` given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        beta: f64,
        /// `gamma` given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        gamma: f64,
    },
    /// A dosing regimen whose parameters do not form a regimen (MOD-MD-11, 12): a name without
    /// its partner, a schedule mixed with an interval, a gap in the numbering of the doses.
    InvalidRegimen {
        /// What is wrong and what to give instead.
        problem: String,
    },
    /// A regimen parameter outside its domain (MOD-MD-12): `tau`, `n_doses`, `dose_time[i]`,
    /// `dose_amount[i]`, `dose_dur[i]`.
    RegimenOutOfDomain {
        /// The parameter.
        name: String,
        /// The value given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        value: f64,
        /// The domain and what the parameter is.
        domain: String,
    },
    /// Steady state with an interval shorter than the input duration (MOD-MD-02 (f), MOD-MD-12).
    IntervalShorterThanInput {
        /// The interval given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        tau: f64,
        /// The input duration.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        dur: f64,
    },
    /// A steady-state time outside the dosing interval [0, tau] (MOD-MD-02 (f), MOD-MD-12).
    TimeOutsideInterval {
        /// Position in `times`.
        index: usize,
        /// The time given.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        time: f64,
        /// The interval.
        #[serde(
            serialize_with = "crate::float::ser",
            deserialize_with = "crate::float::de"
        )]
        tau: f64,
    },
    /// The requested derivatives do not exist for this input.
    DerivativesUnavailable {
        /// The model id.
        model: String,
        /// Why, and what to use instead.
        reason: String,
    },
    /// A schedule or regular regimen whose superposition needs more than
    /// [`crate::MAX_DOSE_EVALUATIONS`] evaluations of one dose at one time (MOD-MD-01, card T-051):
    /// the doses that still contribute at the times given, summed over the times.
    SuperpositionTooLarge {
        /// Number of doses of the regimen.
        doses: usize,
        /// Number of evaluation times.
        times: usize,
        /// The limit on the evaluations.
        limit: u64,
    },
}

/// The parameter sets of a two-compartment model, for the messages.
const PK2_SETS: &str = "cl, vc, q, vp (the default), or k10, k12, k21, vc, or a, b, alpha, beta";

/// The parameter sets of a three-compartment model, for the messages.
const PK3_SETS: &str = "cl, vc, q2, vp2, q3, vp3 (the default), or k10, k12, k21, k13, k31, vc, or a, b, c, alpha, beta, gamma";

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownParameter { name, model } if model.starts_with("pk3.") => write!(
                f,
                "unknown parameter `{name}` for model {model}; use one set of {PK3_SETS}, and the input parameters of the model (ka, dur, tlag)"
            ),
            Self::UnknownParameter { name, model } if model.starts_with("pk2.") => write!(
                f,
                "unknown parameter `{name}` for model {model}; use one set of {PK2_SETS}, and the input parameters of the model (ka, dur, tlag)"
            ),
            Self::UnknownParameter { name, model } => write!(
                f,
                "unknown parameter `{name}` for model {model}; use v, cl or k, and the input parameters of the model (ka, dur, tlag)"
            ),
            Self::ParameterNotInModel { name, model } => write!(
                f,
                "model {model} has no parameter `{name}`; remove it or choose a model that has it"
            ),
            Self::MissingParameter { name, model } => {
                write!(
                    f,
                    "model {model} needs the parameter `{name}`; give its value"
                )
            }
            Self::ClearanceAndRateConstant => write!(
                f,
                "both `cl` and `k` are given; give exactly one of them (k = cl / v)"
            ),
            Self::ParameterOutOfDomain {
                name,
                value,
                domain,
            } => write!(
                f,
                "parameter `{name}` = {value} is not {domain}; correct its value"
            ),
            Self::InvalidDose { value } => write!(
                f,
                "the dose {value} is not a finite number >= 0; give the effective dose (F x dose)"
            ),
            Self::NonFiniteTime { index } => write!(
                f,
                "time {} is missing or not a finite number; give finite times",
                index + 1
            ),
            Self::Overflow { what } => write!(
                f,
                "{what} is too large or undefined in double precision (overflow: not a finite number); rescale V, CL or the dose (for example change the units)"
            ),
            Self::OneCompartment { name, model } => write!(
                f,
                "parameter `{name}` = 0 makes {model} a one-compartment model; use the pk1 model of the same route, or give `{name}` > 0"
            ),
            Self::MixedParameterSets {
                first,
                second,
                model,
            } => write!(
                f,
                "parameters `{first}` and `{second}` belong to different parameter sets of {model}; give exactly one set: {}",
                sets_of(model)
            ),
            Self::NoParameterSet { model } => write!(
                f,
                "model {model} needs one complete parameter set: {}",
                sets_of(model)
            ),
            Self::AlphaNotAboveBeta { alpha, beta } => write!(
                f,
                "`alpha` = {alpha} must be larger than `beta` = {beta}: alpha is the faster exponent; swap alpha with beta (and a with b) if they are the other way round"
            ),
            Self::MacroWithoutDose => write!(
                f,
                "the macro set (a, b, alpha, beta) needs a dose > 0, since vc = dose / (a + b); give the dose or use the clearance set"
            ),
            Self::MacroWithoutDoseThree => write!(
                f,
                "the macro set (a, b, c, alpha, beta, gamma) needs a dose > 0, since vc = dose / (a + b + c); give the dose or use the clearance set"
            ),
            Self::DerivedOutOfRange { name, from, value } => write!(
                f,
                "the derived `{name}` = {from} = {} is not a finite number > 0 (overflow or underflow); rescale the parameters (for example change the units)",
                short(*value)
            ),
            Self::InvalidRegimen { problem } => write!(f, "dosing regimen: {problem}"),
            Self::RegimenOutOfDomain {
                name,
                value,
                domain,
            } => write!(
                f,
                "dosing regimen: `{name}` = {value} is not {domain}; correct its value"
            ),
            Self::IntervalShorterThanInput { tau, dur } => write!(
                f,
                "the dosing interval `tau` = {tau} is shorter than the input duration `dur` = {dur}: at steady state each input would overlap the next one; give tau >= dur, or give the doses as a schedule (`dose_time[i]`, `dose_amount[i]`, `dose_dur[i]`), which allows the inputs to overlap"
            ),
            Self::TimeOutsideInterval { index, time, tau } => write!(
                f,
                "time {} = {time} is outside the dosing interval [0, tau = {tau}] of the steady state; give times since the last dose, from 0 to tau",
                index + 1
            ),
            Self::DerivativesUnavailable { model, reason } => write!(
                f,
                "closed-form derivatives of {model} are unavailable: {reason}"
            ),
            Self::SuperpositionTooLarge {
                doses,
                times,
                limit,
            } => write!(
                f,
                "dosing regimen: {doses} doses at {times} times need more than {limit} evaluations of one dose at one time (the doses that still contribute at each time, summed over the times; a regimen is refused only beyond {limit}, so doses x times at most {limit} always runs); give fewer times or fewer doses, or use the steady state (`tau` alone) for a long regular regimen"
            ),
            Self::DegenerateExponents {
                model,
                detail: None,
            } => write!(
                f,
                "the two exponents of {model} cannot be told apart in double precision (k12 x k21 underflows); the data describe a one-compartment model: use the pk1 model of the same route"
            ),
            Self::DegenerateExponents {
                model,
                detail: Some(detail),
            } if detail.starts_with(NO_VALID_MACRO_MODEL) => write!(
                f,
                "degenerate exponents of {model}: {detail}; the macro doubles cannot decide it: give the micro-constants or the clearances, or macro constants whose exponents are further from the poles of the model"
            ),
            Self::DegenerateExponents {
                model,
                detail: Some(detail),
            } => write!(
                f,
                "degenerate exponents of {model}: {detail}; the data describe a model with fewer compartments or an exponent that is not representable in double precision (rescale the parameters, or use the pk2 or pk1 model of the same route)"
            ),
            Self::PeripheralMissing {
                name,
                model,
                both: false,
            } => write!(
                f,
                "parameter `{name}` = 0 removes a peripheral compartment, which makes {model} a two-compartment model; use the pk2 model of the same route, or give `{name}` > 0"
            ),
            Self::PeripheralMissing {
                name,
                model,
                both: true,
            } => write!(
                f,
                "parameter `{name}` = 0 removes a peripheral compartment, and the other peripheral is missing too, which makes {model} a one-compartment model; use the pk1 model of the same route (or pk2 with one peripheral), or give `{name}` > 0"
            ),
            Self::ExponentsNotDecreasing { alpha, beta, gamma } => write!(
                f,
                "the exponents must satisfy `alpha` > `beta` > `gamma`: alpha = {alpha}, beta = {beta}, gamma = {gamma}; alpha is the fastest exponent and gamma the slowest, give them in that order (the engine does not sort them, and a, b, c with them)"
            ),
        }
    }
}

/// The start of the detail of a macro set that gives no valid three-compartment model (MOD-3C-06
/// item 3, MOD-3C-08): its message says so and does not speak of overflow, underflow or rescaling.
pub(crate) const NO_VALID_MACRO_MODEL: &str =
    "the macro set (a, b, c, alpha, beta, gamma) gives no valid three-compartment model";

/// A number for a message in a short round-trip form: plain decimal for the magnitudes people
/// write (1e-4 up to 1e16, and 0), scientific (`1e-300`, `2.5e300`) beyond, so that no message
/// carries hundreds of zeros. Not finite values print as `NaN`, `inf` or `-inf`.
pub(crate) fn short(x: f64) -> String {
    let a = x.abs();
    if a == 0.0 || !a.is_finite() || (1e-4..1e16).contains(&a) {
        format!("{x}")
    } else {
        format!("{x:e}")
    }
}

/// The sets of the model of this id, for the messages.
fn sets_of(model: &str) -> &'static str {
    if model.starts_with("pk3.") {
        PK3_SETS
    } else {
        PK2_SETS
    }
}

impl std::error::Error for ModelError {}
