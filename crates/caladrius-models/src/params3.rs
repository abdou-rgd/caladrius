//! Three-compartment parameter sets: names, completeness, domains and the conversions between the
//! three parameterisations (`specs/models.md` MOD-3C-02 to 06).
//!
//! Every conversion is a product, a quotient or a sum of positive numbers (MOD-3C-02, 3C-04); the
//! exponents and weights come from `roots3` (MOD-3C-07).

use std::collections::BTreeMap;

use crate::ModelError;
use crate::dd::Dd;
use crate::error::NO_VALID_MACRO_MODEL;
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

/// A derived quantity must be a finite normal number > 0 (MOD-3C-06 item 3): a subnormal value has
/// lost bits (and the products and quotients taken from it more still), so it is refused as an
/// underflow like zero. This function belongs to pk3: pk1 and pk2 have their own.
fn derived(name: &str, from: &str, value: f64) -> Result<f64, ModelError> {
    if value.is_finite() && value >= f64::MIN_POSITIVE {
        Ok(value)
    } else {
        Err(ModelError::DerivedOutOfRange {
            name: name.to_string(),
            from: from.to_string(),
            value,
        })
    }
}

/// The product of `factors` in the order given, each partial product being a derived quantity (a
/// partial product that goes subnormal has lost bits that a normal final product would hide).
fn normal_product(name: &str, from: &str, factors: &[f64]) -> Result<f64, ModelError> {
    let mut product = 1.0;
    for factor in factors {
        product = derived(name, from, product * factor)?;
    }
    Ok(product)
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
/// weights as given; the micro-constants decide whether the macro set is a valid model, and they
/// are reported (MOD-3C-25).
///
/// The conversion is written so that it keeps its digits where the plain evaluation loses them
/// (MOD-3C-08; the review of T-165 measured 2e-2 to 5e-2 on k12 and k13 and 9e-11 on k21 and k31
/// at the edge of zone (ii)):
/// - n1, n0 and the discriminant Δ = n1² − 4·n0 are carried in double-double from the macro
///   doubles as given (`macro_roots`); the gap k21 − k31 is √Δ, never a difference of the two
///   rounded poles;
/// - the offsets of each exponent to the two poles come from the identity
///   (λ − k21)(λ − k31) = w·Π(λ − λj) and from λ − k21 and λ − k31 differing by the gap
///   (`macro_offsets`), and k12 = Π(λi − k21)/(gap·k21), k13 = Π(λi − k31)/(−gap·k31) are products
///   of those offsets, never of differences of the rounded poles.
///
/// The derived constants are finally given to the solver of MOD-3C-07, so that a macro set is
/// never accepted on micro constants that the micro rule refuses (MOD-3C-08, second obligation).
fn from_macro(id: &str, dose: f64, p: [f64; 6]) -> Result<Disposition, ModelError> {
    let [a, b, c, alpha, beta, gamma] = p;
    if !(alpha > beta && beta > gamma) {
        return Err(ModelError::ExponentsNotDecreasing { alpha, beta, gamma });
    }
    if dose == 0.0 {
        return Err(ModelError::MacroWithoutDoseThree);
    }
    let total = derived("a + b + c", "a + b + c", a + b + c)?;
    let vc = derived("vc", "dose / (a + b + c)", dose / total)?;
    let weight = [a / total, b / total, c / total];
    let lambda = [alpha, beta, gamma];
    let (n1, n0, gap, dd) = macro_roots([a, b, c], weight, lambda);
    let n1 = derived("n1", "sum of w x (S - lambda)", n1)?;
    let n0 = derived("n0", "sum of w x lambda x lambda", n0)?;
    let triple = normal_product("alpha x beta x gamma", "alpha x beta x gamma", &lambda)?;
    let k10 = derived("k10", "alpha x beta x gamma / n0", triple / n0)?;
    // The gap of the poles is the square root of the discriminant. A discriminant that is not
    // positive cannot come from a macro set with a, b, c > 0 in exact arithmetic (MOD-3C-06
    // item 3): it is the rounding of a set whose poles are too close to be told apart.
    if !gap.is_finite() {
        return Err(ModelError::DerivedOutOfRange {
            name: "k21 - k31".to_string(),
            from: "the square root of the discriminant of the poles".to_string(),
            value: gap,
        });
    }
    if gap <= 0.0 {
        return Err(no_valid_model(
            id,
            "k21",
            "k12",
            "the poles k21 and k31 of the micro-constants derived from it cannot be told apart (the gap k21 - k31 is not positive)",
        ));
    }
    let k21 = (n1 + gap) / 2.0;
    let k31 = n0 / k21;
    let offsets = macro_offsets(lambda, weight, gap, dd, k21, k31);
    // The exponent next to a pole, judged from the offsets (zone (i) of MOD-3C-08).
    check_macro_offsets(id, offsets, k21, k31)?;
    // The coupling of a pole is the product of the three offsets of the exponents to it over
    // (gap x pole); the offsets are magnitudes, so the value is positive when it is a normal number,
    // and the products on the way are checked to be normal numbers too.
    let coupling = |name: &str, pole: &str, magnitudes: [f64; 3], k: f64| {
        let from = format!("the offsets of the exponents to the pole {pole}");
        let product = normal_product(name, &from, &magnitudes)?;
        let scale = derived(name, &from, gap * k)?;
        derived(name, &from, product / scale)
    };
    let k12 = coupling("k12", "k21", offsets.map(|(o21, _)| o21.abs()), k21)?;
    let k13 = coupling("k13", "k31", offsets.map(|(_, o31)| o31.abs()), k31)?;
    let cl = derived("cl", "k10 x vc", k10 * vc)?;
    let q2 = derived("q2", "k12 x vc", k12 * vc)?;
    let vp2 = derived("vp2", "vc x k12 / k21", vc * k12 / k21)?;
    let q3 = derived("q3", "k13 x vc", k13 * vc)?;
    let vp3 = derived("vp3", "vc x k13 / k31", vc * k13 / k31)?;
    // The second obligation of MOD-3C-08: the micro-constants reported here are accepted by the
    // micro rule. The solver of MOD-3C-07 is that rule; its answer is not used (the curve takes the
    // exponents and weights as given).
    if let Err(detail) = roots3::solve(k10, k12, k21, k13, k31) {
        return Err(ModelError::DegenerateExponents {
            model: id.to_string(),
            detail: Some(format!(
                "{NO_VALID_MACRO_MODEL}: the micro-constants derived from it are refused by the micro rule ({detail})"
            )),
        });
    }
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

/// A refusal of a macro set that gives no valid model where the doubles cannot decide: the
/// message names the pole, the coupling and what failed, and does not speak of overflow, underflow
/// or rescaling (MOD-3C-06 item 3).
fn no_valid_model(id: &str, pole: &str, coupling: &str, what: &str) -> ModelError {
    ModelError::DegenerateExponents {
        model: id.to_string(),
        detail: Some(format!(
            "{NO_VALID_MACRO_MODEL} at the pole {pole} (coupling {coupling}): {what}"
        )),
    }
}

/// What the reverse conversion of a macro set keeps in double-double: T = a + b + c and
/// Δ·T² = N1² − 4·N0·T (the discriminant of s² − n1·s + n0 scaled by T²), from which every
/// quantity below is a sum of terms that do not cancel.
#[derive(Debug, Clone, Copy)]
struct MacroDd {
    /// The coefficients a, b, c times the power of two that brings T near 1 (exact).
    scaled: [f64; 3],
    total: Dd,
    delta: Dd,
    /// N1 − 2·β·T in the same scale: its sign says which pole β lies nearer to.
    mid: f64,
}

/// The power of two 2^-e with 2^e <= t < 2^(e+1), built from the exponent bits of `t` (1 when `t`
/// is zero, subnormal or not finite; `from_macro` refuses such a T before this point): multiplying the macro coefficients by it is exact and
/// brings T = a + b + c near 1, so that the double-double quantities below (of the order of T,
/// T·n1 and T³) neither underflow nor overflow whatever the absolute size of the coefficients.
fn unit_scale(t: f64) -> f64 {
    let field = (t.to_bits() >> 52) & 0x7ff;
    if field == 0 || field == 0x7ff {
        return 1.0;
    }
    f64::from_bits((2046 - field).max(1) << 52)
}

/// n1 = Σ wi·(S − λi), n0 = Σ wi·λj·λk and the gap √(n1² − 4·n0) of the two poles, from the macro
/// doubles as given (`coefficient` the unnormalised weights a, b, c): N1 = Σ ai·(S − λi),
/// N0 = Σ ai·λj·λk and T are carried in double-double, and Δ·T² = N1² − 4·N0·T is formed in
/// double-double, where the plain double loses its digits when the poles are close (with
/// τ = gap/n1 the loss is of the order of 2⁻⁵²/τ² in Δ). Falls back on plain doubles (and `None`)
/// when an operand is too large for the split of the product.
fn macro_roots(
    coefficient: [f64; 3],
    weight: [f64; 3],
    lambda: [f64; 3],
) -> (f64, f64, f64, Option<MacroDd>) {
    let [alpha, beta, gamma] = lambda;
    let scale = unit_scale(coefficient[0] + coefficient[1] + coefficient[2]);
    let scaled = coefficient.map(|x| x * scale);
    let [a, b, c] = scaled;
    let total = Dd::sum(a, b).add_f(c);
    let sum = Dd::sum(alpha, beta).add_f(gamma);
    let term = |w: f64, minus: f64, j: f64, k: f64| {
        (sum.add_f(-minus).mul_f(w), Dd::product(j, k).mul_f(w))
    };
    let (x0, y0) = term(a, alpha, beta, gamma);
    let (x1, y1) = term(b, beta, alpha, gamma);
    let (x2, y2) = term(c, gamma, alpha, beta);
    let n1_num = x0.add(x1).add(x2);
    let n0_num = y0.add(y1).add(y2);
    let delta = n1_num.mul(n1_num).sub(n0_num.mul(total).times_four());
    let mid = n1_num.sub(total.mul_f(2.0 * beta));
    // The double-double quantities are of degree 2 in the exponents; below FLOOR their low parts are
    // subnormal and no longer carry the digits asked of them, so the plain doubles are used (and the
    // derived quantities refuse what is not a normal number).
    let floor = f64::from_bits(123 << 52);
    if total.is_finite()
        && n1_num.hi.abs() >= floor
        && n0_num.hi.abs() >= floor
        && n1_num.is_finite()
        && n0_num.is_finite()
        && delta.is_finite()
        && mid.is_finite()
    {
        let t = total.value();
        let gap = if delta.value() > 0.0 {
            libm::sqrt(delta.value()) / t
        } else {
            0.0
        };
        return (
            n1_num.value() / t,
            n0_num.value() / t,
            gap,
            Some(MacroDd {
                scaled,
                total,
                delta,
                mid: mid.value(),
            }),
        );
    }
    let [wa, wb, wc] = weight;
    let n1 = wa * (beta + gamma) + wb * (alpha + gamma) + wc * (alpha + beta);
    let n0 = wa * beta * gamma + wb * alpha * gamma + wc * alpha * beta;
    (n1, n0, libm::sqrt((n1 * n1 - 4.0 * n0).max(0.0)), None)
}

/// λi − k21 and λi − k31 for each exponent, signed, accurate to a few units in the last place
/// wherever the micro-constants are well conditioned. With N = wi·Π(λi − λj) =
/// (λi − k21)(λi − k31) (the numerator of the transfer function at the root) and the two offsets
/// differing by the gap g = k21 − k31 (k21 ≥ k31), the pair solves a quadratic whose root
/// √(g² + 4N) is the sum of the two offsets in magnitude (|o21 + o31|): for α and γ (N > 0) the
/// larger magnitude is (g + √(g² + 4N))/2 and the smaller one N over it; for β (N < 0) the
/// magnitudes add to g, the larger is again (g + √(g² + 4N))/2 and the smaller one |N| over it, the
/// smaller one belonging to the pole nearer to β. The radicand g² + 4N cancels for β near the
/// middle of the poles: it is formed in double-double, as (Δ·T² + 4·ai·Π(λi − λj)·T)/T².
fn macro_offsets(
    lambda: [f64; 3],
    weight: [f64; 3],
    gap: f64,
    dd: Option<MacroDd>,
    k21: f64,
    k31: f64,
) -> [(f64, f64); 3] {
    let [alpha, beta, gamma] = lambda;
    let [wa, wb, wc] = weight;
    // (N, √(g² + 4N)) for the exponent `li` with the others `lj`, `lk` and its coefficient.
    let at = |index: usize, w: f64, li: f64, lj: f64, lk: f64| -> (f64, f64) {
        let plain_n = w * (li - lj) * (li - lk);
        let exact = dd.and_then(|m| {
            let coefficient = *m.scaled.get(index)?;
            let product = Dd::sum(li, -lj).mul(Dd::sum(li, -lk)).mul_f(coefficient);
            let radicand = m.delta.add(product.mul(m.total).times_four());
            let t = m.total.value();
            (radicand.is_finite() && t > 0.0).then(|| {
                (
                    product.value() / t,
                    libm::sqrt(radicand.value().max(0.0)) / t,
                )
            })
        });
        exact.unwrap_or_else(|| (plain_n, libm::sqrt((gap * gap + 4.0 * plain_n).max(0.0))))
    };
    // α: both offsets positive, o31 = o21 + g.
    let (n, s) = at(0, wa, alpha, beta, gamma);
    let big = (gap + s) / 2.0;
    let alpha_offsets = (n / big, big);
    // γ: both offsets negative, |o21| = |o31| + g.
    let (n, s) = at(2, wc, gamma, alpha, beta);
    let big = (gap + s) / 2.0;
    let gamma_offsets = (-big, -(n / big));
    // β: o21 <= 0 <= o31 with o31 − o21 = g.
    let (n, s) = at(1, wb, beta, alpha, gamma);
    let big = (gap + s) / 2.0;
    let small = -n / big;
    // β is nearer the upper pole k21 when k21 + k31 = n1 <= 2β: decided in double-double when it is
    // available, from the rounded poles otherwise.
    let nearer_upper = dd.map_or(k21 - beta <= beta - k31, |m| m.mid <= 0.0);
    let beta_offsets = if nearer_upper {
        (-small, big)
    } else {
        (-big, small)
    };
    [alpha_offsets, beta_offsets, gamma_offsets]
}

/// MOD-3C-06 item 5 and MOD-3C-08 for the macro set: an exponent that lies within half a unit in
/// the last place of a pole of the micro-constants derived from it is refused, the pole and the
/// exponent cannot be told apart as doubles. The offset of the exponent to its nearer pole comes
/// from the identity of `macro_offsets`, not from the rounded poles.
fn check_macro_offsets(
    id: &str,
    offsets: [(f64, f64); 3],
    k21: f64,
    k31: f64,
) -> Result<(), ModelError> {
    // The threshold is pole·2^-53, a relative stand-in for half the spacing of the doubles at the
    // pole. For a pole in the binade [2^e, 2^(e+1)) that half spacing is 2^(e-53) on the upper
    // side (and on the lower side, except just below 2^e, where it is 2^(e-54)), so the threshold
    // is larger than it by a factor between 1 and 2 (pole / 2^e), reaching 2 for a pole just
    // below the top of a binade and for an exponent just below a pole that is a power of two.
    // The macro check therefore refuses a macro set whose exponent lies up to twice as far from a
    // pole as half a spacing, which the exact rule (the exponent rounds onto the pole) would
    // return; only that band between the exact limit and the threshold differs, and it lies in
    // the free zones of MOD-3C-08.
    let unit = f64::EPSILON / 2.0;
    let names = ["alpha", "beta", "gamma"];
    for (&(o21, o31), name) in offsets.iter().zip(names) {
        let (near, pole, pole_name, coupling) = if o21.abs() <= o31.abs() {
            (o21.abs(), k21, "k21", "k12")
        } else {
            (o31.abs(), k31, "k31", "k13")
        };
        if near.is_nan() || near < pole * unit {
            return Err(ModelError::DegenerateExponents {
                model: id.to_string(),
                detail: Some(format!(
                    "{NO_VALID_MACRO_MODEL}: the exponent {name} lies at {} from the pole {pole_name} = {} of the micro-constants derived from it (coupling {coupling}), below the rounding unit of the pole, so a root would equal a pole (a weight of the macro set is too small)",
                    crate::error::short(near),
                    crate::error::short(pole)
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
            .map(|&(n, v)| {
                if n == "q2" || n == "vp3" {
                    (n, 0.0)
                } else {
                    (n, v)
                }
            })
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

    /// The macro set of a micro set: a, b, c = dose·wi/vc and the exponents.
    fn macro_of(micro: [(&str, f64); 6]) -> Vec<(&'static str, f64)> {
        let c = resolve(ModelId::Pk3IvBolus, &given(&micro), 100.0).unwrap();
        let w = c.weight();
        let l = c.lambda();
        vec![
            ("a", 100.0 * w[0] / c.vc),
            ("b", 100.0 * w[1] / c.vc),
            ("c", 100.0 * w[2] / c.vc),
            ("alpha", l[0]),
            ("beta", l[1]),
            ("gamma", l[2]),
        ]
    }

    fn micro_set(k21: f64, k31: f64, k12: f64, k13: f64) -> [(&'static str, f64); 6] {
        [
            ("k10", 0.4),
            ("k12", k12),
            ("k21", k21),
            ("k13", k13),
            ("k31", k31),
            ("vc", 8.0),
        ]
    }

    /// Poles closer than the rounding of the plain algorithm (the edge rows of T-165): the macro set of
    /// a well conditioned micro set gives its constants back to 1e-12, and the constants it reports are
    /// accepted by the micro rule (MOD-3C-08, obligation of the free zones).
    #[test]
    fn close_poles_keep_their_constants_and_the_reported_micro_set_is_accepted() {
        let mut accepted = 0;
        for exp in [3, 6, 9, 12, 15, 18] {
            let tau = libm::pow(2.0, -f64::from(exp));
            for (k12, k13) in [(0.9, 0.5), (0.3, 1.7), (2.0, 0.05)] {
                let k21 = 1.0;
                let k31 = k21 * (1.0 - tau) / (1.0 + tau);
                let micro = micro_set(k21, k31, k12, k13);
                let m = macro_of(micro);
                let Ok(d) = resolve(ModelId::Pk3IvBolus, &given_owned(&m), 100.0) else {
                    continue;
                };
                accepted += 1;
                for (name, got) in [
                    ("k10", d.k10),
                    ("k12", d.k12),
                    ("k21", d.k21),
                    ("k13", d.k13),
                    ("k31", d.k31),
                ] {
                    let want = micro.iter().find(|(n, _)| *n == name).unwrap().1;
                    assert!(
                        (got - want).abs() <= 1e-9 * want,
                        "tau 2^-{exp}: {name} {got} against {want}"
                    );
                }
                let reported = [
                    ("k10", d.k10),
                    ("k12", d.k12),
                    ("k21", d.k21),
                    ("k13", d.k13),
                    ("k31", d.k31),
                    ("vc", d.vc),
                ];
                assert!(
                    resolve(ModelId::Pk3IvBolus, &given(&reported), 100.0).is_ok(),
                    "tau 2^-{exp}: the reported micro set is refused"
                );
            }
        }
        assert!(accepted >= 10, "only {accepted} sets accepted");
    }

    /// The micro constants a macro set resolves to (dose 100·f for coefficients multiplied by f).
    fn micro_of_macro(m: &[(&str, f64)], f: f64) -> [f64; 6] {
        let scaled: Vec<(&str, f64)> = m
            .iter()
            .map(|&(n, v)| if n.len() == 1 { (n, v * f) } else { (n, v) })
            .collect();
        let d = resolve(ModelId::Pk3IvBolus, &given(&scaled), 100.0 * f).unwrap();
        [d.k10, d.k12, d.k21, d.k13, d.k31, d.vc]
    }

    /// The conversion depends on the coefficients only through their ratios: multiplying a, b, c and
    /// the dose by a power of two, however far (past the underflow of T·n1 and the overflow of the
    /// split of 4·N0·T), gives the same micro constants to the bit (vc included: dose and coefficients scale together).
    #[test]
    fn the_conversion_does_not_depend_on_the_absolute_size_of_the_coefficients() {
        let base = macro_of(micro_set(1.0, 0.2, 0.9, 0.5));
        let tau = 1.1 * libm::pow(2.0, -20.0);
        let edge_tau = macro_of(micro_set(1.0, (1.0 - tau) / (1.0 + tau), 0.9, 0.5));
        for (label, m) in [("base", &base), ("edge_tau", &edge_tau)] {
            let unit = micro_of_macro(m, 1.0);
            for exp in [500.0, -500.0, -600.0, 900.0] {
                let f = libm::pow(2.0, exp);
                let got = micro_of_macro(m, f);
                for (g, u) in got.iter().zip(unit) {
                    assert_eq!(
                        g.to_bits(),
                        u.to_bits(),
                        "{label} x 2^{exp}: {g} against {u}"
                    );
                }
            }
        }
    }

    /// A constructed row of the band: tau = 1/2 and a root next to a pole at rho = 2.2·2^-48
    /// (rho·tau about 1.1·2^-48, k13 = 3e-15). The macro set is accepted, the constants come back to
    /// 1e-12 and are accepted when fed back as a micro set (MOD-3C-08).
    #[test]
    fn a_row_at_the_edge_of_the_band_is_re_accepted_as_micro_input() {
        let micro = micro_set(1.0, 1.0 / 3.0, 0.9, 3.0e-15);
        let m = macro_of(micro);
        let d = resolve(ModelId::Pk3IvBolus, &given(&m), 100.0).unwrap();
        for (name, got) in [
            ("k21", d.k21),
            ("k31", d.k31),
            ("k12", d.k12),
            ("k10", d.k10),
        ] {
            let want = micro.iter().find(|(n, _)| *n == name).unwrap().1;
            assert!(
                (got - want).abs() <= 1e-12 * want,
                "{name} {got} against {want}"
            );
        }
        let reported = [
            ("k10", d.k10),
            ("k12", d.k12),
            ("k21", d.k21),
            ("k13", d.k13),
            ("k31", d.k31),
            ("vc", d.vc),
        ];
        assert!(resolve(ModelId::Pk3IvBolus, &given(&reported), 100.0).is_ok());
    }

    /// A sum a + b + c that is a subnormal number is an underflow (the coefficients have lost bits
    /// already), refused as such and not as a pair of poles that cannot be told apart.
    #[test]
    fn a_subnormal_total_of_the_coefficients_is_an_underflow() {
        let tau = 1.1 * libm::pow(2.0, -20.0);
        for micro in [
            micro_set(1.0, 0.2, 0.9, 0.5),
            micro_set(1.0, (1.0 - tau) / (1.0 + tau), 0.9, 0.5),
        ] {
            let m = macro_of(micro);
            for exp in [-1030.0, -1038.0] {
                let f = libm::pow(2.0, exp);
                let scaled: Vec<(&str, f64)> = m
                    .iter()
                    .map(|&(n, v)| if n.len() == 1 { (n, v * f) } else { (n, v) })
                    .collect();
                let total: f64 = scaled.iter().take(3).map(|x| x.1).sum();
                assert!(total > 0.0 && total < f64::MIN_POSITIVE, "{total}");
                let e = resolve(ModelId::Pk3IvBolus, &given(&scaled), 100.0 * f).unwrap_err();
                assert!(
                    matches!(&e, ModelError::DerivedOutOfRange { name, .. } if name == "a + b + c"),
                    "x 2^{exp}: {e}"
                );
            }
        }
    }

    /// The exponents of the base set multiplied down until a product of three of them is a subnormal
    /// number: refused as an underflow, never answered with a constant that lost its digits.
    #[test]
    fn exponents_whose_product_is_subnormal_are_an_underflow() {
        let base = macro_of(micro_set(1.0, 0.2, 0.9, 0.5));
        let with = |f: f64| -> Result<Disposition, ModelError> {
            let scaled: Vec<(&str, f64)> = base
                .iter()
                .map(|&(n, v)| if n.len() == 1 { (n, v) } else { (n, v * f) })
                .collect();
            resolve(ModelId::Pk3IvBolus, &given(&scaled), 100.0)
        };
        for f in [1e-105, 1e-107, 1e-110] {
            let e = with(f).unwrap_err();
            assert!(
                matches!(e, ModelError::DerivedOutOfRange { .. }),
                "{f}: {e}"
            );
        }
        let kept = with(1e-95).unwrap();
        let unit = with(1.0).unwrap();
        for (g, u) in [
            (kept.k10, unit.k10),
            (kept.k12, unit.k12),
            (kept.k31, unit.k31),
        ] {
            assert!((g / 1e-95 - u).abs() <= 1e-12 * u, "{g} against {u}");
        }
    }

    fn given_owned(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        given(pairs)
    }

    /// A macro set whose smallest weight is far below the rounding unit is not a valid model: the
    /// refusal names the pole and the coupling and never speaks of overflow, underflow or rescaling.
    #[test]
    fn a_macro_set_with_a_vanishing_weight_is_refused() {
        for small in 0..3 {
            let mut w = [0.7, 0.2, 0.1];
            w[small] = 1e-20;
            let e = resolve(
                ModelId::Pk3IvBolus,
                &given(&[
                    ("a", w[0]),
                    ("b", w[1]),
                    ("c", w[2]),
                    ("alpha", 3.0),
                    ("beta", 0.5),
                    ("gamma", 0.05),
                ]),
                100.0,
            )
            .unwrap_err();
            assert!(matches!(e, ModelError::DegenerateExponents { .. }), "{e}");
            let text = e.to_string();
            assert!(text.contains("k21") || text.contains("k31"), "{text}");
            assert!(text.contains("k12") || text.contains("k13"), "{text}");
            assert!(text.contains("valid"), "{text}");
            for word in ["overflow", "underflow", "rescal"] {
                assert!(!text.contains(word), "{text}");
            }
        }
    }
}
