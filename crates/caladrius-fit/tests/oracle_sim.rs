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
//! Written before the engine existed; the engine (T-159) is now behind the one function below, `simulation`,
//! whose body is the only thing that card changed here. The guards at the end are green. The checks live in
//! `caladrius_testkit::sim_check` and are run against an independent reference (`caladrius_testkit::sim_ref`) by
//! `crates/caladrius-testkit/tests/sim_consistency.rs`, so a failing engine test fails because of the engine.
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
//!   "options": { negative: "keep"|"zero", lloq, quantiles, threads } }
//!      -> { "subjects": [ {rep,id,eta,p} ], "observations": [ {rep,id,obs,time,pred,ipred,sigma,eps,y_raw,y,
//!           cens,dv} ], "counts": {..}, "summary": [ {id,obs,time,y:[..],ipred:[..],blq_fraction} ],
//!           "dataset": [ {subject,label,time,event,amt,dv,cens} ] }
//! { "op": "quantile", "x": [..], "p": [..] }                              -> { "q": [..] }
//! ```
//!
//! A refusal is a `SimError { code, message }` (serialised code `invalid_option`, `non_finite_value`, ...). A
//! request with `options.threads` must be answered identically for every value of `threads` (SIM-DET-01), and
//! the answer does not echo it. `threads` is not an option of SIM-MOD-05 and SIM-DET-01 puts threads outside the
//! library: it is an adapter-level knob, and the T-159 adapter must strip it before the request reaches the engine.
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

