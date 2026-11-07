#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: helpers panic on purpose
//! Maximum-likelihood fitting oracle (task T-148): `caladrius-fit` with `objective = ml` against the
//! reference fits of `oracle/expected/fit/ml/` (`oracle/scripts/fit_ml.R`: `stats::optim` from seeded
//! starts, a Newton polish at 256 bits with a gradient check, `specs/fit.md` section 13), on the F
//! data, seeded synthetic profiles (one per error model, from `pk1.iv_bolus` and `pk1.oral_1`),
//! Indometh and Theoph, for the four error models, and against the hand checks of OF-15, the
//! collapse and bound cases of OF-17 and the refusals of OF-18 (ML6).
//!
//! Written before the engine exists: the tests compile against today's API and **fail at run time**,
//! because `FitOptions` does not know `objective`, `error_model`, `error_init` or `information` (the
//! options are built from JSON, so an unknown field is a run-time refusal, not a compile error) and
//! the result has no `m2ll`. The guards at the end are green.
//!
//! # API the engine must provide (assumed; the plainest name where `specs/fit.md` section 13 is silent)
//!
//! Inputs and options are data (golden rule 4) and are built here from JSON, in the names of
//! FIT-ML-21:
//!
//! ```text
//! options: { "objective": "ml", "error_model": "additive" | "proportional" | "combined" | "combined_sd",
//!            "error_init": [a] | [b] | [a, b]         (optional, FIT-ML-10; user units),
//!            "convergence": 1e-10, "max_iterations": 500, "confidence_level": 0.95,
//!            "information": "expected",
//!            "bounds": { "<par>": { "lower": x } }     (optional, FIT-BND-01) }
//! weighting: "uniform" (FIT-ML-01)
//! ```
//!
//! `criterion` is not given: the Newton decrement of FIT-ML-08 is the test of `objective = ml`.
//!
//! The result, read by name with `FitResult::get` (FIT-ML-20, FIT-ML-21 and the names of
//! `tests/oracle_fit.rs`): `n`, `p` (free model parameters), `q` (error parameters), `m2ll`,
//! `aic_ml`, `bic_ml`; `estimate.<par>`, `se.<par>`, `cv_percent.<par>` for `v`, `k`, `ka`,
//! `error_a`, `error_b` (an error parameter on its bound: estimate 0, no `se`, no `cv_percent`);
//! `correlation.<a>.<b>` (a before b in that order) over the free estimates; `wrss`, `aic`, `sbc`
//! (the least-squares forms at the ML estimate with uniform weights, FIT-ML-16); `iwres_mean`,
//! `iwres_sd`, `iwres_sum_sq`, `n_iwres_gt2`, `n_iwres_gt3` (FIT-ML-18). The residual table
//! (`serde_json::to_value(&result)["observations"]`) has `sigma` and `iwres` in each row, in the
//! place of `weight` and `weighted_residual`. `FitStatus` has `Converged` and `AtBound` today
//! (serialised `converged`, `at_bound`). A refusal is a `FitError` whose serialised `code` is
//! `exact_fit`, `non_positive_sigma`, `too_few_observations`, `unbounded_likelihood` or
//! `invalid_option`. `unbounded_likelihood` (decision on FIT-ML-13c, the sentence not yet in the spec):
//! under `combined` or `combined_sd`, an observation of exactly 0 where the prediction is 0 for every
//! parameter value (the sample at the dose time of an oral dose) is refused before fitting; the message
//! names the rows (time 0) and says to exclude them or to use the additive model. The other subjects
//! of the dataset fit as before (cases `ml_theoph_t0_combined` and `ml_theoph_t0_combined_sd`).
//!
//! Tolerances: estimates and statistics `FIT_PARAMETERS` (1e-4 relative; `FIT_PARAMETERS_SMALL_CASE`,
//! 1e-6, on the five-point F data, as for the least-squares oracle); the residual sum of squares
//! `FIT_WEIGHTED_SS`; −2LL, AIC_ML and BIC_ML `AbsoluteTolerance::FIT_M2LL` (a relative rule is
//! meaningless for a log-likelihood that can be negative: FIT-ML-08); IWRES `FIT_IWRES`; a reference
//! value that is zero by structure `FIT_STRUCTURAL_ZERO`. Nothing is loosened.
//!
//! Not compared with the engine: `se_obs.*` and `correlation_obs.*` (the observed information, which
//! `information = expected` does not give: OF-16), and the Gauss-Newton statistics.

use std::collections::BTreeMap;

