//! The minimisation as a value that owns its state (`docs/long-running-commands.md` section 2).
//!
//! [`FitRun`] holds everything the loop of the minimiser carries from one iteration to the next;
//! [`FitRun::step`] runs exactly one iteration of it (FIT-ALG-02 to 04, CNV-01 to 03, BND-01: the
//! same line search, the same Jacobian, the same stopping tests, in the same order), and the step
//! that ends the fit also runs the block after the loop (the rollback to the best iterate, then
//! the statistics of section 7). [`FitRun::run`] drives it and offers each committed iteration to
//! an [`Observer`], which can only say continue or cancel. There is one loop: [`crate::run`] and
//! [`crate::run_model`] are this one, with nobody watching. Nothing here reads a clock, a thread
//! or a flag, so watching changes no floating-point operation (golden rule 10).

use crate::arith::sq;
use crate::engine::{EXACT_FIT, LAMBDA_FACTOR, LAMBDA_MAX, LAMBDA_START, MIN_STEP, Problem};
use crate::model::FitModel;
use crate::result::{self, FitResult, TraceRow};
use crate::{Criterion, Derivatives, FitError, FitInput, FitStatus};

/// What an observer answers after an iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// Go on with the next iteration.
    Continue,
    /// Stop now, between two iterations.
    Cancel,
}

/// The state after a committed iteration (the iterate and the trace row are written). Borrowed
/// from the run: nothing is allocated, and a host serialises it before the next step.
#[derive(Debug, Clone, Copy)]
pub struct Progress<'a> {
    /// Iteration number: 0 for the initial estimates, then 1, 2, ...
    pub iteration: u32,
    /// The weighted sum of squares at the iterate, as in the trace row (`"wrss"` for an individual
    /// fit).
    pub objective: f64,
    /// Names of the fitted parameters, in the order of `values`.
    pub names: &'a [String],
    /// The iterate.
    pub values: &'a [f64],
}

/// What one [`FitRun::step`] gives.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // a value returned once per iteration; boxing would only add an allocation
pub enum Step<'a> {
    /// An iteration was committed; the fit goes on (or ends at the next step).
    Progress(Progress<'a>),
    /// The fit ended; this is the result [`crate::run`] returns. Asking for another step gives the
    /// same result again.
    Finished(FitResult),
}

/// Watches a run between two iterations. It cannot change a number.
pub trait Observer {
    /// Called once, before the first iteration, with the initial estimates (iteration 0): a host
    /// that was asked to cancel before the fit began answers [`Control::Cancel`] here. Continues
    /// by default.
    fn before_first_iteration(&mut self, progress: &Progress<'_>) -> Control {
        let _ = progress;
        Control::Continue
    }

