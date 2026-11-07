//! The maximum-likelihood iteration (`specs/fit.md` section 13, FIT-ML-04 to 10, 14, 15): Fisher
//! scoring written as a Gauss-Newton step on the stacked residual vector of FIT-ML-06, with the
//! step control of FIT-ML-07, the Newton-decrement test of FIT-ML-08 and the start of FIT-ML-10.
//!
//! [`MlRun`] is the state that the stepper carries for `objective = ml`: the iterate with
//! everything computed at it (predictions, σ, −2LL, the stacked system, the direction and the
//! decrement), the damping, and the names and values shown to an observer. One call of
//! [`MlRun::iterate`] is one accepted iteration, as for least squares.

use crate::arith::sq;
use crate::engine::{LAMBDA_FACTOR, LAMBDA_MAX, LAMBDA_START, MIN_STEP, Problem, damped};
use crate::linalg::{Mat, least_squares};
use crate::model::FitModel;
use crate::objective::{ErrorModel, Objective};
use crate::result::TraceRow;
use crate::stepper::Advance;
use crate::{Criterion, FitError, FitInput, FitOptions, FitRun, FitStatus, Weighting};

/// The maximum-likelihood part of a checked problem.
#[derive(Debug, Clone)]
pub(crate) struct Spec {
    pub error_model: ErrorModel,
    /// Initial error parameters in the user's units (FIT-ML-10), validated.
    pub error_init: Option<Vec<f64>>,
}

fn invalid_option(option: &str, reason: &str) -> FitError {
    FitError::InvalidOption {
        option: option.to_string(),
        reason: reason.to_string(),
    }
}

/// The options of `objective` as a [`Spec`] (`None` for least squares), or the refusal of
/// FIT-ML-01 and FIT-ML-14c.
pub(crate) fn spec(o: &FitOptions, weighting: Weighting) -> Result<Option<Spec>, FitError> {
    match o.objective {
        Objective::Wls => {
            if o.error_model.is_some() {
                return Err(invalid_option(
                    "error_model",
                    "only used with `objective = ml`; remove it, or set the objective to `ml`",
                ));
            }
            if o.error_init.is_some() {
                return Err(invalid_option(
                    "error_init",
                    "only used with `objective = ml`; remove it, or set the objective to `ml`",
                ));
            }
            Ok(None)
        }
        Objective::Ml => {
            let Some(error_model) = o.error_model else {
                return Err(invalid_option(
                    "error_model",
                    "the maximum-likelihood fit needs an error model, and the choice changes the answer: use `additive`, `proportional`, `combined` or `combined_sd`",
                ));
            };
            if weighting != Weighting::Uniform {
                return Err(invalid_option(
                    "weighting",
                    "the maximum-likelihood fit derives its weights from the error model; remove the weighting option",
                ));
            }
            if let Some(init) = &o.error_init {
                let q = error_model.q();
                if init.len() != q {
                    return Err(invalid_option(
                        "error_init",
                        &format!(
                            "the {} error model has {q} error parameter(s) ({}), {} value(s) were given",
                            error_model.id(),
                            error_model.names().join(", "),
                            init.len()
                        ),
                    ));
                }
                if let Some(bad) = init.iter().find(|x| !(x.is_finite() && **x > 0.0)) {
                    return Err(invalid_option(
                        "error_init",
                        &format!("{bad} must be a finite number > 0 (the error parameters are standard deviations or fractions of the prediction)"),
                    ));
                }
            }
            Ok(Some(Spec {
                error_model,
                error_init: o.error_init.clone(),
            }))
        }
    }
}

/// ε of the exact-fit test of FIT-ML-15, relative to the natural scale of the moved variable.
const EXACT_FIT_SCALE: f64 = 1e-26;
/// The least-squares fit that gives the start has this iteration limit and this criterion.
const START_MAX_ITERATIONS: usize = 200;
const START_CONVERGENCE: f64 = 1e-10;

// ---------------------------------------------------------------- evaluation

/// Everything that is cheap to know at a point ζ = (θ, ξ).
pub(crate) struct Eval {
    pub zeta: Vec<f64>,
    pub pred: Vec<f64>,
    pub sigma: Vec<f64>,
    pub m2ll: f64,
}

