//! A second, deliberately naive implementation of the three-compartment models of `specs/models.md`
//! section 14, in double precision (task T-142; `AGENTS.md` section 5, source 3). It exists to test
//! the oracle files of `oracle/expected/models/pk3/` (which come from a 256-bit R script) by
//! something written in another language and another arithmetic. It shares no code with
//! `caladrius-models` or with the R script: the engine is never tested against itself.
//!
//! Naive means the textbook forms with none of the safeguards of the specification: the exponents
//! are the three real roots of the characteristic cubic by the trigonometric formula, the weights
//! are `(k21 - l_i)(k31 - l_i) / prod(l_j - l_i)`, the oral profile is the plain sum of three
//! exponentials and the absorption exponential (valid only when `ka` is not close to an exponent
//! and the exponents are not close to each other; the oracle options say which cases those are,
//! `textbook_double_ok`). The areas are the integrals of the same sums of exponentials.

use std::collections::BTreeMap;

/// One term `c * u^p * exp(-l * u)` of the concentration on a segment of constant input.
#[derive(Debug, Clone, Copy)]
struct Term {
    c: f64,
    l: f64,
    p: i32,
}

/// A stretch of time `[s0, s0 + len]` (`len` `None`: to infinity) on which the concentration is a
/// sum of terms in `u = t - s0`.
#[derive(Debug, Clone)]
struct Segment {
    s0: f64,
    len: Option<f64>,
    terms: Vec<Term>,
}

fn term(c: f64, l: f64, p: i32) -> Term {
    Term { c, l, p }
}

fn factorial(n: i32) -> f64 {
    (1..=n).map(f64::from).product()
}

/// Integral over `[0, x]` of `u^p exp(-l u)`.
fn ipow(p: i32, l: f64, x: f64) -> f64 {
    if l == 0.0 {
        return x.powi(p + 1) / f64::from(p + 1);
    }
    let y = l * x;
    let partial: f64 = (0..=p).map(|j| y.powi(j) / factorial(j)).sum();
    factorial(p) / l.powi(p + 1) * (1.0 - (-y).exp() * partial)
}

/// The micro-constants and the central volume of a parameter set (`clearance` or `micro`).
#[derive(Debug, Clone, Copy)]
pub struct Micro {
    pub vc: f64,
    pub k10: f64,
    pub k12: f64,
    pub k21: f64,
    pub k13: f64,
    pub k31: f64,
}

/// Micro-constants of a `clearance` or `micro` set; `None` for the macro set (not implemented here)
/// or a missing parameter.
pub fn micro_of(set: &str, p: &BTreeMap<String, f64>) -> Option<Micro> {
    let get = |n: &str| p.get(n).copied();
    match set {
        "clearance" => {
            let vc = get("vc")?;
            Some(Micro {
                vc,
                k10: get("cl")? / vc,
                k12: get("q2")? / vc,
                k21: get("q2")? / get("vp2")?,
                k13: get("q3")? / vc,
                k31: get("q3")? / get("vp3")?,
            })
        }
        "micro" => Some(Micro {
            vc: get("vc")?,
            k10: get("k10")?,
            k12: get("k12")?,
            k21: get("k21")?,
            k13: get("k13")?,
            k31: get("k31")?,
        }),
        _ => None,
    }
}

/// The three exponents, largest first, as the roots of `x^3 - a2 x^2 + a1 x - a0` by the
/// trigonometric formula.
pub fn exponents(m: &Micro) -> [f64; 3] {
    let a2 = m.k10 + m.k12 + m.k13 + m.k21 + m.k31;
    let a1 = m.k10 * (m.k21 + m.k31) + m.k21 * m.k31 + m.k12 * m.k31 + m.k13 * m.k21;
    let a0 = m.k10 * m.k21 * m.k31;
    let p = a1 - a2 * a2 / 3.0;
    let q = -2.0 * a2 * a2 * a2 / 27.0 + a2 * a1 / 3.0 - a0;
    let radius = 2.0 * (-p / 3.0).sqrt();
    let arg = (3.0 * q / (2.0 * p) * (-3.0 / p).sqrt()).clamp(-1.0, 1.0);
    let phi = arg.acos() / 3.0;
    let shift = a2 / 3.0;
    let mut roots: Vec<f64> = (0..3)
        .map(|k| radius * (phi - 2.0 * std::f64::consts::PI * f64::from(k) / 3.0).cos() + shift)
        .collect();
    roots.sort_by(|a, b| b.total_cmp(a));
    [roots[0], roots[1], roots[2]]
}

