//! Three-compartment parameter sets: names, completeness, domains and the conversions between the
//! three parameterisations (`specs/models.md` MOD-3C-02 to 06).
//!
//! Every conversion is a product, a quotient or a sum of positive numbers (MOD-3C-02, 3C-04); the
//! exponents and weights come from `roots3` (MOD-3C-07).

use std::collections::BTreeMap;

use crate::ModelError;
use crate::model::ModelId;
use crate::roots3::{self, Exponents};

/// The parameterisation given by the user (MOD-3C-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Set {
    /// `cl`, `vc`, `q2`, `vp2`, `q3`, `vp3` (the default).
    Clearance,
    /// `k10`, `k12`, `k21`, `k13`, `k31`, `vc`.
    Micro,
    /// `a`, `b`, `c`, `alpha`, `beta`, `gamma` (the intravenous coefficients; the dose is required).
    Macro,
}

const CLEARANCE: [&str; 6] = ["cl", "vc", "q2", "vp2", "q3", "vp3"];
const MICRO: [&str; 6] = ["k10", "k12", "k21", "k13", "k31", "vc"];
const MACRO: [&str; 6] = ["a", "b", "c", "alpha", "beta", "gamma"];
/// Names whose presence decides the set (`vc` belongs to two sets).
const MARKERS: [(Set, &[&str]); 3] = [
    (Set::Clearance, &["cl", "q2", "vp2", "q3", "vp3", "vc"]),
    (Set::Micro, &["k10", "k12", "k21", "k13", "k31"]),
    (Set::Macro, &MACRO),
];
/// Names of other models that a three-compartment model never takes.
const OTHER_MODELS: [&str; 2] = ["v", "k"];

/// A checked three-compartment parameter set with every derived quantity of MOD-3C-02 to 05. Absent
/// inputs (`ka`, `dur`, `tlag`) are 0 and never read by a model without them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Disposition {
    pub set: Set,
    pub vc: f64,
    pub cl: f64,
    pub q2: f64,
    pub vp2: f64,
    pub q3: f64,
    pub vp3: f64,
    pub k10: f64,
    pub k12: f64,
    pub k21: f64,
    pub k13: f64,
    pub k31: f64,
    /// The exponents α > β > γ, their weights and the offsets to the poles (MOD-3C-07).
    pub ex: Exponents,
    /// The intravenous coefficients D·wα/vc, D·wβ/vc, D·wγ/vc (given for the macro set).
    pub coefficient: [f64; 3],
    /// The amplitudes of the curve, C = Σ amp·Φ(λ): w/vc, or the coefficient over the dose for the
    /// macro set (the form that set is written in).
    pub amp: [f64; 3],
    pub ka: f64,
    pub dur: f64,
    pub tlag: f64,
}

impl Disposition {
    pub fn lambda(&self) -> [f64; 3] {
        self.ex.lambda
    }

    pub fn weight(&self) -> [f64; 3] {
        self.ex.weight
    }
}

fn out_of_domain(name: &str, value: f64, domain: &str) -> ModelError {
    ModelError::ParameterOutOfDomain {
        name: name.to_string(),
        value,
        domain: domain.to_string(),
    }
}

/// A finite number > 0 (or >= 0 when `zero_allowed`).
fn check(name: &str, value: f64, zero_allowed: bool) -> Result<f64, ModelError> {
    let ok = value.is_finite() && (value > 0.0 || (zero_allowed && value == 0.0));
    if ok {
        Ok(value)
    } else if zero_allowed {
        Err(out_of_domain(name, value, "a finite number >= 0"))
    } else {
        Err(out_of_domain(name, value, "a finite number > 0"))
    }
}

/// A derived quantity must be a finite number > 0 (MOD-3C-06 item 3).
fn derived(name: &str, from: &str, value: f64) -> Result<f64, ModelError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(ModelError::DerivedOutOfRange {
            name: name.to_string(),
            from: from.to_string(),
            value,
        })
    }
}