/// −2LL and its parts at ζ (FIT-ML-04); `None` when a prediction is not finite or a σ is not
/// finite and strictly positive (the trial is then rejected, FIT-ML-07).
pub(crate) fn evaluate<M: FitModel>(
    problem: &Problem<M>,
    error_model: ErrorModel,
    zeta: &[f64],
) -> Option<Eval> {
    let (theta, xi) = zeta.split_at(problem.names.len().min(zeta.len()));
    let pred = problem.predict(theta)?;
    let ln_2pi = libm::log(2.0 * std::f64::consts::PI);
    let mut sigma = Vec::with_capacity(pred.len());
    let mut m2ll = 0.0;
    for (&y, &f) in problem.y.iter().zip(&pred) {
        let s = error_model.sigma(xi, f);
        if !(s.is_finite() && s > 0.0) {
            return None;
        }
        m2ll += ln_2pi + 2.0 * libm::log(s) + sq((y - f) / s);
        sigma.push(s);
    }
    m2ll.is_finite().then(|| Eval {
        zeta: zeta.to_vec(),
        pred,
        sigma,
        m2ll,
    })
}

/// The stacked system of FIT-ML-06: unscaled columns of 𝒥 (2N rows each) and the stacked residual.
pub(crate) struct System {
    pub cols: Vec<Vec<f64>>,
    pub rhs: Vec<f64>,
}

fn system(
    error_model: ErrorModel,
    p: usize,
    q: usize,
    ev: &Eval,
    jacobian: &[Vec<f64>],
    y: &[f64],
) -> Option<System> {
    let n_obs = ev.pred.len();
    let xi = ev.zeta.get(p..)?;
    let root2 = 2.0_f64.sqrt();
    let dly: Vec<f64> = ev
        .pred
        .iter()
        .zip(&ev.sigma)
        .map(|(&f, &s)| error_model.dln_sigma_dyhat(xi, f, s))
        .collect();
    let mut cols = Vec::with_capacity(p + q);
    for j in 0..p {
        let col = jacobian.get(j)?;
        if col.len() != n_obs {
            return None;
        }
        let mut c = Vec::with_capacity(2 * n_obs);
        c.extend(col.iter().zip(&ev.sigma).map(|(d, s)| d / s));
        c.extend(col.iter().zip(&dly).map(|(d, l)| root2 * l * d));
        cols.push(c);
    }
    for k in 0..q {
        let mut c = vec![0.0; n_obs];
        c.extend(
            ev.pred
                .iter()
                .zip(&ev.sigma)
                .map(|(&f, &s)| root2 * error_model.dln_sigma_dxi(xi, f, s, k)),
        );
        cols.push(c);
    }
    let z: Vec<f64> = y
        .iter()
        .zip(&ev.pred)
        .zip(&ev.sigma)
        .map(|((&y, &f), &s)| (y - f) / s)
        .collect();
    let mut rhs = z.clone();
    rhs.extend(z.iter().map(|z| (z * z - 1.0) / root2));
    let finite = cols.iter().all(|c| c.iter().all(|x| x.is_finite()))
        && rhs.iter().all(|x| x.is_finite());
    finite.then_some(System { cols, rhs })
}

/// The columns `free` of the system with unit length, and their norms; `None` when a column is
/// zero or not finite.
pub(crate) fn scaled(sys: &System, free: &[usize]) -> Option<(Mat, Vec<f64>)> {
    let subset: Vec<Vec<f64>> = free
        .iter()
        .filter_map(|&j| sys.cols.get(j).cloned())
        .collect();
    if subset.is_empty() || subset.len() != free.len() {
        return None;
    }
    let a = Mat::from_columns(&subset);
    let norms: Vec<f64> = (0..a.cols()).map(|j| a.column_norm(j)).collect();
    if norms.iter().any(|c| !(c.is_finite() && *c > 0.0)) {
        return None;
    }
    let mut unit = a.clone();
    for (j, c) in norms.iter().enumerate() {
        for i in 0..a.rows() {
            unit.set(i, j, a.at(i, j) / c);
        }
    }
    Some((unit, norms))
}

