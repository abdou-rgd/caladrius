//! The minimiser (`specs/fit.md` section 3 to 5): Gauss-Newton direction from a QR factorisation
//! of the column-scaled weighted Jacobian, Hartley step halving, Levenberg-Marquardt damping when
//! halving fails, predicted-value weights recomputed at every iteration (FIT-WGT-03).

use std::collections::BTreeMap;

use crate::arith::{powi, sq};
use crate::linalg::{Mat, least_squares};
use crate::model::FitModel;
use crate::{Derivatives, FitError, FitInput, FitOptions, Weighting};

/// Canonical parameter order (MOD-VOC-01); other names follow in alphabetical order.
const ORDER: [&str; 6] = ["v", "cl", "k", "ka", "dur", "tlag"];
/// Smallest step factor of the halving (FIT-ALG-03).
pub(crate) const MIN_STEP: f64 = 1.0 / 1024.0;
/// Damping: first value, factor, largest value (FIT-ALG-04).
pub(crate) const LAMBDA_START: f64 = 1e-3;
pub(crate) const LAMBDA_FACTOR: f64 = 10.0;
pub(crate) const LAMBDA_MAX: f64 = 1e10;
/// Bounds of the options (a value beyond them is refused, never allocated or run).
const MAX_INCREMENT: f64 = 0.1;
const MAX_CONVERGENCE: f64 = 0.1;
const MAX_ITERATIONS: usize = 100_000;
const MAX_CURVE_POINTS: usize = 1_000_000;
/// A weighted sum of squares at or below this fraction of Σ w·y² is an exact fit (rounding level).
pub(crate) const EXACT_FIT: f64 = 1e-26;
/// Generated lower bound of a positive parameter, relative to its initial estimate.
const GENERATED_FLOOR: f64 = 1e-6;

/// 1/x^power when x > 0 and the weight is finite (x below about 1e-154 under 1/x² is not), else
/// `None` (FIT-WGT-02).
/// Generated lower bound (FIT-BND-03, `assumed`): parameters that must be positive stay above 1e-6
/// times their initial estimate, `tlag` stays >= 0, any other name is unbounded.
fn generated_lower(name: &str, initial: f64) -> f64 {
    match name {
        "v" | "cl" | "k" | "ka" | "dur" => GENERATED_FLOOR * initial,
        "tlag" => 0.0,
        _ => f64::NEG_INFINITY,
    }
}

pub(crate) fn weight_of(x: f64, power: i32) -> Option<f64> {
    let w = 1.0 / powi(x, power);
    (x > 0.0 && w.is_finite()).then_some(w)
}

/// A checked fitting problem.
pub(crate) struct Problem<M: FitModel> {
    pub model: M,
    pub model_id: String,
    pub dose: f64,
    pub time: Vec<f64>,
    pub y: Vec<f64>,
    pub weighting: Weighting,
    /// Fitted parameters, in canonical order.
    pub names: Vec<String>,
    pub theta0: Vec<f64>,
    pub options: FitOptions,
    /// Parameters held at a value.
    pub fixed: BTreeMap<String, f64>,
    /// Bounds of the fitted parameters (−∞, +∞ when absent).
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    /// The maximum-likelihood objective with its error model; `None` for least squares.
    pub ml: Option<crate::ml::Spec>,
}

fn invalid_option(option: &str, reason: &str) -> FitError {
    FitError::InvalidOption {
        option: option.to_string(),
        reason: reason.to_string(),
    }
}