/// Which set the given names select; an error for none or for two sets.
fn select(model: &str, given: &BTreeMap<String, f64>) -> Result<Set, ModelError> {
    let mut found: Option<(Set, &str)> = None;
    for (set, names) in MARKERS {
        let Some(name) = names.iter().copied().find(|n| given.contains_key(*n)) else {
            continue;
        };
        // `vc` alone with the micro markers is the micro set, not a mixture.
        if set == Set::Clearance && name == "vc" && MICRO.iter().any(|n| given.contains_key(*n)) {
            continue;
        }
        match found {
            None => found = Some((set, name)),
            Some((_, first)) => {
                return Err(ModelError::MixedParameterSets {
                    first: first.to_string(),
                    second: name.to_string(),
                    model: model.to_string(),
                });
            }
        }
    }
    found.map(|(set, _)| set).ok_or(ModelError::NoParameterSet {
        model: model.to_string(),
    })
}

/// Checks the names against the model, selects the set, checks every domain (MOD-3C-06,
/// MOD-GEN-04) and converts (MOD-3C-02 to 04). `dose` is checked here too: the macro set needs it.
pub(crate) fn resolve(
    model: ModelId,
    given: &BTreeMap<String, f64>,
    dose: f64,
) -> Result<Disposition, ModelError> {
    let id = model.id();
    let inputs = model.input_parameters();
    for name in given.keys() {
        let known_here = CLEARANCE.contains(&name.as_str())
            || MICRO.contains(&name.as_str())
            || MACRO.contains(&name.as_str());
        if known_here || inputs.contains(&name.as_str()) {
            continue;
        }
        let known_elsewhere =
            OTHER_MODELS.contains(&name.as_str()) || ["ka", "dur", "tlag"].contains(&name.as_str());
        return Err(if known_elsewhere {
            ModelError::ParameterNotInModel {
                name: name.clone(),
                model: id.to_string(),
            }
        } else {
            ModelError::UnknownParameter {
                name: name.clone(),
                model: id.to_string(),
            }
        });
    }
    // Mixed sets come before missing names: a mixture is the more useful message.
    let set = select(id, given)?;
    let names = match set {
        Set::Clearance => CLEARANCE,
        Set::Micro => MICRO,
        Set::Macro => MACRO,
    };
    let required = |name: &str| {
        given
            .get(name)
            .copied()
            .ok_or_else(|| ModelError::MissingParameter {
                name: name.to_string(),
                model: id.to_string(),
            })
    };
    let mut values = [0.0; 6];
    for (slot, name) in values.iter_mut().zip(names) {
        *slot = required(name)?;
    }
    for name in inputs {
        required(name)?;
    }
    // Domains, in the order of the set (MOD-3C-06 items 1 and 2). A peripheral compartment that
    // is missing (a flow, a volume or a rate constant of 0) is reported after the others.
    let mut missing: Option<&str> = None;
    let mut peripherals_missing = [false, false];
    for (value, name) in values.iter().zip(names) {
        let peripheral = match (set, name) {
            (Set::Clearance, "q2" | "vp2") | (Set::Micro, "k12") => Some(0),
            (Set::Clearance, "q3" | "vp3") | (Set::Micro, "k13") => Some(1),
            _ => None,
        };
        match peripheral {
            Some(which) if *value == 0.0 => {
                missing.get_or_insert(name);
                if let Some(flag) = peripherals_missing.get_mut(which) {
                    *flag = true;
                }
            }
            _ => {
                check(name, *value, false)?;
            }
        }
    }
    if let Some(name) = missing {
        return Err(ModelError::PeripheralMissing {
            name: name.to_string(),
            model: id.to_string(),
            both: peripherals_missing[0] && peripherals_missing[1],
        });
    }
    let input = |name: &str, zero_allowed: bool| -> Result<f64, ModelError> {
        if inputs.contains(&name) {
            check(name, required(name)?, zero_allowed)
        } else {
            Ok(0.0)
        }
    };
    let ka = input("ka", false)?;
    let dur = input("dur", false)?;
    let tlag = input("tlag", true)?;
    if !(dose.is_finite() && dose >= 0.0) {
        return Err(ModelError::InvalidDose { value: dose });
    }
    let [p0, p1, p2, p3, p4, p5] = values;
    let mut out = match set {
        Set::Clearance => from_clearance(id, dose, [p0, p1, p2, p3, p4, p5])?,
        Set::Micro => from_micro(id, dose, [p0, p1, p2, p3, p4, p5])?,
        Set::Macro => from_macro(id, dose, [p0, p1, p2, p3, p4, p5])?,
    };
    out.ka = ka;
    out.dur = dur;
    out.tlag = tlag;
    Ok(out)
}

