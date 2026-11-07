//! Three-compartment models (`specs/models.md` section 14): concentrations, areas, first moments,
//! derived quantities and closed-form partial derivatives, all as positive combinations of the
//! one-compartment kernels of `kernel` (MOD-3C-09): C = Σ wi·Φ(λi)/Vc over the three exponents.
//!
//! At the exact tie k21 = k31 the weight of the middle exponent is 0 and its term is skipped
//! (MOD-3C-22).

use std::collections::BTreeMap;

use crate::kernel::{Kernel, Partials, Rate, first_order_peak};
use crate::model::Input;
use crate::params3::{self, Disposition, Set};
use crate::two::{GL_NODES, GL_WEIGHTS};
use crate::{Derivatives, Jacobian, ModelError, ModelInput, ModelOutput};

/// The checked parameters and the kernel of `input`; times are checked here too (MOD-GEN-04).
fn prepare(input: &ModelInput) -> Result<(Disposition, Kernel), ModelError> {
    let p = params3::resolve(input.model, &input.params, input.dose)?;
    if let Some(index) = input.times.iter().position(|t| !t.is_finite()) {
        return Err(ModelError::NonFiniteTime { index });
    }
    let kernel = Kernel {
        input: input.model.input(),
        dose: input.dose,
        ka: p.ka,
        dur: p.dur,
    };
    Ok((p, kernel))
}

/// The terms of the curve that count: (exponent, amplitude) of each exponent with a positive
/// weight, in the order α, β, γ (the middle one has weight 0 at the exact tie).
fn modes(p: &Disposition) -> Vec<(f64, f64)> {
    p.lambda()
        .into_iter()
        .zip(p.weight())
        .zip(p.amp)
        .filter(|((_, w), _)| *w > 0.0)
        .map(|((rate, _), amp)| (rate, amp))
        .collect()
}

/// Concentration, AUC(0, t) and AUMC(0, t) at time `t` since the dose (MOD-3C-09 to 16). The
/// moment is taken about the dose time: with a lag it is M₀(s) + tlag·AUC₀(s), s = t − tlag.
fn at(modes: &[(f64, f64)], tlag: f64, kernel: &Kernel, t: f64) -> (f64, f64, f64) {
    let s = t - tlag;
    let (mut conc, mut auc, mut moment) = (0.0, 0.0, 0.0);
    for &(rate, amp) in modes {
        let (c, a) = kernel.conc_auc(rate, s);
        conc += amp * c;
        auc += amp * a;
        moment += amp * kernel.moment(rate, s);
    }
    let aumc = if s <= 0.0 { 0.0 } else { moment + tlag * auc };
    (conc, auc, aumc)
}

/// MOD-3C-17: the single maximum of first-order input lies between the smallest and the largest
/// one-compartment peak time of (ka, λi); bisection on the sign of C′ until the bracket is two
/// adjacent doubles.
fn first_order_peak_time(modes: &[(f64, f64)], ka: f64, kernel: &Kernel) -> f64 {
    let peaks = modes.iter().map(|&(rate, _)| first_order_peak(rate, ka));
    let mut lo = peaks.clone().fold(f64::INFINITY, f64::min);
    let mut hi = peaks.fold(f64::NEG_INFINITY, f64::max);
    let slope = |s: f64| -> f64 {
        modes
            .iter()
            .map(|&(rate, amp)| amp * kernel.slope(rate, s))
            .sum()
    };
    for _ in 0..2200 {
        let mid = lo + (hi - lo) / 2.0;
        if mid <= lo || mid >= hi {
            break;
        }
        let d = slope(mid);
        if d > 0.0 {
            lo = mid;
        } else if d < 0.0 {
            hi = mid;
        } else {
            return mid;
        }
    }
    lo + (hi - lo) / 2.0
}

