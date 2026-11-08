//! The population objective at fixed parameters (`specs/population.md`): FOCEI (`foce_i`, the
//! default, POP-OBJ-02), plain FOCE with σ frozen at the population prediction (`foce`,
//! POP-OBJ-06) and FO (`fo`, POP-OBJ-03), in the conventions `full` and `no_2pi` (POP-OBJ-05), with
//! the conditional modes η̂ of every subject (POP-INN-01, POP-INN-02) and the refusals of POP-INN-03
//! that are decided before any outer iteration.
//!
//! The outer estimation, the covariance, the shrinkage and the diagnostics are not here yet: a
//! request without `options.evaluate_only = true` is refused with the code `not_available`.
//!
//! The model `identity_linear` (POP-MOD-05) is a test model: it is reachable through this API only,
//! is not in the catalogue of `caladrius-models` and is not named in any message of this module.

mod checks;
mod data;
mod dense;
mod inner;
mod structure;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;

use caladrius_models::ModelId;
use serde::{Deserialize, Serialize};

use crate::linalg::Mat;
use crate::model::FitModel;
use crate::{Bounds, ErrorModel};

use checks::LagDomain;
use data::Subject;
use inner::InnerProblem;
use structure::{Structure, SubjectModel};

// ---------------------------------------------------------------- the request

/// A population request (POP-API-01): the vocabulary of `oracle/expected/pop/errors.json` and of the
/// header of `crates/caladrius-fit/tests/oracle_pop.rs`. Words that select a behaviour are read as
/// text, so that a wrong word is refused with a message that names the option.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopRequest {
    /// A `pk1.*` or `pk2.*` model id.
    pub model: Option<String>,
    /// Typical values by parameter (the free typical values).
    #[serde(default)]
    pub initial: BTreeMap<String, f64>,
    /// The parameters that carry a random effect, in the order of the rows of Ω.
    pub random_effects: Option<Vec<String>>,
    /// Ω, q × q.
    pub omega: Option<Vec<Vec<f64>>>,
    /// `diagonal` (the default) or `block`.
    pub omega_shape: Option<String>,
    /// `additive`, `proportional`, `combined` or `combined_sd`; required.
    pub error_model: Option<String>,
    /// The error parameters in the user's units: `[a]`, `[b]` or `[a, b]` (FIT-ML-21).
    pub error_init: Option<Vec<f64>>,
    /// Options of the model (the test model's design).
    #[serde(default)]
    pub model_options: ModelOptions,
    /// Typical values held at a value (they are not counted as free), and the error parameters `a`
    /// and `b` held at a value in the user's units.
    #[serde(default)]
    pub fixed: BTreeMap<String, f64>,
    /// Bounds by typical value (FIT-BND-01); read for the domain of `tlag` (POP-INN-03 item 1).
    #[serde(default)]
    pub bounds: BTreeMap<String, Bounds>,
    /// A table in the event layout in place of `subjects`; the host reads it into `dataset_text`.
    pub dataset_file: Option<String>,
    /// The text of the table named by `dataset_file`, set by the host (not part of the JSON).
    #[serde(skip)]
    pub dataset_text: Option<String>,
    /// Options of the run.
    #[serde(default)]
    pub options: PopOptions,
    /// The subjects, inline.
    pub subjects: Option<Vec<SubjectInput>>,
}

/// Options of the structural model.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelOptions {
    /// Z column k = t^z_powers[k] (POP-MOD-05).
    pub z_powers: Option<Vec<u32>>,
}

/// Options of a population run.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopOptions {
    /// `foce_i` (the default), `foce` or `fo`.
    pub method: Option<String>,
    /// `full` (the default) or `no_2pi`.
    pub ofv_convention: Option<String>,
    /// `sandwich`, `r` or `s` (checked; the covariance itself is not computed here).
    pub covariance: Option<String>,
    /// Labels of subjects left out of the run.
    #[serde(default)]
    pub exclude_subjects: Vec<String>,
    /// The objective and the modes at the given parameters, with no outer iteration.
    #[serde(default)]
    pub evaluate_only: bool,
}