/// The exponents of the micro-constants, or the error that names why they cannot be returned.
fn exponents(id: &str, [k, k12, k21, k13, k31]: [f64; 5]) -> Result<Exponents, ModelError> {
    roots3::solve(k, k12, k21, k13, k31).map_err(|detail| ModelError::DegenerateExponents {
        model: id.to_string(),
        detail: Some(detail),
    })
}

/// A disposition from the micro-constants, the volume and the dose (the three exponents solved).
#[allow(clippy::too_many_arguments)]
fn assemble(
    id: &str,
    set: Set,
    dose: f64,
    vc: f64,
    cl: f64,
    [q2, vp2, q3, vp3]: [f64; 4],
    [k10, k12, k21, k13, k31]: [f64; 5],
) -> Result<Disposition, ModelError> {
    let ex = exponents(id, [k10, k12, k21, k13, k31])?;
    let coefficient = ex.weight.map(|w| dose * w / vc);
    let amp = ex.weight.map(|w| w / vc);
    Ok(Disposition {
        set,
        vc,
        cl,
        q2,
        vp2,
        q3,
        vp3,
        k10,
        k12,
        k21,
        k13,
        k31,
        ex,
        coefficient,
        amp,
        ka: 0.0,
        dur: 0.0,
        tlag: 0.0,
    })
}

/// Clearance set: k10 = cl/vc, k12 = q2/vc, k21 = q2/vp2, k13 = q3/vc, k31 = q3/vp3 (MOD-3C-02).
fn from_clearance(id: &str, dose: f64, p: [f64; 6]) -> Result<Disposition, ModelError> {
    let [cl, vc, q2, vp2, q3, vp3] = p;
    let k10 = derived("k10", "cl / vc", cl / vc)?;
    let k12 = derived("k12", "q2 / vc", q2 / vc)?;
    let k21 = derived("k21", "q2 / vp2", q2 / vp2)?;
    let k13 = derived("k13", "q3 / vc", q3 / vc)?;
    let k31 = derived("k31", "q3 / vp3", q3 / vp3)?;
    assemble(
        id,
        Set::Clearance,
        dose,
        vc,
        cl,
        [q2, vp2, q3, vp3],
        [k10, k12, k21, k13, k31],
    )
}

/// Micro set: cl = k10·vc, q2 = k12·vc, vp2 = vc·k12/k21, q3 = k13·vc, vp3 = vc·k13/k31.
fn from_micro(id: &str, dose: f64, p: [f64; 6]) -> Result<Disposition, ModelError> {
    let [k10, k12, k21, k13, k31, vc] = p;
    let cl = derived("cl", "k10 x vc", k10 * vc)?;
    let q2 = derived("q2", "k12 x vc", k12 * vc)?;
    let vp2 = derived("vp2", "vc x k12 / k21", vc * k12 / k21)?;
    let q3 = derived("q3", "k13 x vc", k13 * vc)?;
    let vp3 = derived("vp3", "vc x k13 / k31", vc * k13 / k31)?;
    assemble(
        id,
        Set::Micro,
        dose,
        vc,
        cl,
        [q2, vp2, q3, vp3],
        [k10, k12, k21, k13, k31],
    )
}