/// The Fisher-scoring direction over the variables `free`, scattered to all `n` variables (0 for
/// the others), with ‖A·x‖², which for λ = 0 is the Newton decrement Δ = gᵀI⁻¹g (FIT-ML-08).
/// `None` when the free system is singular.
fn direction(sys: &System, n: usize, free: &[usize], lambda: f64) -> Option<(Vec<f64>, f64)> {
    let (a, norms) = scaled(sys, free)?;
    let x = if lambda == 0.0 {
        least_squares(&a, &sys.rhs).map(|(d, _)| d)?
    } else {
        damped(&a, &sys.rhs, lambda)?
    };
    let explained: f64 = (0..a.rows())
        .map(|i| {
            sq((0..a.cols())
                .map(|j| a.at(i, j) * x.get(j).copied().unwrap_or(0.0))
                .sum::<f64>())
        })
        .sum();
    let mut delta = vec![0.0; n];
    for ((&j, d), c) in free.iter().zip(&x).zip(&norms) {
        if let Some(slot) = delta.get_mut(j) {
            *slot = d / c;
        }
    }
    Some((delta, explained))
}

/// The iterate and what is computed at it.
pub(crate) struct Point {
    pub ev: Eval,
    pub sys: System,
    /// Variables not held on a bound (FIT-BND-01, FIT-ML-05).
    pub free: Vec<usize>,
    /// The undamped direction over `free` and Δ; `None` when singular.
    pub gn: Option<(Vec<f64>, f64)>,
    /// The whole system is singular here (a zero column).
    pub singular: bool,
}

impl Point {
    pub(crate) fn decrement(&self) -> Option<f64> {
        self.gn.as_ref().map(|(_, d)| *d)
    }
}

/// Bounds of ζ: those of θ, then 0 and ∞ for the moved error variables.
struct Frame {
    p: usize,
    q: usize,
    lower: Vec<f64>,
    upper: Vec<f64>,
}

impl Frame {
    fn project(&self, zeta: &[f64]) -> Vec<f64> {
        zeta.iter()
            .zip(self.lower.iter().zip(&self.upper))
            .map(|(x, (lo, hi))| x.max(*lo).min(*hi))
            .collect()
    }

    /// The active-set rule of FIT-BND-01: a variable on a bound whose step points outward is held.
    fn free(&self, zeta: &[f64], delta: &[f64]) -> Vec<usize> {
        (0..self.p + self.q)
            .filter(|&j| {
                let x = zeta.get(j).copied().unwrap_or(0.0);
                let d = delta.get(j).copied().unwrap_or(0.0);
                let lo = self.lower.get(j).copied().unwrap_or(f64::NEG_INFINITY);
                let hi = self.upper.get(j).copied().unwrap_or(f64::INFINITY);
                !((x <= lo && d < 0.0) || (x >= hi && d > 0.0))
            })
            .collect()
    }
}

fn point<M: FitModel>(problem: &Problem<M>, frame: &Frame, error_model: ErrorModel, ev: Eval) -> Option<Point> {
    let theta = ev.zeta.get(..frame.p)?;
    let jacobian = problem.jacobian(theta, &ev.pred)?;
    let sys = system(error_model, frame.p, frame.q, &ev, &jacobian, &problem.y)?;
    let n = frame.p + frame.q;
    let all: Vec<usize> = (0..n).collect();
    let full = direction(&sys, n, &all, 0.0);
    let free = match &full {
        Some((delta, _)) => frame.free(&ev.zeta, delta),
        None => all.clone(),
    };
    let gn = if free.len() == n {
        full
    } else if free.is_empty() {
        // Everything is held on a bound: nothing can move, the point is stationary.
        Some((vec![0.0; n], 0.0))
    } else {
        direction(&sys, n, &free, 0.0)
    };
    let singular = gn.is_none() && scaled(&sys, &all).is_none();
    Some(Point {
        ev,
        sys,
        free,
        gn,
        singular,
    })
}

// ---------------------------------------------------------------- the run

/// The state of an `objective = ml` fit between two iterations.
pub(crate) struct MlRun {
    pub spec: Spec,
    frame: Frame,
    /// Names shown to an observer and in the result: θ, then the error parameters.
    pub names: Vec<String>,
    /// Their values at the iterate (error parameters in the user's units).
    pub values: Vec<f64>,
    pub point: Point,
    /// Damping carried to the next iteration (FIT-ALG-04).
    lambda: f64,
    eps: f64,
    /// The estimate of the least-squares fit that gave the start (FIT-ML-20).
    pub start: Vec<f64>,
}

impl MlRun {
    /// ε of the test Δ ≤ ε holds at the iterate.
    pub(crate) fn converged(&self) -> bool {
        self.point.decrement().is_some_and(|d| d <= self.eps)
    }