/// Evaluates a three-compartment model (MOD-3C-01 to 17, 25, 26).
pub(crate) fn run(input: &ModelInput) -> Result<ModelOutput, ModelError> {
    let (p, kernel) = prepare(input)?;
    let dose = input.dose;
    let terms = modes(&p);
    let mut conc = Vec::with_capacity(input.times.len());
    let mut auc = Vec::with_capacity(input.times.len());
    let mut aumc = Vec::with_capacity(input.times.len());
    for &t in &input.times {
        let (c, a, m) = at(&terms, p.tlag, &kernel, t);
        conc.push(c);
        auc.push(a);
        aumc.push(m);
    }

    let mut secondary = BTreeMap::new();
    let mut put = |name: &str, value: f64| {
        secondary.insert(name.to_string(), value);
    };
    let [alpha, beta, gamma] = p.lambda();
    let [w_alpha, w_beta, w_gamma] = p.weight();
    let [a, b, c] = p.coefficient;
    for (name, value) in [
        ("cl", p.cl),
        ("vc", p.vc),
        ("q2", p.q2),
        ("vp2", p.vp2),
        ("q3", p.q3),
        ("vp3", p.vp3),
        ("k10", p.k10),
        ("k12", p.k12),
        ("k21", p.k21),
        ("k13", p.k13),
        ("k31", p.k31),
        ("alpha", alpha),
        ("beta", beta),
        ("gamma", gamma),
        ("w_alpha", w_alpha),
        ("w_beta", w_beta),
        ("w_gamma", w_gamma),
        // The intravenous coefficients, for every route (MOD-3C-02).
        ("a", a),
        ("b", b),
        ("c", c),
        ("k21_ge_k31", if p.k21 >= p.k31 { 1.0 } else { 0.0 }),
    ] {
        put(name, value);
    }
    let ln2 = std::f64::consts::LN_2;
    let vss = p.vc + p.vp2 + p.vp3;
    let mrt_system = vss / p.cl;
    let auc_inf = dose / p.cl;
    put("half_life", ln2 / gamma);
    put("half_life_alpha", ln2 / alpha);
    put("half_life_beta", ln2 / beta);
    put("half_life_gamma", ln2 / gamma);
    put("vss", vss);
    put("vz", p.cl / gamma);
    // D/(terminal intercept) = vc/wγ, also defined at dose 0 (as for two compartments).
    put("v_extrap", p.vc / w_gamma);
    put("auc_inf", auc_inf);
    put("mrt_system", mrt_system);
    // The mean input time m of MOD-3C-16.
    let input_time = match kernel.input {
        Input::Bolus => 0.0,
        Input::ZeroOrder => p.dur / 2.0 + p.tlag,
        Input::FirstOrder => 1.0 / p.ka + p.tlag,
    };
    let mrt = mrt_system + input_time;
    put("mrt", mrt);
    put("aumc_inf", auc_inf * mrt);
    if input.model.has_lag() {
        put("tlag", p.tlag);
    }
    match kernel.input {
        Input::Bolus => put("c0", dose / p.vc),
        Input::ZeroOrder => {
            put("dur", p.dur);
            put("rate", dose / p.dur);
            put("tmax_pred", p.dur + p.tlag);
            put("cmax_pred", at(&terms, p.tlag, &kernel, p.dur + p.tlag).0);
        }
        Input::FirstOrder => {
            put("ka", p.ka);
            // Not defined at dose 0: the curve is flat (as for two compartments).
            if dose > 0.0 {
                let peak = first_order_peak_time(&terms, p.ka, &kernel);
                put("tmax_pred", peak + p.tlag);
                put("cmax_pred", at(&terms, p.tlag, &kernel, peak + p.tlag).0);
            }
        }
    }
    let overflow = |what: String| Err(ModelError::Overflow { what });
    for (i, t) in input.times.iter().enumerate() {
        let finite = |v: &Vec<f64>| v.get(i).is_some_and(|x| x.is_finite());
        if !finite(&conc) {
            return overflow(format!(
                "the concentration at time {}",
                crate::error::short(*t)
            ));
        }
        if !finite(&auc) {
            return overflow(format!("the AUC at time {}", crate::error::short(*t)));
        }
        if !finite(&aumc) {
            return overflow(format!("the AUMC at time {}", crate::error::short(*t)));
        }
    }
    if let Some((name, _)) = secondary.iter().find(|(_, v)| !v.is_finite()) {
        return overflow(format!("the secondary parameter `{name}`"));
    }
    Ok(ModelOutput {
        conc,
        auc,
        aumc,
        accum_c: Vec::new(),
        secondary,
    })
}