/// THE ADAPTER. It builds the input of `caladrius_fit::sim` from `request`, calls it, and returns the answer in
/// the shape described in the header of this file. It strips `threads` (an adapter-level knob, not an option of
/// SIM-MOD-05) and refuses any other option key it does not know with `invalid_option`.
fn simulation(request: &Value) -> Result<Value, SimError> {
    use caladrius_fit::sim as eng;
    use std::collections::BTreeMap;

    fn invalid(message: impl Into<String>) -> SimError {
        SimError::new("invalid_option", message)
    }
    fn lift(e: eng::SimError) -> SimError {
        SimError::new(e.code.as_str(), e.message)
    }
    // A number that cannot be read becomes NaN, so that the engine's own check names the field.
    fn num(v: &Value) -> f64 {
        sim::number(v).unwrap_or(f64::NAN)
    }
    fn u32s<const N: usize>(v: &Value, what: &str) -> Result<[u32; N], SimError> {
        let list = v
            .as_array()
            .filter(|a| a.len() == N)
            .ok_or_else(|| invalid(format!("{what} takes {N} integers")))?;
        let mut out = [0u32; N];
        for (slot, x) in out.iter_mut().zip(list) {
            *slot = x
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| invalid(format!("{what} takes 32-bit integers")))?;
        }
        Ok(out)
    }
    // A label is a string: a missing or numeric one is refused, never read as the empty label.
    fn label_of(v: &Value, what: &str) -> Result<String, SimError> {
        v.as_str()
            .map(str::to_owned)
            .ok_or_else(|| invalid(format!("{what} must be a string")))
    }
    // The seed: a number below 2^53, or a decimal string. Absent or null is left to the engine (`missing_seed`).
    fn seed_of(v: Option<&Value>) -> Result<Option<eng::Seed>, SimError> {
        match v {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => eng::Seed::from_number(n.as_f64().unwrap_or(f64::NAN))
                .map(Some)
                .map_err(lift),
            Some(Value::String(s)) => eng::Seed::from_text(s).map(Some).map_err(lift),
            Some(other) => Err(invalid(format!("seed {other} is not an integer"))),
        }
    }
    fn design_of(design: Option<&Value>) -> Result<eng::Design, SimError> {
        let d = design.ok_or_else(|| invalid("design is missing"))?;
        if let Some(list) = d.get("subjects").and_then(Value::as_array) {
            let mut out = Vec::new();
            for s in list {
                out.push(eng::DesignSubject {
                    label: label_of(&s["id"], "id")?,
                    dose: num(&s["dose"]),
                    dose_time: 0.0,
                    times: sim::numbers(&s["time"])
                        .ok_or_else(|| invalid("time takes a list of numbers"))?,
                });
            }
            Ok(eng::Design::Subjects(out))
        } else if d.get("times").is_some() {
            Ok(eng::Design::Grid {
                times: sim::numbers(&d["times"])
                    .ok_or_else(|| invalid("times takes a list of numbers"))?,
                dose: num(&d["dose"]),
                m: d["m"]
                    .as_u64()
                    .filter(|m| *m >= 1)
                    .ok_or_else(|| invalid("m must be an integer from 1"))?,
            })
        } else if let Some(rows) = d.get("table").and_then(Value::as_array) {
            let mut out = Vec::new();
            for row in rows {
                out.push(eng::TableRow {
                    subject: label_of(&row["subject"], "subject")?,
                    time: num(&row["time"]),
                    kind: if row["event"].as_str() == Some("dose") {
                        eng::RowKind::Dose
                    } else {
                        eng::RowKind::Observation
                    },
                    amt: row.get("amt").filter(|a| !a.is_null()).map(num),
                    dv_given: row.get("dv").is_some_and(|a| !a.is_null()),
                });
            }
            Ok(eng::Design::Table(out))
        } else {
            Err(invalid(
                "design takes subjects, a table, or times with a dose and m",
            ))
        }
    }
    // Everything of a request but the design, the seed and the replicates (SIM-MOD-05).
    fn input_of(
        r: &Value,
        design: eng::Design,
        seed: Option<eng::Seed>,
        replicates: u64,
    ) -> Result<eng::SimInput, SimError> {
        let empty = Map::new();
        let options = r
            .get("options")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        for key in options.keys() {
            // `threads` is not an option of SIM-MOD-05: it is stripped here, before the engine (SIM-DET-01).
            if !["negative", "lloq", "quantiles", "threads"].contains(&key.as_str()) {
                return Err(invalid(format!("unknown option {key}")));
            }
        }
        let negative = match options.get("negative") {
            None | Some(Value::Null) => eng::NegativeRule::Keep,
            Some(Value::String(s)) => eng::NegativeRule::from_name(s).map_err(lift)?,
            Some(other) => {
                return Err(invalid(format!(
                    "option negative {other} is not known: use `keep` or `zero`"
                )));
            }
        };
        let lloq = match options.get("lloq") {
            None | Some(Value::Null) => None,
            Some(v) => Some(num(v)),
        };
        let quantiles = match options.get("quantiles") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                sim::numbers(v)
                    .ok_or_else(|| invalid("option quantiles takes a list of numbers"))?,
            ),
        };
        let xi = r.get("xi").and_then(Value::as_object).unwrap_or(&empty);
        let xi_value = |name: &str| xi.get(name).map(num);
        let random_effects = r["random_effects"]
            .as_array()
            .ok_or_else(|| invalid("random_effects is missing"))?
            .iter()
            .map(|x| {
                x.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("random_effects takes names"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let theta: BTreeMap<String, f64> = r["theta"]
            .as_object()
            .ok_or_else(|| invalid("theta is missing"))?
            .iter()
            .map(|(k, v)| (k.clone(), num(v)))
            .collect();
        Ok(eng::SimInput {
            model: r["model"]
                .as_str()
                .ok_or_else(|| invalid("model is missing"))?
                .to_owned(),
            theta,
            random_effects,
            omega_shape: eng::OmegaShape::from_name(
                r["omega_shape"]
                    .as_str()
                    .ok_or_else(|| invalid("omega_shape is missing: diagonal or block"))?,
            )
            .map_err(lift)?,
            omega: sim::numbers(&r["omega"])
                .ok_or_else(|| invalid("omega takes a list of numbers"))?,
            error_model: eng::ErrorKind::from_name(r["error_model"].as_str().ok_or_else(|| {
                invalid(
                    "error_model is missing: choose additive, proportional, combined or combined_sd",
                )
            })?)
            .map_err(lift)?,
            a: xi_value("a"),
            b: xi_value("b"),
            design,
            seed,
            replicates,
            negative,
            lloq,
            quantiles,
        })
    }

    match request.get("op").and_then(Value::as_str) {
        Some("philox") => {
            let counter = u32s::<4>(&request["counter"], "counter")?;
            let key = u32s::<2>(&request["key"], "key")?;
            Ok(json!({ "out": eng::philox4x32_10(counter, key) }))
        }
        Some("stream") => {
            let seed = seed_of(request.get("seed"))?
                .ok_or_else(|| SimError::new("missing_seed", "a stream needs a seed"))?;
            let replicate = request["replicate"]
                .as_u64()
                .and_then(|r| u32::try_from(r).ok())
                .filter(|r| *r < 1 << 24)
                .ok_or_else(|| invalid("replicate takes an integer below 16777216"))?;
            let (purpose_number, purpose) = match request["purpose"].as_u64() {
                Some(1) => (1u32, eng::Purpose::Eta),
                Some(2) => (2u32, eng::Purpose::Epsilon),
                _ => return Err(invalid("purpose is 1 (eta) or 2 (epsilon)")),
            };
            let label = request["label"].as_str().unwrap_or("");
            let key = eng::LabelKey::of(label);
            let stream = eng::Stream::new(seed, replicate, purpose, key);
            let indices: Vec<u64> = if let Some(list) = request.get("n").and_then(Value::as_array) {
                list.iter().filter_map(Value::as_u64).collect()
            } else {
                (0..request["count"].as_u64().unwrap_or(0)).collect()
            };
            let words = indices
                .iter()
                .map(|&n| {
                    stream
                        .word(n)
                        .ok_or_else(|| invalid(format!("word {n} is beyond the stream")))
                })
                .collect::<Result<Vec<u64>, _>>()?;
            let u: Vec<f64> = words.iter().map(|&w| eng::uniform(w)).collect();
            let z: Vec<f64> = u.iter().map(|&x| eng::as241(x)).collect();
            let mut answer = json!({
                "key": [seed.0 as u32, (seed.0 >> 32) as u32],
                "c1": (purpose_number << 24) | replicate,
                "label_hash": key.hex(),
                "u": u,
                "z": z,
            });
            if request.get("n").is_some() {
                answer["words"] = json!(
                    words
                        .iter()
                        .map(|w| format!("{w:016x}"))
                        .collect::<Vec<_>>()
                );
            }
            Ok(answer)
        }
        Some("transform") => {
            let u =
                sim::numbers(&request["u"]).ok_or_else(|| invalid("u takes a list of numbers"))?;
            Ok(json!({ "z": u.iter().map(|&x| eng::as241(x)).collect::<Vec<_>>() }))
        }
        Some("quantile") => {
            let x =
                sim::numbers(&request["x"]).ok_or_else(|| invalid("x takes a list of numbers"))?;
            let p =
                sim::numbers(&request["p"]).ok_or_else(|| invalid("p takes a list of numbers"))?;
            let q: Option<Vec<f64>> = p.iter().map(|&pp| eng::quantile7(&x, pp)).collect();
            Ok(json!({ "q": q.ok_or_else(|| invalid("quantiles"))? }))
        }
        Some("exact") => {
            let dose = num(&request["dose"]);
            let list = request["subjects"]
                .as_array()
                .ok_or_else(|| invalid("subjects"))?;
            let mut subjects = Vec::new();
            for s in list {
                subjects.push(eng::DesignSubject {
                    label: label_of(&s["id"], "id")?,
                    dose,
                    dose_time: 0.0,
                    times: sim::numbers(&s["time"]).ok_or_else(|| invalid("time"))?,
                });
            }
            let input = input_of(
                request,
                eng::Design::Subjects(subjects),
                Some(eng::Seed(0)),
                1,
            )?;
            let study = eng::Study::for_draws(&input).map_err(lift)?;
            let (mut nz, mut nn, mut nb) = (0, 0, 0);
            let mut out = Vec::new();
            for (i, s) in list.iter().enumerate() {
                let z_eta = sim::numbers(&s["z_eta"]).ok_or_else(|| invalid("z_eta"))?;
                let z_eps = sim::numbers(&s["z_eps"]).ok_or_else(|| invalid("z_eps"))?;
                let cell = study.cell_with_draws(0, i, &z_eta, &z_eps).map_err(lift)?;
                nz += cell.n_zero_sigma;
                nn += cell.n_negative;
                nb += cell.n_blq;
                out.push(json!({
                    "id": s["id"],
                    "eta": cell.eta,
                    "p": cell.p,
                    "pred": study.pred(i),
                    "ipred": cell.ipred,
                    "sigma": cell.sigma,
                    "y_raw": cell.y_raw,
                    "y": cell.y,
                    "cens": cell.cens,
                    "dv": cell.dv,
                    "n_zero_sigma": cell.n_zero_sigma,
                    "n_negative": cell.n_negative,
                    "n_blq": cell.n_blq,
                }));
            }
            Ok(json!({
                "l": study.factor_rows(),
                "subjects": out,
                "n_zero_sigma": nz,
                "n_negative": nn,
                "n_blq": nb,
            }))
        }
        Some("simulate") => {
            let replicates = match request.get("replicates") {
                None | Some(Value::Null) => 1,
                Some(v) => {
                    // Only what a u64 cannot hold is refused here; 0 and the rest are the engine's to judge.
                    let x = sim::number(v).filter(|x| {
                        x.fract() == 0.0 && *x >= 0.0 && *x < 18_446_744_073_709_551_616.0
                    });
                    x.ok_or_else(|| invalid(format!("replicates {v} must be an integer from 1")))?
                        as u64
                }
            };
            let seed = seed_of(request.get("seed"))?;
            let design = design_of(request.get("design"))?;
            let input = input_of(request, design, seed, replicates)?;
            let result = eng::simulate(&input).map_err(lift)?;
            serde_json::to_value(&result).map_err(|e| invalid(e.to_string()))
        }
        other => Err(invalid(format!("op {other:?} is not one of the oracle's"))),
    }
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