impl<M: FitModel> Problem<M> {
    /// Checks the input (FIT-ERR-01, WGT-02) and the initial estimates.
    pub fn new(model: M, input: &FitInput) -> Result<Self, FitError> {
        let o = &input.options;
        let ml = crate::ml::spec(o, input.weighting)?;
        if !(o.increment > 0.0 && o.increment <= MAX_INCREMENT) {
            return Err(invalid_option(
                "increment",
                "must be a number > 0 and <= 0.1 (0.001 is usual)",
            ));
        }
        if !(o.convergence >= 0.0 && o.convergence <= MAX_CONVERGENCE) {
            return Err(invalid_option(
                "convergence",
                "must be a number >= 0 and <= 0.1 (0.0001 is usual)",
            ));
        }
        if o.max_iterations > MAX_ITERATIONS {
            return Err(invalid_option(
                "max_iterations",
                "must be at most 100000 (50 is usual)",
            ));
        }
        let f = &o.flags;
        let checks = [
            ("flags.max_cv_percent", f.max_cv_percent, 0.0, f64::MAX),
            ("flags.max_abs_correlation", f.max_abs_correlation, 0.0, 1.0),
            (
                "flags.max_condition_number",
                f.max_condition_number,
                1.0,
                f64::MAX,
            ),
        ];
        for (option, value, lo, hi) in checks {
            if let Some(x) = value {
                if !(x >= lo && x <= hi) {
                    return Err(invalid_option(
                        option,
                        &format!(
                            "{x} must be a finite number between {lo} and {hi}, or null to turn the flag off"
                        ),
                    ));
                }
            }
        }
        if o.n_curve == 1 || o.n_curve > MAX_CURVE_POINTS {
            return Err(invalid_option(
                "n_curve",
                "must be 0 (no curve) or between 2 and 1000000 (1000 is usual)",
            ));
        }
        if !(o.confidence_level > 0.0 && o.confidence_level < 1.0) {
            return Err(invalid_option(
                "confidence_level",
                "must lie strictly between 0 and 1 (0.95 is usual)",
            ));
        }
        if input.time.len() != input.conc.len() {
            return Err(FitError::LengthMismatch {
                times: input.time.len(),
                concs: input.conc.len(),
            });
        }
        let mut names: Vec<String> = ORDER
            .iter()
            .filter(|n| input.initial.contains_key(**n))
            .map(|n| (*n).to_string())
            .collect();
        names.extend(
            input
                .initial
                .keys()
                .filter(|k| !ORDER.contains(&k.as_str()))
                .cloned(),
        );
        if names.is_empty() {
            return Err(FitError::NoParameters);
        }
        let (n, p) = (input.time.len(), names.len());
        match &ml {
            // FIT-ML-14a: the error is estimated too, so N must exceed P + Q.
            Some(spec) if n <= p + spec.error_model.q() => {
                return Err(FitError::TooFewObservations {
                    n,
                    p,
                    q: Some(spec.error_model.q()),
                    error_model: Some(spec.error_model.id().to_string()),
                });
            }
            None if n < p => {
                return Err(FitError::TooFewObservations {
                    n,
                    p,
                    q: None,
                    error_model: None,
                });
            }
            _ => {}
        }
        for (index, (&t, &c)) in input.time.iter().zip(&input.conc).enumerate() {
            if !t.is_finite() {
                return Err(FitError::NonFiniteObservation {
                    index,
                    field: "time".to_string(),
                });
            }
            if !c.is_finite() {
                return Err(FitError::NonFiniteObservation {
                    index,
                    field: "concentration".to_string(),
                });
            }
            if t < 0.0 {
                return Err(FitError::TimeBeforeDose { index, time: t });
            }
            let observed_weight = match input.weighting {
                Weighting::InvY => weight_of(c, 1),
                Weighting::InvY2 => weight_of(c, 2),
                _ => Some(1.0),
            };
            if observed_weight.is_none() {
                return Err(FitError::UnweightableObservation {
                    index,
                    time: t,
                    value: c,
                    weighting: input.weighting.id().to_string(),
                });
            }
        }
        if !(input.dose.is_finite() && input.dose > 0.0) {
            return Err(FitError::InvalidDose { value: input.dose });
        }
        let theta0: Vec<f64> = names
            .iter()
            .map(|n| input.initial.get(n).copied().unwrap_or(f64::NAN))
            .collect();
        let fixed = &o.fixed;
        if let Some((name, value)) = fixed
            .iter()
            .find(|(n, v)| input.initial.contains_key(*n) || !v.is_finite())
        {
            return Err(invalid_option(
                "fixed",
                &format!(
                    "`{name}` = {value} must be finite and must not also have an initial estimate (a parameter is either fitted or fixed)"
                ),
            ));
        }
        if let Some(name) = o.bounds.keys().find(|n| !names.contains(n)) {
            return Err(invalid_option(
                "bounds",
                &format!(
                    "`{name}` is not a fitted parameter; give bounds for fitted parameters only"
                ),
            ));
        }
        let (mut lower, mut upper) = (Vec::new(), Vec::new());
        for (name, &x) in names.iter().zip(&theta0) {
            let user = o.bounds.get(name).copied().unwrap_or_default();
            let lo = user.lower.unwrap_or_else(|| generated_lower(name, x));
            let hi = user.upper.unwrap_or(f64::INFINITY);
            if lo.is_nan()
                || hi.is_nan()
                || lo >= hi
                || lo == f64::INFINITY
                || hi == f64::NEG_INFINITY
            {
                return Err(invalid_option(
                    "bounds",
                    &format!("`{name}`: the lower bound {lo} must be below the upper bound {hi}"),
                ));
            }
            if !(x >= lo && x <= hi) {
                return Err(FitError::InitialOutsideBounds {
                    parameter: name.clone(),
                    value: x,
                    lower: lo,
                    upper: hi,
                });
            }
            lower.push(lo);
            upper.push(hi);
        }
        let mut problem = Self {
            model,
            model_id: input.model.id().to_string(),
            dose: input.dose,
            time: input.time.clone(),
            y: input.conc.clone(),
            weighting: input.weighting,
            names,
            theta0,
            fixed: fixed.clone(),
            lower,
            upper,
            ml,
            options: o.clone(),
        };
        let pred = problem
            .model
            .predict(
                problem.dose,
                &problem.params(&problem.theta0),
                &problem.time,
            )
            .map_err(|source| FitError::InitialEstimates { source })?;
        let unusable = pred.iter().position(|&f| {
            !f.is_finite()
                || match problem.weighting {
                    Weighting::InvYhat => weight_of(f, 1).is_none(),
                    Weighting::InvYhat2 => weight_of(f, 2).is_none(),
                    _ => false,
                }
        });
        if let Some(index) = unusable {
            return Err(FitError::UnusableInitialPrediction {
                index,
                time: problem.time.get(index).copied().unwrap_or(f64::NAN),
            });
        }
        // FIT-JAC-01, JAC-02: `analytic` is refused without closed forms (never replaced); `auto`
        // is resolved here, once, so the iterations and the result use one method.
        let start = problem.params(&problem.theta0);
        let closed_forms = problem
            .model
            .analytic_derivatives(problem.dose, &start, &problem.time, &problem.names)
            .is_some();
        let regimen = start
            .keys()
            .any(|name| caladrius_models::is_regimen_parameter(name));
        problem.options.derivatives = match (o.derivatives, closed_forms) {
            (Derivatives::Analytic, false) if regimen => {
                return Err(FitError::RegimenDerivativesUnavailable {
                    model: problem.model_id.clone(),
                });
            }
            (Derivatives::Analytic, false) => {
                return Err(FitError::AnalyticDerivativesUnavailable {
                    model: problem.model_id.clone(),
                });
            }
            (Derivatives::Auto, true) | (Derivatives::Analytic, true) => Derivatives::Analytic,
            (Derivatives::Auto, false) | (Derivatives::ForwardDifference, _) => {
                Derivatives::ForwardDifference
            }
        };
        Ok(problem)
    }

