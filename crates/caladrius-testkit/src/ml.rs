//! Loaders and helpers for the maximum-likelihood fitting oracle (task T-148): `oracle/expected/fit/ml/`,
//! produced by `oracle/scripts/fit_ml.R` (`specs/fit.md` section 13, FIT-ML-*).
//!
//! Files, all in `oracle/expected/fit/ml/`:
//!
//! - `<case>.options.json` and `<case>.csv` (`subject,parameter,value`): one case is one dataset, one
//!   model and one error model, every subject of the dataset that has a maximum. The names of the
//!   values are listed at [`MlGroup`].
//! - `<case>.gnls.csv`: what `nlme::gnls` reports on the same data, informational only (OF-14);
//!   [`load_fit_ml_gnls`].
//! - `errors.json`: the inputs the engine must refuse (ML6, OF-18), each with the code of the error and
//!   the words of its message; [`load_fit_ml_errors`].
//!
//! The datasets of the cases are in `oracle/data/` (Theoph, Indometh, the F data of `specs/fit.md`
//! section 11) and `oracle/data/fit/ml/` (seeded synthetic profiles, one per error model, and the
//! first four F observations).
//!
//! [`AbsoluteTolerance`] holds the absolute tolerances that only the new tests use (`Tolerance` is
//! matched exhaustively in `xtask`, so no variant is added to it). [`naive_ml`] is an independent,
//! deliberately simple maximum-likelihood fit in double precision.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use crate::compare::Table;
use crate::oracle::{Dataset, OracleError, load_dataset, oracle_dir, parse_expected};
use crate::step3::{Observations, json, names_in, read};

/// `oracle/expected/fit/ml/`.
pub fn fit_ml_dir() -> PathBuf {
    oracle_dir().join("expected").join("fit").join("ml")
}

/// The four error models of `specs/fit.md` FIT-ML-02.
pub const ML_ERROR_MODELS: [&str; 4] = ["additive", "proportional", "combined", "combined_sd"];

/// An absolute tolerance, for the quantities of the maximum-likelihood tests whose natural scale does
/// not depend on their size (a log-likelihood can be negative or near zero, so a relative rule is
/// meaningless: FIT-ML-08). Used only by the new tests; the relative tolerances of
/// [`crate::Tolerance`] are not touched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsoluteTolerance {
    /// Largest accepted `|actual - expected|`.
    pub abs: f64,
}

impl AbsoluteTolerance {
    /// −2LL, AIC_ML and BIC_ML (AIC and BIC add an exact penalty to −2LL). The engine stops when the
    /// Newton decrement is at most 1e-10, which bounds how far −2LL can be from its minimum by about
    /// 1e-10; 1e-7 leaves three orders of magnitude for the accumulated rounding of the sum of N
    /// terms of order 1 to 10 and does not depend on the sign of −2LL.
    pub const FIT_M2LL: AbsoluteTolerance = AbsoluteTolerance { abs: 1e-7 };
    /// IWRES and sigma are derived from the estimates, which the engine finds to a relative error far
    /// below 1e-4 (the decrement bound 1e-10 puts it near 1e-5 at worst); 1e-5 absolute on a
    /// standardised residual of order 1.
    pub const FIT_IWRES: AbsoluteTolerance = AbsoluteTolerance { abs: 1e-5 };
    /// A quantity whose reference value is zero up to rounding because of a structure of the model,
    /// not an accident (the expected-information correlation between `k` and `error_b` of a
    /// proportional-type fit is exactly 0: [`MlGroup::Correlation`]): the engine must return 0 to this
    /// absolute value. A relative tolerance on a reference of 1e-76 would be meaningless.
    pub const FIT_STRUCTURAL_ZERO: AbsoluteTolerance = AbsoluteTolerance { abs: 1e-8 };
    /// The naive double-precision fit of this module against the reference −2LL.
    pub const NAIVE_ML_M2LL: AbsoluteTolerance = AbsoluteTolerance { abs: 1e-6 };

    /// True when `actual` is finite and within `abs` of `expected`.
    pub fn accepts(&self, actual: f64, expected: f64) -> bool {
        actual.is_finite() && expected.is_finite() && (actual - expected).abs() <= self.abs
    }
}

