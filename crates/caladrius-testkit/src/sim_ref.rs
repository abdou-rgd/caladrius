//! A reference implementation of the adapter of `crates/caladrius-fit/tests/oracle_sim.rs`, written from
//! `specs/simulation.md` in double precision with no engine code (task T-158).
//!
//! Its purpose is to test the tests: every check of the engine tests (`crate::sim_check`) is run here against
//! this implementation, so that a test that fails on the engine fails because of the engine and not because the
//! expected value, the request or the comparison is wrong. It is slow (no threads, a vector of JSON values per
//! table) and is not a candidate for the engine. The engine card (T-159) replaces the body of the adapter of
//! `oracle_sim.rs`; it does not use this module.
//!
//! The adapter is one function from a request to an answer. The request has an `op`:
//!
//! - `philox`: `counter` (4 integers), `key` (2 integers) give `out` (4 integers);
//! - `stream`: `seed`, `replicate`, `purpose`, `label` and either `n` (a list of word indices) or `count`
//!   (the first `count` words) give `key`, `c1`, `label_hash` (16 hexadecimal digits), `u`, `z`, and `words`
//!   (16 hexadecimal digits each) when `n` is given;
//! - `transform`: `u` (a list) gives `z`;
//! - `exact`: the exact part with the draws given (see `crate::sim_check::exact_request`) gives `l`, `subjects`
//!   and the counts;
//! - `simulate`: a whole simulation (see `crate::sim_check`) gives `subjects`, `observations`, `counts`,
//!   `summary` and `dataset`;
//! - `quantile`: `x` and `p` give `q` (type 7).
//!
//! Numbers in a request are JSON numbers, decimal strings or hexadecimal-float strings ([`crate::sim::number`]).

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value, json};

use crate::sim::{
    ExactInput, as241, cholesky, label_key, number, numbers, omega_matrix, philox4x32_10,
    quantile7, replay_subject, seed_key, stream_word, word_uniform,
};

/// A refusal of the adapter: a code (the serialised name of the error) and the message.
#[derive(Debug, Clone, PartialEq)]
pub struct SimError {
    pub code: String,
    pub message: String,
}