    pub(crate) fn error_model(&self) -> ErrorModel {
        self.spec.error_model
    }

    pub(crate) fn p(&self) -> usize {
        self.frame.p
    }

    pub(crate) fn q(&self) -> usize {
        self.frame.q
    }

    pub(crate) fn lower(&self) -> &[f64] {
        &self.frame.lower
    }

    pub(crate) fn upper(&self) -> &[f64] {
        &self.frame.upper
    }

    /// Residual sum of squares with uniform weights at the iterate.
    fn rss<M: FitModel>(&self, problem: &Problem<M>) -> f64 {
        problem
            .y
            .iter()
            .zip(&self.point.ev.pred)
            .map(|(y, f)| sq(y - f))
            .sum()
    }

    fn values_at(&self, zeta: &[f64]) -> Vec<f64> {
        let (theta, xi) = zeta.split_at(self.frame.p.min(zeta.len()));
        let mut v = theta.to_vec();
        v.extend(self.spec.error_model.user_values(xi));
        v
    }

    fn trace_row<M: FitModel>(
        &self,
        problem: &Problem<M>,
        iteration: usize,
        step: Option<f64>,
        lambda: f64,
        halvings: usize,
    ) -> TraceRow {
        TraceRow {
            iteration,
            wrss: self.rss(problem),
            estimates: self.values.clone(),
            step,
            lambda,
            halvings,
            relative_decrease: None,
            m2ll: Some(self.point.ev.m2ll),
            decrement: self.point.decrement(),
        }
    }

    /// FIT-ML-10: the least-squares fits, the error parameters, the better start; the refusals of
    /// FIT-ML-14 and FIT-ML-15 that come before the first iteration.
    pub(crate) fn start<M: FitModel>(
        problem: &Problem<M>,
        spec: &Spec,
        input: &FitInput,
    ) -> Result<(Self, TraceRow), FitError> {
        let em = spec.error_model;
        let (p, q) = (problem.names.len(), em.q());
        refuse_unbounded(problem, em)?;
        let first_time = problem.time.first().copied().unwrap_or(f64::NAN);

        // Candidate 1: uniform least squares; candidate 2: weights 1/y² when every y > 0.
        let uniform = least_squares_estimate(problem, input, Weighting::Uniform)?;
        let pred = problem
            .predict(&uniform)
            .ok_or(FitError::UnusableInitialPrediction {
                index: 0,
                time: first_time,
            })?;
        let rss: f64 = problem.y.iter().zip(&pred).map(|(y, f)| sq(y - f)).sum();
        let ss_y: f64 = problem.y.iter().map(|y| y * y).sum();
        if rss <= EXACT_FIT_SCALE * ss_y {
            return Err(FitError::ExactFit);
        }
        let mut candidates = vec![(uniform, pred)];
        if em != ErrorModel::Additive && problem.y.iter().all(|&y| y > 0.0) {
            if let Ok(theta) = least_squares_estimate(problem, input, Weighting::InvY2) {
                if let Some(pred) = problem.predict(&theta) {
                    candidates.push((theta, pred));
                }
            }
        }

        let mut best: Option<Eval> = None;
        for (index, (theta, pred)) in candidates.iter().enumerate() {
            let xi = initial_xi(em, &problem.y, pred, spec.error_init.as_deref());
            if index == 0 {
                // FIT-ML-14b: a standard deviation that is 0 or not finite at the start.
                let bad = problem
                    .y
                    .iter()
                    .zip(pred)
                    .position(|(_, &f)| !em.sigma(&xi, f).is_finite() || em.sigma(&xi, f) <= 0.0);
                if let Some(i) = bad {
                    return Err(FitError::NonPositiveSigma {
                        error_model: em.id().to_string(),
                        index: i,
                        time: problem.time.get(i).copied().unwrap_or(f64::NAN),
                        predicted: pred.get(i).copied().unwrap_or(f64::NAN),
                    });
                }
            }
            let mut zeta = theta.clone();
            zeta.extend(xi);
            let zeta = clamp_start(problem, &zeta, p);
            if let Some(ev) = evaluate(problem, em, &zeta) {
                // A tie goes to the first candidate.
                if best.as_ref().is_none_or(|b| ev.m2ll < b.m2ll) {
                    best = Some(ev);
                }
            }
        }
        let Some(ev) = best else {
            return Err(FitError::UnusableInitialPrediction {
                index: 0,
                time: first_time,
            });
        };
        let start = candidates
            .first()
            .map(|(theta, _)| theta.clone())
            .unwrap_or_default();

        let mut lower = problem.lower.clone();
        let mut upper = problem.upper.clone();
        lower.extend(std::iter::repeat_n(0.0, q));
        upper.extend(std::iter::repeat_n(f64::INFINITY, q));
        let frame = Frame { p, q, lower, upper };
        let Some(point) = point(problem, &frame, em, ev) else {
            return Err(FitError::UnusableInitialPrediction {
                index: 0,
                time: first_time,
            });
        };
        let mut names = problem.names.clone();
        names.extend(em.names().iter().map(|n| (*n).to_string()));
        let mut run = Self {
            spec: spec.clone(),
            frame,
            names,
            values: Vec::new(),
            point,
            lambda: 0.0,
            eps: problem.options.convergence,
            start,
        };
        run.values = run.values_at(&run.point.ev.zeta);
        let row = run.trace_row(problem, 0, None, 0.0, 0);
        Ok((run, row))
    }

