//! The three exponents and weights of the three-compartment model from its micro-constants
//! (`specs/models.md` MOD-3C-03, 3C-07, 3C-08, 3C-22).
//!
//! The exponents are the roots of the secular equation
//! F(x) = k − x + k12·x/(x − k21) + k13·x/(x − k31) = 0, equivalently (with S0 = k + k12 + k13,
//! C21 = k12·k21, C31 = k13·k31) S0 − x + C21/(x − k21) + C31/(x − k31) = 0. F is strictly
//! decreasing between its poles, so there is exactly one root in each of (d_max, ∞),
//! (d_min, d_max) and (0, d_min). Each root is found as an *offset* from the pole it lies next to
//! (the differences x − k21, x − k31 are then differences of offsets, never a subtraction of two
//! rounded numbers), by a safeguarded Newton iteration seeded by the trigonometric form of the
//! cubic and kept inside the interlacing bracket by bisection. The weights are the second form of
//! MOD-3C-03, wi = 1/[1 + C21/(λi − k21)² + C31/(λi − k31)²], a sum of positive terms evaluated
//! with those offsets.
//!
//! Only sums, products, quotients, `sqrt`, `hypot`, `acos` and `cos` of libm are used (golden rule
//! 10). The iteration has a fixed cap; a root that has not converged is an error, never a partial
//! result.

/// Iteration cap of the safeguarded Newton iteration of one root.
const CAP: usize = 500;

/// The unit roundoff 2⁻⁵² (the machine epsilon), the scale of the convergence test.
const EPS: f64 = f64::EPSILON;

/// A Newton step this small, relative to the offset, is a converged root.
const STEP_TOLERANCE: f64 = 16.0 * EPS;

/// The three exponents α > β > γ > 0 with their weights and their offsets to the two poles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Exponents {
    /// α, β, γ.
    pub lambda: [f64; 3],
    /// wα, wβ, wγ, all > 0 except wβ = 0 at the exact tie (MOD-3C-22).
    pub weight: [f64; 3],
    /// λi − k21 for each root, accurate to rounding (zero for β at the tie).
    pub o21: [f64; 3],
    /// λi − k31 for each root, accurate to rounding (zero for β at the tie).
    pub o31: [f64; 3],
    /// k21 = k31 exactly: β = k21 and wβ = 0.
    pub tie: bool,
}

impl Exponents {
    /// λi − λj as a difference of two offsets to the same pole (the pole whose offsets are
    /// smaller), never a subtraction of the two rounded exponents. NaN for an index above 2.
    pub fn gap(&self, i: usize, j: usize) -> f64 {
        let pick = |a: [f64; 3], k: usize| a.get(k).copied().unwrap_or(f64::NAN);
        let by21 = pick(self.o21, i) - pick(self.o21, j);
        let by31 = pick(self.o31, i) - pick(self.o31, j);
        let size21 = pick(self.o21, i).abs().max(pick(self.o21, j).abs());
        let size31 = pick(self.o31, i).abs().max(pick(self.o31, j).abs());
        if size21 <= size31 { by21 } else { by31 }
    }
}

/// One root next to a pole: the root itself and its offsets to the lower and the upper pole
/// (`lambda − p_lo`, `lambda − p_hi`).
struct Root {
    lambda: f64,
    o_lo: f64,
    o_hi: f64,
}

/// (hi, lo) with hi + lo = a + b exactly.
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

/// The sum S0 = k + k12 + k13 as an unevaluated pair, so that S0 − p keeps its digits.
struct Sum {
    hi: f64,
    lo: f64,
}

impl Sum {
    fn new(k: f64, k12: f64, k13: f64) -> Sum {
        let (s1, e1) = two_sum(k, k12);
        let (s2, e2) = two_sum(s1, k13);
        Sum {
            hi: s2,
            lo: e1 + e2,
        }
    }

    /// S0 − p.
    fn minus(&self, p: f64) -> f64 {
        (self.hi - p) + self.lo
    }
}

