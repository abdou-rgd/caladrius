//! Loaders for the three-compartment oracle (task T-142): `oracle/expected/models/pk3/`, from
//! `oracle/scripts/models_3c_closed_form.R`.
//!
//! The files share the long format `subject,parameter,value` of the model cases (`crate::step3`),
//! each with an `.options.json`:
//!
//! - `model_pk3_<id>_<case>` (kind `model`): values of one of the six `pk3.*` ids on a time grid
//!   (`conc`, `auc`, `aumc` at `t=<time>`) and their scalars. The group `scalar` holds the scalars
//!   the engine is held to; the group `scalar_ill` holds the scalars whose condition number with
//!   respect to the parameters of the case exceeds 1e3 (written with their conditions in the
//!   options; whether the engine owes them is the orchestrator's decision). [`Pk3Kind::Values`].
//! - `model_pk3_<name>` of kind `model_macro_free`: macro inputs in the free zones of MOD-3C-08, [`Pk3Kind::Free`].
//! - `model_pk3_cond_*` (kind `model_conditioning`): scalars only, for the ill-conditioned
//!   parameter points of OM-22. [`Pk3Kind::Conditioning`].
//! - `model_pk3_conv_*`: the conversion cases of OM-23 (scalars: the other two sets and the
//!   labelling flag `k21_ge_k31`). [`Pk3Kind::Conversion`].
//! - `model_pk3_deriv_<id>_<case>` (kind `model_derivatives`): `d_<parameter>` at `t=<time>` in the
//!   parameter set of the case; entries whose condition exceeds 1e3 are absent. [`Pk3Kind::Derivatives`].
//! - `model_pk3_md_*` (kind `model_md`): multiple dosing and steady state (OM-28), loaded with
//!   [`load_pk3_md_case`].
//! - `model_pk3_errors`: the inputs the engine must refuse (OM-26), [`load_pk3_errors`].

use std::path::PathBuf;

use serde::Deserialize;

use crate::md::{MdCase, load_md_case_in};
use crate::oracle::{OracleError, oracle_dir};
use crate::pk2::{Pk2ErrorCase, load_error_suite};
use crate::step3::{ModelCase, json, load_model_case_in, names_in};

/// Name of the error suite (`model_pk3_errors.csv` and `.options.json`).
pub const PK3_ERRORS: &str = "model_pk3_errors";

/// Prefix of the multiple-dosing cases.
const MD_PREFIX: &str = "model_pk3_md_";

/// `oracle/expected/models/pk3/`.
pub fn pk3_dir() -> PathBuf {
    oracle_dir().join("expected").join("models").join("pk3")
}

/// What a three-compartment case holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pk3Kind {
    /// `conc`, `auc`, `aumc` on a grid and the scalars.
    Values,
    /// Scalars only, at a point chosen for its conditioning.
    Conditioning,
    /// Scalars only: a conversion between the parameter sets.
    Conversion,
    /// `d_<parameter>` on a grid.
    Derivatives,
    /// A macro input in a free zone of MOD-3C-08 (zone (0) or (iii)): the engine may refuse it or
    /// accept it; when it accepts, `conc`, `auc`, `aumc` on a grid and the `scalar` group (the
    /// quantities that follow from the macro input alone) are held, [`Pk3Case::zone`].
    Free,
}

/// The claim of a large-coupling case that one root sits next to one pole of the secular function
/// (`_pole_` in the case name): which root, which pole, on which side and how far.
#[derive(Debug, Clone, PartialEq)]
pub struct PoleClaim {
    /// `alpha`, `beta` or `gamma`.
    pub root: String,
    /// `k21` or `k31`.
    pub pole: String,
    /// `above` or `below`: where the root lies relative to the pole.
    pub side: String,
    /// The distance of the root to the pole, in half-spacings of the doubles on that side of the
    /// pole, measured at 256 bits.
    pub half_spacings: f64,
    /// The relative distance the case promises not to exceed.
    pub max_relative_distance: f64,
}