    /// One accepted iteration (FIT-ML-07): the direction, the halving of ν, the damping when
    /// halving fails; then the point, the trace row and the test Δ ≤ ε at the new iterate.
    pub(crate) fn iterate<M: FitModel>(
        &mut self,
        problem: &Problem<M>,
        iteration: usize,
        trace: &mut Vec<TraceRow>,
    ) -> Advance {
        if self.point.singular {
            return Advance::Stopped(FitStatus::Singular);
        }
        let em = self.spec.error_model;
        let n = self.frame.p + self.frame.q;
        let current = self.point.ev.m2ll;
        let mut accepted: Option<(Eval, f64, usize)> = None;
        let mut lam = self.lambda;
        loop {
            let direction = if lam == 0.0 {
                self.point.gn.as_ref().map(|(d, _)| d.clone())
            } else {
                direction(&self.point.sys, n, &self.point.free, lam).map(|(d, _)| d)
            };
            if let Some(delta) = direction {
                let mut nu = 1.0;
                let mut halvings = 0;
                while nu >= MIN_STEP {
                    let trial = self.frame.project(
                        &self
                            .point
                            .ev
                            .zeta
                            .iter()
                            .zip(&delta)
                            .map(|(z, d)| z + nu * d)
                            .collect::<Vec<f64>>(),
                    );
                    if let Some(ev) = evaluate(problem, em, &trial) {
                        if ev.m2ll < current {
                            accepted = Some((ev, nu, halvings));
                            break;
                        }
                    }
                    nu /= 2.0;
                    halvings += 1;
                }
                if accepted.is_some() {
                    break;
                }
            }
            lam = if lam == 0.0 {
                LAMBDA_START
            } else {
                lam * LAMBDA_FACTOR
            };
            if lam > LAMBDA_MAX {
                break;
            }
        }
        let Some((ev, nu, halvings)) = accepted else {
            return Advance::Stopped(FitStatus::NoDecrease);
        };
        // FIT-ML-15: the moved variable of a one-parameter error model has fallen to rounding
        // level of its natural scale, the likelihood has no maximum. There is no error channel in
        // a step, so the run stops as a fit that went non-finite (reported in the thread).
        if self.frame.q == 1 {
            let xi = ev.zeta.get(self.frame.p).copied().unwrap_or(f64::NAN);
            let natural = if em == ErrorModel::Additive {
                problem.y.iter().map(|y| y * y).sum::<f64>() / problem.y.len().max(1) as f64
            } else {
                1.0
            };
            if xi <= EXACT_FIT_SCALE * natural {
                return Advance::Stopped(FitStatus::NonFinite);
            }
        }
        let Some(point) = point(problem, &self.frame, em, ev) else {
            return Advance::Stopped(FitStatus::NonFinite);
        };
        self.lambda = if lam <= LAMBDA_START {
            0.0
        } else {
            lam / LAMBDA_FACTOR
        };
        self.point = point;
        self.values = self.values_at(&self.point.ev.zeta);
        trace.push(self.trace_row(problem, iteration, Some(nu), lam, halvings));
        Advance::Committed {
            converged: self.converged(),
        }
    }
}