// ---------------------------------------------------------------- partial derivatives

/// ∂(λ, w)/∂θ of MOD-3C-20 for each micro-constant θ: `lambda[θ][i]` is ∂λi/∂θ and `weight[θ][i]`
/// is ∂wi/∂θ for i = α, β (the weight of γ is eliminated, MOD-3C-19).
struct Sensitivities {
    lambda: [[f64; 3]; 5],
    weight: [[f64; 2]; 5],
}

/// MOD-3C-20 from the offsets of MOD-3C-07; the exponent with weight 0 (β at the exact tie)
/// contributes nothing, which is its limit (MOD-3C-22). An error when a value is not finite.
fn sensitivities(p: &Disposition, model: &str) -> Result<Sensitivities, ModelError> {
    let mut lambda = [[0.0; 3]; 5];
    let mut weight = [[0.0; 2]; 5];
    let (c21, c31) = (p.k12 * p.k21, p.k13 * p.k31);
    for i in 0..3 {
        let (Some(&w), Some(&rate), Some(&o21), Some(&o31)) = (
            p.ex.weight.get(i),
            p.ex.lambda.get(i),
            p.ex.o21.get(i),
            p.ex.o31.get(i),
        ) else {
            continue;
        };
        if w == 0.0 {
            continue;
        }
        let (t21, t31) = (c21 / o21 / o21, c31 / o31 / o31);
        // g_λ and g_θ of MOD-3C-20 (g = 1/w).
        let g_lambda = -2.0 * (t21 / o21 + t31 / o31);
        let g_theta = [
            0.0,
            p.k21 / o21 / o21,
            p.k12 / o21 / o21 + 2.0 * t21 / o21,
            p.k31 / o31 / o31,
            p.k13 / o31 / o31 + 2.0 * t31 / o31,
        ];
        let d_lambda = [
            w,
            w * rate / o21,
            w * p.k12 * rate / o21 / o21,
            w * rate / o31,
            w * p.k13 * rate / o31 / o31,
        ];
        for theta in 0..5 {
            let (Some(&dl), Some(&gt)) = (d_lambda.get(theta), g_theta.get(theta)) else {
                continue;
            };
            if let Some(slot) = lambda.get_mut(theta).and_then(|row| row.get_mut(i)) {
                *slot = dl;
            }
            if i < 2 {
                let dw = -w * w * (gt + g_lambda * dl);
                if let Some(slot) = weight.get_mut(theta).and_then(|row| row.get_mut(i)) {
                    *slot = dw;
                }
            }
        }
    }
    let finite = lambda.iter().flatten().all(|x| x.is_finite())
        && weight.iter().flatten().all(|x| x.is_finite());
    if !finite {
        return Err(ModelError::Overflow {
            what: format!(
                "the derivatives of the exponents and weights of {model} (an offset to a pole is too small)"
            ),
        });
    }
    Ok(Sensitivities { lambda, weight })
}

/// Whether two exponents are close on the time scale of s (gap·s <= 1).
fn close_on_scale(gap: f64, s: f64) -> bool {
    gap * s <= 1.0
}

/// The rates α, β, γ for the derivatives at s: written about a common centre for the exponents
/// that are close on the time scale of s (see [`Rate`]), as they are otherwise.
fn rates(p: &Disposition, s: f64) -> [Rate; 3] {
    let [alpha, beta, gamma] = p.lambda();
    let (ab, bg) = (p.ex.gap(0, 1), p.ex.gap(1, 2));
    match (close_on_scale(ab, s), close_on_scale(bg, s)) {
        (true, true) => [
            Rate {
                center: beta,
                offset: ab,
            },
            Rate::plain(beta),
            Rate {
                center: beta,
                offset: -bg,
            },
        ],
        (true, false) => {
            let (center, half) = ((alpha + beta) / 2.0, ab / 2.0);
            [
                Rate {
                    center,
                    offset: half,
                },
                Rate {
                    center,
                    offset: -half,
                },
                Rate::plain(gamma),
            ]
        }
        (false, true) => {
            let (center, half) = ((beta + gamma) / 2.0, bg / 2.0);
            [
                Rate::plain(alpha),
                Rate {
                    center,
                    offset: half,
                },
                Rate {
                    center,
                    offset: -half,
                },
            ]
        }
        (false, false) => [Rate::plain(alpha), Rate::plain(beta), Rate::plain(gamma)],
    }
}