/// The secular function of the two poles, F(x) = k − x + r_lo·x/(x − p_lo) + r_hi·x/(x − p_hi) (r the
/// rate attached to the pole: k12 for k21, k13 for k31). A term is written in one of two forms:
/// shifted, r·x/(x − p) = r + C/(x − p) with C = r·p (the singular part C/(x − p) alone, the r
/// joining the smooth part, which keeps its digits as a double-double sum), or direct, r·x/(x − p).
/// The shifted form is the right one next to the pole; for a pole far above x it adds r to a term
/// of about −r and cancels (up to six digits when the coupling is large), so the direct form is
/// used there.
struct Secular {
    k: f64,
    p_lo: f64,
    p_hi: f64,
    r_lo: f64,
    r_hi: f64,
    c_lo: f64,
    c_hi: f64,
}

impl Secular {
    /// The constant part k + (the shifted rates) − p_ref, `p_ref` being the pole the root is
    /// measured from (always shifted); `other_shifted` says whether the other pole is.
    fn base(&self, ref_lo: bool, other_shifted: bool) -> f64 {
        let (r_ref, r_other, p_ref) = if ref_lo {
            (self.r_lo, self.r_hi, self.p_lo)
        } else {
            (self.r_hi, self.r_lo, self.p_hi)
        };
        Sum::new(self.k, r_ref, if other_shifted { r_other } else { 0.0 }).minus(p_ref)
    }

    /// (F, dF/dx) at x with its offsets `o_lo = x − p_lo` and `o_hi = x − p_hi`.
    fn at(
        &self,
        base: f64,
        ref_lo: bool,
        other_shifted: bool,
        x: f64,
        o_lo: f64,
        o_hi: f64,
    ) -> (f64, f64) {
        let o_ref = if ref_lo { o_lo } else { o_hi };
        let lo_term = if ref_lo || other_shifted {
            self.c_lo / o_lo
        } else {
            self.r_lo * x / o_lo
        };
        let hi_term = if !ref_lo || other_shifted {
            self.c_hi / o_hi
        } else {
            self.r_hi * x / o_hi
        };
        (
            base - o_ref + lo_term + hi_term,
            -1.0 - (self.c_lo / o_lo) / o_lo - (self.c_hi / o_hi) / o_hi,
        )
    }
}

/// The root of a strictly monotone `f` in (0, `hi`] by a safeguarded Newton iteration:
/// `f(u)` returns (F, dF/du); `decreasing` says F decreases with u. `start` is the seed (the
/// upper end `hi` when it is not inside the bracket). `None` when the cap is reached, or a value
/// is not a number.
fn refine(start: f64, hi: f64, decreasing: bool, f: impl Fn(f64) -> (f64, f64)) -> Option<f64> {
    let mut lo = 0.0_f64;
    let mut hi = hi;
    let mut u = if start > lo && start < hi { start } else { hi };
    let mut step_old = hi - lo;
    let mut step = step_old;
    for _ in 0..CAP {
        let (fv, dv) = f(u);
        if fv.is_nan() || dv.is_nan() {
            return None;
        }
        if fv == 0.0 {
            return Some(u);
        }
        // F > 0 with F decreasing (or F < 0 with F increasing): the root lies above u.
        if (fv > 0.0) == decreasing {
            lo = u;
        } else {
            hi = u;
        }
        // No double between the ends: the root is known to the last bit.
        let Some(mid) = bisection_point(lo, hi) else {
            return Some(if lo > 0.0 { lo } else { hi });
        };
        let next = if fv.is_finite() && dv.is_finite() && dv != 0.0 {
            u - fv / dv
        } else {
            f64::NAN
        };
        let newton =
            next > lo && next < hi && (2.0 * fv).abs() <= (step_old * dv).abs() && next != u;
        step_old = step;
        if newton {
            step = next - u;
            u = next;
            if step.abs() <= STEP_TOLERANCE * u.abs() {
                return Some(u);
            }
        } else {
            step = mid - u;
            u = mid;
        }
    }
    None
}