    /// Fixed and fitted parameters together.
    pub fn params(&self, theta: &[f64]) -> BTreeMap<String, f64> {
        let mut all = self.fixed.clone();
        all.extend(self.names.iter().cloned().zip(theta.iter().copied()));
        all
    }

    /// θ projected on the bounds.
    pub fn clamp(&self, theta: &[f64]) -> Vec<f64> {
        theta
            .iter()
            .zip(self.lower.iter().zip(&self.upper))
            .map(|(x, (lo, hi))| x.max(*lo).min(*hi))
            .collect()
    }

    /// Which parameters sit exactly on a bound.
    pub fn on_bound(&self, theta: &[f64]) -> Vec<bool> {
        theta
            .iter()
            .zip(self.lower.iter().zip(&self.upper))
            .map(|(x, (lo, hi))| x <= lo || x >= hi)
            .collect()
    }

    /// Predictions, or `None` when the model refuses the parameters or gives a non-finite value.
    pub fn predict_at(&self, theta: &[f64], times: &[f64]) -> Option<Vec<f64>> {
        if theta.iter().any(|x| !x.is_finite()) {
            return None;
        }
        let pred = self
            .model
            .predict(self.dose, &self.params(theta), times)
            .ok()?;
        pred.iter().all(|f| f.is_finite()).then_some(pred)
    }

    pub fn predict(&self, theta: &[f64]) -> Option<Vec<f64>> {
        self.predict_at(theta, &self.time)
    }

    /// Weights for the predictions `pred` (FIT-WGT-01); `None` when a predicted-value weight is
    /// undefined (ŷ <= 0).
    pub fn weights(&self, pred: &[f64]) -> Option<Vec<f64>> {
        let from = weight_of;
        match self.weighting {
            Weighting::Uniform => Some(vec![1.0; self.y.len()]),
            Weighting::InvY => self.y.iter().map(|&y| from(y, 1)).collect(),
            Weighting::InvY2 => self.y.iter().map(|&y| from(y, 2)).collect(),
            Weighting::InvYhat => pred.iter().map(|&f| from(f, 1)).collect(),
            Weighting::InvYhat2 => pred.iter().map(|&f| from(f, 2)).collect(),
        }
    }

    pub fn wrss(&self, pred: &[f64], w: &[f64]) -> f64 {
        self.y
            .iter()
            .zip(pred)
            .zip(w)
            .map(|((y, f), w)| w * sq(y - f))
            .sum()
    }