/// Φ(λ_i, s) − Φ(γ, s) for i = 0 (α) or 1 (β). When the two exponents are close on the time scale
/// of s the difference would cancel, so it is the integral of ∂Φ/∂λ over [γ, λi] by 8-point
/// Gauss–Legendre (exact to rounding there), every node written about the mean of the two
/// exponents; otherwise the difference, which keeps all but a few bits.
fn kernel_difference(
    p: &Disposition,
    kernel: &Kernel,
    s: f64,
    i: usize,
    parts: &[Partials; 3],
) -> f64 {
    let gap = p.ex.gap(i, 2);
    let (Some(upper), Some(lower)) = (parts.get(i), parts.get(2)) else {
        return f64::NAN;
    };
    if !close_on_scale(gap, s) {
        return upper.value - lower.value;
    }
    let (Some(&top), Some(&bottom)) = (p.ex.lambda.get(i), p.ex.lambda.get(2)) else {
        return f64::NAN;
    };
    let (center, half) = ((top + bottom) / 2.0, gap / 2.0);
    let node = |offset: f64| kernel.partials(Rate { center, offset }, s).rate;
    let mut sum = 0.0;
    for (x, w) in GL_NODES.iter().zip(GL_WEIGHTS) {
        sum += w * (node(half * x) + node(-half * x));
    }
    half * sum
}