/// A point strictly inside (lo, hi): the geometric mean when the bracket spans more than a factor
/// 4 (or a fixed fraction of `hi` from 0), else the arithmetic mean. `None` when no double lies
/// strictly between `lo` and `hi`.
fn bisection_point(lo: f64, hi: f64) -> Option<f64> {
    let mid = if lo == 0.0 {
        hi * f64::from_bits(0x3DF0_0000_0000_0000) // 2⁻³²
    } else if hi > 4.0 * lo {
        libm::sqrt(lo) * libm::sqrt(hi)
    } else {
        lo + (hi - lo) / 2.0
    };
    (mid > lo && mid < hi).then_some(mid)
}

/// The trigonometric seeds (α0, β0, γ0) of MOD-3C-07 step 1, or `None` when the cubic is too
/// degenerate for them (non-finite intermediate values).
fn trigonometric_seeds(k: f64, k12: f64, k21: f64, k13: f64, k31: f64) -> Option<(f64, f64, f64)> {
    let a2 = k + k12 + k13 + k21 + k31;
    let a1 = k * k21 + k * k31 + k21 * k13 + k31 * k12 + k21 * k31;
    let a0 = k * k21 * k31;
    let m2 = (a2 * a2 / 3.0 - a1) / 3.0;
    if !(m2.is_finite() && m2 > 0.0) {
        return None;
    }
    let m = libm::sqrt(m2);
    let c3 = (2.0 * a2 * a2 * a2 / 27.0 - a1 * a2 / 3.0 + a0) / (2.0 * m * m * m);
    if !c3.is_finite() {
        return None;
    }
    let theta = libm::acos(c3.clamp(-1.0, 1.0));
    let third = a2 / 3.0;
    let two_pi_third = 2.0 * std::f64::consts::PI / 3.0;
    let at = |angle: f64| third + 2.0 * m * libm::cos(angle);
    let seeds = (
        at(theta / 3.0),
        at(theta / 3.0 - two_pi_third),
        at(theta / 3.0 + two_pi_third),
    );
    (seeds.0.is_finite() && seeds.1.is_finite() && seeds.2.is_finite()).then_some(seeds)
}

/// The weight 1/[1 + C21/o21² + C31/o31²] from the two offsets to the poles (each term formed as
/// (C/o)/o, which neither underflows nor overflows early); `None` unless it is finite and > 0.
fn weight(c_lo: f64, o_lo: f64, c_hi: f64, o_hi: f64) -> Option<f64> {
    let g = 1.0 + (c_lo / o_lo) / o_lo + (c_hi / o_hi) / o_hi;
    let w = 1.0 / g;
    (g.is_finite() && w.is_finite() && w > 0.0).then_some(w)
}

/// What the caller reports: which quantity could not be returned and why.
pub(crate) type Failure = String;