/// MOD-3C-04: vc = D/(a + b + c), w = a/(a + b + c), then k21 and k31 the two roots of
/// s² − n1·s + n0 (the larger first: k21 ≥ k31, MOD-3C-05), k10 = αβγ/n0, and k12, k13 from
/// −P(k21)/[(k21 − k31)·k21] and −P(k31)/[(k31 − k21)·k31]. The curve uses the exponents and
/// weights as given; the micro-constants only decide whether the macro set is a valid model, by
/// the solver of MOD-3C-07 (the derived set must give back distinct exponents).
fn from_macro(id: &str, dose: f64, p: [f64; 6]) -> Result<Disposition, ModelError> {
    let [a, b, c, alpha, beta, gamma] = p;
    if !(alpha > beta && beta > gamma) {
        return Err(ModelError::ExponentsNotDecreasing { alpha, beta, gamma });
    }
    if dose == 0.0 {
        return Err(ModelError::MacroWithoutDose);
    }
    let total = derived("a + b + c", "a + b + c", a + b + c)?;
    let vc = derived("vc", "dose / (a + b + c)", dose / total)?;
    let weight = [a / total, b / total, c / total];
    let [wa, wb, wc] = weight;
    let n1 = wa * (beta + gamma) + wb * (alpha + gamma) + wc * (alpha + beta);
    let n0 = wa * beta * gamma + wb * alpha * gamma + wc * alpha * beta;
    let n1 = derived("n1", "sum of w x (S - lambda)", n1)?;
    let n0 = derived("n0", "sum of w x lambda x lambda", n0)?;
    // A negative discriminant can only come from rounding at a tie (the roots are real in exact
    // arithmetic, MOD-3C-04); it is taken as 0, and the equal roots are refused below.
    let discriminant = (n1 * n1 - 4.0 * n0).max(0.0);
    let k21 = (n1 + libm::sqrt(discriminant)) / 2.0;
    let k31 = n0 / k21;
    let k10 = derived("k10", "alpha x beta x gamma / n0", alpha * beta * gamma / n0)?;
    let gap = k21 - k31;
    if gap <= 0.0 {
        // No macro set with positive a, b, c reaches this in exact arithmetic (MOD-3C-06 item 3).
        return Err(ModelError::DerivedOutOfRange {
            name: "k21 - k31".to_string(),
            from: "the roots of s^2 - n1 s + n0".to_string(),
            value: gap,
        });
    }
    let k12 = derived(
        "k12",
        "-P(k21) / ((k21 - k31) x k21)",
        (alpha - k21) * (k21 - beta) * (k21 - gamma) / (gap * k21),
    )?;
    let k13 = derived(
        "k13",
        "-P(k31) / ((k31 - k21) x k31)",
        (alpha - k31) * (beta - k31) * (k31 - gamma) / (gap * k31),
    )?;
    let cl = derived("cl", "k10 x vc", k10 * vc)?;
    let q2 = derived("q2", "k12 x vc", k12 * vc)?;
    let vp2 = derived("vp2", "vc x k12 / k21", vc * k12 / k21)?;
    let q3 = derived("q3", "k13 x vc", k13 * vc)?;
    let vp3 = derived("vp3", "vc x k13 / k31", vc * k13 / k31)?;
    let lambda = [alpha, beta, gamma];
    check_macro_offsets(id, lambda, weight, k21, k31)?;
    // The offsets of the macro set are only used as differences between the exponents: they are
    // written about β, which is the exact difference of the given exponents.
    let ex = Exponents {
        lambda,
        weight,
        o21: [alpha - beta, 0.0, gamma - beta],
        o31: [alpha - beta, 0.0, gamma - beta],
        tie: false,
    };
    Ok(Disposition {
        set: Set::Macro,
        vc,
        cl,
        q2,
        vp2,
        q3,
        vp3,
        k10,
        k12,
        k21,
        k13,
        k31,
        ex,
        coefficient: [a, b, c],
        amp: [a / dose, b / dose, c / dose],
        ka: 0.0,
        dur: 0.0,
        tlag: 0.0,
    })
}

