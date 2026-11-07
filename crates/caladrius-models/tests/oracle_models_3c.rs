#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: helpers panic on purpose
//! Three-compartment model oracle (task T-142): `caladrius-models` against exact values on time
//! grids (`oracle/expected/models/pk3/`, produced by `oracle/scripts/models_3c_closed_form.R`: the
//! explicit sums of exponentials of `specs/models.md` MOD-3C-10 to 14 in 256-bit arithmetic,
//! cross-checked against a matrix exponential, an ODE solver and the eigenvalues of the 3 x 3
//! system), for the six ids `pk3.*`, their three parameterisations, the derived quantities, the
//! conversions between the sets (OM-23), the points of poor conditioning (OM-22), the partial
//! derivatives (OM-21), multiple dosing and steady state (OM-28) and the inputs the engine must
//! refuse (OM-26).
//!
//! Written before the engine knows any `pk3.*` id: every test below fails today with "unknown model
//! id" (`ModelId::from_id` returns `None`) and must stay red until the engine card for three
//! compartments (T-144) lands. The file compiles against the present API of the crate.
//!
//! # What the engine must provide (all in the crate root, `caladrius_models`)
//!
//! - `ModelId::from_id` for `pk3.iv_bolus`, `pk3.iv_infusion`, `pk3.oral_1`, `pk3.oral_1_lag`,
//!   `pk3.oral_0`, `pk3.oral_0_lag`, with the same `ModelInput` as the other models: `params` are
//!   by name, in exactly one complete set: `cl, vc, q2, vp2, q3, vp3` (clearance), `k10, k12, k21,
//!   k13, k31, vc` (micro) or `a, b, c, alpha, beta, gamma` (macro, the dose then required and > 0),
//!   plus `ka`, `dur`, `tlag` as the id needs.
//! - `ModelOutput::conc()` and `auc()`; `ModelOutput::get(name)` for the scalars of the case files
//!   (the full list is in the `quantities` field of any case's options) and `aumc[i]` for AUMC(0,
//!   t_i), as for the two-compartment models.
//! - `jacobian(&input, Derivatives::Analytic)` with the closed-form partial derivatives (not the
//!   forward-difference fallback) with respect to every parameter of `input.params`.
//! - `ModelInput::with_regimen` for the regimens of the multiple-dosing tests, as for `pk1`/`pk2`.
//!
//! Tolerances: `Tolerance::MODEL_VALUES` for every value and `Tolerance::MODEL_DERIVATIVES` for
//! every derivative, never changed. Two decisions are left to the orchestrator and are marked in the
//! tests: the scalars of the group `scalar_ill` (condition number above 1e3 with respect to the
//! parameters of the case, so that the 1e-12 relative tolerance cannot be demanded of any double
//! precision implementation) are in tests marked `#[ignore]`; run them with `--ignored`. The
//! derivatives with a condition above 1e3 are not written, so they are not tested.

use std::collections::BTreeMap;

use caladrius_models::{
    Derivatives, DoseEvent, ModelId, ModelInput, ModelOutput, Regimen, jacobian, run,
};
use caladrius_testkit::{
    MdCase, ModelCase, Pk3Kind, RegimenKind, Table, Tolerance, compare_tables, list_pk3_cases,
    list_pk3_md_cases, load_pk3_case, load_pk3_errors, load_pk3_md_case,
};

/// How close an infusion of 0.01 h is to a bolus once it is over (an inequality of the model, not a tolerance of the oracle).
const SHORT_INFUSION: f64 = 1e-7;

fn input_of(case: &ModelCase) -> ModelInput {
    ModelInput {
        model: ModelId::from_id(&case.model)
            .unwrap_or_else(|| panic!("{}: unknown model id {:?}", case.name, case.model)),
        dose: case.dose,
        params: case.parameters.clone(),
        times: case.times.clone(),
    }
}

/// The rows of one quantity: `conc`, `auc`, `aumc` at the times, or the scalars of a group.
fn expected_of(case: &ModelCase, quantity: &str) -> Table {
    let mut expected = Table::new();
    for (group, name, value) in case.expected.iter() {
        let wanted = match quantity {
            "scalar" => group == "scalar",
            "scalar_ill" => group == "scalar_ill",
            _ => group.starts_with("t=") && name == quantity,
        };
        if wanted {
            expected.insert(group, name, value);
        }
    }
    expected
}

/// AUMC(0, t_i) of the i-th input time (see the header).
fn aumc_of(output: &ModelOutput, index: usize) -> Option<f64> {
    output.get(&format!("aumc[{index}]"))
}