impl SimError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        SimError {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

fn invalid(message: impl Into<String>) -> SimError {
    SimError::new("invalid_option", message)
}

/// The adapter of the reference implementation.
pub fn reference_simulation(request: &Value) -> Result<Value, SimError> {
    match request.get("op").and_then(Value::as_str) {
        Some("philox") => op_philox(request),
        Some("stream") => op_stream(request),
        Some("transform") => op_transform(request),
        Some("exact") => op_exact(request),
        Some("simulate") => op_simulate(request),
        Some("quantile") => op_quantile(request),
        other => Err(invalid(format!("op {other:?} is not one of the oracle's"))),
    }
}

fn u32_list<const N: usize>(v: &Value, what: &str) -> Result<[u32; N], SimError> {
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

fn op_philox(r: &Value) -> Result<Value, SimError> {
    let counter = u32_list::<4>(&r["counter"], "counter")?;
    let key = u32_list::<2>(&r["key"], "key")?;
    Ok(json!({ "out": philox4x32_10(counter, key) }))
}

fn op_stream(r: &Value) -> Result<Value, SimError> {
    let seed = parse_seed(r.get("seed"))?;
    let key = [seed as u32, (seed >> 32) as u32];
    let rep = r["replicate"].as_u64().unwrap_or(0) as u32;
    let purpose = r["purpose"].as_u64().unwrap_or(1) as u32;
    let label = r["label"].as_str().unwrap_or("");
    let (lkey, hash) = label_key(label);
    let indices: Vec<u64> = if let Some(list) = r.get("n").and_then(Value::as_array) {
        list.iter().filter_map(Value::as_u64).collect()
    } else {
        (0..r["count"].as_u64().unwrap_or(0)).collect()
    };
    let words: Vec<u64> = indices
        .iter()
        .map(|&n| stream_word(key, rep, purpose, lkey, n))
        .collect();
    let u: Vec<f64> = words.iter().map(|&w| word_uniform(w)).collect();
    let z: Vec<f64> = u.iter().map(|&x| as241(x)).collect();
    let mut answer = json!({
        "key": key,
        "c1": (purpose << 24) | rep,
        "label_hash": hash,
        "u": u,
        "z": z,
    });
    if r.get("n").is_some() {
        answer["words"] = json!(
            words
                .iter()
                .map(|w| format!("{w:016x}"))
                .collect::<Vec<_>>()
        );
    }
    Ok(answer)
}

fn op_transform(r: &Value) -> Result<Value, SimError> {
    let u = numbers(&r["u"]).ok_or_else(|| invalid("u takes a list of numbers"))?;
    Ok(json!({ "z": u.iter().map(|&x| as241(x)).collect::<Vec<_>>() }))
}

fn op_quantile(r: &Value) -> Result<Value, SimError> {
    let x = numbers(&r["x"]).ok_or_else(|| invalid("x takes a list of numbers"))?;
    let p = numbers(&r["p"]).ok_or_else(|| invalid("p takes a list of numbers"))?;
    let q: Option<Vec<f64>> = p.iter().map(|&pp| quantile7(&x, pp)).collect();
    Ok(json!({ "q": q.ok_or_else(|| invalid("quantiles"))? }))
}

fn op_exact(r: &Value) -> Result<Value, SimError> {
    let input = ExactInput::from_value(r).map_err(invalid)?;
    let l = input.factor().map_err(|m| {
        SimError::new(
            "omega_not_positive_definite",
            format!("omega is not positive definite: {m}"),
        )
    })?;
    let subjects = r["subjects"]
        .as_array()
        .ok_or_else(|| invalid("subjects"))?;
    let mut out = Vec::new();
    let (mut nz, mut nn, mut nb) = (0, 0, 0);
    for s in subjects {
        let times = numbers(&s["time"]).ok_or_else(|| invalid("time"))?;
        let z_eta = numbers(&s["z_eta"]).ok_or_else(|| invalid("z_eta"))?;
        let z_eps = numbers(&s["z_eps"]).ok_or_else(|| invalid("z_eps"))?;
        let rep = replay_subject(&input, &l, &times, &z_eta, &z_eps).map_err(invalid)?;
        nz += rep.n_zero_sigma;
        nn += rep.n_negative;
        nb += rep.n_blq;
        out.push(json!({
            "id": s["id"],
            "eta": rep.eta,
            "p": rep.p,
            "pred": rep.pred,
            "ipred": rep.ipred,
            "sigma": rep.sigma,
            "y_raw": rep.y_raw,
            "y": rep.y,
            "cens": rep.cens,
            "dv": rep.dv,
            "n_zero_sigma": rep.n_zero_sigma,
            "n_negative": rep.n_negative,
            "n_blq": rep.n_blq,
        }));
    }
    Ok(json!({
        "l": l,
        "subjects": out,
        "n_zero_sigma": nz,
        "n_negative": nn,
        "n_blq": nb,
    }))
}

// ---------------------------------------------------------------- the whole simulation

fn parse_seed(v: Option<&Value>) -> Result<u64, SimError> {
    let missing = || {
        SimError::new(
            "missing_seed",
            "a simulation needs a seed: give an integer from 0 to 18446744073709551615 (a seed above \
             9007199254740991 must be written as a decimal string)",
        )
    };
    match v {
        None | Some(Value::Null) => Err(missing()),
        Some(Value::Number(n)) => {
            if let Some(u) = n.as_u64() {
                if u > 9_007_199_254_740_991 {
                    Err(invalid(format!(
                        "seed {u} is above 9007199254740991: write it as a decimal string"
                    )))
                } else {
                    Ok(u)
                }
            } else if n.as_i64().is_some() {
                Err(invalid(format!(
                    "seed {n} is negative: give an integer from 0"
                )))
            } else {
                Err(invalid(format!("seed {n} is not an integer")))
            }
        }
        Some(Value::String(s)) => {
            if seed_key(s).is_some() {
                Ok(s.parse::<u64>().unwrap_or(0))
            } else {
                Err(invalid(format!(
                    "seed \"{s}\" is not a decimal integer from 0 to 18446744073709551615"
                )))
            }
        }
        Some(other) => Err(invalid(format!("seed {other} is not an integer"))),
    }
}

const MODELS: [(&str, &[&str]); 5] = [
    ("pk1.iv_bolus", &["cl", "v"]),
    ("pk1.iv_infusion", &["cl", "v", "dur"]),
    ("pk1.oral_1", &["cl", "v", "ka"]),
    ("pk1.oral_1_lag", &["cl", "v", "ka", "tlag"]),
    ("pk2.iv_bolus", &["cl", "vc", "q", "vp"]),
];

struct Subject {
    label: String,
    dose: f64,
    dose_time: f64,
    times: Vec<f64>,
}

struct Run {
    input: ExactInput,
    seed: u64,
    reps: u64,
    quantiles: Vec<f64>,
    subjects: Vec<Subject>,
    n_dv_ignored: usize,
}

fn num_field(v: &Value, what: &str) -> Result<f64, SimError> {
    number(v)
        .filter(|x| x.is_finite())
        .ok_or_else(|| invalid(format!("{what} is not a finite number")))
}

fn parse_run(r: &Value) -> Result<Run, SimError> {
    let seed = parse_seed(r.get("seed"))?;
    // the model
    let model = r["model"]
        .as_str()
        .ok_or_else(|| invalid("model is missing"))?;
    let params: &[&str] = match MODELS.iter().find(|(m, _)| *m == model) {
        Some((_, p)) => p,
        None if model.starts_with("pk3.") => {
            return Err(invalid(format!(
                "model not available in this version: {model}"
            )));
        }
        None => {
            return Err(invalid(format!(
                "model {model} is not a model of this version"
            )));
        }
    };
    // options
    let empty = Map::new();
    let options = r
        .get("options")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    for key in options.keys() {
        if !["negative", "lloq", "quantiles", "threads"].contains(&key.as_str()) {
            return Err(invalid(format!("unknown option {key}")));
        }
    }
    let negative_zero = match options.get("negative") {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) if s == "keep" => false,
        Some(Value::String(s)) if s == "zero" => true,
        Some(other) => {
            return Err(invalid(format!(
                "option negative {other} is not known: use `keep` or `zero`"
            )));
        }
    };
    let lloq = match options.get("lloq") {
        None | Some(Value::Null) => None,
        Some(v) => match number(v) {
            Some(x) if x.is_finite() && x > 0.0 => Some(x),
            _ => {
                return Err(invalid(format!(
                    "option lloq {v} must be a finite number > 0"
                )));
            }
        },
    };
    let quantiles = match options.get("quantiles") {
        None | Some(Value::Null) => vec![0.05, 0.5, 0.95],
        Some(v) => {
            let q =
                numbers(v).ok_or_else(|| invalid("option quantiles takes a list of numbers"))?;
            if q.iter().any(|p| !(0.0..=1.0).contains(p)) {
                return Err(invalid("option quantiles takes probabilities from 0 to 1"));
            }
            q
        }
    };
    // replicates
    let reps = match r.get("replicates") {
        None | Some(Value::Null) => 1,
        Some(v) => {
            let x = number(v).filter(|x| x.fract() == 0.0 && *x >= 1.0);
            let x =
                x.ok_or_else(|| invalid(format!("replicates {v} must be an integer from 1")))?;
            if x > 16_777_216.0 {
                return Err(SimError::new(
                    "size_limit",
                    format!(
                        "replicates {x} exceed 16777216, the replicate field holds 16777216 values"
                    ),
                ));
            }
            x as u64
        }
    };
    // the error model
    let error_model = r["error_model"].as_str().ok_or_else(|| {
        invalid("error_model is missing: choose additive, proportional, combined or combined_sd")
    })?;
    let needed: &[&str] = match error_model {
        "additive" => &["a"],
        "proportional" => &["b"],
        "combined" | "combined_sd" => &["a", "b"],
        other => return Err(invalid(format!("error_model {other} is not known"))),
    };
    let xi_obj = r.get("xi").and_then(Value::as_object).unwrap_or(&empty);
    let mut xi = BTreeMap::new();
    for name in needed {
        let v = xi_obj.get(*name).ok_or_else(|| {
            invalid(format!(
                "error parameter {name} is missing for {error_model}"
            ))
        })?;
        let x = number(v)
            .filter(|x| x.is_finite() && *x >= 0.0)
            .ok_or_else(|| {
                invalid(format!(
                    "error parameter {name} must be finite and not negative"
                ))
            })?;
        xi.insert((*name).to_owned(), x);
    }
    // random effects and theta
    let re: Vec<String> = r["random_effects"]
        .as_array()
        .ok_or_else(|| invalid("random_effects is missing"))?
        .iter()
        .filter_map(|x| x.as_str().map(str::to_owned))
        .collect();
    if re.is_empty() {
        return Err(invalid(
            "random_effects is empty: name at least one parameter",
        ));
    }
    for (i, name) in re.iter().enumerate() {
        if re[..i].contains(name) {
            return Err(invalid(format!("random_effects names {name} twice")));
        }
        if name == "tlag" || name == "dur" {
            return Err(invalid(format!(
                "random_effects cannot name {name}: a lag time or a duration has no random effect in this version"
            )));
        }
        if !params.contains(&name.as_str()) {
            return Err(invalid(format!(
                "random_effects names {name}, which {model} does not have"
            )));
        }
    }
    let theta_obj = r["theta"]
        .as_object()
        .ok_or_else(|| invalid("theta is missing"))?;
    let mut theta = BTreeMap::new();
    for key in theta_obj.keys() {
        if !params.contains(&key.as_str()) {
            return Err(invalid(format!(
                "theta has {key}, which {model} does not have"
            )));
        }
    }
    for name in params {
        let v = theta_obj
            .get(*name)
            .ok_or_else(|| invalid(format!("theta is missing {name}")))?;
        let x = num_field(v, &format!("theta {name}")).map_err(|e| invalid(e.message))?;
        if re.iter().any(|n| n == name) && x <= 0.0 {
            return Err(invalid(format!(
                "theta {name} must be > 0 because it has a random effect"
            )));
        }
        theta.insert((*name).to_owned(), x);
    }
    // omega
    let shape = r["omega_shape"]
        .as_str()
        .ok_or_else(|| invalid("omega_shape is missing: diagonal or block"))?;
    let entries = numbers(&r["omega"]).ok_or_else(|| invalid("omega takes a list of numbers"))?;
    let q = re.len();
    let expected_count = if shape == "block" { q * (q + 1) / 2 } else { q };
    if entries.len() != expected_count {
        return Err(invalid(format!(
            "omega has {} numbers; a {shape} omega of {q} random effects needs {expected_count}",
            entries.len()
        )));
    }
    if entries.iter().any(|x| !x.is_finite()) {
        return Err(invalid("omega has a number that is not finite"));
    }
    let matrix = omega_matrix(shape, &entries, q).map_err(invalid)?;
    for (i, name) in re.iter().enumerate() {
        if matrix[i][i] < 0.0 {
            return Err(invalid(format!(
                "omega: the variance of random effect {name} is negative"
            )));
        }
    }
    if let Err((i, s)) = cholesky(&matrix) {
        return Err(SimError::new(
            "omega_not_positive_definite",
            format!(
                "omega is not positive definite: the Cholesky pivot of random effect {} is {s} (it must be \
                 positive); remove the random effect or correct the variances and covariances (a random \
                 effect without variance is the same as no random effect)",
                re[i]
            ),
        ));
    }
    let (subjects, n_dv_ignored) = parse_design(r.get("design"))?;
    let mut seen: BTreeMap<[u32; 2], &str> = BTreeMap::new();
    for s in &subjects {
        let (key, _) = label_key(&s.label);
        if let Some(other) = seen.insert(key, &s.label) {
            if other != s.label {
                return Err(SimError::new(
                    "label_key_collision",
                    format!(
                        "subjects {other} and {} have the same 64-bit stream key; rename one",
                        s.label
                    ),
                ));
            }
        }
    }
    Ok(Run {
        input: ExactInput {
            model: model.to_owned(),
            theta,
            random_effects: re,
            omega_shape: shape.to_owned(),
            omega: entries,
            error_model: error_model.to_owned(),
            xi,
            dose: 0.0,
            negative_zero,
            lloq,
        },
        seed,
        reps,
        quantiles,
        subjects,
        n_dv_ignored,
    })
}

fn check_times(label: &str, times: &[f64]) -> Result<(), SimError> {
    if times.is_empty() {
        return Err(SimError::new(
            "subject_without_observation",
            format!("subject {label} has no observation row: nothing would be simulated for it"),
        ));
    }
    for w in times.windows(2) {
        if w[1] < w[0] {
            return Err(SimError::new(
                "time_not_increasing",
                format!(
                    "time goes backwards in subject {label}: {} after {}",
                    w[1], w[0]
                ),
            ));
        }
        if w[1] == w[0] {
            return Err(SimError::new(
                "duplicate_times",
                format!("subject {label} has two observations at time {}", w[0]),
            ));
        }
    }
    Ok(())
}

fn parse_design(design: Option<&Value>) -> Result<(Vec<Subject>, usize), SimError> {
    let d = design.ok_or_else(|| invalid("design is missing"))?;
    let mut ignored = 0;
    let subjects: Vec<Subject> = if let Some(list) = d.get("subjects").and_then(Value::as_array) {
        let mut out = Vec::new();
        for s in list {
            let label = s["id"].as_str().unwrap_or_default().to_owned();
            let dose = num_field(&s["dose"], &format!("the dose of subject {label}"))?;
            let times =
                numbers(&s["time"]).ok_or_else(|| invalid("time takes a list of numbers"))?;
            out.push(Subject {
                label,
                dose,
                dose_time: 0.0,
                times,
            });
        }
        out
    } else if d.get("times").is_some() {
        let times = numbers(&d["times"]).ok_or_else(|| invalid("times takes a list of numbers"))?;
        let dose = num_field(&d["dose"], "the dose")?;
        let m = d["m"]
            .as_u64()
            .filter(|m| *m >= 1)
            .ok_or_else(|| invalid("m must be an integer from 1"))?;
        (1..=m)
            .map(|i| Subject {
                label: i.to_string(),
                dose,
                dose_time: 0.0,
                times: times.clone(),
            })
            .collect()
    } else if let Some(rows) = d.get("table").and_then(Value::as_array) {
        let mut order: Vec<String> = Vec::new();
        let mut by: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
        for row in rows {
            let id = row["subject"].as_str().unwrap_or_default().to_owned();
            if !by.contains_key(&id) {
                order.push(id.clone());
            }
            by.entry(id).or_default().push(row);
        }
        let mut out = Vec::new();
        for id in order {
            let mut dose = None;
            let mut times = Vec::new();
            for row in &by[&id] {
                let has_amt = row.get("amt").is_some_and(|a| !a.is_null());
                let time = num_field(&row["time"], &format!("a time in subject {id}"))?;
                if row["event"].as_str() == Some("dose") {
                    if dose.is_some() {
                        return Err(SimError::new(
                            "more_than_one_dose",
                            format!(
                                "subject {id} has more than one dose row: multiple dosing is not simulated in this version"
                            ),
                        ));
                    }
                    if !has_amt {
                        return Err(SimError::new(
                            "dose_without_amount",
                            format!("the dose row of subject {id} has no amount"),
                        ));
                    }
                    dose = Some((num_field(&row["amt"], "amt")?, time));
                } else {
                    if has_amt {
                        return Err(SimError::new(
                            "amount_on_observation",
                            format!("a row of subject {id} is an observation but has an amount"),
                        ));
                    }
                    if row.get("dv").is_some_and(|a| !a.is_null()) {
                        ignored += 1;
                    }
                    times.push(time);
                }
            }
            let (dose, dose_time) =
                dose.ok_or_else(|| invalid(format!("subject {id} has no dose row")))?;
            out.push(Subject {
                label: id,
                dose,
                dose_time,
                times,
            });
        }
        out
    } else {
        return Err(invalid(
            "design takes subjects, a table, or times with a dose and m",
        ));
    };
    if subjects.is_empty() {
        return Err(invalid("the design has no subject"));
    }
    for s in &subjects {
        check_times(&s.label, &s.times)?;
    }
    Ok((subjects, ignored))
}

fn finite_or_refuse(x: f64, what: &str, label: &str, rep: u64) -> Result<(), SimError> {
    if x.is_finite() {
        Ok(())
    } else {
        Err(SimError::new(
            "non_finite_value",
            format!("in subject {label}, replicate {rep}, with {what} = {x}: no table receives it"),
        ))
    }
}

/// One cell: a replicate and a subject.
struct Cell {
    eta: Vec<f64>,
    p: BTreeMap<String, f64>,
    obs: Vec<Value>,
    y: Vec<f64>,
    ipred: Vec<f64>,
    cens: Vec<u8>,
    n_zero_sigma: usize,
    n_negative: usize,
}

fn op_simulate(r: &Value) -> Result<Value, SimError> {
    let run = parse_run(r)?;
    let l = run.input.factor().map_err(invalid)?;
    let key = [run.seed as u32, (run.seed >> 32) as u32];
    let q = run.input.random_effects.len();
    let mut cells: Vec<Vec<Cell>> = Vec::new(); // [rep][subject]
    for rep in 0..run.reps {
        let mut row = Vec::new();
        for s in &run.subjects {
            let (lkey, _) = label_key(&s.label);
            let draw = |purpose: u32, n: usize| -> Vec<f64> {
                (0..n as u64)
                    .map(|i| as241(word_uniform(stream_word(key, rep as u32, purpose, lkey, i))))
                    .collect()
            };
            let z_eta = draw(1, q);
            let z_eps = draw(2, s.times.len());
            let mut input = run.input.clone();
            input.dose = s.dose;
            let t: Vec<f64> = s.times.iter().map(|t| t - s.dose_time).collect();
            let rp = replay_subject(&input, &l, &t, &z_eta, &z_eps).map_err(invalid)?;
            for (name, v) in &rp.p {
                finite_or_refuse(*v, name, &s.label, rep)?;
            }
            for (what, list) in [("ipred", &rp.ipred), ("sigma", &rp.sigma), ("y", &rp.y_raw)] {
                for v in list {
                    finite_or_refuse(*v, what, &s.label, rep)?;
                }
            }
            let obs: Vec<Value> = (0..s.times.len())
                .map(|j| {
                    json!({
                        "rep": rep, "id": s.label, "obs": j + 1, "time": s.times[j],
                        "pred": rp.pred[j], "ipred": rp.ipred[j], "sigma": rp.sigma[j],
                        "eps": z_eps[j], "y_raw": rp.y_raw[j], "y": rp.y[j],
                        "cens": rp.cens[j], "dv": rp.dv[j],
                    })
                })
                .collect();
            row.push(Cell {
                eta: rp.eta,
                p: rp.p,
                obs,
                y: rp.y,
                ipred: rp.ipred,
                cens: rp.cens,
                n_zero_sigma: rp.n_zero_sigma,
                n_negative: rp.n_negative,
            });
        }
        cells.push(row);
    }
    let mut subjects = Vec::new();
    let mut observations = Vec::new();
    let mut dataset = Vec::new();
    let (mut nz, mut nn, mut nb, mut rows) = (0, 0, 0, 0);
    let mut neg_by: BTreeMap<String, usize> = BTreeMap::new();
    let mut blq_by: BTreeMap<String, usize> = BTreeMap::new();
    for (rep, row) in cells.iter().enumerate() {
        for (s, cell) in run.subjects.iter().zip(row) {
            subjects.push(json!({ "rep": rep, "id": s.label, "eta": cell.eta, "p": cell.p }));
            let subject_col = format!("{rep}:{}", s.label);
            dataset.push(json!({
                "subject": subject_col, "label": s.label, "time": s.dose_time,
                "event": "dose", "amt": s.dose, "dv": null, "cens": null,
            }));
            for o in &cell.obs {
                dataset.push(json!({
                    "subject": subject_col, "label": s.label, "time": o["time"],
                    "event": "obs", "amt": null, "dv": o["dv"], "cens": o["cens"],
                }));
                observations.push(o.clone());
            }
            nz += cell.n_zero_sigma;
            nn += cell.n_negative;
            let blq = cell.cens.iter().filter(|c| **c == 1).count();
            nb += blq;
            rows += cell.obs.len();
            *neg_by.entry(s.label.clone()).or_default() += cell.n_negative;
            *blq_by.entry(s.label.clone()).or_default() += blq;
        }
    }
    let mut summary = Vec::new();
    for (si, s) in run.subjects.iter().enumerate() {
        for j in 0..s.times.len() {
            let ys: Vec<f64> = cells.iter().map(|row| row[si].y[j]).collect();
            let ips: Vec<f64> = cells.iter().map(|row| row[si].ipred[j]).collect();
            let blq = cells.iter().filter(|row| row[si].cens[j] == 1).count();
            let qs = |x: &[f64]| -> Vec<f64> {
                run.quantiles
                    .iter()
                    .filter_map(|&p| quantile7(x, p))
                    .collect()
            };
            summary.push(json!({
                "id": s.label, "obs": j + 1, "time": s.times[j],
                "y": qs(&ys), "ipred": qs(&ips),
                "blq_fraction": blq as f64 / run.reps as f64,
            }));
        }
    }
    Ok(json!({
        "model": run.input.model,
        "seed": run.seed.to_string(),
        "subjects": subjects,
        "observations": observations,
        "counts": {
            "subjects": run.subjects.len(), "replicates": run.reps, "rows": rows,
            "n_dv_ignored": run.n_dv_ignored, "n_zero_sigma": nz, "n_negative": nn, "n_blq": nb,
            "n_negative_by_subject": neg_by, "n_blq_by_subject": blq_by,
        },
        "summary": summary,
        "dataset": dataset,
    }))
}