/// Closed-form ∂C/∂θ for every parameter of `input` (MOD-3C-19 to 23). Columns in the order of
/// `input.params`; 0 before the dose and before the lag.
pub(crate) fn jacobian(input: &ModelInput) -> Result<Jacobian, ModelError> {
    let (p, kernel) = prepare(input)?;
    let names: Vec<String> = input.params.keys().cloned().collect();
    let mut columns: Vec<Vec<f64>> = vec![Vec::with_capacity(input.times.len()); names.len()];
    let sens = if p.set == Set::Macro {
        None
    } else {
        Some(sensitivities(&p, input.model.id())?)
    };
    let [wa, wb, wg] = p.weight();
    for &t in &input.times {
        let s = t - p.tlag;
        let parts: [Partials; 3] = {
            let [ra, rb, rg] = rates(&p, s);
            [
                kernel.partials(ra, s),
                kernel.partials(rb, s),
                kernel.partials(rg, s),
            ]
        };
        let [pa, pb, pg] = parts;
        let weights = [wa, wb, wg];
        let amps = p.amp;
        let combine = |pick: &dyn Fn(&Partials) -> f64| -> f64 {
            parts
                .iter()
                .zip(weights)
                .zip(amps)
                .filter(|((_, w), _)| *w > 0.0)
                .map(|((part, _), amp)| amp * pick(part))
                .sum()
        };
        let conc = combine(&|x| x.value);
        // ∂C/∂wα and ∂C/∂wβ (MOD-3C-21), without the difference of nearly equal kernels.
        let c_wa = kernel_difference(&p, &kernel, s, 0, &parts) / p.vc;
        let c_wb = if wb > 0.0 {
            kernel_difference(&p, &kernel, s, 1, &parts) / p.vc
        } else {
            0.0
        };
        // C_θ at fixed V for each micro-constant (MOD-3C-23).
        let micro = |theta: usize| -> f64 {
            let Some(sens) = sens.as_ref() else {
                return f64::NAN;
            };
            let (Some(dl), Some(dw)) = (sens.lambda.get(theta), sens.weight.get(theta)) else {
                return f64::NAN;
            };
            let [dwa, dwb] = *dw;
            let mut sum = c_wa * dwa + c_wb * dwb;
            for (((part, w), amp), d) in parts.iter().zip(weights).zip(amps).zip(dl) {
                if w > 0.0 {
                    sum += amp * part.rate * d;
                }
            }
            sum
        };
        let c_k = micro(0);
        let c_k12 = micro(1);
        let c_k21 = micro(2);
        let c_k13 = micro(3);
        let c_k31 = micro(4);
        let (v, dose) = (p.vc, input.dose);
        let [amp_a, amp_b, amp_c] = p.amp;
        for (name, column) in names.iter().zip(columns.iter_mut()) {
            let value = match (p.set, name.as_str()) {
                (_, "ka") => combine(&|x| x.ka),
                (_, "dur") => combine(&|x| x.dur),
                (_, "tlag") => -combine(&|x| x.s),
                (Set::Clearance, "cl") => c_k / v,
                (Set::Clearance, "vc") => {
                    -conc / v - (p.k10 * c_k + p.k12 * c_k12 + p.k13 * c_k13) / v
                }
                (Set::Clearance, "q2") => c_k12 / v + c_k21 / p.vp2,
                (Set::Clearance, "vp2") => -p.k21 * c_k21 / p.vp2,
                (Set::Clearance, "q3") => c_k13 / v + c_k31 / p.vp3,
                (Set::Clearance, "vp3") => -p.k31 * c_k31 / p.vp3,
                (Set::Micro, "k10") => c_k,
                (Set::Micro, "k12") => c_k12,
                (Set::Micro, "k21") => c_k21,
                (Set::Micro, "k13") => c_k13,
                (Set::Micro, "k31") => c_k31,
                (Set::Micro, "vc") => -conc / v,
                // Macro set: C = (a·Φ(α) + b·Φ(β) + c·Φ(γ))/D, D > 0 (MOD-3C-21).
                (Set::Macro, "a") => pa.value / dose,
                (Set::Macro, "b") => pb.value / dose,
                (Set::Macro, "c") => pg.value / dose,
                (Set::Macro, "alpha") => amp_a * pa.rate,
                (Set::Macro, "beta") => amp_b * pb.rate,
                (Set::Macro, "gamma") => amp_c * pg.rate,
                // Unreachable after `prepare`, which refuses every other name; an error rather
                // than a silent zero column all the same.
                (_, other) => {
                    return Err(ModelError::ParameterNotInModel {
                        name: other.to_string(),
                        model: input.model.id().to_string(),
                    });
                }
            };
            column.push(value);
        }
    }
    for (name, column) in names.iter().zip(&columns) {
        if column.iter().any(|x| !x.is_finite()) {
            return Err(ModelError::Overflow {
                what: format!("the derivative with respect to `{name}`"),
            });
        }
    }
    Ok(Jacobian {
        parameters: names,
        columns,
        method: Derivatives::Analytic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelId, jacobian as derivatives};
    use std::collections::BTreeMap;

    fn input(model: ModelId, params: &[(&str, f64)], times: &[f64]) -> ModelInput {
        ModelInput {
            model,
            dose: 100.0,
            params: params.iter().map(|(n, x)| (n.to_string(), *x)).collect(),
            times: times.to_vec(),
        }
    }

    fn concentrations(i: &ModelInput) -> Vec<f64> {
        run(i).map(|o| o.conc().to_vec()).unwrap_or_default()
    }

    /// Every analytic column of the three parameter sets agrees with central differences.
    #[test]
    fn analytic_columns_match_central_differences_in_every_set() {
        let times = [0.3, 1.0, 2.5, 7.0, 20.0, 60.0];
        let sets: [(ModelId, Vec<(&str, f64)>); 3] = [
            (
                ModelId::Pk3Oral1Lag,
                vec![
                    ("cl", 3.0),
                    ("vc", 8.0),
                    ("q2", 9.5),
                    ("vp2", 12.0),
                    ("q3", 2.1),
                    ("vp3", 30.0),
                    ("ka", 1.7),
                    ("tlag", 0.25),
                ],
            ),
            (
                ModelId::Pk3IvInfusion,
                vec![
                    ("k10", 0.4),
                    ("k12", 0.7),
                    ("k21", 0.3),
                    ("k13", 0.05),
                    ("k31", 0.02),
                    ("vc", 9.0),
                    ("dur", 2.0),
                ],
            ),
            (
                ModelId::Pk3IvBolus,
                vec![
                    ("a", 5.0),
                    ("b", 3.0),
                    ("c", 1.0),
                    ("alpha", 2.0),
                    ("beta", 0.4),
                    ("gamma", 0.03),
                ],
            ),
        ];
        for (model, params) in sets {
            let base = input(model, &params, &times);
            let jac = derivatives(&base, Derivatives::Analytic);
            let Ok(jac) = jac else {
                panic!("{model:?}: no analytic Jacobian");
            };
            assert_eq!(jac.method, Derivatives::Analytic);
            for (j, name) in jac.parameters.iter().enumerate() {
                let x = base.params.get(name).copied().unwrap_or(1.0);
                let h = 1e-5 * x;
                let scale = jac.columns[j].iter().fold(0.0_f64, |m, v| m.max(v.abs()));
                let mut up = base.clone();
                let mut down = base.clone();
                up.params.insert(name.clone(), x + h);
                down.params.insert(name.clone(), x - h);
                let (fu, fd) = (concentrations(&up), concentrations(&down));
                for i in 0..times.len() {
                    let numeric = (fu[i] - fd[i]) / (2.0 * h);
                    let exact = jac.columns[j][i];
                    assert!(
                        (exact - numeric).abs() <= 1e-6 * scale,
                        "{model:?} d/d{name} at t = {}: {exact} against {numeric}",
                        times[i]
                    );
                }
            }
        }
    }

    /// The exact tie k21 = k31 is the limit of the nearby untied inputs (MOD-3C-22).
    #[test]
    fn the_exact_tie_is_the_limit_of_nearby_inputs() {
        let times = [0.5, 2.0, 10.0, 40.0];
        let tied = |k31: f64| {
            let p: BTreeMap<&str, f64> = [
                ("k10", 0.4),
                ("k12", 0.6),
                ("k21", 0.25),
                ("k13", 0.1),
                ("k31", k31),
                ("vc", 10.0),
            ]
            .into_iter()
            .collect();
            let params: Vec<(&str, f64)> = p.into_iter().collect();
            concentrations(&input(ModelId::Pk3IvBolus, &params, &times))
        };
        let exact = tied(0.25);
        let near = tied(0.25 * (1.0 + 1e-7));
        assert_eq!(exact.len(), times.len());
        for (e, n) in exact.iter().zip(&near) {
            assert!((e - n).abs() <= 1e-6 * e.abs(), "{e} against {n}");
        }
    }

    /// Unknown or incomplete input is an error, not a panic or a number.
    #[test]
    fn a_refused_input_is_an_error() {
        let bad = input(ModelId::Pk3IvBolus, &[("cl", 3.0), ("vc", 8.0)], &[1.0]);
        assert!(run(&bad).is_err());
        assert!(derivatives(&bad, Derivatives::Analytic).is_err());
    }

    /// MOD-3C-08: beta onto a pole is refused alike by `run` and by `jacobian` (they share one
    /// solver, and the message names the exponent and the coupling).
    #[test]
    fn run_and_jacobian_agree_on_a_degenerate_beta() {
        let p = [
            ("k10", 1.0),
            ("k12", 0.5),
            ("k21", 2.0),
            ("k13", 1e-300),
            ("k31", 1.5),
            ("vc", 5.0),
        ];
        let i = input(ModelId::Pk3IvBolus, &p, &[1.0, 5.0]);
        let from_run = run(&i).unwrap_err();
        let from_jacobian = derivatives(&i, Derivatives::Analytic).unwrap_err();
        let numeric = derivatives(&i, Derivatives::ForwardDifference { increment: 1e-6 });
        assert!(
            matches!(from_run, ModelError::DegenerateExponents { .. }),
            "{from_run:?}"
        );
        assert!(
            matches!(from_jacobian, ModelError::DegenerateExponents { .. }),
            "{from_jacobian:?}"
        );
        assert_eq!(from_run.to_string(), from_jacobian.to_string());
        assert!(from_run.to_string().contains("beta"), "{from_run}");
        assert!(numeric.is_err());
    }
}