/// One subject, inline: the times are measured from the dose (POP-DAT-01).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectInput {
    pub id: String,
    pub dose: f64,
    pub time: Vec<f64>,
    /// `null` for a missing observation.
    pub y: Vec<Option<f64>>,
    /// The duration of a zero-order input (POP-API-01 item 11, the name is `assumed`).
    pub duration: Option<f64>,
}

// ---------------------------------------------------------------- the answer

/// The method of the objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// FOCE with interaction (POP-OBJ-02), the default.
    FoceI,
    /// FOCE without interaction, σ frozen at the population prediction (POP-OBJ-06).
    Foce,
    /// First order, η̂ = 0 (POP-OBJ-03).
    Fo,
}

/// The constant of the objective (POP-OBJ-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OfvConvention {
    /// The constant N·ln(2π) kept (the default).
    #[serde(rename = "full")]
    Full,
    /// OFV_full − N·ln(2π).
    #[serde(rename = "no_2pi")]
    No2Pi,
}

/// The answer of an evaluation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PopAnswer {
    /// `evaluated` (POP-API-01 item 6).
    pub status: String,
    pub method: Method,
    pub ofv_convention: OfvConvention,
    /// The total objective, the per-subject values summed in dataset order.
    pub ofv: f64,
    /// Subjects with at least one observation used.
    pub n_subjects: usize,
    /// Observations used, all subjects.
    pub n_observations: usize,
    pub random_effects: Vec<String>,
    /// Every subject of the run, in dataset order.
    pub subjects: Vec<SubjectAnswer>,
}

/// One subject of the answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubjectAnswer {
    pub id: String,
    pub n_observations: usize,
    /// OFV_i in the convention of the answer (0 for a subject with no observation used).
    pub ofv: f64,
    /// η̂_i in the order of `random_effects`; null after FO and for a subject with no observation.
    pub eta_hat: Option<Vec<f64>>,
    /// Why `eta_hat` is null: `fo` or `no_observations`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_hat_reason: Option<String>,
    /// The Newton decrement Δ of the returned η̂ (convention of H37); null when there is no η̂.
    pub newton_decrement: Option<f64>,
}

// ---------------------------------------------------------------- the refusals

/// The codes of a refusal (POP-INN-03, POP-API-01 item 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopErrorCode {
    InvalidOption,
    UnsupportedHistory,
    UnboundedLikelihood,
    NonPositiveSigma,
    TooFewSubjects,
    TooFewObservations,
    InnerFailed,
    /// The outer estimation is not in this version yet (a request without `evaluate_only`).
    NotAvailable,
}

impl PopErrorCode {
    /// The machine name.
    pub fn id(&self) -> &'static str {
        match self {
            PopErrorCode::InvalidOption => "invalid_option",
            PopErrorCode::UnsupportedHistory => "unsupported_history",
            PopErrorCode::UnboundedLikelihood => "unbounded_likelihood",
            PopErrorCode::NonPositiveSigma => "non_positive_sigma",
            PopErrorCode::TooFewSubjects => "too_few_subjects",
            PopErrorCode::TooFewObservations => "too_few_observations",
            PopErrorCode::InnerFailed => "inner_failed",
            PopErrorCode::NotAvailable => "not_available",
        }
    }
}

/// A refusal: a code and a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PopError {
    pub code: PopErrorCode,
    pub message: String,
}

impl fmt::Display for PopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.id(), self.message)
    }
}

impl std::error::Error for PopError {}

/// "option `{name}`: {what is wrong}; {what to give}" (POP-INN-03 item 5).
pub(crate) fn invalid(option: &str, what: impl fmt::Display) -> PopError {
    PopError {
        code: PopErrorCode::InvalidOption,
        message: format!("option `{option}`: {what}"),
    }
}

