//! The refusals of the simulator (`specs/simulation.md` section 7). Each one stops the run before any output
//! is produced and names what to change (`AGENTS.md` section 7).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Why a simulation was refused: the serialised name is the stable code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimErrorCode {
    /// No seed (SIM-REF-01).
    MissingSeed,
    /// An option, a parameter or a table the simulator does not accept; the message names it (SIM-REF-03).
    InvalidOption,
    /// A Cholesky pivot of Omega that is not finite or not positive (SIM-REF-02).
    OmegaNotPositiveDefinite,
    /// More replicates than the counter holds (SIM-REF-06).
    SizeLimit,
    /// Two subject labels with the same 64-bit stream key (SIM-REF-06).
    LabelKeyCollision,
    /// Two observations at the same time for one subject (DAT-ORD-01).
    DuplicateTimes,
    /// Time going backwards inside a subject (DAT-ERR-01).
    TimeNotIncreasing,
    /// A subject with no observation row (DAT-ERR-04).
    SubjectWithoutObservation,
    /// A subject with more than one dose row (POP-DAT-01; multiple dosing is not simulated in v1).
    MoreThanOneDose,
    /// A dose row without an amount (DAT-ERR-02).
    DoseWithoutAmount,
    /// An observation row with an amount (DAT-ERR-03).
    AmountOnObservation,
    /// The structural model refused the parameters of a subject (SIM-REF-05).
    ModelRefused,
    /// A parameter, a prediction or an observation that is not finite (SIM-REF-05).
    NonFiniteValue,
}

impl SimErrorCode {
    /// The code as it is written in a request and an answer.
    pub fn as_str(self) -> &'static str {
        match self {
            SimErrorCode::MissingSeed => "missing_seed",
            SimErrorCode::InvalidOption => "invalid_option",
            SimErrorCode::OmegaNotPositiveDefinite => "omega_not_positive_definite",
            SimErrorCode::SizeLimit => "size_limit",
            SimErrorCode::LabelKeyCollision => "label_key_collision",
            SimErrorCode::DuplicateTimes => "duplicate_times",
            SimErrorCode::TimeNotIncreasing => "time_not_increasing",
            SimErrorCode::SubjectWithoutObservation => "subject_without_observation",
            SimErrorCode::MoreThanOneDose => "more_than_one_dose",
            SimErrorCode::DoseWithoutAmount => "dose_without_amount",
            SimErrorCode::AmountOnObservation => "amount_on_observation",
            SimErrorCode::ModelRefused => "model_refused",
            SimErrorCode::NonFiniteValue => "non_finite_value",
        }
    }
}

/// A refusal: a code and a sentence that names what to change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimError {
    /// The stable code.
    pub code: SimErrorCode,
    /// The sentence.
    pub message: String,
}

impl SimError {
    /// A refusal with this code and sentence.
    pub fn new(code: SimErrorCode, message: impl Into<String>) -> SimError {
        SimError {
            code,
            message: message.into(),
        }
    }

    /// `invalid_option`; the message must name the option.
    pub fn invalid(message: impl Into<String>) -> SimError {
        SimError::new(SimErrorCode::InvalidOption, message)
    }
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for SimError {}