fn compare(case_name: &str, case: &ModelCase, output: &ModelOutput, quantity: &str) {
    let expected = expected_of(case, quantity);
    let mut actual = Table::new();
    match quantity {
        "conc" | "auc" | "aumc" => {
            for (i, key) in case.time_keys.iter().enumerate() {
                let value = match quantity {
                    "conc" => output.conc().get(i).copied(),
                    "auc" => output.auc().get(i).copied(),
                    _ => aumc_of(output, i),
                };
                actual.insert(key.as_str(), quantity, value);
            }
        }
        _ => {
            for (group, name, _) in expected.iter() {
                actual.insert(group, name, output.get(name));
            }
        }
    }
    let report = compare_tables(&expected, &actual, Tolerance::MODEL_VALUES);
    assert!(report.is_ok(), "{case_name} {quantity}: {report}");
}

fn check(case_name: &str, kind: Pk3Kind, quantity: &str) {
    let loaded = load_pk3_case(case_name).expect("the three-compartment oracle case loads");
    assert_eq!(loaded.kind, kind, "{case_name}");
    let case = &loaded.case;
    let input = input_of(case);
    let expected = expected_of(case, quantity);
    assert!(
        !expected.is_empty(),
        "{case_name}: nothing expected for {quantity}"
    );
    let output = run(&input)
        .unwrap_or_else(|e| panic!("{case_name}: the engine refuses a valid model: {e}"));
    compare(case_name, case, &output, quantity);
}

fn check_derivatives(case_name: &str) {
    let loaded = load_pk3_case(case_name).expect("the three-compartment oracle case loads");
    assert_eq!(loaded.kind, Pk3Kind::Derivatives, "{case_name}");
    let case = &loaded.case;
    let input = input_of(case);
    let jac = jacobian(&input, Derivatives::Analytic)
        .unwrap_or_else(|e| panic!("{case_name}: no Jacobian for a valid model: {e}"));
    assert_eq!(
        jac.method,
        Derivatives::Analytic,
        "{case_name}: closed-form derivatives expected, not the forward-difference fallback"
    );
    let mut actual = Table::new();
    for (group, name, _) in case.expected.iter() {
        let column = name
            .strip_prefix("d_")
            .and_then(|p| jac.parameters.iter().position(|q| q == p))
            .and_then(|j| jac.columns.get(j));
        let index = case.time_keys.iter().position(|k| k == group);
        let value = column.zip(index).and_then(|(c, i)| c.get(i)).copied();
        actual.insert(group, name, value);
    }
    let report = compare_tables(&case.expected, &actual, Tolerance::MODEL_DERIVATIVES);
    assert!(report.is_ok(), "{case_name} derivatives: {report}");
}

// ---------------------------------------------------------------- multiple dosing (OM-28)

/// The one place where a regimen becomes a `ModelInput`: the oracle's regimen as the engine's
/// `Regimen` type, written into the input by `ModelInput::with_regimen`.
fn md_input_of(case: &MdCase) -> ModelInput {
    let name = &case.case.name;
    let r = &case.regimen;
    let regimen = match r.kind {
        RegimenKind::Schedule => Regimen::Schedule {
            doses: r
                .records
                .iter()
                .map(|d| DoseEvent {
                    time: d.time,
                    amount: d.dose,
                    dur: d.dur,
                })
                .collect(),
        },
        RegimenKind::Regular => Regimen::Regular {
            tau: r.tau.unwrap_or_else(|| panic!("{name}: no tau")),
            n_doses: r.n_doses.unwrap_or_else(|| panic!("{name}: no n_doses")),
        },
        RegimenKind::SteadyState => Regimen::SteadyState {
            tau: r.tau.unwrap_or_else(|| panic!("{name}: no tau")),
        },
    };
    input_of(&case.case).with_regimen(&regimen)
}

