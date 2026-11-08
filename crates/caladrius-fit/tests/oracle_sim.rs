#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: helpers panic on purpose
#![allow(non_snake_case)] // some case names carry a capital (a subject label "S1")
//! Population-simulation oracle (card T-158, rules SIM-* of `specs/simulation.md`): the ground truth for the
//! simulator, written before any engine code exists.
//!
//! What is compared, and with what (`oracle/README.md`, section "Population simulation"):
//!
//! * `oracle/expected/sim/gen/`: the Philox4x32-10 known answers and the blocks of SX1 (integers), and the words,
//!   uniforms and normals of 37 streams (SIM-ORA-01): seeds from 0 to 2^64-1, labels from empty to 200 bytes,
//!   replicates up to 2^24-1, far words. Doubles are exchanged as hexadecimal floats and compared as parsed values.
//! * `oracle/expected/sim/transform/`: the AS 241 transform against a 256-bit inverse at the required grid
//!   points (SIM-ORA-02): bit for bit where the file says so (|q| <= 0.425), within 1e-14 elsewhere.
//! * `oracle/expected/sim/exact/`: the exact part with the draws given, 22 cases (SIM-ORA-03), computed at 256 bits.
//! * `oracle/expected/sim/study/`: nine whole studies (tables, counts, summary, dataset; SIM-OUT-01, -02, SIM-SUM-01).
//! * `oracle/expected/sim/dist/`: the distribution of the draws against stated bounds (SIM-ORA-04, SX3).
//! * `oracle/expected/sim/quantile/`: the type 7 quantile, bit for bit (SX5).
//! * `oracle/expected/sim/invariance/`: pairs of requests whose answers agree or differ (SIM-ORA-05).
//! * `oracle/expected/sim/errors.json`: refusals as rows, each refused row with a control (SIM-REF-*).
//! * `oracle/expected/sim/roundtrip/`: simulate then fit (SIM-ORA-06), `#[ignore]`d until the population fit
//!   (T-154) exists.
//!
//! Written before the engine exists: every engine test fails at run time with "simulation not built", from the
//! one function below, `simulation`. **The engine card (T-159) replaces the body of that function and nothing
//! else.** The guards at the end are green today. The checks live in `caladrius_testkit::sim_check` and are run
//! against an independent reference (`caladrius_testkit::sim_ref`) by `crates/caladrius-testkit/tests/
//! sim_consistency.rs`, so a failing engine test fails because of the engine.
//!
//! # The request and the answer (assumed; the plainest name where `specs/simulation.md` is silent)
//!
//! One function, one JSON request with an `op`; numbers may be JSON numbers, decimal strings or hexadecimal-float
//! strings (`0x1.8p+1`), and the adapter reads all three.
//!
//! ```text
//! { "op": "philox", "counter": [c0,c1,c2,c3], "key": [k0,k1] }            -> { "out": [4 integers] }
//! { "op": "stream", "seed": "<decimal>", "replicate", "purpose": 1|2, "label": "..",
//!   "n": [word indices] | "count": N }
//!      -> { "key": [2], "c1": int, "label_hash": "16 hex", "u": [..], "z": [..], "words": ["16 hex", ..] }
//! { "op": "transform", "u": [..] }                                         -> { "z": [..] }
//! { "op": "exact", model, theta, random_effects, omega_shape, omega, error_model, xi, dose,
//!   "options": { negative, lloq }, "subjects": [ { id, time: [..], z_eta: [..], z_eps: [..] } ] }
//!      -> { "l": [[..]], "subjects": [ { id, eta, p, pred, ipred, sigma, y_raw, y, cens, dv,
//!                                         n_zero_sigma, n_negative, n_blq } ], n_zero_sigma, n_negative, n_blq }
//! { "op": "simulate", model, theta, random_effects, omega_shape, omega, error_model, xi,
//!   "design": { subjects | times+dose+m | table }, seed, replicates,
//!   "options": { negative: "keep"|"zero"|"reject", lloq, quantiles, threads } }
//!      -> { "subjects": [ {rep,id,eta,p} ], "observations": [ {rep,id,obs,time,pred,ipred,sigma,eps,y_raw,y,
//!           cens,dv} ], "counts": {..}, "summary": [ {id,obs,time,y:[..],ipred:[..],blq_fraction} ],
//!           "dataset": [ {subject,label,time,event,amt,dv,cens} ] }
//! { "op": "quantile", "x": [..], "p": [..] }                              -> { "q": [..] }
//! ```
//!
//! A refusal is a `SimError { code, message }` (serialised code `invalid_option`, `non_finite_value`, ...). A
//! request with `options.threads` must be answered identically for every value of `threads` (SIM-DET-01), and
//! the answer does not echo it.
//!
//! # Tolerances
//!
//! All from `caladrius_testkit::sim` (`TOL_Z` 1e-14, `TOL_EXACT` 1e-12, `TOL_STAT` 1e-9). Each was measured on a
//! correct double implementation against the 256-bit values and has a margin of more than 10 (14 for the
//! transform, more than 1000 for the exact part, about 10^4 for the statistics); the figures are in
//! `oracle/README.md`. Bit-exact asserts are on the known answers, the counters, the words, the uniforms, the
//! Cholesky factor and the central normals only.