fn invalid_plain(message: String) -> PopError {
    PopError {
        code: PopErrorCode::InvalidOption,
        message,
    }
}

fn inner_failed(id: &str) -> PopError {
    PopError {
        code: PopErrorCode::InnerFailed,
        message: format!(
            "the inner problem of subject {id} did not converge at the starting values; check the starting values, the model and the data of that subject"
        ),
    }
}

// ---------------------------------------------------------------- the run

/// Evaluates the population objective at the parameters of the request (`options.evaluate_only`).
pub fn run(request: &PopRequest) -> Result<PopAnswer, PopError> {
    run_with(request, &|_| false)
}

/// [`run`], with a hook that makes the inner problem of the named subjects fail (the unit test of
/// `inner_failed`, POP-INN-03 item 4; never an option of the request).
pub(crate) fn run_with(
    request: &PopRequest,
    fail_inner: &dyn Fn(&str) -> bool,
) -> Result<PopAnswer, PopError> {
    let setup = Setup::new(request)?;
    if !request.options.evaluate_only {
        return Err(PopError {
            code: PopErrorCode::NotAvailable,
            message: "the outer estimation of the population fit is not available in this version yet; give `options.evaluate_only = true` to evaluate the objective at the given parameters".to_string(),
        });
    }
    setup.evaluate(fail_inner)
}

/// Everything the evaluation needs, checked.
struct Setup {
    method: Method,
    convention: OfvConvention,
    error_model: ErrorModel,
    structure: Structure,
    theta: BTreeMap<String, f64>,
    random_effects: Vec<String>,
    omega: Mat,
    l_inv: Mat,
    /// The moved error variables.
    xi: Vec<f64>,
    subjects: Vec<Subject>,
    /// The population prediction of each subject.
    pred0: Vec<Vec<f64>>,
}

fn method_of(word: Option<&str>) -> Result<Method, PopError> {
    match word {
        None | Some("foce_i") => Ok(Method::FoceI),
        Some("foce") => Ok(Method::Foce),
        Some("fo") => Ok(Method::Fo),
        Some(other) => Err(invalid(
            "method",
            format!("unknown method `{other}`; use `foce_i` (the default), `foce` or `fo`"),
        )),
    }
}

fn convention_of(word: Option<&str>) -> Result<OfvConvention, PopError> {
    match word {
        None | Some("full") => Ok(OfvConvention::Full),
        Some("no_2pi") => Ok(OfvConvention::No2Pi),
        Some(other) => Err(invalid(
            "ofv_convention",
            format!("unknown convention `{other}`; use `full` (the default) or `no_2pi`"),
        )),
    }
}

fn check_covariance(word: Option<&str>) -> Result<(), PopError> {
    match word {
        None | Some("sandwich" | "r" | "s") => Ok(()),
        Some(other) => Err(invalid(
            "covariance",
            format!("unknown covariance kind `{other}`; use `sandwich`, `r` or `s`"),
        )),
    }
}

/// `true` for a block Ω, `false` for a diagonal one.
fn block_of(word: Option<&str>) -> Result<bool, PopError> {
    match word {
        None | Some("diagonal") => Ok(false),
        Some("block") => Ok(true),
        Some(other) => Err(invalid(
            "omega_shape",
            format!("unknown shape of omega `{other}`; use `diagonal` (the default) or `block`"),
        )),
    }
}

fn error_model_of(word: Option<&str>) -> Result<ErrorModel, PopError> {
    let give = "give `additive`, `proportional`, `combined` or `combined_sd`";
    match word {
        None => Err(invalid(
            "error_model",
            format!("no error model is given and there is no default; {give}"),
        )),
        Some(id) => ErrorModel::from_id(id)
            .ok_or_else(|| invalid("error_model", format!("unknown error model `{id}`; {give}"))),
    }
}