fn check_md(case_name: &str, quantity: &str) {
    let loaded = load_pk3_md_case(case_name).expect("the multiple-dosing case loads");
    let case = &loaded.case;
    let input = md_input_of(&loaded);
    let expected = expected_of(case, quantity);
    if quantity == "accum_c" && expected.is_empty() {
        return; // only a steady state has the per-time accumulation ratio
    }
    assert!(
        !expected.is_empty(),
        "{case_name}: nothing expected for {quantity}"
    );
    let output = run(&input).unwrap_or_else(|e| {
        panic!("{case_name}: unsupported dosing: the engine refuses the regimen: {e}")
    });
    let mut actual = Table::new();
    match quantity {
        "conc" | "auc" | "accum_c" => {
            for (i, key) in case.time_keys.iter().enumerate() {
                let value = match quantity {
                    "conc" => output.conc().get(i).copied(),
                    "auc" => output.auc().get(i).copied(),
                    _ => output.get(&format!("accum_c[{i}]")),
                };
                actual.insert(key.as_str(), quantity, value);
            }
        }
        _ => {
            for (_, name, _) in expected.iter() {
                actual.insert("scalar", name, output.get(name));
            }
        }
    }
    let report = compare_tables(&expected, &actual, Tolerance::MODEL_VALUES);
    assert!(report.is_ok(), "{case_name} {quantity}: {report}");
}

// ---------------------------------------------------------------- generated tests

macro_rules! value_tests {
    ($($case:ident),* $(,)?) => {
        $(
            mod $case {
                use super::{check, Pk3Kind};
                #[test]
                fn concentration() { check(concat!("model_pk3_", stringify!($case)), Pk3Kind::Values, "conc"); }
                #[test]
                fn auc() { check(concat!("model_pk3_", stringify!($case)), Pk3Kind::Values, "auc"); }
                #[test]
                fn aumc() { check(concat!("model_pk3_", stringify!($case)), Pk3Kind::Values, "aumc"); }
                #[test]
                fn scalars() { check(concat!("model_pk3_", stringify!($case)), Pk3Kind::Values, "scalar"); }
            }
        )*
    };
}

macro_rules! conditioning_tests {
    ($($case:ident),* $(,)?) => {
        mod conditioning {
            $(
                mod $case {
                    use super::super::{check, Pk3Kind};
                    #[test]
                    fn scalars() { check(concat!("model_pk3_", stringify!($case)), Pk3Kind::Conditioning, "scalar"); }
                }
            )*
        }
    };
}

macro_rules! conversion_tests {
    ($($case:ident),* $(,)?) => {
        mod conversions {
            $(
                mod $case {
                    use super::super::{check, Pk3Kind};
                    #[test]
                    fn scalars() { check(concat!("model_pk3_", stringify!($case)), Pk3Kind::Conversion, "scalar"); }
                }
            )*
        }
    };
}

/// The scalars with a condition number above 1e3 (`scalar_ill`), for the cases that have such rows only
/// (the generated lists `ILL_*`): the tolerance decision is the orchestrator's, so the tests are ignored.
macro_rules! ill_tests {
    ($module:ident, $kind:ident; $($case:ident),* $(,)?) => {
        mod $module {
            $(
                mod $case {
                    use super::super::{check, Pk3Kind};
                    #[test]
                    #[ignore = "ill-conditioned scalars (kappa > 1e3): the tolerance decision is the orchestrator's"]
                    fn scalars() { check(concat!("model_pk3_", stringify!($case)), Pk3Kind::$kind, "scalar_ill"); }
                }
            )*
        }
    };
}

macro_rules! derivative_tests {
    ($($case:ident),* $(,)?) => {
        mod derivatives {
            $(
                #[test]
                fn $case() { super::check_derivatives(concat!("model_pk3_", stringify!($case))); }
            )*
        }
    };
}

macro_rules! md_tests {
    ($($case:ident),* $(,)?) => {
        mod multiple_dosing {
            $(
                mod $case {
                    use super::super::check_md;
                    #[test]
                    fn concentration() { check_md(concat!("model_pk3_", stringify!($case)), "conc"); }
                    #[test]
                    fn auc() { check_md(concat!("model_pk3_", stringify!($case)), "auc"); }
                    #[test]
                    fn accumulation_by_time() { check_md(concat!("model_pk3_", stringify!($case)), "accum_c"); }
                    #[test]
                    fn scalars() { check_md(concat!("model_pk3_", stringify!($case)), "scalar"); }
                }
            )*
        }
    };
}

// Generated by oracle/scripts/models_3c_closed_form.R: the constants used by the guard below and the
// invocations of the macros above, one name per case file (without the prefix `model_pk3_`).
include!("oracle_models_3c.cases");