/// One three-compartment case.
#[derive(Debug, Clone, PartialEq)]
pub struct Pk3Case {
    /// Model id (`pk3.iv_bolus`, ...), dose, parameters (of `parameterisation`, plus `ka`, `dur`,
    /// `tlag`), time grid and expected values.
    pub case: ModelCase,
    pub kind: Pk3Kind,
    /// `clearance` (cl, vc, q2, vp2, q3, vp3), `micro` (k10, k12, k21, k13, k31, vc) or `macro`
    /// (a, b, c, alpha, beta, gamma).
    pub parameterisation: String,
    /// True when the textbook forms in double precision keep their digits for this case.
    ///
    /// Where `textbook_basis` is `measured` the flag comes from running the naive forms and
    /// overrides the older rule `textbook_ok && no scalar_ill row` of the generator: the flag concerns
    /// the concentration curve, so 39 measured-true cases also carry `scalar_ill` rows (scalars whose
    /// condition number is above 1e3). Three cases sit near the 1e-8 threshold of the naive test:
    /// `oral_1_lag_ka_alpha_p1e_06` (worst ratio 0.78, true), `oral_1_ka_beta_m1e_06` (1.89, false) and
    /// `oral_1_lag_ka_beta_p1e_06` (2.08, false); a different libm `exp()` could flip them. The test
    /// `the_textbook_flag_is_what_the_naive_forms_do` is not changed for them.
    pub textbook_double_ok: bool,
    /// Names of the scalars in the group `scalar_ill`.
    pub scalar_ill: Vec<String>,
    /// `measured` when `textbook_double_ok` was decided by running the naive forms at the testkit's
    /// tolerance (T-165), `declared` when it is the rule of the generator.
    pub textbook_basis: Option<String>,
    /// The largest ratio of the naive error to the tolerance over the times and the three
    /// quantities when the flag is measured: at most 1 exactly when the flag is true. `None` when
    /// declared; infinite when the naive forms returned a non-finite value.
    pub textbook_worst_ratio: Option<f64>,
    /// The pole claim of a `_pole_` case.
    pub pole: Option<PoleClaim>,
    /// For a [`Pk3Kind::Free`] case, the zone of MOD-3C-08 (`0` or `iii`).
    pub zone: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Extra {
    kind: String,
    parameterisation: String,
    #[serde(default)]
    textbook_double_ok: bool,
    #[serde(default)]
    scalar_ill_names: Vec<String>,
    #[serde(default)]
    textbook_basis: Option<String>,
    #[serde(default)]
    textbook_worst_ratio: Option<serde_json::Value>,
    #[serde(default)]
    pole_root: Option<String>,
    #[serde(default)]
    pole: Option<String>,
    #[serde(default)]
    pole_side: Option<String>,
    #[serde(default)]
    pole_distance_half_spacings: Option<f64>,
    #[serde(default)]
    pole_max_relative_distance: Option<f64>,
    #[serde(default)]
    zone: Option<ZoneField>,
}

#[derive(Debug, Deserialize)]
struct ZoneField {
    zone: String,
}

/// Names of the value, conditioning, conversion and derivative cases, sorted (the multiple-dosing
/// cases are [`list_pk3_md_cases`], the error suite is [`PK3_ERRORS`]).
pub fn list_pk3_cases() -> Result<Vec<String>, OracleError> {
    let mut names = names_in(&pk3_dir())?;
    names.retain(|n| n != PK3_ERRORS && !n.starts_with(MD_PREFIX));
    Ok(names)
}

/// Names of the multiple-dosing and steady-state cases, sorted.
pub fn list_pk3_md_cases() -> Result<Vec<String>, OracleError> {
    let mut names = names_in(&pk3_dir())?;
    names.retain(|n| n.starts_with(MD_PREFIX));
    Ok(names)
}

/// Loads one case and checks that its files agree.
pub fn load_pk3_case(name: &str) -> Result<Pk3Case, OracleError> {
    let dir = pk3_dir();
    let case = load_model_case_in(&dir, name)?;
    let path = dir.join(format!("{name}.options.json"));
    let extra: Extra = json(&path)?;
    let kind = match extra.kind.as_str() {
        "model" if name.contains("_conv_") => Pk3Kind::Conversion,
        "model" => Pk3Kind::Values,
        "model_conditioning" if name.contains("_conv_") => Pk3Kind::Conversion,
        "model_conditioning" => Pk3Kind::Conditioning,
        "model_derivatives" => Pk3Kind::Derivatives,
        "model_macro_free" => Pk3Kind::Free,
        other => {
            return Err(OracleError::Inconsistent {
                path: path.display().to_string(),
                message: format!("unknown kind {other:?}"),
            });
        }
    };
    Ok(Pk3Case {
        case,
        kind,
        parameterisation: extra.parameterisation,
        textbook_double_ok: extra.textbook_double_ok,
        scalar_ill: extra.scalar_ill_names,
        textbook_basis: extra.textbook_basis,
        textbook_worst_ratio: match extra.textbook_worst_ratio {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) if s == "Infinity" => Some(f64::INFINITY),
            Some(serde_json::Value::Number(n)) => n.as_f64(),
            Some(other) => {
                return Err(OracleError::Inconsistent {
                    path: path.display().to_string(),
                    message: format!("textbook_worst_ratio {other}"),
                });
            }
        },
        pole: match (
            extra.pole_root,
            extra.pole,
            extra.pole_side,
            extra.pole_distance_half_spacings,
            extra.pole_max_relative_distance,
        ) {
            (
                Some(root),
                Some(pole),
                Some(side),
                Some(half_spacings),
                Some(max_relative_distance),
            ) => Some(PoleClaim {
                root,
                pole,
                side,
                half_spacings,
                max_relative_distance,
            }),
            (None, None, None, None, None) => None,
            _ => {
                return Err(OracleError::Inconsistent {
                    path: path.display().to_string(),
                    message: "a pole claim must carry pole_root, pole, pole_side, pole_distance_half_spacings and pole_max_relative_distance".to_string(),
                });
            }
        },
        zone: extra.zone.map(|z| z.zone),
    })
}

/// Loads one multiple-dosing case.
pub fn load_pk3_md_case(name: &str) -> Result<MdCase, OracleError> {
    load_md_case_in(&pk3_dir(), name)
}

/// Loads the error suite and checks that its table lists exactly its cases, all not available.
pub fn load_pk3_errors() -> Result<Vec<Pk2ErrorCase>, OracleError> {
    load_error_suite(&pk3_dir(), PK3_ERRORS)
}