    /// Called after each committed iteration, never inside the line search or the Jacobian.
    fn after_iteration(&mut self, progress: &Progress<'_>) -> Control;
}

/// How [`FitRun::run`] ended.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)] // returned once per fit
pub enum Outcome {
    /// The fit ended by itself; the result is the one [`crate::run`] gives.
    Done(FitResult),
    /// The observer cancelled after this iteration. A record for the screen, never an answer:
    /// `trace` holds the rows of FIT-OUT-11 from iteration 0 to `after_iteration`.
    Cancelled {
        /// The last committed iteration (0: before the first).
        after_iteration: u32,
        /// Rows 0 to `after_iteration` of the minimisation trace.
        trace: Vec<TraceRow>,
    },
}

enum Phase {
    /// Iterations remain to try.
    Running,
    /// The last committed iteration met the criterion: the next step finishes.
    Converged,
    /// The result has been built.
    Done,
}

enum Advance {
    Committed { converged: bool },
    Stopped(FitStatus),
}

/// A fit in progress. Owns its model (pass `&M` to lend one: `&M` is a model too) and every
/// variable the loop carries across iterations.
pub struct FitRun<M: FitModel> {
    problem: Problem<M>,
    /// The iterate.
    theta: Vec<f64>,
    /// The predictions at the iterate.
    pred: Vec<f64>,
    /// Rows 0 to the last committed iteration.
    trace: Vec<TraceRow>,
    /// Damping carried to the next iteration (FIT-ALG-04).
    lambda: f64,
    /// Relative decrease of the previous accepted iteration (the FIT-CNV-05 test).
    previous_decrease: Option<f64>,
    /// How the fit ends, `MaxIterations` until something else stops it.
    status: FitStatus,
    /// Σ w·y² with the weights of the initial estimates (the exact-fit test).
    ss_weighted_start: f64,
    /// FIT-CNV-01 (a): the predicted relative gain below which no measurable step remains.
    gain_floor: f64,
    /// The next iteration to try.
    next_iteration: usize,
    phase: Phase,
}

impl<M: FitModel> FitRun<M> {
    /// Checks the input as [`crate::run_model`] does and sets the loop at iteration 0.
    /// `input.initial` must name the fitted parameters.
    pub fn new(model: M, input: &FitInput) -> Result<Self, FitError> {
        let problem = Problem::new(model, input)?;
        let theta = problem.theta0.clone();
        // Checked in `new`: the initial prediction exists and its weights are defined.
        let pred = problem.predict(&theta).unwrap_or_default();
        let ss_weighted_start = problem.weights(&pred).map_or(0.0, |w| {
            problem
                .y
                .iter()
                .zip(&w)
                .map(|(y, w)| w * y * y)
                .sum::<f64>()
        });
        let trace = vec![TraceRow {
            iteration: 0,
            wrss: problem.wrss_own(&pred),
            estimates: theta.clone(),
            step: None,
            lambda: 0.0,
            halvings: 0,
            relative_decrease: None,
        }];
        // FIT-CNV-01 (a): below this predicted relative gain a point from which no step decreases
        // the WRSS is the minimum. With forward differences of increment h the Jacobian is only
        // good to O(h), so the gain it predicts at the true minimum is O(h²)·WRSS: a smaller
        // predicted gain that no step can realise is the error of the derivatives, not a failure
        // (T-030b). The floor is h² only when it exceeds ε (never with the presets: 1e-6 < 1e-4 for
        // the reference conventions, 1e-10 = ε for the default's forward-difference fallback).
        let eps = problem.options.convergence;
        let gain_floor = match problem.options.derivatives {
            Derivatives::ForwardDifference => eps.max(sq(problem.options.increment)),
            // `auto` is resolved in `new`; it cannot reach here.
            Derivatives::Analytic | Derivatives::Auto => eps,
        };
        Ok(Self {
            problem,
            theta,
            pred,
            trace,
            lambda: 0.0,
            previous_decrease: None,
            status: FitStatus::MaxIterations,
            ss_weighted_start,
            gain_floor,
            next_iteration: 1,
            phase: Phase::Running,
        })
    }

    /// The state after the last committed iteration (iteration 0 before the first step).
    pub fn progress(&self) -> Progress<'_> {
        let (iteration, objective) = self
            .trace
            .last()
            .map_or((0, f64::NAN), |row| (row.iteration, row.wrss));
        Progress {
            iteration: u32::try_from(iteration).unwrap_or(u32::MAX),
            objective,
            names: &self.problem.names,
            values: &self.theta,
        }
    }