/// Solves the secular equation for the micro-constants (all finite and > 0) and returns the
/// exponents with their weights and offsets (MOD-3C-07).
pub(crate) fn solve(k: f64, k12: f64, k21: f64, k13: f64, k31: f64) -> Result<Exponents, Failure> {
    let c21 = k12 * k21;
    let c31 = k13 * k31;
    let usable = |x: f64| x.is_finite() && x > 0.0;
    if !usable(c21) {
        return Err(format!(
            "k12 x k21 = {c21:e} underflows (or overflows) in double precision"
        ));
    }
    if !usable(c31) {
        return Err(format!(
            "k13 x k31 = {c31:e} underflows (or overflows) in double precision"
        ));
    }
    let a0 = k * k21 * k31;
    if !usable(a0) {
        return Err(format!(
            "a0 = k10 x k21 x k31 = {a0:e} underflows (or overflows) in double precision"
        ));
    }
    let sum = Sum::new(k, k12, k13);
    let a2 = k + k12 + k13 + k21 + k31;
    if !(usable(sum.hi) && usable(a2)) {
        return Err("the sum of the rate constants overflows in double precision".to_string());
    }
    if k21 == k31 {
        return tie(k, k21, c21 + c31, &sum);
    }
    // Poles in increasing order, with the coupling attached to each.
    let lo_is_21 = k21 < k31;
    let (p_lo, c_lo, r_lo, p_hi, c_hi, r_hi) = if lo_is_21 {
        (k21, c21, k12, k31, c31, k13)
    } else {
        (k31, c31, k13, k21, c21, k12)
    };
    let delta = p_hi - p_lo;
    let (name_lo, name_hi) = if lo_is_21 {
        ("k12 x k21", "k13 x k31")
    } else {
        ("k13 x k31", "k12 x k21")
    };
    let f = Secular {
        k,
        p_lo,
        p_hi,
        r_lo,
        r_hi,
        c_lo,
        c_hi,
    };
    // The far pole is written in the shifted form too when it is within a factor 4 of the near
    // one; otherwise in the direct form. A far pole p above the root x makes the shifted term
    // r + C/(x − p) cancel by the factor (|r| + |C/(x − p)|) / |r·x/(x − p)| = (2p − x)/x. Measured
    // over 400 000 draws with p_hi <= 4·p_lo (T-167): at most 6.997 for β measured from the lower
    // pole (x >= p_lo, so at most 7 by the formula) and at most 14.96 for γ in (p_lo/2, p_lo) (at
    // most 15); the digits lost are those of that factor, about one, against up to six without it.
    let near_other = p_hi <= 4.0 * p_lo;
    let seeds = trigonometric_seeds(k, k12, k21, k13, k31);
    let unconverged = |what: &str| {
        format!("the iteration for the exponent {what} did not converge in {CAP} steps")
    };

    // α = p_hi + u, u > 0: F decreasing in u.
    let base_alpha = f.base(false, true);
    let f_alpha = |u: f64| f.at(base_alpha, false, true, p_hi + u, u + delta, u);
    // F(u) < 0 at the upper end of the bracket: a2 − p_hi bounds the root, enlarged until it does.
    let mut hi = sum.hi + p_lo;
    for _ in 0..64 {
        if f_alpha(hi).0 < 0.0 {
            break;
        }
        hi *= 2.0;
    }
    let u_alpha = refine(seeds.map_or(f64::NAN, |s| s.0 - p_hi), hi, true, f_alpha)
        .ok_or_else(|| unconverged("alpha"))?;
    let alpha = Root {
        lambda: p_hi + u_alpha,
        o_lo: u_alpha + delta,
        o_hi: u_alpha,
    };
    if alpha.lambda <= p_hi {
        return Err(format!(
            "the exponent alpha would equal the pole {}: its offset {u_alpha:e} is below the rounding unit of the pole ({name_hi} is too small to separate them in double precision)",
            pole_name(lo_is_21, false)
        ));
    }

    // γ: below p_lo.
    let half = p_lo / 2.0;
    let f_direct = |x: f64| {
        // F(x) = k − x − k12·x/(k21 − x) − k13·x/(k31 − x), decreasing, 0 < x < min(k21, k31).
        let (d21, d31) = (k21 - x, k31 - x);
        (
            k - x - k12 * x / d21 - k13 * x / d31,
            -1.0 - c21 / d21 / d21 - c31 / d31 / d31,
        )
    };
    let f_half = f_direct(half).0;
    let base_lo = f.base(true, near_other);
    let gamma = if f_half <= 0.0 {
        // The root is in (0, p_lo/2]: solved for itself; both offsets are far from it.
        let x = if f_half == 0.0 {
            half
        } else {
            refine(seeds.map_or(f64::NAN, |s| s.2), half, true, f_direct)
                .ok_or_else(|| unconverged("gamma"))?
        };
        Root {
            lambda: x,
            o_lo: x - p_lo,
            o_hi: x - p_hi,
        }
    } else {
        // The root is in (p_lo/2, p_lo): γ = p_lo − u, F increasing in u.
        let f_gamma = |u: f64| {
            let (value, slope) = f.at(base_lo, true, near_other, p_lo - u, -u, -(u + delta));
            (value, -slope)
        };
        let u = refine(seeds.map_or(f64::NAN, |s| p_lo - s.2), half, false, f_gamma)
            .ok_or_else(|| unconverged("gamma"))?;
        Root {
            lambda: p_lo - u,
            o_lo: -u,
            o_hi: -(u + delta),
        }
    };
    if gamma.lambda >= p_lo {
        return Err(format!(
            "the exponent gamma would equal the pole {}: its offset is below the rounding unit of the pole ({name_lo} is too small to separate them in double precision)",
            pole_name(lo_is_21, true)
        ));
    }
    if !usable(gamma.lambda) {
        return Err("the exponent gamma is not a positive number".to_string());
    }

    // β between the poles, as an offset from the nearer one.
    let h = delta / 2.0;
    let f_mid = f.at(base_lo, true, near_other, p_lo + h, h, -h).0;
    let beta = if f_mid == 0.0 {
        Root {
            lambda: p_lo + h,
            o_lo: h,
            o_hi: -h,
        }
    } else if f_mid < 0.0 {
        // The root is in (p_lo, mid]: β = p_lo + v, F decreasing in v.
        let f_beta = |v: f64| f.at(base_lo, true, near_other, p_lo + v, v, v - delta);
        let v = refine(seeds.map_or(f64::NAN, |s| s.1 - p_lo), h, true, f_beta)
            .ok_or_else(|| unconverged("beta"))?;
        Root {
            lambda: p_lo + v,
            o_lo: v,
            o_hi: v - delta,
        }
    } else {
        // The root is in (mid, p_hi): β = p_hi − w, F increasing in w.
        let base_hi = f.base(false, true);
        let f_beta = |w: f64| {
            let (value, slope) = f.at(base_hi, false, true, p_hi - w, delta - w, -w);
            (value, -slope)
        };
        let w = refine(seeds.map_or(f64::NAN, |s| p_hi - s.1), h, false, f_beta)
            .ok_or_else(|| unconverged("beta"))?;
        Root {
            lambda: p_hi - w,
            o_lo: delta - w,
            o_hi: -w,
        }
    };
    // MOD-3C-08: β rounding onto a pole is refused as α and γ are. The one exception is a pair of
    // poles that are neighbours among the doubles (k21 and k31 one unit in the last place apart):
    // no double lies strictly between them, so β has to be one of them, and its offsets, kept
    // apart from it, are what the weights use.
    let adjacent = p_hi.to_bits() == p_lo.to_bits().saturating_add(1);
    if !adjacent && (beta.lambda <= p_lo || beta.lambda >= p_hi) {
        let onto_lo = beta.lambda <= p_lo;
        return Err(format!(
            "the exponent beta would equal the pole {}: its offset is below the rounding unit of the pole ({} is too small to separate them in double precision)",
            pole_name(lo_is_21, onto_lo),
            if onto_lo { name_lo } else { name_hi }
        ));
    }

    // Weights from the second form, with the offsets.
    let roots = [alpha, beta, gamma];
    let mut weights = [0.0; 3];
    for (slot, root) in weights.iter_mut().zip(roots.iter()) {
        *slot = weight(c_lo, root.o_lo, c_hi, root.o_hi).ok_or_else(|| {
            format!(
                "a weight of the exponents is not a positive number (the offset {:e} of an exponent to a pole is too small)",
                root.o_lo.abs().min(root.o_hi.abs())
            )
        })?;
    }
    // Strictly ordered as doubles (α > p_hi ≥ β ≥ p_lo > γ was checked above).
    let lambda = [roots[0].lambda, roots[1].lambda, roots[2].lambda];
    let mut o21 = [0.0; 3];
    let mut o31 = [0.0; 3];
    for (root, (to21, to31)) in roots.iter().zip(o21.iter_mut().zip(o31.iter_mut())) {
        (*to21, *to31) = if lo_is_21 {
            (root.o_lo, root.o_hi)
        } else {
            (root.o_hi, root.o_lo)
        };
    }
    Ok(Exponents {
        lambda,
        weight: weights,
        o21,
        o31,
        tie: false,
    })
}