/// θ on its bounds (the least-squares fit already respects them; this is for the rounding of a
/// value that sits on one).
fn clamp_start<M: FitModel>(problem: &Problem<M>, zeta: &[f64], p: usize) -> Vec<f64> {
    let (theta, xi) = zeta.split_at(p.min(zeta.len()));
    let mut out = problem.clamp(theta);
    out.extend_from_slice(xi);
    out
}

/// Moment estimates of the moved error variables (FIT-ML-10), or the user's `error_init`.
fn initial_xi(error_model: ErrorModel, y: &[f64], pred: &[f64], init: Option<&[f64]>) -> Vec<f64> {
    if let Some(user) = init {
        return error_model.moved_from_user(user);
    }
    let n = y.len().max(1) as f64;
    let a0 = y.iter().zip(pred).map(|(y, f)| sq(y - f)).sum::<f64>() / n;
    let b0 = y
        .iter()
        .zip(pred)
        .map(|(y, f)| {
            let m2 = (f * f + y * y) / 2.0;
            if m2 > 0.0 { sq(y - f) / m2 } else { 0.0 }
        })
        .sum::<f64>()
        / n;
    match error_model {
        ErrorModel::Additive => vec![a0],
        ErrorModel::Proportional => vec![b0],
        ErrorModel::Combined => vec![a0 / 2.0, b0 / 2.0],
        ErrorModel::CombinedSd => vec![a0.sqrt() / 2.0, b0.sqrt() / 2.0],
    }
}

/// The estimate of θ of the least-squares fit of sections 3 to 8 with `weighting`, from the
/// initial estimates of the problem (FIT-ML-10). The nested fit goes through a trait object so
/// that the generic code does not instantiate itself without end.
fn least_squares_estimate<M: FitModel>(
    problem: &Problem<M>,
    input: &FitInput,
    weighting: Weighting,
) -> Result<Vec<f64>, FitError> {
    let mut options = input.options.clone();
    options.objective = Objective::Wls;
    options.error_model = None;
    options.error_init = None;
    options.criterion = Criterion::RelativeDecrease;
    options.convergence = START_CONVERGENCE;
    options.max_iterations = START_MAX_ITERATIONS;
    options.derivatives = problem.options.derivatives;
    options.n_curve = 0;
    let nested = FitInput {
        weighting,
        options,
        ..input.clone()
    };
    let model: &dyn FitModel = &problem.model;
    let result = nested_fit(model, &nested)?;
    Ok(problem
        .names
        .iter()
        .zip(&problem.theta0)
        .map(|(name, &x0)| result.get(&format!("estimate.{name}")).unwrap_or(x0))
        .collect())
}

fn nested_fit(model: &dyn FitModel, input: &FitInput) -> Result<crate::FitResult, FitError> {
    Ok(FitRun::new(model, input)?.solve())
}

/// FIT-ML-13c, decision of T-148: a combined error model cannot describe an observation of
/// exactly 0 where the model predicts exactly 0 whatever θ is. The probe looks at the start and at
/// a few other points (every parameter scaled together, then each alone).
fn refuse_unbounded<M: FitModel>(problem: &Problem<M>, em: ErrorModel) -> Result<(), FitError> {
    if !matches!(em, ErrorModel::Combined | ErrorModel::CombinedSd) {
        return Ok(());
    }
    let mut rows: Vec<usize> = problem
        .y
        .iter()
        .enumerate()
        .filter(|(_, y)| **y == 0.0)
        .map(|(i, _)| i)
        .collect();
    if rows.is_empty() {
        return Ok(());
    }
    let mut probes: Vec<Vec<f64>> = vec![problem.theta0.clone()];
    for factor in [0.5, 2.0, 3.0] {
        probes.push(problem.theta0.iter().map(|x| x * factor).collect());
    }
    for j in 0..problem.theta0.len() {
        for factor in [0.5, 2.0] {
            let mut moved = problem.theta0.clone();
            if let Some(slot) = moved.get_mut(j) {
                *slot *= factor;
            }
            probes.push(moved);
        }
    }
    for theta in &probes {
        if let Some(pred) = problem.predict(theta) {
            rows.retain(|&i| pred.get(i).is_some_and(|f| *f == 0.0));
        }
    }
    if rows.is_empty() {
        return Ok(());
    }
    Err(FitError::UnboundedLikelihood {
        error_model: em.id().to_string(),
        times: rows
            .iter()
            .map(|&i| problem.time.get(i).copied().unwrap_or(f64::NAN))
            .collect(),
        indices: rows,
    })
}