/// The weights of the three exponentials (they sum to 1).
pub fn weights(m: &Micro, l: &[f64; 3]) -> [f64; 3] {
    let w = |i: usize| {
        let (j, k) = match i {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        (m.k21 - l[i]) * (m.k31 - l[i]) / ((l[j] - l[i]) * (l[k] - l[i]))
    };
    [w(0), w(1), w(2)]
}

fn segments(model: &str, m: &Micro, dose: f64, p: &BTreeMap<String, f64>) -> Option<Vec<Segment>> {
    let l = exponents(m);
    let w = weights(m, &l);
    let a: Vec<f64> = w.iter().map(|wi| dose * wi / m.vc).collect();
    let tau = p.get("tlag").copied().unwrap_or(0.0);
    match model {
        "pk3.iv_bolus" => Some(vec![Segment {
            s0: 0.0,
            len: None,
            terms: (0..3).map(|i| term(a[i], l[i], 0)).collect(),
        }]),
        "pk3.iv_infusion" | "pk3.oral_0" | "pk3.oral_0_lag" => {
            let big_t = p.get("dur").copied()?;
            let mut during = Vec::new();
            let mut after = Vec::new();
            for i in 0..3 {
                let x = a[i] / (l[i] * big_t);
                during.push(term(x, 0.0, 0));
                during.push(term(-x, l[i], 0));
                after.push(term(x * (1.0 - (-l[i] * big_t).exp()), l[i], 0));
            }
            Some(vec![
                Segment {
                    s0: tau,
                    len: Some(big_t),
                    terms: during,
                },
                Segment {
                    s0: tau + big_t,
                    len: None,
                    terms: after,
                },
            ])
        }
        "pk3.oral_1" | "pk3.oral_1_lag" => {
            let ka = p.get("ka").copied()?;
            let mut terms = Vec::new();
            let mut sum = 0.0;
            for i in 0..3 {
                let c = ka * a[i] / (ka - l[i]);
                sum += c;
                terms.push(term(c, l[i], 0));
            }
            terms.push(term(-sum, ka, 0));
            Some(vec![Segment {
                s0: tau,
                len: None,
                terms,
            }])
        }
        _ => None,
    }
}

/// Concentration, AUC(0, t) and AUMC(0, t) of a three-compartment model at `t`, by the naive
/// textbook forms in double precision. `set` is `clearance` or `micro`; `params` holds the names of
/// that set plus `ka`, `dur`, `tlag` as the model needs them. `None` for an unknown model, set
/// or a missing parameter.
pub fn conc_auc_aumc(
    model: &str,
    set: &str,
    params: &BTreeMap<String, f64>,
    dose: f64,
    t: f64,
) -> Option<[f64; 3]> {
    let m = micro_of(set, params)?;
    let segs = segments(model, &m, dose, params)?;
    let bolus = model == "pk3.iv_bolus";
    let mut conc = 0.0;
    for sg in &segs {
        let inside = t >= sg.s0 && sg.len.is_none_or(|len| t <= sg.s0 + len);
        if inside {
            let u = t - sg.s0;
            conc = if !bolus && u == 0.0 {
                0.0
            } else {
                sg.terms
                    .iter()
                    .map(|x| x.c * u.powi(x.p) * (-x.l * u).exp())
                    .sum()
            };
            break;
        }
    }
    let (mut auc, mut aumc) = (0.0, 0.0);
    for sg in &segs {
        if t <= sg.s0 {
            continue;
        }
        let upper = sg.len.map_or(t - sg.s0, |len| len.min(t - sg.s0));
        for x in &sg.terms {
            auc += x.c * ipow(x.p, x.l, upper);
            aumc += x.c * (sg.s0 * ipow(x.p, x.l, upper) + ipow(x.p + 1, x.l, upper));
        }
    }
    Some([conc, auc, aumc])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> BTreeMap<String, f64> {
        [
            ("cl", 3.0),
            ("vc", 8.0),
            ("q2", 9.5),
            ("vp2", 9.5),
            ("q3", 6.3),
            ("vp3", 31.5),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }

    /// Worked example Q1 of `specs/models.md` section 14: exponents 3, 0.5, 0.05 and the bolus
    /// concentrations at 1 h and 12 h.
    #[test]
    fn the_base_case_has_the_exponents_and_bolus_values_of_q1() {
        let p = base();
        let m = micro_of("clearance", &p).unwrap_or(Micro {
            vc: f64::NAN,
            k10: f64::NAN,
            k12: f64::NAN,
            k21: f64::NAN,
            k13: f64::NAN,
            k31: f64::NAN,
        });
        let l = exponents(&m);
        assert!(
            (l[0] - 3.0).abs() < 1e-12 && (l[1] - 0.5).abs() < 1e-12 && (l[2] - 0.05).abs() < 1e-12
        );
        let at = |t: f64| conc_auc_aumc("pk3.iv_bolus", "clearance", &p, 100.0, t);
        let [c1, ..] = at(1.0).unwrap_or([f64::NAN; 3]);
        let [c12, ..] = at(12.0).unwrap_or([f64::NAN; 3]);
        assert!((c1 - 2.759_806_823_685).abs() < 5e-11, "{c1}");
        assert!((c12 - 0.740_531_047_822).abs() < 5e-11, "{c12}");
    }

    #[test]
    fn unknown_models_sets_and_missing_parameters_are_none() {
        let p = base();
        assert!(conc_auc_aumc("pk3.nothing", "clearance", &p, 100.0, 1.0).is_none());
        assert!(conc_auc_aumc("pk3.iv_bolus", "macro", &p, 100.0, 1.0).is_none());
        assert!(conc_auc_aumc("pk3.oral_1", "clearance", &p, 100.0, 1.0).is_none());
    }
}