/// Guards the guard: every case file of the three-compartment oracle is run by a test above, and
/// every case named above exists.
#[test]
fn every_pk3_case_has_tests() {
    let on_disk = list_pk3_cases().unwrap();
    let mut listed: Vec<String> = VALUE_CASES
        .iter()
        .chain(CONDITIONING_CASES)
        .chain(CONVERSION_CASES)
        .chain(DERIVATIVE_CASES)
        .map(|c| format!("model_pk3_{c}"))
        .collect();
    listed.sort();
    assert_eq!(
        on_disk, listed,
        "the cases on disk and the cases tested differ"
    );
    // the ignored tests of ill-conditioned scalars exist exactly for the cases that have such rows
    let mut with_rows: Vec<String> = on_disk
        .iter()
        .filter(|n| {
            let c = load_pk3_case(n).unwrap();
            c.case
                .expected
                .iter()
                .any(|(group, _, _)| group == "scalar_ill")
        })
        .cloned()
        .collect();
    with_rows.sort();
    let mut ill_listed: Vec<String> = ILL_VALUE_CASES
        .iter()
        .chain(ILL_CONDITIONING_CASES)
        .chain(ILL_CONVERSION_CASES)
        .map(|c| format!("model_pk3_{c}"))
        .collect();
    ill_listed.sort();
    assert_eq!(
        with_rows, ill_listed,
        "the cases with scalar_ill rows and the cases with an ignored test differ"
    );
    let md_on_disk = list_pk3_md_cases().unwrap();
    let mut md_listed: Vec<String> = MD_CASES.iter().map(|c| format!("model_pk3_{c}")).collect();
    md_listed.sort();
    assert_eq!(
        md_on_disk, md_listed,
        "the multiple-dosing cases on disk and tested differ"
    );
}

// ---------------------------------------------------------------- errors (MOD-3C-06, MOD-3C-26, MOD-GEN-04)