    /// ∂f(t_i)/∂θ_j (unweighted), one column per parameter (FIT-JAC-01, JAC-02).
    pub fn jacobian(&self, theta: &[f64], pred: &[f64]) -> Option<Vec<Vec<f64>>> {
        if self.options.derivatives == Derivatives::Analytic {
            let columns = self.model.analytic_derivatives(
                self.dose,
                &self.params(theta),
                &self.time,
                &self.names,
            )?;
            return columns
                .iter()
                .all(|c| c.iter().all(|x| x.is_finite()))
                .then_some(columns);
        }
        let h = self.options.increment;
        let mut columns = Vec::with_capacity(theta.len());
        for (j, &x) in theta.iter().enumerate() {
            let step = if x == 0.0 { h } else { h * x.abs() };
            let mut moved = theta.to_vec();
            if let Some(slot) = moved.get_mut(j) {
                *slot = x + step;
            }
            let shifted = self.predict(&moved)?;
            columns.push(
                shifted
                    .iter()
                    .zip(pred)
                    .map(|(a, b)| (a - b) / step)
                    .collect(),
            );
        }
        Some(columns)
    }

    /// The weighted Jacobian with unit-length columns, the column norms, and the weighted
    /// residuals. `None` when a column is zero or not finite.
    pub fn scaled_system(
        &self,
        columns: &[Vec<f64>],
        pred: &[f64],
        w: &[f64],
    ) -> Option<(Mat, Vec<f64>, Vec<f64>)> {
        let sqrt_w: Vec<f64> = w.iter().map(|x| x.sqrt()).collect();
        let weighted: Vec<Vec<f64>> = columns
            .iter()
            .map(|c| c.iter().zip(&sqrt_w).map(|(d, s)| d * s).collect())
            .collect();
        let a = Mat::from_columns(&weighted);
        let norms: Vec<f64> = (0..a.cols()).map(|j| a.column_norm(j)).collect();
        if norms.iter().any(|c| !(c.is_finite() && *c > 0.0)) {
            return None;
        }
        let mut scaled = a.clone();
        for (j, c) in norms.iter().enumerate() {
            for i in 0..a.rows() {
                scaled.set(i, j, a.at(i, j) / c);
            }
        }
        let r: Vec<f64> = self
            .y
            .iter()
            .zip(pred)
            .zip(&sqrt_w)
            .map(|((y, f), s)| s * (y - f))
            .collect();
        Some((scaled, norms, r))
    }

    /// The Gauss-Newton direction on the parameters `free` (indices), scattered back to all
    /// parameters (0 for the others), with the decrease it predicts (‖J·δ‖²); `lambda` > 0 gives
    /// the damped direction. `None` when the free system is singular.
    pub fn direction(
        &self,
        columns: &[Vec<f64>],
        pred: &[f64],
        w: &[f64],
        free: &[usize],
        lambda: f64,
    ) -> Option<(Vec<f64>, f64)> {
        let subset: Vec<Vec<f64>> = free
            .iter()
            .filter_map(|&j| columns.get(j).cloned())
            .collect();
        let (a, norms, r) = self.scaled_system(&subset, pred, w)?;
        let scaled = if lambda == 0.0 {
            least_squares(&a, &r).map(|(d, _)| d)?
        } else {
            damped(&a, &r, lambda)?
        };
        let predicted: f64 = (0..a.rows())
            .map(|i| {
                sq((0..a.cols())
                    .map(|j| a.at(i, j) * scaled.get(j).copied().unwrap_or(0.0))
                    .sum::<f64>())
            })
            .sum();
        let mut delta = vec![0.0; columns.len()];
        for ((&j, d), c) in free.iter().zip(&scaled).zip(&norms) {
            if let Some(slot) = delta.get_mut(j) {
                *slot = d / c;
            }
        }
        Some((delta, predicted))
    }

    /// WRSS with the weights of the predictions themselves (the trace column); NaN when they are
    /// undefined.
    pub fn wrss_own(&self, pred: &[f64]) -> f64 {
        self.weights(pred).map_or(f64::NAN, |w| self.wrss(pred, &w))
    }
}

/// Damped direction: least squares of [A; √λ·I]·δ ≈ [r; 0] on the scaled columns, which is
/// Marquardt's (JᵀJ + λ·diag(JᵀJ))·δ = Jᵀr in the original scale (FIT-ALG-02, ALG-04).
pub(crate) fn damped(a: &Mat, r: &[f64], lambda: f64) -> Option<Vec<f64>> {
    let (m, n) = (a.rows(), a.cols());
    let mut aug = Mat::zeros(m + n, n);
    for i in 0..m {
        for j in 0..n {
            aug.set(i, j, a.at(i, j));
        }
    }
    for j in 0..n {
        aug.set(m + j, j, lambda.sqrt());
    }
    let mut rhs = r.to_vec();
    rhs.extend(std::iter::repeat_n(0.0, n));
    least_squares(&aug, &rhs).map(|(d, _)| d)
}