/// The kinds of value in the expected table of a case, with the name patterns that make them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MlGroup {
    /// `n`, `p` (free model parameters), `q` (error parameters).
    Counts,
    /// `m2ll`, `aic_ml`, `bic_ml` (absolute tolerance, [`AbsoluteTolerance::FIT_M2LL`]).
    Likelihood,
    /// `rss_uniform`: the residual sum of squares with uniform weights at the ML estimate (the
    /// least-squares forms of FIT-ML-16 are computed from it).
    UniformRss,
    /// `estimate.<name>` of the model parameters and of `error_a`, `error_b` (0 on a bound).
    Estimate,
    /// `se.<name>` and `cv_percent.<name>`, expected information; empty (NC) for a component on its
    /// bound.
    StandardError,
    /// `correlation.<a>.<b>`, expected information, over the free estimates.
    Correlation,
    /// `se_obs.<name>`, `correlation_obs.<a>.<b>`: observed information (half the Hessian of −2LL).
    /// Informational until the option `information` offers it (OF-16).
    Observed,
    /// `sigma.<i>`: the standard deviation of observation `i` (1-based, in the order of the fitted
    /// observations).
    Sigma,
    /// `iwres.<i>`.
    Iwres,
    /// `iwres_mean`, `iwres_sd`, `iwres_sum_sq`.
    IwresSummary,
    /// `n_iwres_gt2`, `n_iwres_gt3`.
    IwresCount,
}

impl MlGroup {
    /// The group of a value name of the expected table.
    pub fn of(name: &str) -> Option<MlGroup> {
        let base = name.split('.').next().unwrap_or(name);
        Some(match base {
            "n" | "p" | "q" => MlGroup::Counts,
            "m2ll" | "aic_ml" | "bic_ml" => MlGroup::Likelihood,
            "rss_uniform" => MlGroup::UniformRss,
            "estimate" => MlGroup::Estimate,
            "se" | "cv_percent" => MlGroup::StandardError,
            "correlation" => MlGroup::Correlation,
            "se_obs" | "correlation_obs" => MlGroup::Observed,
            "sigma" => MlGroup::Sigma,
            "iwres" => MlGroup::Iwres,
            "iwres_mean" | "iwres_sd" | "iwres_sum_sq" => MlGroup::IwresSummary,
            "n_iwres_gt2" | "n_iwres_gt3" => MlGroup::IwresCount,
            _ => return None,
        })
    }
}

/// Options the reference was computed with, to be given to the engine.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MlFitOptions {
    /// Threshold of the Newton decrement (FIT-ML-08): 1e-10, absolute.
    pub convergence: f64,
    /// `"decrement"` (descriptive: the criterion of `objective = ml`).
    pub criterion: String,
    /// 500 (FIT-ML-08).
    pub max_iterations: usize,
    pub confidence_level: f64,
    /// `"expected"` (FIT-ML-17, FIT-ML-21).
    pub information: String,
}