fn structure_of(request: &PopRequest) -> Result<Structure, PopError> {
    let give = "give a `pk1.*` or a `pk2.*` model id";
    let Some(id) = request.model.as_deref() else {
        return Err(invalid("model", format!("no model is given; {give}")));
    };
    if id == "identity_linear" {
        let Some(z) = &request.model_options.z_powers else {
            return Err(invalid(
                "model_options",
                "the test model needs `z_powers`; give one power per random effect",
            ));
        };
        let z_powers = z
            .iter()
            .map(|&p| i32::try_from(p))
            .collect::<Result<Vec<i32>, _>>()
            .map_err(|_| invalid("model_options", "a power is too large; give small powers"))?;
        return Ok(Structure::IdentityLinear { z_powers });
    }
    if request.model_options.z_powers.is_some() {
        return Err(invalid(
            "model_options",
            format!("`z_powers` does not apply to `{id}`; leave it out"),
        ));
    }
    ModelId::from_id(id)
        .map(Structure::Catalogue)
        .ok_or_else(|| invalid("model", format!("unknown model `{id}`; {give}")))
}

/// POP-MOD-02: the list of random effects.
fn random_effects_of(request: &PopRequest, structure: &Structure) -> Result<Vec<String>, PopError> {
    let Some(list) = &request.random_effects else {
        return Err(invalid(
            "random_effects",
            "no random effect is declared and there is no default; list the parameters that carry one (for example `cl` and `v`)",
        ));
    };
    if list.is_empty() {
        return Err(invalid(
            "random_effects",
            "the list is empty; list at least one parameter (with none the fit is a pooled fit)",
        ));
    }
    for (i, p) in list.iter().enumerate() {
        if list.iter().take(i).any(|x| x == p) {
            return Err(invalid(
                "random_effects",
                format!("`{p}` is listed twice; list each parameter once"),
            ));
        }
    }
    let Structure::Catalogue(model) = structure else {
        // POP-MOD-05: free labels, rules 1 and 5 of POP-MOD-02 do not apply.
        return Ok(list.clone());
    };
    for p in list {
        match p.as_str() {
            "tlag" => {
                return Err(invalid_plain(
                    "random effect on `tlag`: not available in this version (the prediction has a kink at the lag time); give `tlag` as a shared typical value".to_string(),
                ));
            }
            "dur" => {
                return Err(invalid_plain(
                    "random effect on `dur`: not available in this version (the prediction has a kink at the end of the input); give `dur` as a shared typical value".to_string(),
                ));
            }
            _ => {}
        }
        let names = structure::model_parameters(*model);
        if !names.contains(&p.as_str()) {
            return Err(invalid(
                "random_effects",
                format!(
                    "`{p}` is not a parameter of `{}`; list parameters of that model ({})",
                    model.id(),
                    names.join(", ")
                ),
            ));
        }
        if !request.initial.contains_key(p) && !request.fixed.contains_key(p) {
            return Err(invalid(
                "random_effects",
                format!("`{p}` has no typical value; give it in `initial`"),
            ));
        }
    }
    Ok(list.clone())
}