use caladrius_fit::{FitError, FitInput, FitResult, run};
use caladrius_testkit::{
    AbsoluteTolerance, ML_LS_FORMULA_RELATIVE, ML_STRUCTURAL_ZERO_CUTOFF, MlCase, MlGroup,
    Tolerance, list_fit_ml_cases, load_fit_ml_case, load_fit_ml_errors,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------- inputs

/// The options of the reference run for `case` (FIT-ML-21 names).
fn ml_options(case: &MlCase) -> Value {
    let o = &case.options;
    let mut options = json!({
        "objective": "ml",
        "error_model": case.error_model,
        "convergence": o.convergence,
        "max_iterations": o.max_iterations,
        "confidence_level": o.confidence_level,
        "information": o.information,
    });
    if let Some(init) = &case.error_init {
        options["error_init"] = json!(init);
    }
    if !case.bounds.is_empty() {
        let mut bounds = serde_json::Map::new();
        for (par, lower, upper) in &case.bounds {
            let mut b = serde_json::Map::new();
            if let Some(l) = lower {
                b.insert("lower".into(), json!(l));
            }
            if let Some(u) = upper {
                b.insert("upper".into(), json!(u));
            }
            bounds.insert(par.clone(), Value::Object(b));
        }
        options["bounds"] = Value::Object(bounds);
    }
    options
}

fn input_value(
    model: &str,
    dose: f64,
    time: &[f64],
    conc: &[f64],
    weighting: &str,
    initial: &BTreeMap<String, f64>,
    options: Value,
) -> Value {
    json!({
        "model": model,
        "dose": dose,
        "time": time,
        "conc": conc,
        "weighting": weighting,
        "initial": initial,
        "options": options,
    })
}

fn parse_input(value: &Value) -> Result<FitInput, String> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}

/// The input of one subject of a reference case.
fn case_input(case: &MlCase, subject: &str) -> Value {
    let profile = case
        .profile(subject)
        .unwrap_or_else(|| panic!("{}: no subject {subject}", case.name));
    let (time, conc) = case.fitted_observations(profile);
    input_value(
        &case.model,
        profile.dose,
        &time,
        &conc,
        &case.weighting,
        &case.initial_estimates[subject],
        ml_options(case),
    )
}

fn fit_value(label: &str, value: &Value) -> FitResult {
    let input = parse_input(value).unwrap_or_else(|e| panic!("{label}: the input is refused: {e}"));
    run(&input).unwrap_or_else(|e| panic!("{label}: the engine refuses a valid fit: {e}"))
}

fn fit_case(case: &MlCase, subject: &str) -> FitResult {
    fit_value(
        &format!("{} subject {subject}", case.name),
        &case_input(case, subject),
    )
}

/// The least-squares fit (uniform weights) of the same observations, with today's options.
fn wls_fit(case: &MlCase, subject: &str) -> FitResult {
    let profile = case.profile(subject).unwrap();
    let (time, conc) = case.fitted_observations(profile);
    let mut initial = case.initial_estimates[subject].clone();
    initial.retain(|k, _| !k.starts_with("error"));
    fit_value(
        &format!("{} subject {subject} (least squares)", case.name),
        &input_value(
            &case.model,
            profile.dose,
            &time,
            &conc,
            "uniform",
            &initial,
            json!({"convergence": 1e-10, "max_iterations": 200, "derivatives": "analytic"}),
        ),
    )
}

/// `(sigma, iwres)` of each fitted observation, from the residual table of the result.
fn residual_table(result: &FitResult) -> Vec<(f64, f64)> {
    let value = serde_json::to_value(result).unwrap();
    value["observations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let get = |k: &str| {
                row[k]
                    .as_f64()
                    .unwrap_or_else(|| panic!("the residual table has no `{k}`: {row}"))
            };
            (get("sigma"), get("iwres"))
        })
        .collect()
}