#[derive(Debug, Deserialize)]
struct ObservationsOptions {
    #[serde(rename = "use")]
    rule: String,
    #[serde(default)]
    time: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct BoundOptions {
    #[serde(default)]
    lower: Option<f64>,
    #[serde(default)]
    upper: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawCase {
    case: String,
    dataset: String,
    data_file: String,
    model: String,
    objective: String,
    error_model: String,
    weighting: String,
    #[serde(default)]
    note: String,
    observations: ObservationsOptions,
    initial_estimates: BTreeMap<String, BTreeMap<String, f64>>,
    fit_options: MlFitOptions,
    subjects: Vec<String>,
    // An empty R list is written `[]`, a filled one `{...}`: read as a value, then converted.
    #[serde(default)]
    status: Value,
    #[serde(default)]
    at_bound: Value,
    #[serde(default)]
    bounds: BTreeMap<String, BoundOptions>,
    #[serde(default)]
    error_init: Option<Vec<f64>>,
    n_values: usize,
}

/// One maximum-likelihood case: a dataset, a model, an error model, start values per subject and
/// the reference results of every subject that has a maximum.
#[derive(Debug, Clone, PartialEq)]
pub struct MlCase {
    pub name: String,
    pub dataset: Dataset,
    /// Model id of `specs/models.md`: `pk1.iv_bolus` or `pk1.oral_1`.
    pub model: String,
    /// `additive`, `proportional`, `combined` (variance sum) or `combined_sd`.
    pub error_model: String,
    /// Always `uniform` (FIT-ML-01: the option as nobody set it).
    pub weighting: String,
    pub observations: Observations,
    /// Initial estimates of the model parameters by subject, then by name.
    pub initial_estimates: BTreeMap<String, BTreeMap<String, f64>>,
    /// User bounds on model parameters: `(parameter, lower, upper)`.
    pub bounds: Vec<(String, Option<f64>, Option<f64>)>,
    /// The `error_init` option (initial a, b in user units), when the case gives it.
    pub error_init: Option<Vec<f64>>,
    pub options: MlFitOptions,
    /// Subjects that have reference results. A subject of the dataset that is not listed is refused
    /// before fitting (an oral profile with an observation of 0 at time 0 under a combined error
    /// model: the likelihood is unbounded; the refusal rows `ml6e_*` of `errors.json`).
    pub subjects: Vec<String>,
    /// `converged` or `at_bound`, by subject.
    pub status: BTreeMap<String, String>,
    /// Names of the error or model parameters held on a bound at the reference optimum, by subject.
    pub at_bound: BTreeMap<String, Vec<String>>,
    /// `(subject, name)` to value, for the subjects in `subjects`.
    pub expected: Table,
    pub note: String,
}

impl MlCase {
    /// The profile of `subject`, if it is in the dataset.
    pub fn profile(&self, subject: &str) -> Option<&crate::oracle::Profile> {
        self.dataset
            .profiles
            .iter()
            .find(|p| p.subject.to_string() == subject)
    }

    /// The `(time, concentration)` pairs of `profile` that are fitted under [`Self::observations`].
    pub fn fitted_observations(&self, profile: &crate::oracle::Profile) -> (Vec<f64>, Vec<f64>) {
        profile
            .time
            .iter()
            .zip(&profile.conc)
            .filter(|&(&t, _)| match self.observations {
                Observations::All => true,
                Observations::TimeGreaterThan(limit) => t > limit,
            })
            .map(|(&t, &c)| (t, c))
            .unzip()
    }

    /// The reference value `name` of `subject`; `None` when absent or not available (NC).
    pub fn value(&self, subject: &str, name: &str) -> Option<f64> {
        self.expected.get(subject, name).flatten()
    }

    /// The names of the model parameters followed by `error_a` (and `error_b`), in the order of the
    /// estimates.
    pub fn parameter_names(&self) -> Vec<String> {
        let mut names: Vec<String> = if self.model == "pk1.iv_bolus" {
            vec!["v".into(), "k".into()]
        } else {
            vec!["v".into(), "k".into(), "ka".into()]
        };
        match self.error_model.as_str() {
            "additive" => names.push("error_a".into()),
            "proportional" => names.push("error_b".into()),
            _ => {
                names.push("error_a".into());
                names.push("error_b".into());
            }
        }
        names
    }
}

fn map_of<T: for<'de> Deserialize<'de>>(
    value: &Value,
    path: &str,
    what: &str,
) -> Result<BTreeMap<String, T>, OracleError> {
    match value {
        Value::Null => Ok(BTreeMap::new()),
        Value::Array(a) if a.is_empty() => Ok(BTreeMap::new()),
        Value::Object(_) => serde_json::from_value(value.clone()).map_err(|e| OracleError::Json {
            path: path.to_string(),
            message: format!("{what}: {e}"),
        }),
        _ => Err(OracleError::Json {
            path: path.to_string(),
            message: format!("{what} must be an object"),
        }),
    }
}

/// Names of the cases in `oracle/expected/fit/ml/`, sorted.
pub fn list_fit_ml_cases() -> Result<Vec<String>, OracleError> {
    names_in(&fit_ml_dir())
}

/// Loads one maximum-likelihood case and checks that its files agree.
pub fn load_fit_ml_case(name: &str) -> Result<MlCase, OracleError> {
    let root = oracle_dir();
    let dir = fit_ml_dir();
    let options_path = dir.join(format!("{name}.options.json"));
    let here = options_path.display().to_string();
    let raw: RawCase = json(&options_path)?;
    let inconsistent = |message: String| OracleError::Inconsistent {
        path: here.clone(),
        message,
    };
    if raw.case != name {
        return Err(inconsistent(format!(
            "case field is {:?}, file name says {name:?}",
            raw.case
        )));
    }
    if raw.objective != "ml" || raw.weighting != "uniform" {
        return Err(inconsistent(format!(
            "objective {:?} and weighting {:?}: a maximum-likelihood case has objective ml and weighting uniform",
            raw.objective, raw.weighting
        )));
    }
    if !ML_ERROR_MODELS.contains(&raw.error_model.as_str()) {
        return Err(inconsistent(format!(
            "unknown error model {:?}",
            raw.error_model
        )));
    }
    let observations = match (raw.observations.rule.as_str(), raw.observations.time) {
        ("all", _) => Observations::All,
        ("time_greater_than", Some(t)) => Observations::TimeGreaterThan(t),
        (other, _) => return Err(inconsistent(format!("unknown observations rule {other:?}"))),
    };
    let dataset = load_dataset(&raw.dataset, &root.join(&raw.data_file))?;
    let expected_path = dir.join(format!("{name}.csv"));
    let expected = parse_expected(&expected_path.display().to_string(), &read(&expected_path)?)?;
    if expected.len() != raw.n_values {
        return Err(inconsistent(format!(
            "options say {} values, the expected table has {}",
            raw.n_values,
            expected.len()
        )));
    }
    let status: BTreeMap<String, String> = map_of(&raw.status, &here, "status")?;
    let at_bound: BTreeMap<String, Vec<String>> = map_of(&raw.at_bound, &here, "at_bound")?;
    for subject in &raw.subjects {
        if !dataset
            .profiles
            .iter()
            .any(|p| p.subject.to_string() == *subject)
        {
            return Err(inconsistent(format!(
                "subject {subject} is not in the dataset"
            )));
        }
    }
    for subject in &raw.subjects {
        if !raw.initial_estimates.contains_key(subject) {
            return Err(inconsistent(format!(
                "subject {subject} has no initial estimates"
            )));
        }
        if expected.get(subject, "m2ll").is_none() {
            return Err(inconsistent(format!("no results for subject {subject}")));
        }
        match status.get(subject).map(String::as_str) {
            Some("converged" | "at_bound") => {}
            other => {
                return Err(inconsistent(format!(
                    "subject {subject}: status {other:?} is neither converged nor at_bound"
                )));
            }
        }
    }
    let bounds = raw
        .bounds
        .into_iter()
        .map(|(p, b)| (p, b.lower, b.upper))
        .collect();
    Ok(MlCase {
        name: name.to_string(),
        dataset,
        model: raw.model,
        error_model: raw.error_model,
        weighting: raw.weighting,
        observations,
        initial_estimates: raw.initial_estimates,
        bounds,
        error_init: raw.error_init,
        options: raw.fit_options,
        subjects: raw.subjects,
        status,
        at_bound,
        expected,
        note: raw.note,
    })
}

/// What `nlme::gnls` reports on the data of a case, by subject: `converged` (1 or 0), `m2ll`
/// (`-2 logLik`), the parameters, `sigma` and the variance-function parameters `varstruct.<i>`.
/// Informational only (OF-14): gnls estimates the variance covariate `fitted(.)` along the
/// iteration, so its optimum can differ from the true maximum; see the report of T-148.
pub fn load_fit_ml_gnls(name: &str) -> Result<Table, OracleError> {
    let path = fit_ml_dir().join(format!("{name}.gnls.csv"));
    parse_expected(&path.display().to_string(), &read(&path)?)
}

/// A reference correlation below this is a structural zero (exactly 0 in theory, about 1e-76 in the
/// reference): the engine is compared in absolute value with `AbsoluteTolerance::FIT_STRUCTURAL_ZERO`.
pub const ML_STRUCTURAL_ZERO_CUTOFF: f64 = 1e-12;

/// Relative agreement between the engine own `aic` and `sbc` and the formulas `N ln(WRSS) + penalty` from
/// its own `wrss` (FIT-ML-16): both sides come from the same number, so only rounding separates them.
pub const ML_LS_FORMULA_RELATIVE: f64 = 1e-9;

/// Name of the error suite (`errors.json`).
pub const FIT_ML_ERRORS: &str = "errors";

/// One input the engine must refuse before fitting (ML6, FIT-ML-14, FIT-ML-15).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MlErrorCase {
    pub case: String,
    pub description: String,
    pub model: String,
    pub dose: f64,
    pub time: Vec<f64>,
    pub conc: Vec<f64>,
    /// Initial estimates by parameter name.
    pub initial: BTreeMap<String, f64>,
    /// The weighting id given with the input (`uniform` unless the case tests FIT-ML-01).
    pub weighting: String,
    /// The options to set, by their ids in `specs/fit.md` FIT-ML-21: `objective`, `error_model`,
    /// `error_init`.
    pub options: BTreeMap<String, Value>,
    /// When present: options that differ from `options` only in what the case refuses (a valid
    /// `error_init`, the additive model for the unbounded likelihood). The engine must accept and fit
    /// the same observations with them, so that the refusal cannot come from an option it does not know.
    #[serde(default)]
    pub control_options: Option<BTreeMap<String, Value>>,
    pub error: MlExpectedError,
}

/// The refusal expected: the `code` of the error (the snake_case tag of `FitError`) and words that
/// the lower-cased message must all contain.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MlExpectedError {
    pub code: String,
    pub words: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawErrors {
    kind: String,
    cases: Vec<MlErrorCase>,
}

/// Loads the refusal suite of the maximum-likelihood fit.
pub fn load_fit_ml_errors() -> Result<Vec<MlErrorCase>, OracleError> {
    let path = fit_ml_dir().join(format!("{FIT_ML_ERRORS}.json"));
    let here = path.display().to_string();
    let raw: RawErrors = json(&path)?;
    if raw.kind != "fit_ml_errors" {
        return Err(OracleError::Inconsistent {
            path: here,
            message: format!("kind is {:?}", raw.kind),
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    for c in &raw.cases {
        if !seen.insert(c.case.clone()) || c.time.len() != c.conc.len() || c.error.words.is_empty()
        {
            return Err(OracleError::Inconsistent {
                path: here,
                message: format!(
                    "case {:?}: duplicate name, unequal time and conc, or no words",
                    c.case
                ),
            });
        }
    }
    Ok(raw.cases)
}

// ---------------------------------------------------------------- naive maximum likelihood

/// The result of [`naive_ml`].
#[derive(Debug, Clone, PartialEq)]
pub struct NaiveMl {
    /// Model parameters in the order of the model (`v, k` or `v, k, ka`).
    pub theta: Vec<f64>,
    /// Error parameters in user units (`a`, `b` as the error model uses them).
    pub xi: Vec<f64>,
    /// −2LL with the constant kept.
    pub m2ll: f64,
}

fn predict(model: &str, theta: &[f64], dose: f64, t: f64) -> f64 {
    let (v, k) = (theta[0], theta[1]);
    if model == "pk1.iv_bolus" {
        dose / v * (-k * t).exp()
    } else {
        let ka = theta[2];
        if (ka - k).abs() < 1e-12 * k.abs().max(1e-300) {
            dose * k * t / v * (-k * t).exp()
        } else {
            dose * ka / (v * (ka - k)) * ((-k * t).exp() - (-ka * t).exp())
        }
    }
}

fn sigma_of(error_model: &str, xi: &[f64], yhat: f64) -> f64 {
    match error_model {
        "additive" => xi[0],
        "proportional" => xi[0] * yhat.abs(),
        "combined" => (xi[0] * xi[0] + xi[1] * xi[1] * yhat * yhat).sqrt(),
        _ => xi[0] + xi[1] * yhat.abs(),
    }
}

/// −2LL of one profile, constant kept (FIT-ML-04): `sum(ln(2 pi sigma^2) + r^2 / sigma^2)`.
/// Infinite when a standard deviation is not strictly positive and finite.
pub fn m2ll(
    model: &str,
    error_model: &str,
    theta: &[f64],
    xi: &[f64],
    dose: f64,
    t: &[f64],
    y: &[f64],
) -> f64 {
    let mut total = 0.0;
    for (&ti, &yi) in t.iter().zip(y) {
        let yhat = predict(model, theta, dose, ti);
        let s = sigma_of(error_model, xi, yhat);
        if !(s.is_finite() && s > 0.0 && yhat.is_finite()) {
            return f64::INFINITY;
        }
        let r = yi - yhat;
        total += (2.0 * std::f64::consts::PI * s * s).ln() + r * r / (s * s);
    }
    total
}

/// Nelder-Mead minimisation of `f` from `start` with a simplex of relative size `size`.
fn nelder_mead(
    f: &dyn Fn(&[f64]) -> f64,
    start: &[f64],
    size: f64,
    iterations: usize,
) -> (Vec<f64>, f64) {
    let n = start.len();
    let mut simplex: Vec<Vec<f64>> = vec![start.to_vec()];
    for i in 0..n {
        let mut p = start.to_vec();
        p[i] += if p[i].abs() > 1e-8 {
            size * p[i].abs()
        } else {
            size
        };
        simplex.push(p);
    }
    let mut values: Vec<f64> = simplex.iter().map(|p| f(p)).collect();
    for _ in 0..iterations {
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
        simplex = order.iter().map(|&i| simplex[i].clone()).collect();
        values = order.iter().map(|&i| values[i]).collect();
        if (values[n] - values[0]).abs() < 1e-15 * (1.0 + values[0].abs())
            && simplex[n]
                .iter()
                .zip(&simplex[0])
                .all(|(a, b)| (a - b).abs() < 1e-13 * (1.0 + b.abs()))
        {
            break;
        }
        let centroid: Vec<f64> = (0..n)
            .map(|j| simplex[..n].iter().map(|p| p[j]).sum::<f64>() / n as f64)
            .collect();
        let along = |c: f64| -> Vec<f64> {
            (0..n)
                .map(|j| centroid[j] + c * (simplex[n][j] - centroid[j]))
                .collect()
        };
        let reflected = along(-1.0);
        let fr = f(&reflected);
        if fr < values[0] {
            let expanded = along(-2.0);
            let fe = f(&expanded);
            if fe < fr {
                simplex[n] = expanded;
                values[n] = fe;
            } else {
                simplex[n] = reflected;
                values[n] = fr;
            }
        } else if fr < values[n - 1] {
            simplex[n] = reflected;
            values[n] = fr;
        } else {
            let contracted = if fr < values[n] {
                along(-0.5)
            } else {
                along(0.5)
            };
            let fc = f(&contracted);
            if fc < values[n].min(fr) {
                simplex[n] = contracted;
                values[n] = fc;
            } else {
                let base = simplex[0].clone();
                for i in 1..=n {
                    for (x, b) in simplex[i].iter_mut().zip(&base) {
                        *x = b + 0.5 * (*x - b);
                    }
                    values[i] = f(&simplex[i]);
                }
            }
        }
    }
    let mut best = 0;
    for i in 1..=n {
        if values[i] < values[best] {
            best = i;
        }
    }
    (simplex[best].clone(), values[best])
}

/// Nelder-Mead on the logarithms of all parameters from one start, restarted from its own result
/// until it stops improving.
fn descend(
    model: &str,
    error_model: &str,
    dose: f64,
    t: &[f64],
    y: &[f64],
    theta0: &[f64],
    xi0: &[f64],
) -> Option<NaiveMl> {
    let n_theta = theta0.len();
    let unpack = |u: &[f64]| -> (Vec<f64>, Vec<f64>) {
        let e: Vec<f64> = u.iter().map(|x| x.clamp(-60.0, 60.0).exp()).collect();
        (e[..n_theta].to_vec(), e[n_theta..].to_vec())
    };
    let objective = |u: &[f64]| -> f64 {
        let (theta, xi) = unpack(u);
        m2ll(model, error_model, &theta, &xi, dose, t, y)
    };
    let mut u: Vec<f64> = theta0.iter().chain(xi0).map(|x| x.ln()).collect();
    let mut value = objective(&u);
    if !value.is_finite() {
        return None;
    }
    for round in 0..40 {
        let size = if round % 2 == 0 { 0.3 } else { 0.05 };
        let (next, v) = nelder_mead(&objective, &u, size, 4000);
        let improved = value - v;
        if v <= value {
            u = next;
            value = v;
        }
        if round >= 3 && improved.abs() < 1e-13 {
            break;
        }
    }
    let (theta, xi) = unpack(&u);
    Some(NaiveMl {
        theta,
        xi,
        m2ll: value,
    })
}

/// A naive maximum-likelihood fit in double precision: Nelder-Mead on the logarithms of the model
/// parameters and of the error parameters (so that they stay positive), restarted from its own
/// result until it stops improving. No derivatives, no scoring, no bounds: a second implementation
/// that shares nothing with the engine or with the R reference. A component that the optimum puts at
/// 0 is approached, not reached, so −2LL agrees with the reference from above.
///
/// `initial` are the model parameters to start from (`v, k[, ka]`); the error parameters start from
/// the moments of the residuals at those parameters. A two-parameter error model has the interior
/// optimum and two local optima on the boundary (one component at 0, which is the additive or the
/// proportional fit): it is started from the interior moments and from the additive and the
/// proportional fits, and the lowest −2LL is kept. `None` when no start has a finite −2LL.
pub fn naive_ml(
    model: &str,
    error_model: &str,
    dose: f64,
    t: &[f64],
    y: &[f64],
    initial: &[f64],
) -> Option<NaiveMl> {
    let n = t.len() as f64;
    let (mut a0, mut b0) = (0.0, 0.0);
    for (&ti, &yi) in t.iter().zip(y) {
        let yhat = predict(model, initial, dose, ti);
        let r = yi - yhat;
        a0 += r * r / n;
        let m2 = 0.5 * (yhat * yhat + yi * yi);
        b0 += if m2 > 0.0 { r * r / m2 / n } else { 0.0 };
    }
    let (a0, b0) = (a0.sqrt().max(1e-6), b0.sqrt().max(1e-6));
    let mut starts: Vec<(Vec<f64>, Vec<f64>)> = match error_model {
        "additive" => vec![(initial.to_vec(), vec![a0])],
        "proportional" => vec![(initial.to_vec(), vec![b0])],
        _ => vec![(initial.to_vec(), vec![a0 / 2.0, b0 / 2.0])],
    };
    if !matches!(error_model, "additive" | "proportional") {
        if let Some(f) = naive_ml(model, "additive", dose, t, y, initial) {
            starts.push((f.theta, vec![f.xi[0], 1e-6]));
        }
        if let Some(f) = naive_ml(model, "proportional", dose, t, y, initial) {
            starts.push((f.theta, vec![1e-6, f.xi[0]]));
        }
    }
    starts
        .iter()
        .filter_map(|(theta0, xi0)| descend(model, error_model, dose, t, y, theta0, xi0))
        .min_by(|a, b| a.m2ll.total_cmp(&b.m2ll))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn every_case_loads_and_the_suites_agree() {
        let cases = list_fit_ml_cases().expect("list");
        assert!(cases.len() >= 20, "{} cases", cases.len());
        let mut subjects = 0usize;
        for name in &cases {
            let case = load_fit_ml_case(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            subjects += case.subjects.len();
            // the gnls table exists, with a row `converged` per subject
            let gnls = load_fit_ml_gnls(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            for s in &case.subjects {
                assert!(gnls.get(s, "converged").is_some(), "{name} subject {s}");
            }
            // every value name is classified
            for (_, value_name, _) in case.expected.iter() {
                assert!(MlGroup::of(value_name).is_some(), "{name}: {value_name}");
            }
        }
        assert!(subjects >= 90, "{subjects} fitted subjects");
        let errors = load_fit_ml_errors().expect("errors");
        assert!(errors.len() >= 15, "{} refusals", errors.len());
    }

    #[test]
    fn the_reference_obeys_its_own_identities() {
        for name in list_fit_ml_cases().expect("list") {
            let case = load_fit_ml_case(&name).expect("case");
            for s in &case.subjects {
                let v = |k: &str| {
                    case.value(s, k)
                        .unwrap_or_else(|| panic!("{name} {s}: no {k}"))
                };
                let (n, p, q) = (v("n"), v("p"), v("q"));
                let m = v("m2ll");
                let tag = format!("{name} subject {s}");
                // AIC and BIC (FIT-ML-16), to rounding of the written digits
                assert!(
                    (v("aic_ml") - (m + 2.0 * (p + q))).abs() < 1e-12 * (1.0 + m.abs()),
                    "{tag} aic"
                );
                assert!(
                    (v("bic_ml") - (m + (p + q) * n.ln())).abs() < 1e-12 * (1.0 + m.abs()),
                    "{tag} bic"
                );
                // Sum of z^2 = N (FIT-ML-13), the IWRES summaries (FIT-ML-18)
                let z: Vec<f64> = (1..=n as usize).map(|i| v(&format!("iwres.{i}"))).collect();
                let ss: f64 = z.iter().map(|x| x * x).sum();
                assert!((ss - n).abs() < 1e-9 * n, "{tag}: sum z^2 = {ss}, N = {n}");
                assert!((v("iwres_sum_sq") - ss).abs() < 1e-12 * n, "{tag}");
                let mean = z.iter().sum::<f64>() / n;
                assert!((v("iwres_mean") - mean).abs() < 1e-12, "{tag} mean");
                let sd = (z.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
                assert!((v("iwres_sd") - sd).abs() < 1e-12, "{tag} sd");
                let c2 = z.iter().filter(|x| x.abs() > 2.0).count() as f64;
                let c3 = z.iter().filter(|x| x.abs() > 3.0).count() as f64;
                assert_eq!((v("n_iwres_gt2"), v("n_iwres_gt3")), (c2, c3), "{tag}");
                // sigma_i follows the error model from the observation and its own residual
                let profile = case.profile(s).expect("profile");
                let (_, y) = case.fitted_observations(profile);
                assert_eq!(y.len() as f64, n, "{tag}: N");
                let xi: Vec<f64> = match case.error_model.as_str() {
                    "additive" => vec![v("estimate.error_a")],
                    "proportional" => vec![v("estimate.error_b")],
                    _ => vec![v("estimate.error_a"), v("estimate.error_b")],
                };
                for i in 0..y.len() {
                    let sigma = v(&format!("sigma.{}", i + 1));
                    let yhat = y[i] - z[i] * sigma;
                    let expected = sigma_of(&case.error_model, &xi, yhat);
                    assert!(
                        (sigma - expected).abs() <= 1e-9 * sigma,
                        "{tag}: sigma.{} {sigma} against the model {expected}",
                        i + 1
                    );
                }
                // the additive model: A = RSS / N exactly (FIT-ML-11)
                if case.error_model == "additive" {
                    let rss = v("rss_uniform");
                    let a = xi[0];
                    assert!(
                        (a * a - rss / n).abs() < 1e-12 * rss / n,
                        "{tag}: a^2 = RSS/N"
                    );
                }
                // a component on a bound has no SE; an error component is written as 0, a model
                // parameter as its bound
                for par in case.at_bound.get(s).into_iter().flatten() {
                    let at = v(&format!("estimate.{par}"));
                    if par.starts_with("error_") {
                        assert_eq!(at, 0.0, "{tag}: {par}");
                    } else {
                        let lower = case
                            .bounds
                            .iter()
                            .find(|(p, _, _)| p == par)
                            .and_then(|(_, l, _)| *l)
                            .unwrap_or(f64::NAN);
                        assert_eq!(at, lower, "{tag}: {par} on its lower bound");
                    }
                    assert!(case.value(s, &format!("se.{par}")).is_none(), "{tag}");
                }
            }
        }
    }

    #[test]
    fn the_naive_fit_never_beats_the_reference_and_reaches_it() {
        let mut compared = 0usize;
        for name in list_fit_ml_cases().expect("list") {
            // Theoph is left to the reference alone: Nelder-Mead on twelve oral profiles with
            // flip-flop minima is not a quick, reliable cross-check
            if name.contains("theoph") || name.contains("kbound") || name.contains("collapse") {
                continue;
            }
            let case = load_fit_ml_case(&name).expect("case");
            for s in &case.subjects {
                let profile = case.profile(s).expect("profile");
                let (t, y) = case.fitted_observations(profile);
                let init = &case.initial_estimates[s];
                let theta0: Vec<f64> = case
                    .parameter_names()
                    .iter()
                    .take_while(|n| !n.starts_with("error"))
                    .map(|n| init[n])
                    .collect();
                let fit = naive_ml(
                    &case.model,
                    &case.error_model,
                    profile.dose,
                    &t,
                    &y,
                    &theta0,
                )
                .unwrap_or_else(|| panic!("{name} subject {s}: no finite start"));
                let reference = case.value(s, "m2ll").expect("m2ll");
                assert!(
                    fit.m2ll >= reference - 1e-9,
                    "{name} subject {s}: the naive fit {} is below the reference {reference}",
                    fit.m2ll
                );
                assert!(
                    AbsoluteTolerance::NAIVE_ML_M2LL.accepts(fit.m2ll, reference),
                    "{name} subject {s}: the naive fit {} is not within {:e} of the reference {reference}",
                    fit.m2ll,
                    AbsoluteTolerance::NAIVE_ML_M2LL.abs
                );
                compared += 1;
            }
        }
        assert!(compared >= 40, "{compared} fits compared");
    }
    #[test]
    fn the_refusals_carry_their_controls() {
        let errors = load_fit_ml_errors().expect("errors");
        // y = 0 at time 0 under a combined model: both forms, several subjects, additive control
        let unbounded: Vec<&MlErrorCase> = errors
            .iter()
            .filter(|c| c.case.starts_with("ml6e_"))
            .collect();
        assert!(unbounded.len() >= 4, "{} rows", unbounded.len());
        for em in ["combined", "combined_sd"] {
            assert!(
                unbounded
                    .iter()
                    .any(|c| c.options.get("error_model").and_then(Value::as_str) == Some(em)),
                "no unbounded-likelihood row for {em}"
            );
        }
        for c in &unbounded {
            assert_eq!(c.error.code, "unbounded_likelihood", "{}", c.case);
            assert_eq!(c.model, "pk1.oral_1", "{}", c.case);
            assert!(
                c.time
                    .iter()
                    .zip(&c.conc)
                    .any(|(&t, &y)| t == 0.0 && y == 0.0),
                "{}: no observation 0 at time 0",
                c.case
            );
            let control = c.control_options.as_ref().expect("control");
            assert_eq!(
                control.get("error_model").and_then(Value::as_str),
                Some("additive"),
                "{}",
                c.case
            );
        }
        // a refusal of `error_init` has a control with a valid `error_init`: same error model
        let init_rows: Vec<&MlErrorCase> = errors
            .iter()
            .filter(|c| c.case.starts_with("ml6d_error_init"))
            .collect();
        assert!(init_rows.len() >= 3, "{} rows", init_rows.len());
        for c in init_rows {
            let control = c.control_options.as_ref().expect("control");
            assert_eq!(control.get("error_model"), c.options.get("error_model"));
            let values = control
                .get("error_init")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("{}: no error_init in the control", c.case));
            let wanted = if control.get("error_model").and_then(Value::as_str) == Some("combined") {
                2
            } else {
                1
            };
            assert_eq!(values.len(), wanted, "{}", c.case);
            assert!(values.iter().all(|v| v.as_f64().is_some_and(|x| x > 0.0)));
        }
    }

    #[test]
    fn minus_two_log_likelihood_scales_as_two_n_ln_c() {
        // y and D multiplied by c: the predictions scale by c, sigma by c, the residual ratio is
        // unchanged, so -2LL changes by exactly 2 N ln c (OF-15, scaling invariance)
        let t = [0.5, 1.0, 2.0, 4.0, 8.0];
        let y = [9.31, 7.92, 6.85, 4.31, 2.11];
        for em in ML_ERROR_MODELS {
            let (theta, xi) = (
                vec![10.0, 0.2],
                if em == "additive" {
                    vec![0.3]
                } else if em == "proportional" {
                    vec![0.05]
                } else {
                    vec![0.1, 0.04]
                },
            );
            let c = 7.5_f64;
            let ys: Vec<f64> = y.iter().map(|x| x * c).collect();
            let xis: Vec<f64> = match em {
                "additive" => vec![xi[0] * c],
                "proportional" => xi.clone(),
                "combined" => vec![xi[0] * c, xi[1]],
                _ => vec![xi[0] * c, xi[1]],
            };
            let base = m2ll("pk1.iv_bolus", em, &theta, &xi, 100.0, &t, &y);
            let scaled = m2ll("pk1.iv_bolus", em, &theta, &xis, 100.0 * c, &t, &ys);
            let want = 2.0 * 5.0 * c.ln();
            assert!(
                (scaled - base - want).abs() < 1e-12,
                "{em}: {} against {want}",
                scaled - base
            );
        }
    }
}