/// Every case of the group must be refused, with a readable message that names what the oracle
/// says it must name.
fn refused(group: &str) {
    let cases: Vec<_> = load_pk3_errors()
        .unwrap()
        .into_iter()
        .filter(|c| c.group == group)
        .collect();
    assert!(!cases.is_empty(), "no error case in group {group}");
    let mut failures = Vec::new();
    for c in &cases {
        let model = ModelId::from_id(&c.model)
            .unwrap_or_else(|| panic!("{}: unknown model id {:?}", c.id, c.model));
        let result = run(&ModelInput {
            model,
            dose: c.dose,
            params: c.parameters.clone(),
            times: c.times.clone(),
        });
        match result {
            Ok(_) => failures.push(format!("{}: accepted ({})", c.id, c.reason)),
            Err(e) => {
                let message = e.to_string().to_lowercase();
                for alternatives in &c.message_contains {
                    if !alternatives.iter().any(|w| message.contains(w.as_str())) {
                        failures.push(format!(
                            "{}: message {message:?} contains none of {alternatives:?} ({})",
                            c.id, c.reason
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn errors_domain() {
    refused("domain");
}
#[test]
fn errors_one_compartment() {
    refused("one_compartment");
}
#[test]
fn errors_macro() {
    refused("macro");
}
#[test]
fn errors_sets_and_names() {
    refused("sets");
}
#[test]
fn errors_numeric() {
    refused("numeric");
}
#[test]
fn errors_times() {
    refused("times");
}

// ---------------------------------------------------------------- properties of the models

fn params(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn id(text: &str) -> ModelId {
    ModelId::from_id(text).unwrap_or_else(|| panic!("unknown model id {text:?}"))
}

fn conc_at(model: &str, dose: f64, p: &[(&str, f64)], times: &[f64]) -> Vec<f64> {
    run(&ModelInput {
        model: id(model),
        dose,
        params: params(p),
        times: times.to_vec(),
    })
    .unwrap()
    .conc()
    .to_vec()
}

/// Worked example Q1 of `specs/models.md` section 14: exponents 3, 0.5, 0.05.
const BASE: [(&str, f64); 6] = [
    ("cl", 3.0),
    ("vc", 8.0),
    ("q2", 9.5),
    ("vp2", 9.5),
    ("q3", 6.3),
    ("vp3", 31.5),
];

fn with(extra: &[(&'static str, f64)]) -> Vec<(&'static str, f64)> {
    BASE.iter().chain(extra).copied().collect()
}

/// MOD-3C-11: the value at ka = an exponent is the limit of its neighbours (no 0/0).
#[test]
fn ka_equal_alpha_is_the_limit_of_its_neighbours() {
    let times = [0.2, 1.0, 12.0];
    let at = |ka: f64| conc_at("pk3.oral_1", 100.0, &with(&[("ka", ka)]), &times);
    let exact = at(3.0);
    for (delta, bound) in [(1e-3, 1e-2), (1e-6, 1e-5), (1e-9, 1e-8)] {
        let (below, above) = (at(3.0 * (1.0 - delta)), at(3.0 * (1.0 + delta)));
        for i in 0..times.len() {
            assert!((above[i] - exact[i]).abs() <= bound * exact[i]);
            assert!((below[i] - exact[i]).abs() <= bound * exact[i]);
        }
    }
}

/// The infusion tends to the bolus when the duration tends to 0 with the dose fixed.
#[test]
fn a_very_short_infusion_is_a_bolus_after_the_end() {
    let times = [1.0, 3.0, 10.0];
    let short = conc_at("pk3.iv_infusion", 100.0, &with(&[("dur", 1e-8)]), &times);
    let bolus = conc_at("pk3.iv_bolus", 100.0, &BASE, &times);
    for (x, y) in short.iter().zip(&bolus) {
        assert!((x - y).abs() <= SHORT_INFUSION * y.abs(), "{x} against {y}");
    }
}

/// Zero-order absorption is the infusion function.
#[test]
fn zero_order_absorption_is_the_infusion_function() {
    let times = [0.5, 1.0, 2.0, 2.5, 6.0, 24.0];
    let a = conc_at("pk3.oral_0", 100.0, &with(&[("dur", 2.0)]), &times);
    let b = conc_at("pk3.iv_infusion", 100.0, &with(&[("dur", 2.0)]), &times);
    assert_eq!(a, b);
}

/// The lag shifts the curve: C_lag(t) = C(t - tlag), and 0 up to the lag.
#[test]
fn the_lag_shifts_the_curve() {
    let late = [0.5, 1.5, 3.0, 8.0];
    for (lag_id, plain_id, extra) in [
        ("pk3.oral_1_lag", "pk3.oral_1", [("ka", 2.0)].as_slice()),
        ("pk3.oral_0_lag", "pk3.oral_0", [("dur", 2.0)].as_slice()),
    ] {
        let tlag = 0.75;
        let shifted: Vec<f64> = late.iter().map(|t| t + tlag).collect();
        let mut lagged = with(extra);
        lagged.push(("tlag", tlag));
        let a = conc_at(lag_id, 100.0, &lagged, &shifted);
        let b = conc_at(plain_id, 100.0, &with(extra), &late);
        for (x, y) in a.iter().zip(&b) {
            assert!(
                Tolerance::MODEL_VALUES.accepts(*x, *y),
                "{lag_id}: {x} against {y}"
            );
        }
        let before = conc_at(lag_id, 100.0, &lagged, &[-1.0, 0.0, 0.5, tlag]);
        assert!(before.iter().all(|&x| x == 0.0), "{lag_id}: {before:?}");
    }
}

/// Values before the dose are 0, also for the bolus; the bolus at t = 0 is D / vc.
#[test]
fn time_origin_conventions() {
    let c = conc_at("pk3.iv_bolus", 100.0, &BASE, &[-5.0, -1e-9, 0.0]);
    assert_eq!(c[0], 0.0);
    assert_eq!(c[1], 0.0);
    assert!(Tolerance::MODEL_VALUES.accepts(c[2], 12.5));
}

/// The order of the times does not matter and a repeated time gives the same value twice.
#[test]
fn times_may_be_unsorted_or_repeated() {
    let p = with(&[("ka", 1.5)]);
    let a = conc_at("pk3.oral_1", 100.0, &p, &[6.0, 0.5, 6.0, 2.0]);
    let b = conc_at("pk3.oral_1", 100.0, &p, &[0.5, 2.0, 6.0]);
    assert_eq!(a[0], a[2]);
    assert_eq!(a[1], b[0]);
    assert_eq!(a[3], b[1]);
    assert_eq!(a[0], b[2]);
}

/// An empty grid is a valid request with empty answers; the scalars still exist (Q1: AUC = D / CL).
#[test]
fn an_empty_time_grid_is_not_an_error() {
    let out = run(&ModelInput {
        model: id("pk3.iv_bolus"),
        dose: 100.0,
        params: params(&BASE),
        times: Vec::new(),
    })
    .unwrap();
    assert!(out.conc().is_empty() && out.auc().is_empty());
    assert!(Tolerance::MODEL_VALUES.accepts(out.get("auc_inf").unwrap(), 100.0 / 3.0));
    assert_eq!(out.get("no_such_quantity"), None);
}

/// A dose of 0 gives 0 everywhere with the clearance and micro sets.
#[test]
fn a_zero_dose_gives_zero_concentrations() {
    let c = conc_at("pk3.iv_bolus", 0.0, &BASE, &[0.0, 1.0, 10.0]);
    assert!(c.iter().all(|&x| x == 0.0), "{c:?}");
}