use caladrius_testkit::sim::{self, list_sim_cases, load_sim, number};
use caladrius_testkit::sim_check as check;
use caladrius_testkit::sim_ref::{SimError, reference_simulation};
use serde_json::{Map, Value, json};

// ---------------------------------------------------------------- the adapter (the only thing T-159 changes)

/// THE ADAPTER. Today: the simulator is not built. T-159 replaces this body (and only this body) with the call
/// into the engine, building its input from `request` and returning the answer in the shape described in the
/// header of this file.
fn simulation(request: &Value) -> Result<Value, SimError> {
    let _ = request;
    Err(SimError::new("not_built", "simulation not built"))
}

/// The population fit, for the round trip (SIM-ORA-06). Today: not built (T-154). The request is the one of
/// `oracle_pop.rs` (`population_fit`), the answer carries `estimate`, `se` and `status`.
fn population_fit(request: &Value) -> Result<Value, SimError> {
    let _ = request;
    Err(SimError::new("not_built", "population fit not built"))
}

fn ok(result: Result<(), String>) {
    if let Err(e) = result {
        panic!("{e}");
    }
}

// ---------------------------------------------------------------- engine tests, from the generated list

macro_rules! philox_tests {
    ($($case:ident),* $(,)?) => {
        mod philox {
            use super::*;
            $( #[test] fn $case() { ok(check::check_philox(&simulation, stringify!($case))); } )*
        }
    };
}

macro_rules! stream_tests {
    ($($case:ident),* $(,)?) => {
        mod streams {
            use super::*;
            $( #[test] fn $case() { ok(check::check_stream(&simulation, stringify!($case))); } )*
        }
    };
}

macro_rules! exact_tests {
    ($($case:ident),* $(,)?) => {
        mod exact {
            use super::*;
            $( #[test] fn $case() { ok(check::check_exact(&simulation, stringify!($case))); } )*
        }
    };
}

macro_rules! study_tests {
    ($($case:ident),* $(,)?) => {
        mod studies {
            use super::*;
            $( #[test] fn $case() { ok(check::check_study(&simulation, stringify!($case))); } )*
        }
    };
}

macro_rules! quantile_tests {
    ($($case:ident),* $(,)?) => {
        mod quantiles {
            use super::*;
            $( #[test] fn $case() { ok(check::check_quantile(&simulation, stringify!($case))); } )*
        }
    };
}

macro_rules! invariance_tests {
    ($($case:ident),* $(,)?) => {
        mod invariance {
            use super::*;
            $( #[test] fn $case() { ok(check::check_invariance(&simulation, stringify!($case))); } )*
        }
    };
}

macro_rules! refusal_tests {
    ($($case:ident),* $(,)?) => {
        mod refusals {
            use super::*;
            $( #[test] fn $case() { ok(check::check_refused(&simulation, stringify!($case))); } )*
        }
    };
}

macro_rules! accepted_tests {
    ($($case:ident),* $(,)?) => {
        mod accepted {
            use super::*;
            $( #[test] fn $case() { ok(check::check_accepted(&simulation, stringify!($case))); } )*
        }
    };
}

macro_rules! roundtrip_tests {
    ($($case:ident),* $(,)?) => {
        mod roundtrip {
            use super::*;
            $(
                #[test]
                #[ignore = "waiting for T-154 (the population fit)"]
                fn $case() { ok(check_roundtrip(stringify!($case), &simulation, &population_fit)); }
            )*
        }
    };
}

/// The transform grid, the correlated draws and the statistics of SX3 are single tests.
mod transform {
    use super::*;
    #[test]
    fn grid() {
        ok(check::check_transform_grid(&simulation));
    }
}

mod distribution {
    use super::*;
    #[test]
    fn sx3_streams() {
        ok(check::check_sx3_streams(&simulation));
    }
    #[test]
    fn sx3_correlated_eta() {
        ok(check::check_sx3_correlated_eta(&simulation));
    }
}

// ---------------------------------------------------------------- simulate then fit (SIM-ORA-06)

/// The population-fit request of one replicate: the `dataset` rows of the replicate, the model of the case and a
/// start moved from the truth by the factors of the file (the same as `oracle_pop.rs`, written before any
/// engine code).
fn fit_request(case: &Value, rows: &[&Value]) -> Value {
    let fit = &case["fit"];
    let factors = &fit["start_factors"];
    let (ft, fo, fx) = (
        factors["theta"].as_f64().unwrap(),
        factors["omega"].as_f64().unwrap(),
        factors["xi"].as_f64().unwrap(),
    );
    let scaled = |v: &Value, factor: f64| -> f64 { number(v).unwrap() * factor };
    let initial: Map<String, Value> = fit["truth"]["typical_values"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), json!(scaled(v, ft))))
        .collect();
    let omega: Vec<Vec<f64>> = fit["truth"]["omega_full"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r.as_array()
                .unwrap()
                .iter()
                .map(|t| scaled(t, fo))
                .collect()
        })
        .collect();
    let xi = &fit["truth"]["xi"];
    let error_init: Vec<f64> = match fit["error_model"].as_str().unwrap() {
        "additive" => vec![scaled(&xi["a"], fx)],
        "proportional" => vec![scaled(&xi["b"], fx)],
        _ => vec![scaled(&xi["a"], fx), scaled(&xi["b"], fx)],
    };
    let mut subjects: Vec<Value> = Vec::new();
    let mut order: Vec<String> = Vec::new();
    let mut by_label: std::collections::BTreeMap<String, (f64, Vec<f64>, Vec<f64>)> =
        std::collections::BTreeMap::new();
    for r in rows {
        let label = r["label"].as_str().unwrap().to_owned();
        let entry = by_label.entry(label.clone()).or_insert_with(|| {
            order.push(label.clone());
            (0.0, Vec::new(), Vec::new())
        });
        if r["event"].as_str() == Some("dose") {
            entry.0 = number(&r["amt"]).unwrap();
        } else {
            entry.1.push(number(&r["time"]).unwrap());
            entry.2.push(number(&r["dv"]).unwrap());
        }
    }
    for label in &order {
        let (dose, time, y) = &by_label[label];
        subjects.push(json!({ "id": label, "dose": dose, "time": time, "y": y }));
    }
    json!({
        "model": fit["model"],
        "initial": initial,
        "random_effects": fit["random_effects"],
        "omega": omega,
        "omega_shape": "diagonal",
        "error_model": fit["error_model"],
        "error_init": error_init,
        "options": { "method": "foce_i", "ofv_convention": "full", "evaluate_only": false },
        "subjects": subjects,
    })
}

/// The rows of the dataset table of each replicate, in order of replicate (the prefix `k:` of `subject`).
fn replicate_rows(dataset: &[Value], replicates: usize) -> Vec<Vec<&Value>> {
    let mut out: Vec<Vec<&Value>> = vec![Vec::new(); replicates];
    for row in dataset {
        let subject = row["subject"].as_str().unwrap();
        let k: usize = subject.split(':').next().unwrap().parse().unwrap();
        out[k].push(row);
    }
    out
}

fn mean_sd(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let mean = x.iter().sum::<f64>() / n;
    let var = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0);
    (mean, var.sqrt())
}

/// Simulate the case's call, fit every replicate from the start, and assert the aggregates (bias and coverage on
/// each typical value, constants from the file) when the file says so. Per-replicate deviations are printed,
/// never asserted; a fit that did not converge is counted and left out of the aggregates.
fn check_roundtrip(
    name: &str,
    sim: &dyn Fn(&Value) -> Result<Value, SimError>,
    fit: &dyn Fn(&Value) -> Result<Value, SimError>,
) -> Result<(), String> {
    let case = load_sim("roundtrip", name).map_err(|e| e.to_string())?;
    let answer = sim(&case["simulate"])
        .map_err(|e| format!("{name}: the simulator refuses the call: {e}"))?;
    let dataset = answer["dataset"]
        .as_array()
        .ok_or("the answer has no dataset")?;
    let k = case["simulate"]["replicates"].as_u64().unwrap() as usize;
    let rows = replicate_rows(dataset, k);
    let pars: Vec<String> = case["fit"]["truth"]["typical_values"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let mut estimates: Vec<Vec<f64>> = vec![Vec::new(); pars.len()];
    let mut covered: Vec<usize> = vec![0; pars.len()];
    let c = &case["aggregate_checks"]["constants"];
    let (c_bias, c_interval, c_cov, nominal) = (
        c["c_bias"].as_f64().unwrap(),
        c["c_interval"].as_f64().unwrap(),
        c["c_coverage"].as_f64().unwrap(),
        c["nominal_coverage"].as_f64().unwrap(),
    );
    let mut not_converged = 0usize;
    for (rep, r) in rows.iter().enumerate() {
        let a = fit(&fit_request(&case, r))
            .map_err(|e| format!("{name} replicate {rep}: the fit refuses valid data: {e}"))?;
        if a["status"].as_str().is_some_and(|s| s != "converged") {
            not_converged += 1;
            continue;
        }
        for (i, par) in pars.iter().enumerate() {
            let est = a["estimate"][par.as_str()]
                .as_f64()
                .ok_or(format!("replicate {rep}: no estimate of {par}"))?;
            let se = a["se"][par.as_str()]
                .as_f64()
                .ok_or(format!("replicate {rep}: no se of {par}"))?;
            let truth = number(&case["fit"]["truth"]["typical_values"][par.as_str()]).unwrap();
            estimates[i].push(est);
            if (est - truth).abs() <= c_interval * se {
                covered[i] += 1;
            }
        }
    }
    let used = estimates[0].len();
    eprintln!("{name}: {used} of {k} replicates used, {not_converged} not converged");
    let mut problems = Vec::new();
    for (i, par) in pars.iter().enumerate() {
        let truth = number(&case["fit"]["truth"]["typical_values"][par.as_str()]).unwrap();
        let (mean, sd) = mean_sd(&estimates[i]);
        let n = used as f64;
        let bias_limit = c_bias * sd / n.sqrt();
        let share = covered[i] as f64 / n;
        let cov_limit = c_cov * (nominal * (1.0 - nominal) / n).sqrt();
        eprintln!(
            "{name} {par}: truth {truth:e} mean {mean:e} sd {sd:e} bias {:e} (limit {bias_limit:e}) coverage {share:.3} (nominal {nominal} +- {cov_limit:.3})",
            mean - truth
        );
        if (mean - truth).abs() > bias_limit {
            problems.push(format!(
                "{par}: mean {mean:e} differs from the truth {truth:e} by more than {bias_limit:e}"
            ));
        }
        if (share - nominal).abs() > cov_limit {
            problems.push(format!(
                "{par}: coverage {share:.3} is not {nominal} within {cov_limit:.3}"
            ));
        }
    }
    if case["aggregate_checks"]["asserted"].as_bool() == Some(true) {
        if problems.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{name}: aggregate checks failed\n  {}",
                problems.join("\n  ")
            ))
        }
    } else {
        eprintln!(
            "{name}: recorded only; {} aggregate lines outside the checks",
            problems.len()
        );
        Ok(())
    }
}

// ---------------------------------------------------------------- the generated list of cases

include!("oracle_sim.cases");

// ---------------------------------------------------------------- guards (green today)

fn strings(list: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = list.iter().map(|s| (*s).to_owned()).collect();
    v.sort();
    v
}

/// Guards the guard: every case file of the oracle is run by a test above, and every case named above exists.
#[test]
fn every_case_row_and_design_has_tests() {
    assert_eq!(
        check::kat_names().unwrap(),
        strings(SIM_KAT),
        "known answers and blocks"
    );
    assert_eq!(
        check::stream_names().unwrap(),
        strings(SIM_STREAMS),
        "streams"
    );
    assert_eq!(
        check::exact_names().unwrap(),
        strings(SIM_EXACT),
        "exact cases"
    );
    assert_eq!(check::study_names().unwrap(), strings(SIM_STUDY), "studies");
    assert_eq!(
        check::quantile_names().unwrap(),
        strings(SIM_QUANTILE),
        "quantile cases"
    );
    assert_eq!(
        check::invariance_names().unwrap(),
        strings(SIM_INVARIANCE),
        "invariance checks"
    );
    assert_eq!(
        check::error_row_names("refused", true).unwrap(),
        strings(SIM_REFUSALS),
        "refused rows"
    );
    assert_eq!(
        check::error_row_names("accepted", true).unwrap(),
        strings(SIM_ACCEPTED),
        "accepted rows"
    );
    assert_eq!(
        check::error_row_names("refused", false).unwrap(),
        strings(SIM_SENTENCE_ONLY),
        "sentence-only rows"
    );
    assert_eq!(
        list_sim_cases("roundtrip").unwrap(),
        strings(SIM_ROUNDTRIP),
        "round-trip designs"
    );
    // the sentence-only rows are the ones the file says cannot be built, and the others all have requests
    let rows = check::error_rows().unwrap();
    assert_eq!(
        rows.iter()
            .filter(|r| r["constructible"].as_bool() == Some(false))
            .count(),
        SIM_SENTENCE_ONLY.len()
    );
    assert!(
        rows.iter()
            .filter(|r| r["constructible"].as_bool() != Some(false))
            .all(|r| r["request"].is_object())
    );
}

/// The manifest hashes every file; the files on disk are those it lists.
#[test]
fn the_manifest_is_recomputed() {
    let manifest = sim::load_sim_top("manifest").unwrap();
    let files = manifest["files"].as_array().unwrap();
    assert!(files.len() >= 40, "{} files", files.len());
    for f in files {
        let path = f["path"].as_str().unwrap();
        let bytes =
            std::fs::read(sim::sim_dir().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(
            caladrius_testkit::pop::sha256_hex(&bytes),
            f["sha256"].as_str().unwrap(),
            "{path}"
        );
    }
}

/// The checks themselves are sound: they pass on the independent reference (the whole set is run by
/// `crates/caladrius-testkit/tests/sim_consistency.rs`; here the cheap ones, so a broken helper is caught in this
/// crate too), so that a red engine test is red because of the engine.
#[test]
fn the_checks_pass_on_the_reference() {
    for n in SIM_KAT {
        ok(check::check_philox(&reference_simulation, n));
    }
    for n in SIM_QUANTILE {
        ok(check::check_quantile(&reference_simulation, n));
    }
    for n in SIM_EXACT {
        ok(check::check_exact(&reference_simulation, n));
    }
    ok(check::check_transform_grid(&reference_simulation));
    for n in SIM_REFUSALS {
        ok(check::check_refused(&reference_simulation, n));
    }
    for n in SIM_ACCEPTED {
        ok(check::check_accepted(&reference_simulation, n));
    }
}

/// Today the engine adapter reports `not_built` and nothing else, which is why every engine test is red, and the
/// checks turn it into a failure (a refusal row whose request is refused with the wrong code would pass the
/// guard above and fail here).
#[test]
fn the_adapter_says_not_built() {
    let e = simulation(&json!({ "op": "quantile", "x": [1], "p": [0.5] })).unwrap_err();
    assert_eq!(e.code, "not_built");
    assert!(
        check::check_quantile(&simulation, SIM_QUANTILE[0])
            .unwrap_err()
            .contains("not_built")
    );
    assert!(
        check::check_exact(&simulation, SIM_EXACT[0])
            .unwrap_err()
            .contains("not_built")
    );
}

/// The round trip: a short simulation by the reference (3 replicates of the call) gives datasets whose fit
/// requests are well formed, so that a red round trip is red because of the fit.
#[test]
fn the_roundtrip_requests_are_well_formed() {
    for name in SIM_ROUNDTRIP {
        let mut case = load_sim("roundtrip", name).unwrap();
        case["simulate"]["replicates"] = json!(3);
        let answer = reference_simulation(&case["simulate"]).unwrap();
        let dataset = answer["dataset"].as_array().unwrap();
        let rows = replicate_rows(dataset, 3);
        assert_eq!(rows.len(), 3);
        for r in rows {
            let request = fit_request(&case, &r);
            let subjects = request["subjects"].as_array().unwrap();
            assert_eq!(
                subjects.len(),
                case["simulate"]["design"]["m"].as_u64().unwrap() as usize,
                "{name}"
            );
            for s in subjects {
                assert_eq!(s["dose"].as_f64(), Some(100.0), "{name}");
                assert_eq!(
                    s["time"].as_array().unwrap().len(),
                    s["y"].as_array().unwrap().len(),
                    "{name}"
                );
                assert_eq!(
                    s["time"].as_array().unwrap().len(),
                    case["simulate"]["design"]["times"]
                        .as_array()
                        .unwrap()
                        .len(),
                    "{name}"
                );
            }
            assert_eq!(request["initial"].as_object().unwrap().len(), 3, "{name}");
        }
        assert!(
            case["pending"].as_str().unwrap().contains("T-154"),
            "{name}"
        );
    }
}