/// MOD-3C-06 item 5 and MOD-3C-08 for the macro set: the micro-constants derived from it are rounded,
/// so the offset of an exponent to the pole it lies next to is not read from them. It is the
/// partial-fraction identity N(λ) = (λ − k21)(λ − k31) = w·Π(λ − λj) of the numerator N(s) =
/// s² − n1·s + n0 of MOD-3C-04: the offset to the nearer pole is N(λ) over the (large) distance to
/// the other pole. An exponent that lies within half a unit in the last place of a pole is
/// refused: the pole and the exponent cannot be told apart as doubles.
fn check_macro_offsets(
    id: &str,
    lambda: [f64; 3],
    weight: [f64; 3],
    k21: f64,
    k31: f64,
) -> Result<(), ModelError> {
    // Half a unit in the last place, relative to the pole: 2^-53.
    let unit = f64::EPSILON / 2.0;
    let names = ["alpha", "beta", "gamma"];
    for (i, ((&root, &w), name)) in lambda.iter().zip(&weight).zip(names).enumerate() {
        let others: Vec<f64> = lambda
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, x)| *x)
            .collect();
        let numerator = others.iter().fold(w, |acc, other| acc * (root - other));
        let (d21, d31) = (root - k21, root - k31);
        // The farther pole by subtraction (large), the nearer by the identity.
        let (near, pole, pole_name) = if d21.abs() >= d31.abs() {
            (numerator / d21, k31, "k31")
        } else {
            (numerator / d31, k21, "k21")
        };
        if near.abs() < pole * unit {
            return Err(ModelError::DegenerateExponents {
                model: id.to_string(),
                detail: Some(format!(
                    "the macro set (a, b, c, alpha, beta, gamma) is not valid: the exponent {name} lies at {:e} from the pole {pole_name} = {pole:e} of the micro-constants derived from it, below the rounding unit of the pole, so a root would equal a pole (a weight of the macro set is too small)",
                    near.abs()
                )),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn given(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-13 * b.abs()
    }

    /// Worked example Q1: the three sets of one point give the same disposition.
    #[test]
    fn the_three_sets_of_worked_example_q1_agree() {
        let c = resolve(
            ModelId::Pk3IvBolus,
            &given(&[
                ("cl", 3.0),
                ("vc", 8.0),
                ("q2", 9.5),
                ("vp2", 9.5),
                ("q3", 6.3),
                ("vp3", 31.5),
            ]),
            100.0,
        )
        .unwrap();
        assert_eq!(c.set, Set::Clearance);
        assert!(close(c.k10, 0.375) && close(c.k12, 1.1875) && close(c.k21, 1.0));
        assert!(close(c.k13, 0.7875) && close(c.k31, 0.2));
        for (got, want) in c.lambda().iter().zip([3.0, 0.5, 0.05]) {
            assert!(close(*got, want), "{got} against {want}");
        }
        let m = resolve(
            ModelId::Pk3IvBolus,
            &given(&[
                ("k10", 0.375),
                ("k12", 1.1875),
                ("k21", 1.0),
                ("k13", 0.7875),
                ("k31", 0.2),
                ("vc", 8.0),
            ]),
            100.0,
        )
        .unwrap();
        assert_eq!(m.set, Set::Micro);
        assert!(close(m.cl, 3.0) && close(m.q2, 9.5) && close(m.vp2, 9.5));
        assert!(close(m.q3, 6.3) && close(m.vp3, 31.5));
        let x = resolve(
            ModelId::Pk3IvBolus,
            &given(&[
                ("a", 100.0 * c.weight()[0] / 8.0),
                ("b", 100.0 * c.weight()[1] / 8.0),
                ("c", 100.0 * c.weight()[2] / 8.0),
                ("alpha", 3.0),
                ("beta", 0.5),
                ("gamma", 0.05),
            ]),
            100.0,
        )
        .unwrap();
        assert_eq!(x.set, Set::Macro);
        for (got, want) in [
            (x.vc, 8.0),
            (x.k10, 0.375),
            (x.k12, 1.1875),
            (x.k21, 1.0),
            (x.k13, 0.7875),
            (x.k31, 0.2),
            (x.cl, 3.0),
            (x.q2, 9.5),
            (x.vp2, 9.5),
            (x.q3, 6.3),
            (x.vp3, 31.5),
        ] {
            assert!((got - want).abs() <= 1e-12 * want, "{got} against {want}");
        }
    }

    #[test]
    fn refusals_name_what_to_fix() {
        let e = |pairs: &[(&str, f64)], dose: f64| {
            resolve(ModelId::Pk3Oral1, &given(pairs), dose)
                .unwrap_err()
                .to_string()
        };
        let base = [
            ("cl", 3.0),
            ("vc", 8.0),
            ("q2", 9.5),
            ("vp2", 9.5),
            ("q3", 6.3),
            ("vp3", 31.5),
            ("ka", 1.0),
        ];
        let with = |name: &str, value: f64| -> Vec<(&str, f64)> {
            base.iter()
                .map(|&(n, v)| if n == name { (n, value) } else { (n, v) })
                .collect()
        };
        assert!(e(&with("q2", 0.0), 100.0).contains("pk2"));
        let both: Vec<(&str, f64)> = base
            .iter()
            .map(|&(n, v)| if n == "q2" || n == "vp3" { (n, 0.0) } else { (n, v) })
            .collect();
        assert!(e(&both, 100.0).contains("pk1"));
        assert!(e(&with("vp3", -1.0), 100.0).contains("`vp3`"));
        assert!(e(&with("ka", 0.0), 100.0).contains("`ka`"));
        assert!(e(&base, -1.0).contains("dose"));
        assert!(e(&[], 1.0).contains("alpha"));
        let mixed = e(&[("cl", 3.0), ("k12", 0.4), ("ka", 1.0)], 1.0);
        assert!(mixed.contains("`cl`") && mixed.contains("`k12`"), "{mixed}");
        let macro_set = |alpha: f64, beta: f64, gamma: f64| {
            [
                ("a", 0.7),
                ("b", 0.2),
                ("c", 0.1),
                ("alpha", alpha),
                ("beta", beta),
                ("gamma", gamma),
                ("ka", 1.0),
            ]
        };
        assert!(e(&macro_set(3.0, 0.5, 0.05), 0.0).contains("dose"));
        let order = e(&macro_set(3.0, 0.05, 0.5), 1.0);
        assert!(order.contains("beta") && order.contains("gamma"), "{order}");
        assert!(
            e(&[("v", 2.0), ("cl", 3.0), ("vc", 8.0), ("ka", 1.0)], 1.0).contains("`v`"),
            "v is a one-compartment name"
        );
    }

    /// A macro set whose smallest weight is far below the rounding unit is not a valid model.
    #[test]
    fn a_macro_set_with_a_vanishing_weight_is_refused() {
        let e = resolve(
            ModelId::Pk3IvBolus,
            &given(&[
                ("a", 1e-20),
                ("b", 0.7),
                ("c", 0.3),
                ("alpha", 3.0),
                ("beta", 0.5),
                ("gamma", 0.05),
            ]),
            100.0,
        )
        .unwrap_err();
        assert!(matches!(e, ModelError::DegenerateExponents { .. }), "{e}");
    }
}
