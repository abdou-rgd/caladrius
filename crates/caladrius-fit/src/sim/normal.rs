//! From a uniform to a standard normal: Wichura's AS 241 (PPND16), one normal per uniform (SIM-RNG-05).
//!
//! The coefficients are the numerical constants of the published algorithm, taken from the table of
//! `specs/simulation.md` SIM-RNG-05 (see `ATTRIBUTION.md`); the code is written from the spec. Every product
//! and sum is rounded separately (no fused multiply-add), polynomials by Horner's rule from the highest
//! degree down, `ln` from `libm` and `sqrt` the exact IEEE square root.

// The digits are the published constants of the algorithm, kept as published; the compiler reads each to the
// nearest double.
#[allow(clippy::excessive_precision)]
mod table {
    pub(super) const A: [f64; 8] = [
        3.387132872796366608,
        133.14166789178437745,
        1971.5909503065514427,
        13731.693765509461125,
        45921.953931549871457,
        67265.770927008700853,
        33430.575583588128105,
        2509.0809287301226727,
    ];
    pub(super) const B: [f64; 8] = [
        1.0,
        42.313330701600911252,
        687.1870074920579083,
        5394.1960214247511077,
        21213.794301586595867,
        39307.89580009271061,
        28729.085735721942674,
        5226.495278852854561,
    ];
    pub(super) const C: [f64; 8] = [
        1.42343711074968357734,
        4.6303378461565452959,
        5.7694972214606914055,
        3.64784832476320460504,
        1.27045825245236838258,
        0.24178072517745061177,
        0.0227238449892691845833,
        7.7454501427834140764e-4,
    ];
    pub(super) const D: [f64; 8] = [
        1.0,
        2.05319162663775882187,
        1.6763848301838038494,
        0.68976733498510000455,
        0.14810397642748007459,
        0.0151986665636164571966,
        5.475938084995344946e-4,
        1.05075007164441684324e-9,
    ];
    pub(super) const E: [f64; 8] = [
        6.6579046435011037772,
        5.4637849111641143699,
        1.7848265399172913358,
        0.29656057182850489123,
        0.026532189526576123093,
        0.0012426609473880784386,
        2.71155556874348757815e-5,
        2.01033439929228813265e-7,
    ];
    pub(super) const F: [f64; 8] = [
        1.0,
        0.59983220655588793769,
        0.13692988092273580531,
        0.0148753612908506148525,
        7.868691311456132591e-4,
        1.8463183175100546818e-5,
        1.4215117583164458887e-7,
        2.04426310338993978564e-15,
    ];
}

/// poly(x; c) = (...((c7 x + c6) x + c5) x + ... + c1) x + c0, coefficients with degree 0 first.
fn poly(x: f64, c: &[f64; 8]) -> f64 {
    c.iter().rev().fold(0.0, |acc, &ci| acc * x + ci)
}

/// Phi^-1(u) for u in (0, 1) (SIM-RNG-05); |z| <= 8.2096 for the uniforms of SIM-RNG-04.
pub fn as241(u: f64) -> f64 {
    let q = u - 0.5;
    if q.abs() <= 0.425 {
        let rho = 0.180625 - q * q;
        return (q * poly(rho, &table::A)) / poly(rho, &table::B);
    }
    let smaller = if q < 0.0 { u } else { 1.0 - u };
    let t = (-libm::log(smaller)).sqrt();
    let v = if t <= 5.0 {
        let s = t - 1.6;
        poly(s, &table::C) / poly(s, &table::D)
    } else {
        let s = t - 5.0;
        poly(s, &table::E) / poly(s, &table::F)
    };
    if q < 0.0 { -v } else { v }
}