/// The name of the pole an exponent rounds onto, for a message.
fn pole_name(lo_is_21: bool, lower: bool) -> &'static str {
    match (lo_is_21, lower) {
        (true, true) | (false, false) => "k21",
        (true, false) | (false, true) => "k31",
    }
}

/// The exact tie k21 = k31 = p (MOD-3C-22): β = p, wβ = 0, and α, γ are the roots of the
/// two-compartment polynomial with k12 + k13 as the coupling, C = (k12 + k13)·p.
fn tie(k: f64, p: f64, coupling: f64, sum: &Sum) -> Result<Exponents, Failure> {
    let e = sum.minus(p);
    let r = libm::hypot(e, 2.0 * libm::sqrt(coupling));
    // α − p = (E + r)/2 and p − γ = C/(α − p), by the stable branch for the sign of E.
    let (u_alpha, u_gamma) = if e >= 0.0 {
        let u = (e + r) / 2.0;
        (u, coupling / u)
    } else {
        let v = (r - e) / 2.0;
        (coupling / v, v)
    };
    let alpha = p + u_alpha;
    // γ = k·p/α (a product and a quotient: it keeps its digits however small it is).
    let gamma = k * p / alpha;
    let usable = |x: f64| x.is_finite() && x > 0.0;
    if !(usable(u_alpha) && usable(u_gamma) && usable(alpha) && usable(gamma)) {
        return Err(
            "the exponents of the tie k21 = k31 are not positive finite numbers".to_string(),
        );
    }
    if alpha <= p {
        return Err(
            "the exponent alpha would equal the pole k21 = k31: its offset is below the rounding unit of the pole (k12 x k21 and k13 x k31 are too small to separate them in double precision)"
                .to_string(),
        );
    }
    if gamma >= p {
        return Err(
            "the exponent gamma would equal the pole k21 = k31: its offset is below the rounding unit of the pole (k12 x k21 and k13 x k31 are too small to separate them in double precision)"
                .to_string(),
        );
    }
    let total = u_alpha + u_gamma;
    let (w_alpha, w_gamma) = (u_alpha / total, u_gamma / total);
    if !(usable(w_alpha) && usable(w_gamma)) {
        return Err("a weight of the exponents of the tie is not a positive number".to_string());
    }
    Ok(Exponents {
        lambda: [alpha, p, gamma],
        weight: [w_alpha, 0.0, w_gamma],
        o21: [u_alpha, 0.0, -u_gamma],
        o31: [u_alpha, 0.0, -u_gamma],
        tie: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs()
    }

    /// Worked example Q1: exponents 3, 0.5, 0.05 from the micro set of the clearance base case.
    #[test]
    fn the_base_case_gives_exponents_3_half_and_a_twentieth() {
        let e = solve(0.375, 1.1875, 1.0, 0.7875, 0.2).unwrap();
        assert!(close(e.lambda[0], 3.0, 1e-14), "{:?}", e.lambda);
        assert!(close(e.lambda[1], 0.5, 1e-14));
        assert!(close(e.lambda[2], 0.05, 1e-14));
        let total: f64 = e.weight.iter().sum();
        assert!(close(total, 1.0, 1e-14), "{total}");
        assert!(e.weight.iter().all(|&w| w > 0.0));
        assert!(!e.tie);
    }

    /// The exponents are the roots of the characteristic polynomial: their sum is a2 and their
    /// product a0, whatever the order of the poles.
    #[test]
    fn the_exponents_are_the_roots_of_the_polynomial() {
        for (k, k12, k21, k13, k31) in [
            (0.375, 1.1875, 1.0, 0.7875, 0.2),
            (0.375, 1.1875, 0.2, 0.7875, 1.0),
            (1e-4, 2.0, 30.0, 1e-3, 0.01),
            (100.0, 0.5, 300.0, 250.0, 2e-3),
        ] {
            let e = solve(k, k12, k21, k13, k31).unwrap();
            let a2 = k + k12 + k13 + k21 + k31;
            let sum: f64 = e.lambda.iter().sum();
            assert!(close(sum, a2, 1e-14), "{sum} against {a2}");
            let product: f64 = e.lambda.iter().product();
            assert!(close(product, k * k21 * k31, 1e-13), "{product}");
            assert!(e.lambda[0] > e.lambda[1] && e.lambda[1] > e.lambda[2] && e.lambda[2] > 0.0);
        }
    }

    /// The exact tie: β = k21, wβ = 0, α and γ from the merged compartment.
    #[test]
    fn the_exact_tie_has_a_zero_middle_weight() {
        let e = solve(0.5, 0.3, 1.0, 0.2, 1.0).unwrap();
        assert!(e.tie);
        assert_eq!(e.lambda[1], 1.0);
        assert_eq!(e.weight[1], 0.0);
        let total: f64 = e.weight.iter().sum();
        assert!(close(total, 1.0, 1e-15));
        // α + γ = k + k12 + k13 + k21 and αγ = k·k21.
        assert!(close(e.lambda[0] + e.lambda[2], 2.0, 1e-14));
        assert!(close(e.lambda[0] * e.lambda[2], 0.5, 1e-14));
    }

    /// A coupling too small for the exponent to leave the pole in doubles is refused, naming the
    /// coupling.
    #[test]
    fn a_vanishing_coupling_is_refused() {
        let e = solve(1.0, 1e-18, 10.0, 0.5, 0.2).unwrap_err();
        assert!(e.contains("alpha") && e.contains("k21"), "{e}");
        let e = solve(1.0, 0.5, 1.0, 5e-18, 0.2).unwrap_err();
        assert!(e.contains("gamma") && e.contains("k31"), "{e}");
        let e = solve(1.0, 1e-200, 1e-200, 0.5, 0.2).unwrap_err();
        assert!(e.contains("underflows"), "{e}");
    }

    /// Poles one unit in the last place apart: all three roots are found, β between them.
    #[test]
    fn poles_a_unit_apart_give_distinct_exponents() {
        let e = solve(0.375, 1.1875, 1.0, 0.7875, 0.999_999_999_999_999_9).unwrap();
        assert!(e.lambda[0] > e.lambda[1] && e.lambda[1] > e.lambda[2]);
        assert!(e.weight[1] > 0.0 && e.weight[1] < 1e-20, "{:?}", e.weight);
    }

    /// Large couplings (k12·k21/k21² about 1 or more, the lc_rev inputs of the oracle): the roots
    /// obey the product, the sum and the pair-sum identities, the weights sum to 1 and Σwλ = S0,
    /// to a few units in the last place (the old shifted form lost up to six digits here).
    #[test]
    fn large_couplings_keep_the_identities() {
        let sets = [
            (0.783, 452_000.0, 1.27e10, 0.000169, 0.108),
            (5.38e-5, 1040.0, 903.0, 2.65e-5, 0.000172),
            (1.16e-6, 3.35, 12.2, 1.51e-11, 6.01e-9),
            (1.0, 1.0e3, 1.0, 1.0e3, 0.5),
            (0.2, 40.0, 2.0, 90.0, 1.0e-3),
        ];
        for (k10, k12, k21, k13, k31) in sets {
            let e = solve(k10, k12, k21, k13, k31).unwrap_or_else(|m| panic!("{k10}: {m}"));
            let [a, b, g] = e.lambda;
            let s2 = k10 + k12 + k13 + k21 + k31;
            let s1 = k10 * k21 + k10 * k31 + k21 * k13 + k12 * k31 + k21 * k31;
            assert!(a > b && b > g && g > 0.0, "{:?}", e.lambda);
            assert!(
                close(a * b * g, k10 * k21 * k31, 1e-12),
                "{k10} {:?}",
                e.lambda
            );
            assert!(close(a + b + g, s2, 1e-13), "{k10} {:?}", e.lambda);
            assert!(
                close(a * b + a * g + b * g, s1, 1e-12),
                "{k10} {:?}",
                e.lambda
            );
            let w: f64 = e.weight.iter().sum();
            assert!((w - 1.0).abs() < 1e-12, "{k10}: sum of weights {w}");
            let first: f64 = e.weight.iter().zip(e.lambda).map(|(w, l)| w * l).sum();
            assert!(close(first, k10 + k12 + k13, 1e-12), "{k10}: {first}");
        }
    }

    /// MOD-3C-08: beta rounding onto a pole is refused, naming beta and the coupling.
    #[test]
    fn beta_onto_a_pole_is_refused() {
        let e = solve(1.0, 0.5, 2.0, 1e-300, 1.5).unwrap_err();
        assert!(e.contains("beta") && e.contains("k13"), "{e}");
    }
}