/// The typical values: `initial` and `fixed` (error parameters excluded), checked.
fn theta_of(
    request: &PopRequest,
    structure: &Structure,
    random_effects: &[String],
) -> Result<BTreeMap<String, f64>, PopError> {
    let mut theta = request.initial.clone();
    for (p, x) in &request.fixed {
        if matches!(p.as_str(), "a" | "b") {
            continue;
        }
        if theta.insert(p.clone(), *x).is_some() {
            return Err(invalid(
                "fixed",
                format!("`{p}` is in both `initial` and `fixed`; give it in one of them"),
            ));
        }
    }
    for (p, x) in &theta {
        if !x.is_finite() {
            return Err(invalid(
                "initial",
                format!("the typical value of `{p}` is {x}; give a finite number"),
            ));
        }
    }
    match structure {
        Structure::IdentityLinear { z_powers } => {
            let keys: Vec<&str> = theta.keys().map(String::as_str).collect();
            if keys != ["theta1", "theta2"] {
                return Err(invalid(
                    "initial",
                    "the test model takes `theta1` and `theta2`; give those two",
                ));
            }
            if z_powers.len() != random_effects.len() {
                return Err(invalid(
                    "model_options",
                    format!(
                        "`z_powers` has {} entries for {} random effects; give one power per random effect",
                        z_powers.len(),
                        random_effects.len()
                    ),
                ));
            }
        }
        Structure::Catalogue(model) => {
            if model.compartments() == 2 {
                let clearance = ["cl", "vc", "q", "vp"];
                if !clearance.iter().all(|k| theta.contains_key(*k)) {
                    return Err(invalid(
                        "initial",
                        "the population fit takes a two-compartment model in the clearance set only (POP-MOD-04); give `cl`, `vc`, `q` and `vp`",
                    ));
                }
            }
            for (p, &x) in &theta {
                let zero_allowed = p == "tlag";
                let inside = if zero_allowed { x >= 0.0 } else { x > 0.0 };
                if !inside {
                    let (domain, give) = if zero_allowed {
                        ("0 or greater", "give a value of 0 or more")
                    } else {
                        ("greater than 0", "give a positive value")
                    };
                    return Err(invalid(
                        "initial",
                        format!(
                            "the typical value of `{p}` is {x}, outside its domain (it must be {domain}); {give}"
                        ),
                    ));
                }
            }
            // The names and the parameter set, checked by the model itself.
            if let Err(e) = model.predict(1.0, &theta, &[0.0]) {
                return Err(invalid("initial", format!("{e}")));
            }
        }
    }
    Ok(theta)
}

/// Ω, its shape and its lower factor.
fn omega_of(request: &PopRequest, q: usize) -> Result<(Mat, Mat), PopError> {
    let block = block_of(request.omega_shape.as_deref())?;
    let Some(rows) = &request.omega else {
        return Err(invalid(
            "omega",
            "no starting omega is given; give the q × q covariance matrix of the random effects",
        ));
    };
    if rows.len() != q || rows.iter().any(|r| r.len() != q) {
        return Err(invalid(
            "omega",
            format!("the matrix is not {q} × {q}; give one row and one column per random effect"),
        ));
    }
    let mut omega = Mat::zeros(q, q);
    for (i, row) in rows.iter().enumerate() {
        for (j, &x) in row.iter().enumerate() {
            omega.set(i, j, x);
        }
    }
    for i in 0..q {
        for j in 0..q {
            let x = omega.at(i, j);
            if !x.is_finite() || x != omega.at(j, i) {
                return Err(invalid(
                    "omega",
                    "the matrix is not symmetric with finite elements; give a symmetric matrix",
                ));
            }
            if !block && i != j && x != 0.0 {
                return Err(invalid(
                    "omega",
                    format!(
                        "the shape is `diagonal` but element ({}, {}) is {x}; give zeros off the diagonal, or set `omega_shape` to `block`",
                        i + 1,
                        j + 1
                    ),
                ));
            }
        }
    }
    let Some(l) = dense::cholesky(&omega) else {
        return Err(invalid(
            "omega",
            "the starting matrix is not positive definite; give positive variances and correlations strictly between -1 and 1",
        ));
    };
    let l_inv = dense::lower_inverse(&l);
    Ok((omega, l_inv))
}

