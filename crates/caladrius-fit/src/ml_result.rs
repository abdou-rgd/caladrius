//! The result of an `objective = ml` fit (`specs/fit.md` FIT-ML-15 to 18, FIT-ML-20): −2LL and the
//! likelihood criteria, the expected-information covariance in the user's units, the IWRES and its
//! summaries, the least-squares forms at the ML estimate, the flags.

use std::collections::BTreeMap;

use crate::arith::sq;
use crate::engine::{EXACT_FIT, Problem};
use crate::flags::{self, FitFlag};
use crate::linalg::{Mat, inverse_gram_from_r, least_squares, symmetric_eigenvalues};
use crate::ml::{MlRun, scaled};
use crate::model::FitModel;
use crate::result::{FitResult, ObservationRow, TraceRow, Values, curve, secondary};
use crate::{FitStatus, Objective};

/// More iterations than this are flagged as slow (FIT-ML-20).
const SLOW_ITERATIONS: usize = 50;

/// The covariance of the free estimates in the user's units (FIT-ML-17), or `None` when the
/// expected information is singular.
fn covariance(run: &MlRun, free: &[usize], user: &[f64]) -> Option<Mat> {
    let (a, norms) = scaled(&run.point.sys, free)?;
    let zero = vec![0.0; a.rows()];
    let (_, r) = least_squares(&a, &zero)?;
    let inv = inverse_gram_from_r(&r)?;
    let n = free.len();
    // The delta method: a variance-scale error variable A (reported as a = √A) has dU/dA = 1/(2a).
    let factor: Vec<f64> = free
        .iter()
        .map(|&j| {
            if j < run.p() {
                1.0
            } else {
                run.error_model()
                    .delta_factor(user.get(j).copied().unwrap_or(f64::NAN))
            }
        })
        .collect();
    let at = |v: &[f64], i: usize| v.get(i).copied().unwrap_or(f64::NAN);
    let mut cov = Mat::zeros(n, n);
    for a in 0..n {
        for b in 0..n {
            cov.set(
                a,
                b,
                at(&factor, a) * at(&factor, b) * inv.at(a, b) / (at(&norms, a) * at(&norms, b)),
            );
        }
    }
    Some(cov)
}