    /// Exactly one iteration of the minimisation, or, when none is left, the end of the fit.
    pub fn step(&mut self) -> Step<'_> {
        if matches!(self.phase, Phase::Done | Phase::Converged)
            || self.next_iteration > self.problem.options.max_iterations
        {
            return Step::Finished(self.finish());
        }
        match self.iterate(self.next_iteration) {
            Advance::Stopped(status) => {
                self.status = status;
                Step::Finished(self.finish())
            }
            Advance::Committed { converged } => {
                self.next_iteration += 1;
                if converged {
                    self.status = FitStatus::Converged;
                    self.phase = Phase::Converged;
                }
                Step::Progress(self.progress())
            }
        }
    }

    /// Runs to the end, offering the initial estimates and each committed iteration to `obs`.
    pub fn run(mut self, obs: &mut dyn Observer) -> Outcome {
        if obs.before_first_iteration(&self.progress()) == Control::Cancel {
            return self.cancel();
        }
        loop {
            match self.step() {
                Step::Finished(result) => return Outcome::Done(result),
                Step::Progress(progress) => {
                    if obs.after_iteration(&progress) == Control::Cancel {
                        return self.cancel();
                    }
                }
            }
        }
    }

    /// The same loop with nobody watching: what [`crate::run_model`] returns.
    pub(crate) fn solve(mut self) -> FitResult {
        loop {
            if let Step::Finished(result) = self.step() {
                return result;
            }
        }
    }

    /// The rows of the minimisation trace so far (FIT-OUT-11): iteration 0 alone before the first
    /// step, then one row more per committed iteration. After `Finished` it is the trace the
    /// result carries (the rollback of the end removes no row).
    pub fn trace(&self) -> &[TraceRow] {
        &self.trace
    }

    /// Gives up between two iterations: `Outcome::Cancelled` with the rows 0 to the last committed
    /// iteration, one row (iteration 0) before the first step. After `Finished` it still returns
    /// the whole trace, as a cancellation and not as the result: keep the `FitResult` that
    /// `Finished` carried.
    pub fn cancel(self) -> Outcome {
        let after_iteration = self.progress().iteration;
        Outcome::Cancelled {
            after_iteration,
            trace: self.trace,
        }
    }

    /// One pass of the body of the loop (FIT-ALG-02 to 04, CNV-01 to 03, BND-01).
    fn iterate(&mut self, iteration: usize) -> Advance {
        let problem = &self.problem;
        let eps = problem.options.convergence;
        let p = self.theta.len();
        // FIT-WGT-03: weights of this iteration, constants in the derivatives.
        let Some(w) = problem.weights(&self.pred) else {
            return Advance::Stopped(FitStatus::NonFinite);
        };
        let s_cur = problem.wrss(&self.pred, &w);
        // An exact fit (residuals at rounding level) cannot decrease any further.
        if s_cur <= EXACT_FIT * self.ss_weighted_start {
            return Advance::Stopped(FitStatus::Converged);
        }
        let Some(columns) = problem.jacobian(&self.theta, &self.pred) else {
            return Advance::Stopped(FitStatus::NonFinite);
        };
        // FIT-BND-01 active set: a parameter on a bound whose Gauss-Newton direction points
        // outward is held there for this iteration.
        let all: Vec<usize> = (0..p).collect();
        let full = problem.direction(&columns, &self.pred, &w, &all, 0.0);
        let on_bound = problem.on_bound(&self.theta);
        let free: Vec<usize> = match &full {
            Some((delta, _)) => all
                .iter()
                .copied()
                .filter(|&j| {
                    let d = delta.get(j).copied().unwrap_or(0.0);
                    let x = self.theta.get(j).copied().unwrap_or(0.0);
                    let lo = problem.lower.get(j).copied().unwrap_or(f64::NEG_INFINITY);
                    let held = on_bound.get(j).copied().unwrap_or(false)
                        && ((x <= lo && d < 0.0) || (x > lo && d > 0.0));
                    !held
                })
                .collect(),
            None => all.clone(),
        };
        if free.is_empty() {
            // Every parameter is held on a bound: nothing can move.
            return Advance::Stopped(FitStatus::Converged);
        }
        let gn = if free.len() == p {
            full
        } else {
            problem.direction(&columns, &self.pred, &w, &free, 0.0)
        };
        if gn.is_none() && problem.scaled_system(&columns, &self.pred, &w).is_none() {
            return Advance::Stopped(FitStatus::Singular);
        }
        let predicted = gn.as_ref().map(|(_, q)| *q);
        if problem.options.criterion == Criterion::RelativeOffset {
            let (n, pf) = (problem.y.len(), free.len());
            if let Some(q) = predicted {
                if n > pf {
                    let offset =
                        ((q / pf as f64) / ((s_cur - q).max(0.0) / (n - pf) as f64)).sqrt();
                    if offset <= eps {
                        return Advance::Stopped(FitStatus::Converged);
                    }
                }
            }
        }
        let mut accepted = None;
        let mut lam = self.lambda;
        let mut at_minimum = false;
        loop {
            let direction = if lam == 0.0 {
                gn.as_ref().map(|(d, _)| d.clone())
            } else {
                problem
                    .direction(&columns, &self.pred, &w, &free, lam)
                    .map(|(d, _)| d)
            };
            if let Some(delta) = direction {
                let mut nu = 1.0;
                let mut halvings = 0;
                while nu >= MIN_STEP {
                    // FIT-ALG-03 with the trial point projected on the bounds.
                    let trial = problem.clamp(
                        &self
                            .theta
                            .iter()
                            .zip(&delta)
                            .map(|(t, d)| t + nu * d)
                            .collect::<Vec<f64>>(),
                    );
                    if let Some(pt) = problem.predict(&trial) {
                        let weights_ok =
                            !problem.weighting.uses_predictions() || problem.weights(&pt).is_some();
                        let s_trial = problem.wrss(&pt, &w);
                        if weights_ok && s_trial < s_cur {
                            accepted = Some((trial, pt, s_trial, nu, halvings));
                            break;
                        }
                    }
                    nu /= 2.0;
                    halvings += 1;
                }
                if accepted.is_some() {
                    break;
                }
                if lam == 0.0
                    && self.trace.len() > 1
                    && predicted.is_some_and(|q| q <= self.gain_floor * s_cur)
                {
                    // Nothing measurable is left to gain: the current point is the minimum. Only
                    // after an accepted step: at the initial estimates a tiny predicted gain can
                    // come from a few huge weights far from the solution (1/ŷ with ŷ ~ 1e-17).
                    at_minimum = true;
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
        if at_minimum {
            return Advance::Stopped(FitStatus::Converged);
        }
        let Some((new_theta, new_pred, s_new, nu, halvings)) = accepted else {
            return Advance::Stopped(FitStatus::NoDecrease);
        };
        self.lambda = if lam <= LAMBDA_START {
            0.0
        } else {
            lam / LAMBDA_FACTOR
        };
        self.theta = new_theta;
        self.pred = new_pred;
        let decrease = s_cur - s_new;
        self.trace.push(TraceRow {
            iteration,
            wrss: self.problem.wrss_own(&self.pred),
            estimates: self.theta.clone(),
            step: Some(nu),
            lambda: lam,
            halvings,
            relative_decrease: Some(decrease / s_new).filter(|x| x.is_finite()),
        });
        let relative = decrease / s_new;
        let met = if self.problem.weighting.uses_predictions() {
            // Reweighted iterations converge linearly: each step is a fraction ρ of the one
            // before (decreases scale as the square of the step), so the WRSS still to gain is
            // about the last decrease / (1 − ρ)². Require that to be below ε; a cycle (ρ ≈ 1)
            // never converges (FIT-WGT-03, no fixed point).
            relative == 0.0
                || self.previous_decrease.is_some_and(|prev| {
                    let rho = (relative / prev).sqrt();
                    prev > 0.0 && rho < 1.0 && relative <= eps * sq(1.0 - rho)
                })
        } else {
            decrease <= eps * s_new
        };
        self.previous_decrease = Some(relative);
        Advance::Committed {
            converged: self.problem.options.criterion == Criterion::RelativeDecrease && met,
        }
    }

    /// The block after the loop, then the statistics. Repeating it gives the same result: the
    /// rollback leaves the best iterate, which the second pass keeps.
    fn finish(&mut self) -> FitResult {
        self.phase = Phase::Done;
        // With predicted-value weights each step lowers the WRSS of its own (frozen) weights, which
        // does not guarantee that the WRSS with the weights of the new estimates falls: from a far
        // start it can explode. A fit worse than its start is not converged: return the best
        // iterate with `no_decrease`.
        if self.problem.weighting.uses_predictions() {
            let start = self.trace.first().map_or(f64::NAN, |row| row.wrss);
            let end = self.problem.wrss_own(&self.pred);
            if !end.is_finite() || end > start {
                let best = self
                    .trace
                    .iter()
                    .filter(|row| row.wrss.is_finite())
                    .min_by(|a, b| a.wrss.total_cmp(&b.wrss))
                    .map(|row| row.estimates.clone());
                if let Some(best) = best {
                    if let Some(best_pred) = self.problem.predict(&best) {
                        self.theta = best;
                        self.pred = best_pred;
                    }
                }
                self.status = FitStatus::NoDecrease;
            }
        }
        result::build(
            &self.problem,
            self.theta.clone(),
            self.pred.clone(),
            self.trace.clone(),
            self.status,
        )
    }
}