/// The error parameters in the user's units given by `error_init` and `fixed` (a, b), one entry
/// per parameter of the error model, `None` where neither gives it.
fn error_given(request: &PopRequest, em: ErrorModel) -> Result<Vec<Option<f64>>, PopError> {
    let names = em.names();
    let mut out: Vec<Option<f64>> = vec![None; names.len()];
    if let Some(init) = &request.error_init {
        if init.len() != names.len() {
            return Err(invalid(
                "error_init",
                format!(
                    "the {} error model has {} parameter(s) ({}) but {} value(s) are given; give one value per parameter",
                    em.id(),
                    names.len(),
                    names.join(", "),
                    init.len()
                ),
            ));
        }
        out = init.iter().map(|x| Some(*x)).collect();
    }
    for (k, name) in names.iter().enumerate() {
        if let (Some(x), Some(slot)) = (request.fixed.get(*name), out.get_mut(k)) {
            *slot = Some(*x);
        }
    }
    for (name, k) in [("a", "a"), ("b", "b")] {
        if request.fixed.contains_key(name) && !names.contains(&k) {
            return Err(invalid(
                "fixed",
                format!(
                    "the {} error model has no parameter `{name}`; leave it out",
                    em.id()
                ),
            ));
        }
    }
    for x in out.iter().flatten() {
        if !(x.is_finite() && *x >= 0.0) {
            return Err(invalid(
                "error_init",
                format!(
                    "the error parameter {x} is not a finite number at or above 0; give such a number"
                ),
            ));
        }
    }
    Ok(out)
}

/// FIT-ML-10: the moment estimates of the moved error variables, here at the population prediction
/// pooled over the observations of all subjects. Used for a parameter the request does not give.
fn moment_xi(em: ErrorModel, subjects: &[Subject], pred0: &[Vec<f64>]) -> Vec<f64> {
    let pairs: Vec<(f64, f64)> = subjects
        .iter()
        .zip(pred0)
        .flat_map(|(s, p)| s.y.iter().copied().zip(p.iter().copied()))
        .collect();
    let n = pairs.len().max(1) as f64;
    let a0 = pairs.iter().map(|(y, f)| (y - f) * (y - f)).sum::<f64>() / n;
    let b0 = pairs
        .iter()
        .map(|(y, f)| {
            let m2 = (f * f + y * y) / 2.0;
            if m2 > 0.0 {
                (y - f) * (y - f) / m2
            } else {
                0.0
            }
        })
        .sum::<f64>()
        / n;
    match em {
        ErrorModel::Additive => vec![a0],
        ErrorModel::Proportional => vec![b0],
        ErrorModel::Combined => vec![a0 / 2.0, b0 / 2.0],
        ErrorModel::CombinedSd => vec![a0.sqrt() / 2.0, b0.sqrt() / 2.0],
    }
}

fn lag_domain(request: &PopRequest, model: ModelId) -> Option<LagDomain> {
    let lag = model.lag_parameter()?;
    if let Some(&c) = request.fixed.get(lag) {
        return Some(LagDomain { lower: c, upper: c });
    }
    request.initial.get(lag)?;
    let bounds = request.bounds.get(lag).copied().unwrap_or_default();
    Some(LagDomain {
        lower: bounds.lower.unwrap_or(0.0).max(0.0),
        upper: bounds.upper.unwrap_or(f64::INFINITY),
    })
}