/// Statistics at the iterate that ended the run.
pub(crate) fn build<M: FitModel>(
    problem: &Problem<M>,
    run: &MlRun,
    trace: Vec<TraceRow>,
    mut status: FitStatus,
) -> FitResult {
    let em = run.error_model();
    let (p_all, q) = (run.p(), run.q());
    let ev = &run.point.ev;
    let (theta, _) = ev.zeta.split_at(p_all.min(ev.zeta.len()));
    let pred = &ev.pred;
    let n = problem.y.len();
    let nf = n as f64;
    let names = &run.names;
    let user = &run.values;

    // FIT-BND-02, FIT-ML-15: a variable on a bound is reported with its value only.
    let on_bound: Vec<bool> = ev
        .zeta
        .iter()
        .zip(run.lower().iter().zip(run.upper()))
        .map(|(x, (lo, hi))| x <= lo || x >= hi)
        .collect();
    let free: Vec<usize> = (0..p_all + q)
        .filter(|&j| !on_bound.get(j).copied().unwrap_or(false))
        .collect();
    let at_bound: Vec<(String, f64)> = (0..p_all + q)
        .filter(|&j| on_bound.get(j).copied().unwrap_or(false))
        .filter_map(|j| {
            let x = ev.zeta.get(j).copied()?;
            let lo = run.lower().get(j).copied()?;
            let hi = run.upper().get(j).copied()?;
            // The error variables are reported in the user's units: their lower bound is 0.
            Some((names.get(j)?.clone(), if x <= lo { lo } else { hi }))
        })
        .collect();
    if status == FitStatus::Converged && !at_bound.is_empty() {
        status = FitStatus::AtBound;
    }
    let free_names: Vec<String> = free.iter().filter_map(|&j| names.get(j).cloned()).collect();
    let p = free.iter().filter(|&&j| j < p_all).count();
    let df = n.saturating_sub(p + q);

    let mut v = Values(BTreeMap::new());
    v.put("n", nf);
    v.put("p", p as f64);
    v.put("q", q as f64);
    v.put("df", df as f64);
    for (name, x) in names.iter().zip(user) {
        v.put(format!("estimate.{name}"), *x);
    }
    for (name, x) in problem.names.iter().zip(&run.start) {
        v.put(format!("start.{name}"), *x);
    }
    // FIT-ML-16.
    let k = (p + q) as f64;
    v.put("m2ll", ev.m2ll);
    v.put("aic_ml", ev.m2ll + 2.0 * k);
    v.put("bic_ml", ev.m2ll + k * libm::log(nf));
    // The least-squares forms at the ML estimate, uniform weights.
    let wrss: f64 = problem.y.iter().zip(pred).map(|(y, f)| sq(y - f)).sum();
    let ss_y: f64 = problem.y.iter().map(|y| y * y).sum();
    v.put("wrss", wrss);
    if wrss > EXACT_FIT * ss_y {
        v.put("aic", nf * libm::log(wrss) + 2.0 * p as f64);
        v.put("sbc", nf * libm::log(wrss) + p as f64 * libm::log(nf));
    }

    // FIT-ML-18.
    let z: Vec<f64> = problem
        .y
        .iter()
        .zip(pred)
        .zip(&ev.sigma)
        .map(|((y, f), s)| (y - f) / s)
        .collect();
    // NC (the key is absent) when a sigma_i is not finite, and for the standard deviation with
    // fewer than 2 observations. A fit that did not converge keeps its summaries, flagged with
    // the "not converged" mark (FIT-ML-18, FIT-CNV-03).
    if z.iter().all(|z| z.is_finite()) {
        let mean = z.iter().sum::<f64>() / nf;
        v.put("iwres_mean", mean);
        v.put("iwres_sum_sq", z.iter().map(|z| z * z).sum());
        if n >= 2 {
            v.put(
                "iwres_sd",
                (z.iter().map(|z| sq(z - mean)).sum::<f64>() / (nf - 1.0)).sqrt(),
            );
        }
        v.put(
            "n_iwres_gt2",
            z.iter().filter(|z| z.abs() > 2.0).count() as f64,
        );
        v.put(
            "n_iwres_gt3",
            z.iter().filter(|z| z.abs() > 3.0).count() as f64,
        );
    }

    // FIT-OUT-08 for the model parameters only.
    let partials = problem.jacobian(theta, pred).unwrap_or_default();

    // FIT-ML-17: the expected information of the free estimates.
    match covariance(run, &free, user) {
        None => {
            if status == FitStatus::Converged {
                status = FitStatus::Singular;
            }
        }
        Some(cov) => {
            let nfree = free.len();
            let se: Vec<f64> = (0..nfree).map(|j| cov.at(j, j).sqrt()).collect();
            let mut corr = Mat::zeros(nfree, nfree);
            for (a, (&ja, na)) in free.iter().zip(&free_names).enumerate() {
                let xa = user.get(ja).copied().unwrap_or(f64::NAN);
                let sa = se.get(a).copied().unwrap_or(f64::NAN);
                v.put(format!("se.{na}"), sa);
                if xa != 0.0 {
                    v.put(format!("cv_percent.{na}"), 100.0 * sa / xa.abs());
                }
                for (b, nb) in free_names.iter().enumerate() {
                    if a <= b {
                        v.put(format!("covariance.{na}.{nb}"), cov.at(a, b));
                    }
                    let c = cov.at(a, b) / (sa * se.get(b).copied().unwrap_or(f64::NAN));
                    corr.set(a, b, c);
                    if a < b {
                        v.put(format!("correlation.{na}.{nb}"), c);
                    }
                }
            }
            let e = symmetric_eigenvalues(&corr);
            for (i, x) in e.iter().enumerate() {
                v.put(format!("eigenvalue.{}", i + 1), *x);
            }
            if let (Some(max), Some(min)) = (e.first(), e.last()) {
                if *min > 0.0 {
                    v.put("condition_number", max / min);
                }
            }
            // Secondary parameters of the model parameters (no interval for ml, FIT-ML-17).
            let theta_free: Vec<usize> = free.iter().copied().filter(|&j| j < p_all).collect();
            let mut block = Mat::zeros(theta_free.len(), theta_free.len());
            for a in 0..theta_free.len() {
                for b in 0..theta_free.len() {
                    block.set(a, b, cov.at(a, b));
                }
            }
            let theta_names: Vec<String> = theta_free
                .iter()
                .filter_map(|&j| problem.names.get(j).cloned())
                .collect();
            secondary(problem, &theta_names, theta, &block, None, &mut v);
        }
    }

    let observations = problem
        .time
        .iter()
        .zip(&problem.y)
        .zip(pred.iter().zip(ev.sigma.iter().zip(&z)))
        .map(
            |((&time, &observed), (&predicted, (&sigma, &iwres)))| ObservationRow {
                time,
                observed,
                predicted,
                residual: observed - predicted,
                weighted_residual: iwres,
                weight: 1.0 / (sigma * sigma),
                sigma: Some(sigma),
                iwres: Some(iwres),
            },
        )
        .collect();

    let mut flag_list = flags::flags(
        &problem.options.flags,
        &flags::Inputs {
            status,
            last_step: trace
                .iter()
                .rev()
                .find(|row| !row.polish)
                .and_then(|row| row.step.map(|step| (row.lambda, step))),
            at_bound,
            free: &free_names,
            df,
            get: &|name: &str| v.0.get(name).copied(),
        },
    );
    // The Newton steps of the endgame are not iterations of the scoring (FIT-ML-20).
    let iterations = trace
        .iter()
        .rev()
        .find(|row| !row.polish)
        .map_or(0, |row| row.iteration);
    if iterations > SLOW_ITERATIONS {
        flag_list.push(FitFlag::SlowConvergence { iterations });
    }
    FitResult {
        status,
        parameters: names.clone(),
        values: v.0,
        observations,
        curve: curve(problem, theta),
        trace,
        partials,
        flags: flag_list,
        objective: Some(Objective::Ml),
        error_model: Some(em),
        derivatives: Some(problem.options.derivatives),
    }
}