fn status_id(result: &FitResult) -> String {
    serde_json::to_value(result.status())
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn need(result: &FitResult, name: &str) -> f64 {
    result
        .get(name)
        .unwrap_or_else(|| panic!("the result has no `{name}`"))
}

// ---------------------------------------------------------------- comparison with the reference

fn rel_tolerance(case: &MlCase) -> Tolerance {
    // the five-point F data are small enough to be worked on paper (specs/fit.md section 11)
    if case.dataset.name == "f" {
        Tolerance::FIT_PARAMETERS_SMALL_CASE
    } else {
        Tolerance::FIT_PARAMETERS
    }
}

/// One expected quantity against the engine's. `None` when it is within tolerance, else a line.
fn compare(case: &MlCase, result: &FitResult, name: &str, expected: Option<f64>) -> Option<String> {
    let group = MlGroup::of(name).unwrap();
    let actual = result.get(name);
    let Some(expected) = expected else {
        // not available in the reference (NC: a component on its bound): not available here
        return actual.map(|a| format!("{name}: the reference has no value (NC), got {a:e}"));
    };
    let Some(actual) = actual else {
        return Some(format!("{name}: expected {expected:e}, not calculated"));
    };
    let (ok, rule) = match group {
        MlGroup::Counts => (actual == expected, "exact".to_string()),
        MlGroup::Likelihood => (
            AbsoluteTolerance::FIT_M2LL.accepts(actual, expected),
            format!("absolute {:e}", AbsoluteTolerance::FIT_M2LL.abs),
        ),
        MlGroup::UniformRss => (
            Tolerance::FIT_WEIGHTED_SS.accepts(actual, expected),
            "1e-6 relative".to_string(),
        ),
        MlGroup::Estimate if expected == 0.0 => (actual == 0.0, "exactly 0 on the bound".into()),
        MlGroup::Correlation if expected.abs() < ML_STRUCTURAL_ZERO_CUTOFF => (
            AbsoluteTolerance::FIT_STRUCTURAL_ZERO.accepts(actual, 0.0),
            format!(
                "absolute {:e}, a structural zero",
                AbsoluteTolerance::FIT_STRUCTURAL_ZERO.abs
            ),
        ),
        MlGroup::Iwres | MlGroup::IwresSummary => (
            AbsoluteTolerance::FIT_IWRES.accepts(actual, expected),
            format!("absolute {:e}", AbsoluteTolerance::FIT_IWRES.abs),
        ),
        MlGroup::IwresCount => (actual == expected, "exact".into()),
        _ => (
            rel_tolerance(case).accepts(actual, expected),
            "parameter tolerance".into(),
        ),
    };
    (!ok).then(|| format!("{name}: expected {expected:e}, got {actual:e} ({rule})"))
}

/// Compares the quantities of `groups` for every subject of a case, listing every value outside
/// tolerance.
fn check_groups(case_name: &str, groups: &[MlGroup]) {
    let case = load_fit_ml_case(case_name).expect("the ML oracle case loads");
    let mut problems = Vec::new();
    let mut checked = 0usize;
    for subject in &case.subjects {
        let result = fit_case(&case, subject);
        let mut of_subject = 0usize;
        for (s, name, value) in case.expected.iter() {
            if s != subject || !groups.contains(&MlGroup::of(name).unwrap()) {
                continue;
            }
            // sigma.<i> and iwres.<i> are read from the residual table, the rest by name
            checked += 1;
            of_subject += 1;
            if let Some(line) = compare_named(&case, &result, name, value) {
                problems.push(format!("[{subject}] {line}"));
            }
        }
        // every subject has every group: a subject without an expected value would pass empty
        assert!(
            of_subject > 0,
            "{case_name} subject {subject}: no expected value in {groups:?}"
        );
    }
    assert!(checked > 0, "{case_name}: no expected value in {groups:?}");
    assert!(
        problems.is_empty(),
        "{case_name} {groups:?}: {} of {checked} values outside tolerance\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
}

/// As [`compare`], with `sigma.<i>` and `iwres.<i>` taken from the residual table.
fn compare_named(
    case: &MlCase,
    result: &FitResult,
    name: &str,
    expected: Option<f64>,
) -> Option<String> {
    let (base, index) = name.split_once('.').unwrap_or((name, ""));
    if matches!(base, "sigma" | "iwres") {
        let i: usize = index.parse().unwrap();
        let table = residual_table(result);
        let Some(&(sigma, iwres)) = table.get(i - 1) else {
            return Some(format!(
                "{name}: the residual table has {} rows",
                table.len()
            ));
        };
        let expected = expected.unwrap();
        let (actual, ok) = if base == "sigma" {
            (sigma, rel_tolerance(case).accepts(sigma, expected))
        } else {
            (iwres, AbsoluteTolerance::FIT_IWRES.accepts(iwres, expected))
        };
        return (!ok).then(|| format!("{name}: expected {expected:e}, got {actual:e}"));
    }
    if matches!(name, "aic_ml" | "bic_ml") {
        return compare(case, result, name, expected);
    }
    compare(case, result, name, expected)
}

fn check_status(case_name: &str) {
    let case = load_fit_ml_case(case_name).expect("the ML oracle case loads");
    let mut problems = Vec::new();
    let mut checked = 0usize;
    for subject in &case.subjects {
        let result = fit_case(&case, subject);
        let want = &case.status[subject];
        let got = status_id(&result);
        checked += 1;
        if &got != want {
            problems.push(format!("subject {subject}: status {got}, expected {want}"));
        }
        // the components on a bound: their SE and CV% are not available, the others are
        let mut of_subject = 0usize;
        for par in case.parameter_names() {
            let on_bound = case.at_bound.get(subject).is_some_and(|b| b.contains(&par));
            for stat in ["se", "cv_percent"] {
                let name = format!("{stat}.{par}");
                checked += 1;
                of_subject += 1;
                if on_bound && result.get(&name).is_some() {
                    problems.push(format!("subject {subject}: {name} must be NC on a bound"));
                }
                if !on_bound && result.get(&name).is_none() {
                    // cv_percent is NC at an estimate of exactly 0 only
                    problems.push(format!("subject {subject}: {name} is not calculated"));
                }
            }
        }
        assert!(
            of_subject > 0,
            "{case_name} subject {subject}: no parameter checked"
        );
    }
    assert!(checked > 0, "{case_name}: nothing checked");
    assert!(problems.is_empty(), "{case_name}: {}", problems.join("; "));
}

macro_rules! ml_case_tests {
    ($($case:ident),* $(,)?) => {
        $(
            mod $case {
                use super::*;
                const NAME: &str = stringify!($case);
                #[test]
                fn status_and_components_on_bounds() { check_status(NAME); }
                #[test]
                fn counts() { check_groups(NAME, &[MlGroup::Counts]); }
                #[test]
                fn likelihood() { check_groups(NAME, &[MlGroup::Likelihood]); }
                #[test]
                fn estimates() { check_groups(NAME, &[MlGroup::Estimate]); }
                #[test]
                fn standard_errors() { check_groups(NAME, &[MlGroup::StandardError]); }
                #[test]
                fn correlations() { check_groups(NAME, &[MlGroup::Correlation]); }
                #[test]
                fn least_squares_forms() {
                    super::check_least_squares_forms(NAME);
                }
                #[test]
                fn iwres() {
                    check_groups(NAME, &[
                        MlGroup::Sigma, MlGroup::Iwres, MlGroup::IwresSummary, MlGroup::IwresCount,
                    ]);
                }
            }
        )*
    };
}

macro_rules! ml_refusal_tests {
    ($($case:ident),* $(,)?) => {
        mod refusals {
            $(
                #[test]
                fn $case() { super::check_refusal(stringify!($case)); }
            )*
        }
    };
}

/// FIT-ML-16: `wrss`, `aic` and `sbc` are still computed, with uniform weights at the ML estimate.
/// `wrss` is the reference `rss_uniform`; AIC and SBC follow `N ln(WRSS) + 2 P` and `+ P ln N` (P the
/// free model parameters) from the engine's own WRSS, as in `tests/common/mod.rs`.
fn check_least_squares_forms(case_name: &str) {
    let case = load_fit_ml_case(case_name).expect("the ML oracle case loads");
    let mut problems = Vec::new();
    let mut checked = 0usize;
    for subject in &case.subjects {
        let result = fit_case(&case, subject);
        let rss = case.value(subject, "rss_uniform").unwrap();
        let (n, p) = (
            case.value(subject, "n").unwrap(),
            case.value(subject, "p").unwrap(),
        );
        let wrss = need(&result, "wrss");
        checked += 1;
        if !Tolerance::FIT_WEIGHTED_SS.accepts(wrss, rss) {
            problems.push(format!("[{subject}] wrss: expected {rss:e}, got {wrss:e}"));
        }
        for (name, penalty) in [("aic", 2.0 * p), ("sbc", p * libm::log(n))] {
            checked += 1;
            let formula = n * libm::log(wrss) + penalty;
            let actual = need(&result, name);
            if (actual - formula).abs() > ML_LS_FORMULA_RELATIVE * formula.abs().max(1.0) {
                problems.push(format!(
                    "[{subject}] {name}: {actual:e} is not N ln(WRSS) + penalty = {formula:e}"
                ));
            }
        }
    }
    assert!(checked > 0, "{case_name}: nothing checked");
    assert!(problems.is_empty(), "{case_name}: {}", problems.join("; "));
}

// ---------------------------------------------------------------- refusals (ML6, OF-18)

fn check_refusal(name: &str) {
    let all = load_fit_ml_errors().expect("the refusal suite loads");
    let case = all
        .iter()
        .find(|c| c.case == name)
        .unwrap_or_else(|| panic!("{name}: not in errors.json"));
    // Control: the same observations with valid maximum-likelihood options are accepted, so that a
    // refusal below cannot be the refusal of an option the engine does not know yet.
    let control = input_value(
        &case.model,
        case.dose,
        &case.time,
        &case.conc,
        "uniform",
        &case.initial,
        json!({"objective": "ml", "error_model": "additive"}),
    );
    parse_input(&control)
        .unwrap_or_else(|e| panic!("{name}: valid maximum-likelihood options are refused: {e}"));
    // A control that differs from the refused input only in what is refused (a valid `error_init`,
    // the additive model for the unbounded likelihood) is accepted and fitted. Without it an engine
    // that does not know the option could pass the refusal through "unknown field".
    if let Some(control_options) = &case.control_options {
        let control = input_value(
            &case.model,
            case.dose,
            &case.time,
            &case.conc,
            &case.weighting,
            &case.initial,
            Value::Object(control_options.clone().into_iter().collect()),
        );
        let input = parse_input(&control)
            .unwrap_or_else(|e| panic!("{name}: the control input is refused: {e}"));
        if let Err(e) = run(&input) {
            panic!("{name}: the control input {control_options:?} is refused: {e}");
        }
    }
    let options = Value::Object(case.options.clone().into_iter().collect());
    let value = input_value(
        &case.model,
        case.dose,
        &case.time,
        &case.conc,
        &case.weighting,
        &case.initial,
        options,
    );
    let (code, message) = match parse_input(&value) {
        // an option that is a closed set of values in the engine's types is refused when the input is
        // read: the message must name the option, and only `invalid_option` can be refused so
        Err(message) => ("invalid_option".to_string(), message),
        Ok(input) => match run(&input) {
            Ok(_) => panic!("{name}: the engine accepts {}", case.description),
            Err(e) => (error_code(&e), e.to_string()),
        },
    };
    let lower = message.to_lowercase();
    assert_eq!(
        code, case.error.code,
        "{name}: refused with {code} ({message}), expected {}",
        case.error.code
    );
    let missing: Vec<&String> = case
        .error
        .words
        .iter()
        .filter(|w| !lower.contains(&w.to_lowercase()))
        .collect();
    assert!(
        missing.is_empty(),
        "{name}: the message {message:?} does not contain {missing:?}"
    );
}

fn error_code(e: &FitError) -> String {
    serde_json::to_value(e).unwrap()["code"]
        .as_str()
        .unwrap()
        .to_string()
}

// ---------------------------------------------------------------- hand checks (OF-15)

/// (case prefix, subject): profiles that every hand check runs on. The case is `ml_<prefix>_<model>`.
const HAND_PROFILES: &[(&str, &str)] = &[
    ("f", "1"),
    ("iv_s1", "1"),
    ("iv_s2", "1"),
    ("iv_s3", "1"),
    ("indometh", "1"),
    ("indometh", "4"),
    ("oral_s1", "1"),
    ("oral_s2", "1"),
    ("theoph", "1"),
    ("theoph", "7"),
];

fn hand_case(prefix: &str, error_model: &str) -> MlCase {
    load_fit_ml_case(&format!("ml_{prefix}_{error_model}")).expect("the ML oracle case loads")
}

fn sum_z2(result: &FitResult) -> (f64, f64, usize) {
    let table = residual_table(result);
    let (mut s1, mut s2) = (0.0, 0.0);
    for &(_, z) in &table {
        s1 += z;
        s2 += z * z;
    }
    (s1, s2, table.len())
}

/// The additive model has an exact solution (FIT-ML-11): theta is the ordinary least-squares
/// estimate, A = RSS/N, and -2LL = N (ln(2 pi RSS/N) + 1).
#[test]
fn additive_theta_is_ols_and_a_is_rss_over_n() {
    for &(prefix, subject) in HAND_PROFILES {
        let case = hand_case(prefix, "additive");
        let ml = fit_case(&case, subject);
        let ls = wls_fit(&case, subject);
        let tolerance = rel_tolerance(&case);
        for par in case.parameter_names() {
            if par.starts_with("error") {
                continue;
            }
            let name = format!("estimate.{par}");
            assert!(
                tolerance.accepts(need(&ml, &name), need(&ls, &name)),
                "{prefix} subject {subject}: {name} {} against the OLS estimate {}",
                need(&ml, &name),
                need(&ls, &name)
            );
        }
        let n = need(&ml, "n");
        let rss = need(&ls, "wrss");
        let a = need(&ml, "estimate.error_a");
        assert!(
            tolerance.accepts(a * a, rss / n),
            "{prefix} subject {subject}: a^2 = {} against RSS/N = {}",
            a * a,
            rss / n
        );
        let m2ll = n * (libm::log(2.0 * std::f64::consts::PI * rss / n) + 1.0);
        assert!(
            AbsoluteTolerance::FIT_M2LL.accepts(need(&ml, "m2ll"), m2ll),
            "{prefix} subject {subject}: -2LL {} against N (ln(2 pi RSS/N) + 1) = {m2ll}",
            need(&ml, "m2ll")
        );
    }
}

/// AIC_ML - AIC_WRSS = N (1 + ln(2 pi / N)) + 2 and BIC_ML - SBC_WRSS = N (1 + ln(2 pi / N)) + ln N
/// for the additive model (FIT-ML-16).
#[test]
fn additive_information_criteria_differ_by_a_constant() {
    for &(prefix, subject) in HAND_PROFILES {
        let case = hand_case(prefix, "additive");
        let r = fit_case(&case, subject);
        let n = need(&r, "n");
        let k = n * (1.0 + libm::log(2.0 * std::f64::consts::PI / n));
        for (ml, ls, extra, label) in [
            ("aic_ml", "aic", 2.0, "AIC"),
            ("bic_ml", "sbc", libm::log(n), "BIC"),
        ] {
            let difference = need(&r, ml) - need(&r, ls);
            assert!(
                AbsoluteTolerance::FIT_M2LL.accepts(difference, k + extra),
                "{prefix} subject {subject}: {label}_ML - {label}_WRSS = {difference}, expected {}",
                k + extra
            );
        }
    }
}

/// Sum of z^2 = N at the optimum of every error model (FIT-ML-13, FIT-ML-18); the sum is a property
/// of a scale parameter, so it holds for the combined models too (also at their collapse).
#[test]
fn the_squared_iwres_sum_to_n() {
    let mut checked = 0usize;
    for em in caladrius_testkit::ML_ERROR_MODELS {
        for &(prefix, subject) in HAND_PROFILES {
            let name = format!("ml_{prefix}_{em}");
            let case = hand_case(prefix, em);
            assert!(
                case.subjects.iter().any(|s| s == subject),
                "{name}: no reference for subject {subject}"
            );
            checked += 1;
            let r = fit_case(&case, subject);
            let (_, s2, n) = sum_z2(&r);
            assert!(
                Tolerance::FIT_PARAMETERS.accepts(s2, n as f64),
                "{name} subject {subject}: sum of z^2 = {s2}, N = {n}"
            );
            assert!(
                Tolerance::FIT_PARAMETERS.accepts(need(&r, "iwres_sum_sq"), n as f64),
                "{name} subject {subject}: iwres_sum_sq = {}",
                need(&r, "iwres_sum_sq")
            );
        }
    }
    assert_eq!(
        checked,
        caladrius_testkit::ML_ERROR_MODELS.len() * HAND_PROFILES.len(),
        "fits checked"
    );
}

/// The standard deviation of the IWRES (divisor N − 1) is sqrt(N / (N − 1)) for the proportional
/// model: V is a scale of the prediction, so the mean is 0 and the sum of z^2 is N (OF-15, FIT-ML-13,
/// FIT-ML-18). It is checked on the residual table and on the engine's own `iwres_sd`, with the
/// tolerance of the sum of z^2 = N (`FIT_PARAMETERS`): the standard deviation follows from it.
#[test]
fn the_proportional_iwres_have_standard_deviation_sqrt_n_over_n_minus_1() {
    let mut checked = 0usize;
    for &(prefix, subject) in HAND_PROFILES {
        let case = hand_case(prefix, "proportional");
        let r = fit_case(&case, subject);
        let (_, s2, n) = sum_z2(&r);
        assert!(n >= 2, "{prefix} subject {subject}: {n} observations");
        let want = (n as f64 / (n as f64 - 1.0)).sqrt();
        let table = residual_table(&r);
        let mean = table.iter().map(|&(_, z)| z).sum::<f64>() / n as f64;
        let variance = table
            .iter()
            .map(|&(_, z)| (z - mean) * (z - mean))
            .sum::<f64>()
            / (n as f64 - 1.0);
        assert!(
            Tolerance::FIT_PARAMETERS.accepts(variance.sqrt(), want),
            "{prefix} subject {subject}: SD of the residual table {} against {want} (sum of z^2 = {s2})",
            variance.sqrt()
        );
        assert!(
            Tolerance::FIT_PARAMETERS.accepts(need(&r, "iwres_sd"), want),
            "{prefix} subject {subject}: iwres_sd {} against sqrt(N/(N-1)) = {want}",
            need(&r, "iwres_sd")
        );
        checked += 1;
    }
    assert_eq!(checked, HAND_PROFILES.len(), "fits checked");
}

/// The proportional profile: b^2 = mean((r / yhat)^2) at the estimate of theta, and, because V is
/// a scale of the prediction (yhat is proportional to 1/V), sum of z = 0 as well (FIT-ML-18).
#[test]
fn the_proportional_profile_in_b_and_zero_mean_iwres() {
    for &(prefix, subject) in HAND_PROFILES {
        let case = hand_case(prefix, "proportional");
        let r = fit_case(&case, subject);
        let rows = serde_json::to_value(&r).unwrap()["observations"].clone();
        let rows = rows.as_array().unwrap();
        let n = rows.len() as f64;
        let mut s = 0.0;
        for row in rows {
            let ratio = row["residual"].as_f64().unwrap() / row["predicted"].as_f64().unwrap();
            s += ratio * ratio;
        }
        let b = need(&r, "estimate.error_b");
        assert!(
            Tolerance::FIT_PARAMETERS.accepts(b * b, s / n),
            "{prefix} subject {subject}: b^2 = {} against mean (r/yhat)^2 = {}",
            b * b,
            s / n
        );
        let (s1, _, _) = sum_z2(&r);
        assert!(
            AbsoluteTolerance::FIT_IWRES.accepts(s1 / n, 0.0),
            "{prefix} subject {subject}: mean IWRES = {}",
            s1 / n
        );
    }
}

/// The standard errors of theta in the additive model are the least-squares ones times
/// sqrt(DF/N): Cov = sigma^2 (J'J)^-1 with sigma^2 = RSS/N, against s^2 (J'J)^-1 with s^2 = RSS/DF
/// (FIT-ML-17, hand check of FIT-ML-11).
#[test]
fn additive_standard_errors_are_least_squares_times_sqrt_df_over_n() {
    for &(prefix, subject) in HAND_PROFILES {
        let case = hand_case(prefix, "additive");
        let ml = fit_case(&case, subject);
        let ls = wls_fit(&case, subject);
        let n = need(&ml, "n");
        let df = n - need(&ml, "p");
        let factor = (df / n).sqrt();
        for par in case.parameter_names() {
            if par.starts_with("error") {
                continue;
            }
            let name = format!("se.{par}");
            let want = need(&ls, &name) * factor;
            assert!(
                rel_tolerance(&case).accepts(need(&ml, &name), want),
                "{prefix} subject {subject}: {name} {} against {want} (LS SE {} * sqrt({df}/{n}))",
                need(&ml, &name),
                need(&ls, &name)
            );
        }
    }
}

/// A combined model that ends with one component at 0 is the one-parameter model of the other
/// component (FIT-ML-13c, FIT-ML-15): same -2LL, same theta, status `at_bound`, and the component
/// that is not 0 equals the estimate of the one-parameter fit. The collapse pattern is the
/// reference's (`at_bound` in the options file).
#[test]
fn a_collapsed_combined_model_equals_the_one_parameter_model() {
    let mut checked = 0usize;
    for name in list_fit_ml_cases().unwrap() {
        let case = load_fit_ml_case(&name).unwrap();
        if !matches!(case.error_model.as_str(), "combined" | "combined_sd")
            || !case.bounds.is_empty()
        {
            continue;
        }
        let prefix = name
            .strip_prefix("ml_")
            .and_then(|n| n.strip_suffix(&format!("_{}", case.error_model)));
        let Some(prefix) = prefix else { continue };
        // the `_t0` cases keep the sample at time 0 (all rows): the one-parameter fit to compare with
        // is the additive case of the same data (a proportional fit refuses the row at time 0)
        let with_t0 = prefix.ends_with("_t0");
        let prefix = prefix.strip_suffix("_t0").unwrap_or(prefix);
        for subject in &case.subjects {
            let Some(bound) = case.at_bound.get(subject).filter(|b| !b.is_empty()) else {
                continue;
            };
            let (alone, par) = if bound[0] == "error_a" {
                ("proportional", "error_b")
            } else {
                ("additive", "error_a")
            };
            assert_eq!(bound.len(), 1, "{name} subject {subject}");
            assert!(
                !with_t0 || alone == "additive",
                "{name} subject {subject}: a fit with the row at time 0 cannot collapse onto {alone}"
            );
            let simple = hand_case(prefix, alone);
            let combined = fit_case(&case, subject);
            let one = fit_case(&simple, subject);
            assert_eq!(status_id(&combined), "at_bound", "{name} subject {subject}");
            assert!(
                AbsoluteTolerance::FIT_M2LL.accepts(need(&combined, "m2ll"), need(&one, "m2ll")),
                "{name} subject {subject}: -2LL {} against the {alone} fit {}",
                need(&combined, "m2ll"),
                need(&one, "m2ll")
            );
            for p in simple.parameter_names() {
                let key = format!("estimate.{p}");
                let reference = case.value(subject, &key).unwrap_or(0.0);
                assert!(
                    rel_tolerance(&case).accepts(need(&combined, &key), need(&one, &key)),
                    "{name} subject {subject}: {key} {} against the {alone} fit {} (reference {reference})",
                    need(&combined, &key),
                    need(&one, &key)
                );
            }
            assert_eq!(
                need(&combined, &format!("estimate.{}", bound[0])),
                0.0,
                "{name} subject {subject}: {} must be exactly 0",
                bound[0]
            );
            // a combined model costs one more parameter in the criteria (FIT-ML-15): AIC differs by 2
            assert!(
                AbsoluteTolerance::FIT_M2LL
                    .accepts(need(&combined, "aic_ml") - need(&one, "aic_ml"), 2.0),
                "{name} subject {subject}: AIC_ML must be larger by 2 (P + Q counts both)"
            );
            let _ = par;
            checked += 1;
        }
    }
    assert!(checked >= 10, "{checked} collapsed fits checked");
}

/// Multiplying y and the dose by c scales every prediction by c: -2LL changes by exactly 2 N ln c,
/// theta and b do not change, a is multiplied by c (OF-15).
#[test]
fn scaling_y_and_dose_shifts_m2ll_by_two_n_ln_c() {
    let c = 7.5_f64;
    let profiles = [
        ("f", "1"),
        ("iv_s1", "1"),
        ("oral_s1", "1"),
        ("indometh", "1"),
    ];
    let mut checked = 0usize;
    for em in caladrius_testkit::ML_ERROR_MODELS {
        for &(prefix, subject) in &profiles {
            let name = format!("ml_{prefix}_{em}");
            let case = hand_case(prefix, em);
            assert!(
                case.subjects.iter().any(|s| s == subject),
                "{name}: no reference for subject {subject}"
            );
            checked += 1;
            let base = fit_case(&case, subject);
            let mut value = case_input(&case, subject);
            let scale = |key: &str, value: &mut Value| {
                let scaled: Vec<f64> = value[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap() * c)
                    .collect();
                value[key] = json!(scaled);
            };
            scale("conc", &mut value);
            value["dose"] = json!(value["dose"].as_f64().unwrap() * c);
            if let Some(init) = value["options"].get("error_init").cloned() {
                let mut init: Vec<f64> = serde_json::from_value(init).unwrap();
                init[0] *= c; // the first error parameter is the additive one for every model that has it
                if em == "proportional" {
                    init[0] /= c;
                }
                value["options"]["error_init"] = json!(init);
            }
            let scaled = fit_value(&format!("{name} subject {subject} scaled"), &value);
            let n = need(&base, "n");
            let want = need(&base, "m2ll") + 2.0 * n * libm::log(c);
            assert!(
                AbsoluteTolerance::FIT_M2LL.accepts(need(&scaled, "m2ll"), want),
                "{name} subject {subject}: -2LL {} against {want} (base + 2 N ln c)",
                need(&scaled, "m2ll")
            );
            for par in case.parameter_names() {
                let key = format!("estimate.{par}");
                let factor = if par == "error_a" { c } else { 1.0 };
                let want = need(&base, &key) * factor;
                let got = need(&scaled, &key);
                let ok = if want == 0.0 {
                    got == 0.0
                } else {
                    Tolerance::FIT_PARAMETERS.accepts(got, want)
                };
                assert!(
                    ok,
                    "{name} subject {subject}: {key} {got:e} against {want:e}"
                );
            }
        }
    }
    assert_eq!(
        checked,
        caladrius_testkit::ML_ERROR_MODELS.len() * profiles.len(),
        "scaled fits checked"
    );
}

// ---------------------------------------------------------------- the generated list of cases

include!("oracle_fit_ml.cases");

/// Guards the guard (green today): every case file of the oracle is run by a test above, every
/// case named above exists, and every refusal of `errors.json` is tested.
#[test]
fn every_ml_case_and_refusal_has_tests() {
    let on_disk = list_fit_ml_cases().unwrap();
    let listed: Vec<String> = ML_CASES.iter().map(|c| (*c).to_string()).collect();
    assert_eq!(
        on_disk, listed,
        "the cases on disk and the cases tested differ"
    );
    let mut refusals: Vec<String> = load_fit_ml_errors()
        .unwrap()
        .into_iter()
        .map(|c| c.case)
        .collect();
    refusals.sort();
    let listed: Vec<String> = ML_REFUSALS.iter().map(|c| (*c).to_string()).collect();
    assert_eq!(refusals, listed, "the refusals on disk and tested differ");
}

/// Green today: the reference loads, every subject has the quantities this file compares, and the
/// inputs of the cases are well formed (so a red test is red because of the engine).
#[test]
fn the_reference_is_complete_for_the_tests() {
    for name in ML_CASES {
        let case = load_fit_ml_case(name).unwrap();
        assert!(
            !case.subjects.is_empty(),
            "{name}: no subject has a maximum"
        );
        for s in &case.subjects {
            for key in [
                "n",
                "p",
                "q",
                "m2ll",
                "aic_ml",
                "bic_ml",
                "rss_uniform",
                "iwres_mean",
            ] {
                assert!(case.value(s, key).is_some(), "{name} subject {s}: no {key}");
            }
            let n = case.value(s, "n").unwrap() as usize;
            for i in 1..=n {
                assert!(
                    case.value(s, &format!("iwres.{i}")).is_some(),
                    "{name} {s} iwres.{i}"
                );
            }
            let input = case_input(&case, s);
            assert_eq!(
                input["time"].as_array().unwrap().len(),
                n,
                "{name} subject {s}"
            );
        }
    }
}

/// Green today: a subject of a dataset that a case does not fit is one that the engine must refuse
/// before fitting (observation 0 at time 0 under a combined error model: `unbounded_likelihood`), and
/// every such refusal row names a subject that a case leaves out and whose profile has that
/// observation. No subject is dropped without a refusal, and no refusal row stands alone.
#[test]
fn every_subject_left_out_of_a_case_has_its_refusal() {
    let rows: Vec<_> = load_fit_ml_errors()
        .unwrap()
        .into_iter()
        .filter(|c| c.case.starts_with("ml6e_"))
        .collect();
    let mut left_out = 0usize;
    for name in ML_CASES {
        let case = load_fit_ml_case(name).unwrap();
        let dataset = name.split('_').nth(1).unwrap_or("");
        for profile in &case.dataset.profiles {
            let subject = profile.subject.to_string();
            if case.subjects.contains(&subject) {
                continue;
            }
            left_out += 1;
            let row = format!("ml6e_y0_at_t0_{}_{dataset}{subject}", case.error_model);
            let found = rows.iter().find(|c| c.case == row);
            let found = found.unwrap_or_else(|| {
                panic!("{name}: subject {subject} is neither fitted nor refused ({row})")
            });
            assert_eq!(found.error.code, "unbounded_likelihood", "{row}");
            assert!(
                profile
                    .time
                    .iter()
                    .zip(&profile.conc)
                    .any(|(&t, &y)| t == 0.0 && y == 0.0),
                "{row}: the profile has no observation 0 at time 0"
            );
        }
    }
    assert_eq!(left_out, rows.len(), "refusal rows without a case");
    assert!(left_out > 0, "no subject is refused");
}
