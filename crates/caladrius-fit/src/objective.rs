//! The objective and the error models of the maximum-likelihood fit (`specs/fit.md` FIT-ML-01 to
//! 05): which function is minimised, the standard deviation σ of an observation as a function of
//! the prediction and of the error parameters, and the derivatives of ln σ that the Fisher-scoring
//! step needs (FIT-ML-06). Pure functions of numbers: no model, no data.

use serde::{Deserialize, Serialize};

/// What is minimised (FIT-ML-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    /// Weighted least squares (sections 2 to 8 of `specs/fit.md`; the default).
    #[default]
    Wls,
    /// Gaussian likelihood with an estimated error model (section 13).
    Ml,
}

impl Objective {
    /// The id of FIT-ML-21.
    pub fn id(&self) -> &'static str {
        match self {
            Objective::Wls => "wls",
            Objective::Ml => "ml",
        }
    }

    /// The objective with this id.
    pub fn from_id(id: &str) -> Option<Objective> {
        [Objective::Wls, Objective::Ml]
            .into_iter()
            .find(|o| o.id() == id)
    }
}

/// How the covariance of the maximum-likelihood fit is formed (FIT-ML-17, FIT-ML-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Information {
    /// The expected information of the stacked system (the only value for now).
    #[default]
    Expected,
}

impl Information {
    /// The id of FIT-ML-21.
    pub fn id(&self) -> &'static str {
        "expected"
    }

    /// The information with this id.
    pub fn from_id(id: &str) -> Option<Information> {
        (id == "expected").then_some(Information::Expected)
    }
}

/// The residual error model (FIT-ML-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorModel {
    /// σ = a.
    Additive,
    /// σ = b·|ŷ|.
    Proportional,
    /// σ = √(a² + b²·ŷ²): the variances add (the default combined form, FIT-ML-03).
    Combined,
    /// σ = a + b·|ŷ|: the standard deviations add.
    CombinedSd,
}

impl ErrorModel {
    const ALL: [ErrorModel; 4] = [
        ErrorModel::Additive,
        ErrorModel::Proportional,
        ErrorModel::Combined,
        ErrorModel::CombinedSd,
    ];