impl Setup {
    fn new(request: &PopRequest) -> Result<Self, PopError> {
        let method = method_of(request.options.method.as_deref())?;
        let convention = convention_of(request.options.ofv_convention.as_deref())?;
        check_covariance(request.options.covariance.as_deref())?;
        block_of(request.omega_shape.as_deref())?;
        let error_model = error_model_of(request.error_model.as_deref())?;
        let structure = structure_of(request)?;
        let random_effects = random_effects_of(request, &structure)?;
        let theta = theta_of(request, &structure, &random_effects)?;
        let (omega, l_inv) = omega_of(request, random_effects.len())?;
        let given = error_given(request, error_model)?;
        let subjects = data::subjects(request)?;

        // POP-INN-03 item 3. The counts are left out of an evaluation that gives every parameter
        // (θ, Ω and ξ): it fits nothing (design choice, see the report of T-153).
        let complete = given.iter().all(Option::is_some);
        if !(request.options.evaluate_only && complete) {
            let free_theta = request.initial.len();
            let free_xi = given.len()
                - error_model
                    .names()
                    .iter()
                    .filter(|n| request.fixed.contains_key(**n))
                    .count();
            checks::refuse_too_few(error_model, &subjects, free_theta, free_xi)?;
        }
        // POP-INN-03 item 1, before anything that needs σ.
        if let Structure::Catalogue(model) = &structure {
            checks::refuse_unbounded(*model, error_model, &subjects, lag_domain(request, *model))?;
        }
        let mut pred0 = Vec::with_capacity(subjects.len());
        for s in &subjects {
            let theta_i = subject_theta(&theta, s)?;
            let model = SubjectModel {
                structure: &structure,
                theta: &theta_i,
                random_effects: &random_effects,
                dose: s.dose,
                times: &s.times,
            };
            let zero = vec![0.0; random_effects.len()];
            let Some(p) = model.predict(&zero) else {
                return Err(invalid(
                    "initial",
                    format!(
                        "the model gives no finite prediction for subject {} at the typical values; check the typical values and the dose",
                        s.id
                    ),
                ));
            };
            pred0.push(p);
        }
        let moments = moment_xi(error_model, &subjects, &pred0);
        let user: Vec<f64> = given
            .iter()
            .zip(error_model.user_values(&moments))
            .map(|(g, m)| g.unwrap_or(m))
            .collect();
        let xi = error_model.moved_from_user(&user);
        // POP-INN-03 item 2.
        checks::refuse_non_positive_sigma(error_model, &xi, &subjects, &pred0)?;
        Ok(Self {
            method,
            convention,
            error_model,
            structure,
            theta,
            random_effects,
            omega,
            l_inv,
            xi,
            subjects,
            pred0,
        })
    }

    fn evaluate(&self, fail_inner: &dyn Fn(&str) -> bool) -> Result<PopAnswer, PopError> {
        let ln_2pi = libm::log(std::f64::consts::TAU);
        let q = self.random_effects.len();
        let steps: Vec<f64> = (0..q).map(|k| inner_step(self.omega.at(k, k))).collect();
        let mut answers = Vec::with_capacity(self.subjects.len());
        let mut total = 0.0;
        for (s, pred0) in self.subjects.iter().zip(&self.pred0) {
            let n = s.y.len();
            if n == 0 {
                answers.push(SubjectAnswer {
                    id: s.id.clone(),
                    n_observations: 0,
                    ofv: 0.0,
                    eta_hat: None,
                    eta_hat_reason: Some("no_observations".to_string()),
                    newton_decrement: None,
                });
                continue;
            }
            let theta_i = subject_theta(&self.theta, s)?;
            let model = SubjectModel {
                structure: &self.structure,
                theta: &theta_i,
                random_effects: &self.random_effects,
                dose: s.dose,
                times: &s.times,
            };
            let sigma0: Vec<f64> = pred0
                .iter()
                .map(|&f| self.error_model.sigma(&self.xi, f))
                .collect();
            let zero = vec![0.0; q];
            let (core, eta_hat, decrement) = match self.method {
                Method::Fo => {
                    let ev = model.evaluate(&zero).ok_or_else(|| inner_failed(&s.id))?;
                    let core = linearised(&s.y, &ev.pred, &ev.g, &zero, &sigma0, &self.omega);
                    (core, None, None)
                }
                Method::FoceI | Method::Foce => {
                    let frozen = (self.method == Method::Foce).then_some(sigma0.as_slice());
                    let problem = InnerProblem {
                        model: &model,
                        y: &s.y,
                        error_model: self.error_model,
                        xi: &self.xi,
                        frozen,
                        l_inv: &self.l_inv,
                        steps: &steps,
                    };
                    let mode = if fail_inner(&s.id) {
                        None
                    } else {
                        problem.solve(&zero)
                    };
                    let p = mode.ok_or_else(|| inner_failed(&s.id))?;
                    // POP-OBJ-02: σ̂ at the individual prediction; POP-OBJ-06: σ⁰.
                    let sigma = if frozen.is_some() { &sigma0 } else { &p.sigma };
                    let core = linearised(&s.y, &p.ev.pred, &p.ev.g, &p.eta, sigma, &self.omega);
                    (core, Some(p.eta), Some(p.decrement))
                }
            };
            let core = core.ok_or_else(|| inner_failed(&s.id))?;
            let ofv = match self.convention {
                OfvConvention::Full => n as f64 * ln_2pi + core,
                OfvConvention::No2Pi => core,
            };
            total += ofv;
            answers.push(SubjectAnswer {
                id: s.id.clone(),
                n_observations: n,
                ofv,
                eta_hat_reason: eta_hat.is_none().then(|| "fo".to_string()),
                eta_hat,
                newton_decrement: decrement,
            });
        }
        Ok(PopAnswer {
            status: "evaluated".to_string(),
            method: self.method,
            ofv_convention: self.convention,
            ofv: total,
            n_subjects: self.subjects.iter().filter(|s| !s.y.is_empty()).count(),
            n_observations: self.subjects.iter().map(|s| s.y.len()).sum(),
            random_effects: self.random_effects.clone(),
            subjects: answers,
        })
    }
}

