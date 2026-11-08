//! Omega and its Cholesky factor (SIM-MOD-03, SIM-RNG-07).
//!
//! Omega is given as the variances (`diagonal`) or as the lower triangle by rows (`block`): Omega_11,
//! Omega_21, Omega_22, Omega_31, ... (a note for the population fit, T-153, which must read the same order).
//! The factor is the lower Cholesky-Banachiewicz factor, row by row, and eta = L z.
//!
//! Matrices are held flat, row by row, q by q.

use serde::{Deserialize, Serialize};

/// The two shapes of Omega.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmegaShape {
    /// q variances (the default).
    #[default]
    Diagonal,
    /// q (q + 1) / 2 numbers, the lower triangle by rows.
    Block,
}

impl OmegaShape {
    /// The name used in a request.
    pub fn name(self) -> &'static str {
        match self {
            OmegaShape::Diagonal => "diagonal",
            OmegaShape::Block => "block",
        }
    }

    /// How many numbers Omega holds for `q` random effects.
    pub fn count(self, q: usize) -> usize {
        match self {
            OmegaShape::Diagonal => q,
            OmegaShape::Block => q * (q + 1) / 2,
        }
    }
}

/// The full symmetric matrix (flat, q by q) of an Omega given in `shape`; `None` if the count does not fit.
pub(super) fn matrix(shape: OmegaShape, entries: &[f64], q: usize) -> Option<Vec<f64>> {
    if entries.len() != shape.count(q) {
        return None;
    }
    let mut m = vec![0.0; q * q];
    match shape {
        OmegaShape::Diagonal => {
            for (i, &x) in entries.iter().enumerate() {
                *m.get_mut(i * q + i)? = x;
            }
        }
        OmegaShape::Block => {
            let mut values = entries.iter();
            for i in 0..q {
                for j in 0..=i {
                    let x = *values.next()?;
                    *m.get_mut(i * q + j)? = x;
                    *m.get_mut(j * q + i)? = x;
                }
            }
        }
    }
    Some(m)
}

/// Cholesky-Banachiewicz, row by row: for i, for j <= i, s = Omega_ij - T with T = sum over l < j of
/// L_il L_jl accumulated from 0 left to right and subtracted once; L_ii = sqrt(s) after checking that s is
/// finite and > 0, else L_ij = s / L_jj. `Err((i, s))` names the random effect i and the pivot s.
pub(super) fn cholesky(omega: &[f64], q: usize) -> Result<Vec<f64>, (usize, f64)> {
    let mut l = vec![0.0; q * q];
    for i in 0..q {
        for j in 0..=i {
            let mut t = 0.0;
            for k in 0..j {
                t += at(&l, q, i, k) * at(&l, q, j, k);
            }
            let s = at(omega, q, i, j) - t;
            let value = if i == j {
                if !(s.is_finite() && s > 0.0) {
                    return Err((i, s));
                }
                s.sqrt()
            } else {
                s / at(&l, q, j, j)
            };
            if let Some(slot) = l.get_mut(i * q + j) {
                *slot = value;
            }
        }
    }
    Ok(l)
}

/// eta_i = sum over j <= i of L_ij z_j, accumulated from 0 left to right in j (SIM-RNG-07 item 3).
pub(super) fn apply(l: &[f64], q: usize, z: &[f64]) -> Vec<f64> {
    (0..q)
        .map(|i| {
            let mut acc = 0.0;
            for (j, zj) in z.iter().enumerate().take(i + 1) {
                acc += at(l, q, i, j) * zj;
            }
            acc
        })
        .collect()
}

fn at(m: &[f64], q: usize, i: usize, j: usize) -> f64 {
    m.get(i * q + j).copied().unwrap_or(0.0)
}