    /// The id of FIT-ML-21.
    pub fn id(&self) -> &'static str {
        match self {
            ErrorModel::Additive => "additive",
            ErrorModel::Proportional => "proportional",
            ErrorModel::Combined => "combined",
            ErrorModel::CombinedSd => "combined_sd",
        }
    }

    /// The error model with this id.
    pub fn from_id(id: &str) -> Option<ErrorModel> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    /// Number of error parameters Q.
    pub fn q(&self) -> usize {
        match self {
            ErrorModel::Additive | ErrorModel::Proportional => 1,
            ErrorModel::Combined | ErrorModel::CombinedSd => 2,
        }
    }

    /// Names of the error parameters in the result (FIT-ML-21), in the order of the moved
    /// variables.
    pub fn names(&self) -> &'static [&'static str] {
        match self {
            ErrorModel::Additive => &["error_a"],
            ErrorModel::Proportional => &["error_b"],
            ErrorModel::Combined | ErrorModel::CombinedSd => &["error_a", "error_b"],
        }
    }

    /// True when the moved variable is the square of the reported one (FIT-ML-05).
    pub(crate) fn is_variance_scale(&self) -> bool {
        !matches!(self, ErrorModel::CombinedSd)
    }

    /// The error parameters in the user's units, from the moved variables.
    pub(crate) fn user_values(&self, xi: &[f64]) -> Vec<f64> {
        xi.iter()
            .map(|&x| {
                if self.is_variance_scale() {
                    x.sqrt()
                } else {
                    x
                }
            })
            .collect()
    }

    /// The moved variables from the error parameters in the user's units.
    pub(crate) fn moved_from_user(&self, user: &[f64]) -> Vec<f64> {
        user.iter()
            .map(|&x| if self.is_variance_scale() { x * x } else { x })
            .collect()
    }

    /// σ for the prediction `yhat` and the moved variables `xi`.
    pub(crate) fn sigma(&self, xi: &[f64], yhat: f64) -> f64 {
        let at = |k: usize| xi.get(k).copied().unwrap_or(f64::NAN);
        match self {
            ErrorModel::Additive => at(0).sqrt(),
            ErrorModel::Proportional => at(0).sqrt() * yhat.abs(),
            ErrorModel::Combined => (at(0) + at(1) * yhat * yhat).sqrt(),
            ErrorModel::CombinedSd => at(0) + at(1) * yhat.abs(),
        }
    }

    /// ∂ ln σ/∂ŷ (FIT-ML-06); `sigma` is σ at the same point.
    pub(crate) fn dln_sigma_dyhat(&self, xi: &[f64], yhat: f64, sigma: f64) -> f64 {
        let at = |k: usize| xi.get(k).copied().unwrap_or(f64::NAN);
        match self {
            ErrorModel::Additive => 0.0,
            ErrorModel::Proportional => 1.0 / yhat,
            ErrorModel::Combined => at(1) * yhat / (sigma * sigma),
            ErrorModel::CombinedSd => {
                let sign = if yhat > 0.0 {
                    1.0
                } else if yhat < 0.0 {
                    -1.0
                } else {
                    0.0
                };
                at(1) * sign / sigma
            }
        }
    }

    /// ∂ ln σ/∂(moved variable k) (FIT-ML-06); `sigma` is σ at the same point.
    pub(crate) fn dln_sigma_dxi(&self, xi: &[f64], yhat: f64, sigma: f64, k: usize) -> f64 {
        let at = |j: usize| xi.get(j).copied().unwrap_or(f64::NAN);
        match (self, k) {
            (ErrorModel::Additive | ErrorModel::Proportional, _) => 1.0 / (2.0 * at(0)),
            (ErrorModel::Combined, 0) => 1.0 / (2.0 * sigma * sigma),
            (ErrorModel::Combined, _) => yhat * yhat / (2.0 * sigma * sigma),
            (ErrorModel::CombinedSd, 0) => 1.0 / sigma,
            (ErrorModel::CombinedSd, _) => yhat.abs() / sigma,
        }
    }

    /// dU/dξ: the derivative of the reported value with respect to the moved variable, for the
    /// delta method (FIT-ML-17): 1/(2·value) on the variance scale, 1 for `combined_sd`.
    pub(crate) fn delta_factor(&self, value: f64) -> f64 {
        if self.is_variance_scale() {
            1.0 / (2.0 * value)
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ids_round_trip() {
        for m in ErrorModel::ALL {
            assert_eq!(ErrorModel::from_id(m.id()), Some(m));
        }
        assert_eq!(ErrorModel::from_id("poisson"), None);
        assert_eq!(Objective::from_id("ml"), Some(Objective::Ml));
        assert_eq!(Objective::from_id("reml"), None);
        assert_eq!(Information::from_id("observed"), None);
    }

    #[test]
    fn sigma_follows_fit_ml_02() {
        assert_eq!(ErrorModel::Additive.sigma(&[4.0], -3.0), 2.0);
        assert_eq!(ErrorModel::Proportional.sigma(&[4.0], -3.0), 6.0);
        assert_eq!(ErrorModel::Combined.sigma(&[9.0, 16.0], 0.0), 3.0);
        assert_eq!(
            ErrorModel::Combined.sigma(&[9.0, 16.0], 0.75),
            18.0_f64.sqrt()
        );
        assert_eq!(ErrorModel::CombinedSd.sigma(&[1.0, 2.0], -3.0), 7.0);
    }

    #[test]
    fn the_derivatives_of_ln_sigma_match_central_differences() {
        for model in ErrorModel::ALL {
            let xi = [0.7, 0.3];
            let xi = &xi[..model.q()];
            let yhat = 1.3;
            let sigma = model.sigma(xi, yhat);
            let h = 1e-6;
            let ln = |xi: &[f64], y: f64| libm::log(model.sigma(xi, y));
            let by_y = (ln(xi, yhat + h) - ln(xi, yhat - h)) / (2.0 * h);
            assert!((model.dln_sigma_dyhat(xi, yhat, sigma) - by_y).abs() < 1e-7);
            for k in 0..model.q() {
                let (mut up, mut down) = (xi.to_vec(), xi.to_vec());
                up[k] += h;
                down[k] -= h;
                let by_xi = (ln(&up, yhat) - ln(&down, yhat)) / (2.0 * h);
                assert!((model.dln_sigma_dxi(xi, yhat, sigma, k) - by_xi).abs() < 1e-7);
            }
        }
    }

    #[test]
    fn user_units_and_moved_variables_convert_both_ways() {
        let m = ErrorModel::Combined;
        assert_eq!(m.user_values(&[4.0, 9.0]), vec![2.0, 3.0]);
        assert_eq!(m.moved_from_user(&[2.0, 3.0]), vec![4.0, 9.0]);
        let sd = ErrorModel::CombinedSd;
        assert_eq!(sd.user_values(&[4.0, 9.0]), vec![4.0, 9.0]);
        assert_eq!(sd.delta_factor(2.0), 1.0);
        assert_eq!(m.delta_factor(2.0), 0.25);
    }
}