/// The step of the central differences of the polish for a random effect of prior variance ω²:
/// [`inner::POLISH_H`] times its prior standard deviation.
fn inner_step(variance: f64) -> f64 {
    let sd = variance.sqrt();
    let scale = if sd.is_finite() && sd > 0.0 { sd } else { 1.0 };
    inner::POLISH_H * scale
}

/// The typical values with the subject's own duration merged in (POP-API-01 item 11).
fn subject_theta(
    theta: &BTreeMap<String, f64>,
    s: &Subject,
) -> Result<BTreeMap<String, f64>, PopError> {
    let mut t = theta.clone();
    if let Some(d) = s.duration {
        if t.insert("dur".to_string(), d).is_some() {
            return Err(invalid(
                "subjects",
                format!(
                    "subject {} carries a `duration` and `dur` is also a typical value; give it in one place",
                    s.id
                ),
            ));
        }
    }
    Ok(t)
}

/// ln|C| + (y − E)ᵀC⁻¹(y − E) with E = ŷ − G·η and C = G·Ω·Gᵀ + diag(σ²) (POP-OBJ-02), from the
/// Cholesky factor of C; `None` when C is not positive definite.
fn linearised(
    y: &[f64],
    pred: &[f64],
    g: &[Vec<f64>],
    eta: &[f64],
    sigma: &[f64],
    omega: &Mat,
) -> Option<f64> {
    let n = y.len();
    let q = eta.len();
    let at = |j: usize, k: usize| g.get(k).and_then(|c| c.get(j)).copied().unwrap_or(f64::NAN);
    let mut e = Vec::with_capacity(n);
    for (j, (&yj, &fj)) in y.iter().zip(pred).enumerate() {
        let shift: f64 = eta.iter().enumerate().map(|(k, x)| at(j, k) * x).sum();
        e.push(yj - (fj - shift));
    }
    // G·Ω, n × q.
    let mut g_omega = Mat::zeros(n, q);
    for j in 0..n {
        for l in 0..q {
            let s: f64 = (0..q).map(|k| at(j, k) * omega.at(k, l)).sum();
            g_omega.set(j, l, s);
        }
    }
    let mut c = Mat::zeros(n, n);
    for i in 0..n {
        for j in 0..=i {
            let mut s: f64 = (0..q).map(|l| g_omega.at(i, l) * at(j, l)).sum();
            if i == j {
                let sd = sigma.get(i).copied()?;
                s += sd * sd;
            }
            c.set(i, j, s);
            c.set(j, i, s);
        }
    }
    let factor = dense::cholesky(&c)?;
    let log_det: f64 = (0..n).map(|i| 2.0 * libm::log(factor.at(i, i))).sum();
    let w = dense::forward(&factor, &e);
    let distance: f64 = w.iter().map(|x| x * x).sum();
    let core = log_det + distance;
    core.is_finite().then_some(core)
}
