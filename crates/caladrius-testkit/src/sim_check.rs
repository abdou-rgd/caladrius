//! The requests and the checks of the population-simulation oracle (task T-158), shared by the engine tests
//! (`crates/caladrius-fit/tests/oracle_sim.rs`) and by the consistency test of this crate, which runs every
//! check against the reference implementation (`crate::sim_ref`). A check that passes on the reference and
//! fails on the engine fails because of the engine.
//!
//! Every check takes the adapter, a function from a request to an answer, builds its requests from the case
//! files of `oracle/expected/sim/`, and returns `Err(text)` listing every value outside its tolerance (never
//! only the first). The tolerances are `crate::sim::{TOL_Z, TOL_EXACT, TOL_STAT}`; the forms are those of
//! `oracle/README.md` (section "Population simulation").

use std::fmt::Debug;

use serde_json::{Map, Value, json};

use crate::oracle::OracleError;
use crate::sim::{
    ExactInput, TOL_EXACT, TOL_STAT, TOL_Z, chisq_bins, correlation, list_sim_cases, load_sim,
    load_sim_top, moments, number, numbers, parse_hex_float, within,
};
use crate::sim_ref::SimError;

/// The adapter: a request in, an answer or a refusal out. The engine card supplies its own.
pub type Adapter<'a> = &'a dyn Fn(&Value) -> Result<Value, SimError>;

fn load_err(e: OracleError) -> String {
    e.to_string()
}

fn call(adapter: Adapter, what: &str, request: &Value) -> Result<Value, String> {
    adapter(request).map_err(|e| format!("{what}: the engine refuses a valid request: {e}"))
}

/// Collects the values outside tolerance, one line each.
pub struct Problems {
    lines: Vec<String>,
    pub compared: usize,
}

impl Problems {
    pub fn new() -> Self {
        Problems {
            lines: Vec::new(),
            compared: 0,
        }
    }
    pub fn fail(&mut self, line: String) {
        self.compared += 1;
        self.lines.push(line);
    }
    pub fn eq<T: PartialEq + Debug>(&mut self, label: &str, actual: T, expected: T) {
        self.compared += 1;
        if actual != expected {
            self.lines
                .push(format!("{label}: expected {expected:?}, got {actual:?}"));
        }
    }
    /// `|a - e| <= tol max(|e|, scale)`; scale 0 asks for a plain relative error (zero needs zero).
    pub fn close(&mut self, label: &str, actual: Option<f64>, expected: f64, scale: f64, tol: f64) {
        self.compared += 1;
        match actual {
            Some(a) if within(a, expected, scale, tol) => {}
            Some(a) => self.lines.push(format!(
                "{label}: expected {expected:e}, got {a:e} (error {:e}, bound {:e})",
                (a - expected).abs(),
                tol * expected.abs().max(scale)
            )),
            None => self
                .lines
                .push(format!("{label}: the answer has no number here")),
        }
    }
    /// The values must be equal as doubles.
    pub fn same(&mut self, label: &str, actual: Option<f64>, expected: f64) {
        self.compared += 1;
        match actual {
            Some(a) if a == expected => {}
            Some(a) => self.lines.push(format!(
                "{label}: expected {expected:e} ({:016x}), got {a:e} ({:016x})",
                expected.to_bits(),
                a.to_bits()
            )),
            None => self
                .lines
                .push(format!("{label}: the answer has no number here")),
        }
    }
    pub fn finish(self, case: &str) -> Result<(), String> {
        if self.compared == 0 {
            return Err(format!("{case}: nothing was compared"));
        }
        if self.lines.is_empty() {
            return Ok(());
        }
        let shown: Vec<&String> = self.lines.iter().take(30).collect();
        Err(format!(
            "{case}: {} of {} values outside tolerance\n  {}{}",
            self.lines.len(),
            self.compared,
            shown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n  "),
            if self.lines.len() > shown.len() {
                "\n  ..."
            } else {
                ""
            }
        ))
    }
}

impl Default for Problems {
    fn default() -> Self {
        Self::new()
    }
}

fn num(v: &Value) -> Option<f64> {
    number(v)
}

fn hex_u32(v: &Value) -> Option<u32> {
    u32::from_str_radix(v.as_str()?, 16).ok()
}

fn hex_u32_list(v: &Value) -> Option<Vec<u32>> {
    v.as_array()?.iter().map(hex_u32).collect()
}

fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

// ---------------------------------------------------------------- names

/// `kat_0..kat_2` and the names of the blocks of `gen/kat.json`.
pub fn kat_names() -> Result<Vec<String>, String> {
    let kat = load_sim("gen", "kat").map_err(load_err)?;
    let mut names: Vec<String> = (0..array(&kat, "kat").len())
        .map(|i| format!("kat_{i}"))
        .collect();
    names.extend(
        array(&kat, "blocks")
            .iter()
            .filter_map(|b| b["name"].as_str().map(str::to_owned)),
    );
    names.sort();
    Ok(names)
}

/// The names of the stream cases of `gen/streams.json`.
pub fn stream_names() -> Result<Vec<String>, String> {
    let streams = load_sim("gen", "streams").map_err(load_err)?;
    let mut names: Vec<String> = array(&streams, "cases")
        .iter()
        .filter_map(|c| c["name"].as_str().map(str::to_owned))
        .collect();
    names.sort();
    Ok(names)
}

/// The names of the quantile cases.
pub fn quantile_names() -> Result<Vec<String>, String> {
    let q = load_sim("quantile", "cases").map_err(load_err)?;
    let mut names: Vec<String> = array(&q, "cases")
        .iter()
        .filter_map(|c| c["name"].as_str().map(str::to_owned))
        .collect();
    names.sort();
    Ok(names)
}

/// The names of the invariance checks.
pub fn invariance_names() -> Result<Vec<String>, String> {
    let q = load_sim("invariance", "cases").map_err(load_err)?;
    let mut names: Vec<String> = array(&q, "checks")
        .iter()
        .filter_map(|c| c["name"].as_str().map(str::to_owned))
        .collect();
    names.sort();
    Ok(names)
}

/// The names of the exact cases and of the study cases, from the files on disk.
pub fn exact_names() -> Result<Vec<String>, String> {
    list_sim_cases("exact").map_err(load_err)
}
pub fn study_names() -> Result<Vec<String>, String> {
    list_sim_cases("study").map_err(load_err)
}

/// The rows of `errors.json`.
pub fn error_rows() -> Result<Vec<Value>, String> {
    let e = load_sim_top("errors").map_err(load_err)?;
    Ok(array(&e, "cases").to_vec())
}

fn is_constructible(row: &Value) -> bool {
    row.get("constructible")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// Names of the rows that expect `expect` (`refused` or `accepted`) and are (or are not) constructible.
pub fn error_row_names(expect: &str, constructible: bool) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = error_rows()?
        .iter()
        .filter(|r| r["expect"].as_str() == Some(expect) && is_constructible(r) == constructible)
        .filter_map(|r| r["case"].as_str().map(str::to_owned))
        .collect();
    names.sort();
    Ok(names)
}

// ---------------------------------------------------------------- SIM-ORA-01: the generator

fn find_kat(name: &str) -> Result<Value, String> {
    let kat = load_sim("gen", "kat").map_err(load_err)?;
    if let Some(i) = name
        .strip_prefix("kat_")
        .and_then(|i| i.parse::<usize>().ok())
    {
        return array(&kat, "kat")
            .get(i)
            .cloned()
            .ok_or(format!("no known answer {name}"));
    }
    array(&kat, "blocks")
        .iter()
        .find(|b| b["name"].as_str() == Some(name))
        .cloned()
        .ok_or(format!("no block {name}"))
}

/// A known answer or a block of SX1: the 4 output words of Philox4x32-10, as integers.
pub fn check_philox(adapter: Adapter, name: &str) -> Result<(), String> {
    let entry = find_kat(name)?;
    let counter = hex_u32_list(&entry["counter"]).ok_or("counter")?;
    let key = hex_u32_list(&entry["key"]).ok_or("key")?;
    let expected = hex_u32_list(&entry["out"]).ok_or("out")?;
    let answer = call(
        adapter,
        name,
        &json!({ "op": "philox", "counter": counter, "key": key }),
    )?;
    let got: Vec<u64> = array(&answer, "out")
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    let mut p = Problems::new();
    p.eq(
        &format!("{name} output"),
        got,
        expected.iter().map(|&x| u64::from(x)).collect(),
    );
    p.finish(name)
}

/// A stream: key, counter, label hash, the 64-bit words, the uniforms (bit for bit) and the normals (bit for
/// bit where the file says so, else within `TOL_Z` of the 256-bit inverse).
pub fn check_stream(adapter: Adapter, name: &str) -> Result<(), String> {
    let streams = load_sim("gen", "streams").map_err(load_err)?;
    let case = array(&streams, "cases")
        .iter()
        .find(|c| c["name"].as_str() == Some(name))
        .ok_or(format!("no stream case {name}"))?;
    let words = array(case, "words");
    let indices: Vec<u64> = words.iter().filter_map(|w| w["n"].as_u64()).collect();
    let request = json!({
        "op": "stream", "seed": case["seed"], "replicate": case["replicate"], "purpose": case["purpose"],
        "label": case["label"], "n": indices,
    });
    let answer = call(adapter, name, &request)?;
    let mut p = Problems::new();
    let key: Vec<u64> = array(&answer, "key")
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    p.eq(
        &format!("{name} key"),
        key,
        hex_u32_list(&case["key"])
            .ok_or("key")?
            .iter()
            .map(|&x| u64::from(x))
            .collect(),
    );
    p.eq(
        &format!("{name} label hash"),
        answer["label_hash"].as_str().unwrap_or(""),
        case["label_hash"].as_str().unwrap_or("-"),
    );
    p.eq(
        &format!("{name} c1"),
        answer["c1"].as_u64(),
        hex_u32(&case["c1"]).map(u64::from),
    );
    let (aw, au, az) = (
        array(&answer, "words"),
        array(&answer, "u"),
        array(&answer, "z"),
    );
    for (i, w) in words.iter().enumerate() {
        let n = w["n"].as_u64().unwrap_or(0);
        p.eq(
            &format!("{name} word {n}"),
            aw.get(i).and_then(Value::as_str).unwrap_or(""),
            w["word"].as_str().unwrap_or("-"),
        );
        p.same(
            &format!("{name} u {n}"),
            au.get(i).and_then(num),
            num(&w["u"]).unwrap_or(f64::NAN),
        );
        let zr = num(&w["z_ref"]).unwrap_or(f64::NAN);
        if w["z_bit_exact"].as_bool() == Some(true) {
            p.same(
                &format!("{name} z {n}"),
                az.get(i).and_then(num),
                num(&w["z"]).unwrap_or(f64::NAN),
            );
        } else {
            p.close(
                &format!("{name} z {n}"),
                az.get(i).and_then(num),
                zr,
                0.0,
                TOL_Z,
            );
        }
    }
    p.finish(name)
}

// ---------------------------------------------------------------- SIM-ORA-02: the transform

/// The grid of required points: bit for bit where the file says so, else within `TOL_Z`.
pub fn check_transform_grid(adapter: Adapter) -> Result<(), String> {
    let grid = load_sim("transform", "grid").map_err(load_err)?;
    let points = array(&grid, "points");
    let u: Vec<f64> = points
        .iter()
        .map(|p| num(&p["u"]).unwrap_or(f64::NAN))
        .collect();
    let answer = call(adapter, "transform", &json!({ "op": "transform", "u": u }))?;
    let z = array(&answer, "z");
    let mut p = Problems::new();
    p.eq("number of z", z.len(), points.len());
    for (pt, a) in points.iter().zip(z) {
        let label = format!(
            "{} (u = {})",
            pt["name"].as_str().unwrap_or("?"),
            pt["u"].as_str().unwrap_or("?")
        );
        if pt["z_bit_exact"].as_bool() == Some(true) {
            p.same(&label, num(a), num(&pt["z"]).unwrap_or(f64::NAN));
        } else {
            p.close(
                &label,
                num(a),
                num(&pt["z_ref"]).unwrap_or(f64::NAN),
                0.0,
                TOL_Z,
            );
        }
    }
    p.finish("transform_grid")
}

// ---------------------------------------------------------------- SIM-ORA-03: the exact part

/// The request of the exact part for a case of `exact/`: the draws are given.
pub fn exact_request(case: &Value) -> Value {
    let mut options = Map::new();
    options.insert("negative".to_owned(), case["negative"].clone());
    if let Some(l) = case.get("lloq").filter(|l| !l.is_null()) {
        options.insert("lloq".to_owned(), l.clone());
    }
    json!({
        "op": "exact",
        "model": case["model"], "theta": case["theta"], "random_effects": case["random_effects"],
        "omega_shape": case["omega_shape"], "omega": case["omega"],
        "error_model": case["error_model"], "xi": case["xi"], "dose": case["dose"],
        "options": options,
        "subjects": case["subjects"],
    })
}

fn vec_close(
    p: &mut Problems,
    label: &str,
    actual: &Value,
    expected: &[Value],
    scale: &[f64],
    tol: f64,
) {
    let a = actual.as_array().map_or(&[][..], Vec::as_slice);
    p.eq(&format!("{label} length"), a.len(), expected.len());
    for (i, (x, e)) in a.iter().zip(expected).enumerate() {
        p.close(
            &format!("{label}[{i}]"),
            num(x),
            num(e).unwrap_or(f64::NAN),
            scale.get(i).copied().unwrap_or(0.0),
            tol,
        );
    }
}

/// One case of the exact part: L bit for bit, eta against its scale, P, PRED, IPRED and sigma relative, the y
/// family against its scale, `cens` and the counts exactly.
pub fn check_exact(adapter: Adapter, name: &str) -> Result<(), String> {
    let case = load_sim("exact", name).map_err(load_err)?;
    let answer = call(adapter, name, &exact_request(&case))?;
    let exp = &case["expected"];
    let mut p = Problems::new();
    let tol = case["tol_exact"].as_f64().unwrap_or(TOL_EXACT);
    for (i, row) in array(exp, "l").iter().enumerate() {
        for (j, e) in row
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .enumerate()
        {
            let a = answer["l"].get(i).and_then(|r| r.get(j)).and_then(num);
            p.same(&format!("L[{i}][{j}]"), a, num(e).unwrap_or(f64::NAN));
        }
    }
    let subjects = array(&answer, "subjects");
    let expected = array(exp, "subjects");
    p.eq("number of subjects", subjects.len(), expected.len());
    for (a, e) in subjects.iter().zip(expected) {
        let id = e["id"].as_str().unwrap_or("?");
        let eta_scale: Vec<f64> = array(e, "eta_scale")
            .iter()
            .map(|x| num(x).unwrap_or(0.0))
            .collect();
        let y_scale: Vec<f64> = array(e, "y_scale")
            .iter()
            .map(|x| num(x).unwrap_or(0.0))
            .collect();
        vec_close(
            &mut p,
            &format!("subject {id} eta"),
            &a["eta"],
            array(e, "eta"),
            &eta_scale,
            tol,
        );
        for (par, ev) in e["p"]
            .as_object()
            .map_or(Vec::new(), |o| o.iter().collect())
        {
            p.close(
                &format!("subject {id} {par}"),
                a["p"].get(par).and_then(num),
                num(ev).unwrap_or(f64::NAN),
                0.0,
                tol,
            );
        }
        for key in ["pred", "ipred", "sigma"] {
            vec_close(
                &mut p,
                &format!("subject {id} {key}"),
                &a[key],
                array(e, key),
                &[],
                tol,
            );
        }
        for key in ["y_raw", "y", "dv"] {
            vec_close(
                &mut p,
                &format!("subject {id} {key}"),
                &a[key],
                array(e, key),
                &y_scale,
                tol,
            );
        }
        let cens: Vec<u64> = array(a, "cens").iter().filter_map(Value::as_u64).collect();
        p.eq(
            &format!("subject {id} cens"),
            cens,
            array(e, "cens").iter().filter_map(Value::as_u64).collect(),
        );
        for key in ["n_zero_sigma", "n_negative", "n_blq"] {
            p.eq(
                &format!("subject {id} {key}"),
                a[key].as_u64(),
                e[key].as_u64(),
            );
        }
    }
    for key in ["n_zero_sigma", "n_negative", "n_blq"] {
        p.eq(
            &format!("total {key}"),
            answer[key].as_u64(),
            exp[key].as_u64(),
        );
    }
    p.finish(name)
}

// ---------------------------------------------------------------- SIM-ORA-03 and -05: whole studies

fn counts_u64(p: &mut Problems, answer: &Value, expected: &Value, what: &str) {
    for key in [
        "subjects",
        "replicates",
        "rows",
        "n_zero_sigma",
        "n_negative",
        "n_blq",
    ] {
        if let Some(e) = expected.get(key).and_then(Value::as_u64) {
            p.eq(&format!("{what} {key}"), answer[key].as_u64(), Some(e));
        }
    }
    for key in ["n_negative_by_subject", "n_blq_by_subject"] {
        if let Some(obj) = expected.get(key).and_then(Value::as_object) {
            for (id, e) in obj {
                p.eq(
                    &format!("{what} {key}[{id}]"),
                    answer[key].get(id).and_then(Value::as_u64),
                    e.as_u64(),
                );
            }
        }
    }
}

/// A whole study against the file: the tables of SIM-OUT-01 row by row, the counts, the summary of SIM-SUM-01
/// (tolerance, with the scale of the largest `y_scale` of the row) and the dataset of SIM-OUT-02.
pub fn check_study(adapter: Adapter, name: &str) -> Result<(), String> {
    let case = load_sim("study", name).map_err(load_err)?;
    let answer = call(adapter, name, &case["request"])?;
    check_study_answer(name, &case, &answer)
}

/// The comparison of `check_study`, on an answer already obtained.
pub fn check_study_answer(name: &str, case: &Value, answer: &Value) -> Result<(), String> {
    let exp = &case["expected"];
    let tol = case["tol_exact"].as_f64().unwrap_or(TOL_EXACT);
    let tol_z = case["tol_z"].as_f64().unwrap_or(TOL_Z);
    let mut p = Problems::new();
    let l: Vec<Vec<f64>> = array(exp, "l")
        .iter()
        .map(|r| {
            r.as_array().map_or(Vec::new(), |r| {
                r.iter().map(|x| num(x).unwrap_or(f64::NAN)).collect()
            })
        })
        .collect();
    // the subject table
    let (sa, se) = (array(answer, "subjects"), array(exp, "subjects"));
    p.eq("rows of the subject table", sa.len(), se.len());
    for (a, e) in sa.iter().zip(se) {
        let tag = format!(
            "rep {} subject {}",
            e["rep"],
            e["id"].as_str().unwrap_or("?")
        );
        p.eq(
            &format!("{tag} key"),
            (a["rep"].as_u64(), a["id"].as_str()),
            (e["rep"].as_u64(), e["id"].as_str()),
        );
        let z: Vec<f64> = array(e, "z_eta")
            .iter()
            .map(|x| num(x).unwrap_or(f64::NAN))
            .collect();
        let scale: Vec<f64> = (0..z.len())
            .map(|i| (0..=i).map(|j| (l[i][j] * z[j]).abs()).sum())
            .collect();
        vec_close(
            &mut p,
            &format!("{tag} eta"),
            &a["eta"],
            array(e, "eta"),
            &scale,
            tol,
        );
        for (par, ev) in e["p"]
            .as_object()
            .map_or(Vec::new(), |o| o.iter().collect())
        {
            p.close(
                &format!("{tag} {par}"),
                a["p"].get(par).and_then(num),
                num(ev).unwrap_or(f64::NAN),
                0.0,
                tol,
            );
        }
    }
    // the observation table
    let (oa, oe) = (array(answer, "observations"), array(exp, "observations"));
    p.eq("rows of the observation table", oa.len(), oe.len());
    let mut y_scale_of: std::collections::BTreeMap<(String, u64), f64> =
        std::collections::BTreeMap::new();
    for (a, e) in oa.iter().zip(oe) {
        let id = e["id"].as_str().unwrap_or("?");
        let tag = format!("rep {} subject {id} obs {}", e["rep"], e["obs"]);
        p.eq(
            &format!("{tag} key"),
            (a["rep"].as_u64(), a["id"].as_str(), a["obs"].as_u64()),
            (e["rep"].as_u64(), e["id"].as_str(), e["obs"].as_u64()),
        );
        p.same(
            &format!("{tag} time"),
            a.get("time").and_then(num),
            num(&e["time"]).unwrap_or(f64::NAN),
        );
        let ys = num(&e["y_scale"]).unwrap_or(0.0);
        let slot = y_scale_of
            .entry((id.to_owned(), e["obs"].as_u64().unwrap_or(0)))
            .or_insert(0.0);
        if ys > *slot {
            *slot = ys;
        }
        for key in ["pred", "ipred", "sigma"] {
            p.close(
                &format!("{tag} {key}"),
                a.get(key).and_then(num),
                num(&e[key]).unwrap_or(f64::NAN),
                0.0,
                tol,
            );
        }
        p.close(
            &format!("{tag} eps"),
            a.get("eps").and_then(num),
            num(&e["z_eps"]).unwrap_or(f64::NAN),
            0.0,
            tol_z,
        );
        for key in ["y_raw", "y", "dv"] {
            p.close(
                &format!("{tag} {key}"),
                a.get(key).and_then(num),
                num(&e[key]).unwrap_or(f64::NAN),
                ys,
                tol,
            );
        }
        p.eq(
            &format!("{tag} cens"),
            a["cens"].as_u64(),
            e["cens"].as_u64(),
        );
    }
    counts_u64(&mut p, &answer["counts"], &exp["counts"], "counts");
    // the summary
    let (ma, me) = (array(answer, "summary"), array(exp, "summary"));
    p.eq("rows of the summary", ma.len(), me.len());
    for (a, e) in ma.iter().zip(me) {
        let id = e["id"].as_str().unwrap_or("?");
        let obs = e["obs"].as_u64().unwrap_or(0);
        let tag = format!("summary subject {id} obs {obs}");
        p.eq(
            &format!("{tag} key"),
            (a["id"].as_str(), a["obs"].as_u64()),
            (Some(id), Some(obs)),
        );
        let scale = y_scale_of
            .get(&(id.to_owned(), obs))
            .copied()
            .unwrap_or(0.0);
        for (key, s) in [("y", scale), ("ipred", 0.0)] {
            let (av, ev) = (array(a, key), array(e, key));
            p.eq(&format!("{tag} {key} length"), av.len(), ev.len());
            for (i, (x, y)) in av.iter().zip(ev).enumerate() {
                p.close(
                    &format!("{tag} {key}[{i}]"),
                    num(x),
                    num(y).unwrap_or(f64::NAN),
                    s,
                    tol,
                );
            }
        }
        let bf = a["blq_fraction"].as_f64();
        let ebf = e["blq_fraction"].as_f64().unwrap_or(f64::NAN);
        p.close(&format!("{tag} blq_fraction"), bf, ebf, 1.0, 1e-12);
    }
    // the dataset
    let (da, de) = (array(answer, "dataset"), array(exp, "dataset"));
    p.eq("rows of the dataset", da.len(), de.len());
    let mut obs_index = 0usize;
    for (i, (a, e)) in da.iter().zip(de).enumerate() {
        let tag = format!("dataset row {i}");
        for key in ["subject", "label", "event"] {
            p.eq(&format!("{tag} {key}"), a[key].as_str(), e[key].as_str());
        }
        p.same(
            &format!("{tag} time"),
            a.get("time").and_then(num),
            num(&e["time"]).unwrap_or(f64::NAN),
        );
        if e["event"].as_str() == Some("dose") {
            p.same(
                &format!("{tag} amt"),
                a.get("amt").and_then(num),
                num(&e["amt"]).unwrap_or(f64::NAN),
            );
            p.eq(
                &format!("{tag} dv and cens are empty"),
                (a["dv"].is_null(), a["cens"].is_null()),
                (true, true),
            );
        } else {
            let ys = oe
                .get(obs_index)
                .and_then(|o| num(&o["y_scale"]))
                .unwrap_or(0.0);
            obs_index += 1;
            p.eq(&format!("{tag} amt is empty"), a["amt"].is_null(), true);
            p.close(
                &format!("{tag} dv"),
                a.get("dv").and_then(num),
                num(&e["dv"]).unwrap_or(f64::NAN),
                ys,
                tol,
            );
            p.eq(
                &format!("{tag} cens"),
                a["cens"].as_u64(),
                e["cens"].as_u64(),
            );
        }
    }
    p.finish(name)
}

// ---------------------------------------------------------------- SIM-SUM-01: the quantile

/// One case of type 7 quantiles: bit for bit.
pub fn check_quantile(adapter: Adapter, name: &str) -> Result<(), String> {
    let all = load_sim("quantile", "cases").map_err(load_err)?;
    let case = array(&all, "cases")
        .iter()
        .find(|c| c["name"].as_str() == Some(name))
        .ok_or(format!("no quantile case {name}"))?;
    let x = numbers(&case["x"]).ok_or("x")?;
    let pr = numbers(&case["p"]).ok_or("p")?;
    let answer = call(adapter, name, &json!({ "op": "quantile", "x": x, "p": pr }))?;
    let got = array(&answer, "q");
    let want = array(case, "expected");
    let mut p = Problems::new();
    p.eq("number of quantiles", got.len(), want.len());
    for (i, (a, e)) in got.iter().zip(want).enumerate() {
        p.same(
            &format!("{name} q[{i}] (p = {})", case["p"][i]),
            num(a),
            num(e).unwrap_or(f64::NAN),
        );
    }
    p.finish(name)
}

// ---------------------------------------------------------------- SIM-ORA-04: the distribution

fn stream_values(
    adapter: Adapter,
    seed: &Value,
    label: &str,
    rep: u64,
    purpose: u64,
    count: u64,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let request = json!({ "op": "stream", "seed": seed, "replicate": rep, "purpose": purpose, "label": label, "count": count });
    let answer = call(
        adapter,
        &format!("stream {label}/{rep}/{purpose}"),
        &request,
    )?;
    let u: Vec<f64> = array(&answer, "u").iter().filter_map(num).collect();
    let z: Vec<f64> = array(&answer, "z").iter().filter_map(num).collect();
    if u.len() as u64 != count || z.len() as u64 != count {
        return Err(format!(
            "stream {label}/{rep}/{purpose}: {} uniforms and {} normals, expected {count}",
            u.len(),
            z.len()
        ));
    }
    Ok((u, z))
}

fn recorded(v: &Value) -> f64 {
    num(v).unwrap_or(f64::NAN)
}

/// The statistics of SX3 on the first `n_words` words of four streams, against the stated bounds, the
/// recorded statistics (within `TOL_STAT`) and the exact counts.
pub fn check_sx3_streams(adapter: Adapter) -> Result<(), String> {
    let d = load_sim("dist", "sx3").map_err(load_err)?;
    let n = d["n_words"].as_u64().ok_or("n_words")?;
    let tol_stat = d["tol_stat"].as_f64().unwrap_or(TOL_STAT);
    let s = &d["streams"];
    let get_stream = |key: &str| -> Result<(Vec<f64>, Vec<f64>), String> {
        let c = &s[key];
        stream_values(
            adapter,
            &d["seed"],
            c["label"].as_str().unwrap_or(""),
            c["replicate"].as_u64().unwrap_or(0),
            c["purpose"].as_u64().unwrap_or(1),
            n,
        )
    };
    let (u, z) = get_stream("eta_subject1")?;
    let (_, z_eps) = get_stream("eps_subject1")?;
    let (_, z_s2) = get_stream("eta_subject2")?;
    let (_, z_r1) = get_stream("eta_subject1_replicate1")?;
    let mut p = Problems::new();
    let in_bounds = |p: &mut Problems, label: &str, actual: f64, center: f64, bound: f64| {
        p.compared += 1;
        if !(actual - center).abs().is_finite() || (actual - center).abs() > bound {
            p.fail(format!(
                "{label}: {actual:e} is farther than {bound:e} from {center}"
            ));
        }
    };
    let near = |p: &mut Problems, label: &str, actual: f64, rec: &Value| {
        p.close(label, Some(actual), recorded(rec), 1.0, tol_stat);
    };
    // uniform
    let (um, uv, _, _) = moments(&u);
    let ud = &d["uniform"];
    in_bounds(
        &mut p,
        "uniform mean",
        um,
        ud["mean"]["bound_around"].as_f64().unwrap_or(0.5),
        ud["mean"]["bound"].as_f64().unwrap_or(0.0),
    );
    near(&mut p, "uniform mean recorded", um, &ud["mean"]["recorded"]);
    near(
        &mut p,
        "uniform variance recorded",
        uv,
        &ud["variance_recorded"],
    );
    let chi = chisq_bins(&u, 100);
    p.compared += 1;
    let (lo, hi) = (
        ud["chisq100"]["lower"].as_f64().unwrap_or(0.0),
        ud["chisq100"]["upper"].as_f64().unwrap_or(0.0),
    );
    if !(chi >= lo && chi <= hi) {
        p.fail(format!(
            "chi-square of 100 bins: {chi} is outside [{lo}, {hi}]"
        ));
    }
    near(
        &mut p,
        "chi-square recorded",
        chi,
        &ud["chisq100"]["recorded"],
    );
    let umin = u.iter().copied().fold(f64::INFINITY, f64::min);
    let umax = u.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    p.same(
        "uniform min",
        Some(umin),
        parse_hex_float(ud["min_recorded"].as_str().unwrap_or("")).unwrap_or(f64::NAN),
    );
    p.same(
        "uniform max",
        Some(umax),
        parse_hex_float(ud["max_recorded"].as_str().unwrap_or("")).unwrap_or(f64::NAN),
    );
    // normal
    let (zm, zv, zs, zk) = moments(&z);
    let nd = &d["normal"];
    in_bounds(
        &mut p,
        "normal mean",
        zm,
        0.0,
        nd["mean"]["bound"].as_f64().unwrap_or(0.0),
    );
    near(&mut p, "normal mean recorded", zm, &nd["mean"]["recorded"]);
    in_bounds(
        &mut p,
        "normal variance",
        zv,
        nd["variance"]["bound_around"].as_f64().unwrap_or(1.0),
        nd["variance"]["bound"].as_f64().unwrap_or(0.0),
    );
    near(
        &mut p,
        "normal variance recorded",
        zv,
        &nd["variance"]["recorded"],
    );
    in_bounds(
        &mut p,
        "normal skewness",
        zs,
        0.0,
        nd["skewness"]["bound"].as_f64().unwrap_or(0.0),
    );
    near(
        &mut p,
        "normal skewness recorded",
        zs,
        &nd["skewness"]["recorded"],
    );
    in_bounds(
        &mut p,
        "normal excess kurtosis",
        zk,
        0.0,
        nd["excess_kurtosis"]["bound"].as_f64().unwrap_or(0.0),
    );
    near(
        &mut p,
        "normal excess kurtosis recorded",
        zk,
        &nd["excess_kurtosis"]["recorded"],
    );
    near(
        &mut p,
        "largest |z| recorded",
        z.iter().fold(0.0, |m: f64, v| m.max(v.abs())),
        &nd["max_abs_recorded"],
    );
    // counts: exact, and inside the interval
    for q in array(&d, "quantile_counts") {
        let thr = parse_hex_float(q["threshold"].as_str().unwrap_or("")).unwrap_or(f64::NAN);
        let count = z.iter().filter(|v| **v < thr).count() as u64;
        let tag = format!("count of z below the quantile {}", q["p"]);
        p.eq(&tag, Some(count), q["count_below"].as_u64());
        let (lo, hi) = (
            q["interval"][0].as_u64().unwrap_or(0),
            q["interval"][1].as_u64().unwrap_or(0),
        );
        p.compared += 1;
        if count < lo || count > hi {
            p.fail(format!("{tag}: {count} is outside [{lo}, {hi}]"));
        }
    }
    for (key, limit) in [("abs_gt_3", 3.0), ("abs_gt_4", 4.0)] {
        let t = &d["tails"][key];
        let count = z.iter().filter(|v| v.abs() > limit).count() as u64;
        p.eq(
            &format!("count of |z| > {limit}"),
            Some(count),
            t["count"].as_u64(),
        );
        let (lo, hi) = (
            t["interval"][0].as_u64().unwrap_or(0),
            t["interval"][1].as_u64().unwrap_or(0),
        );
        p.compared += 1;
        if count < lo || count > hi {
            p.fail(format!(
                "count of |z| > {limit}: {count} is outside [{lo}, {hi}]"
            ));
        }
    }
    // independence
    let c = &d["correlations"];
    let bound = c["bound"].as_f64().unwrap_or(0.0);
    let m = z.len();
    let pairs: [(&str, f64); 5] = [
        ("eta_vs_eps_subject1", correlation(&z, &z_eps)),
        ("subject1_vs_subject2", correlation(&z, &z_s2)),
        ("replicate0_vs_1", correlation(&z, &z_r1)),
        ("lag1", correlation(&z[..m - 1], &z[1..])),
        ("lag2", correlation(&z[..m - 2], &z[2..])),
    ];
    for (key, r) in pairs {
        in_bounds(&mut p, &format!("correlation {key}"), r, 0.0, bound);
        near(&mut p, &format!("correlation {key} recorded"), r, &c[key]);
    }
    p.finish("sx3_streams")
}

/// The request for the correlated-eta test: `m` subjects labelled "1".."m", one replicate, one time.
pub fn sx3_correlated_request(d: &Value) -> Value {
    json!({
        "op": "simulate", "model": "pk1.iv_bolus",
        "theta": { "cl": "5", "v": "50" }, "random_effects": ["cl", "v"],
        "omega_shape": "block", "omega": d["correlated_eta"]["omega_lower_by_rows"],
        "error_model": "additive", "xi": { "a": "0.2" },
        "design": { "times": ["1"], "dose": "100", "m": d["correlated_eta"]["m"] },
        "seed": d["seed"], "replicates": 1, "options": { "negative": "keep", "quantiles": [0.5] },
    })
}

/// The mean and the covariance of the first two eta of 100 000 subjects against Omega, with the bounds of SX3.
pub fn check_sx3_correlated_eta(adapter: Adapter) -> Result<(), String> {
    let d = load_sim("dist", "sx3").map_err(load_err)?;
    let answer = call(adapter, "correlated eta", &sx3_correlated_request(&d))?;
    let c = &d["correlated_eta"];
    let rows = array(&answer, "subjects");
    let mut p = Problems::new();
    p.eq("subjects", Some(rows.len() as u64), c["m"].as_u64());
    let col = |k: usize| -> Vec<f64> {
        rows.iter()
            .map(|r| r["eta"].get(k).and_then(num).unwrap_or(f64::NAN))
            .collect()
    };
    let (e1, e2) = (col(0), col(1));
    let n = e1.len() as f64;
    let (m1, m2) = (e1.iter().sum::<f64>() / n, e2.iter().sum::<f64>() / n);
    let (mut s11, mut s21, mut s22) = (0.0, 0.0, 0.0);
    for (a, b) in e1.iter().zip(&e2) {
        s11 += (a - m1) * (a - m1);
        s21 += (a - m1) * (b - m2);
        s22 += (b - m2) * (b - m2);
    }
    let (s11, s21, s22) = (s11 / (n - 1.0), s21 / (n - 1.0), s22 / (n - 1.0));
    let within_bound = |p: &mut Problems, label: &str, actual: f64, center: f64, bound: f64| {
        p.compared += 1;
        if !(actual - center).abs().is_finite() || (actual - center).abs() > bound {
            p.fail(format!(
                "{label}: {actual:e} is farther than {bound:e} from {center}"
            ));
        }
    };
    let list = |v: &Value| -> Vec<f64> {
        v.as_array().map_or(Vec::new(), |a| {
            a.iter().map(|x| num(x).unwrap_or(f64::NAN)).collect()
        })
    };
    let (mb, cb, ct) = (
        list(&c["mean_bound"]),
        list(&c["covariance_bound"]),
        list(&c["covariance_target"]),
    );
    within_bound(&mut p, "mean of eta 1", m1, 0.0, mb[0]);
    within_bound(&mut p, "mean of eta 2", m2, 0.0, mb[1]);
    within_bound(&mut p, "variance of eta 1", s11, ct[0], cb[0]);
    within_bound(&mut p, "covariance", s21, ct[1], cb[1]);
    within_bound(&mut p, "variance of eta 2", s22, ct[2], cb[2]);
    let rec = list(&c["covariance_recorded"]);
    let mrec = list(&c["mean_recorded"]);
    p.close("recorded mean 1", Some(m1), mrec[0], 1.0, TOL_STAT);
    p.close("recorded mean 2", Some(m2), mrec[1], 1.0, TOL_STAT);
    p.close("recorded variance 1", Some(s11), rec[0], 1.0, TOL_STAT);
    p.close("recorded covariance", Some(s21), rec[1], 1.0, TOL_STAT);
    p.close("recorded variance 2", Some(s22), rec[2], 1.0, TOL_STAT);
    p.close(
        "recorded correlation",
        Some(s21 / (s11 * s22).sqrt()),
        recorded(&c["correlation_recorded"]),
        1.0,
        TOL_STAT,
    );
    p.finish("sx3_correlated_eta")
}

// ---------------------------------------------------------------- SIM-ORA-05: invariance

fn strip_threads(mut v: Value) -> Value {
    if let Some(o) = v.get_mut("options").and_then(Value::as_object_mut) {
        o.remove("threads");
    }
    v
}

fn row_key(table: &str, row: &Value) -> String {
    if table == "subjects" {
        format!("{}|{}", row["rep"], row["id"])
    } else {
        format!("{}|{}|{}", row["rep"], row["id"], row["obs"])
    }
}

fn restricted<'a>(rows: &'a [Value], check: &Value) -> Vec<&'a Value> {
    let ids: Option<Vec<&str>> = check
        .get("ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect());
    let max_rep = check.get("max_rep").and_then(Value::as_u64);
    rows.iter()
        .filter(|r| {
            ids.as_ref()
                .is_none_or(|ids| ids.contains(&r["id"].as_str().unwrap_or("")))
        })
        .filter(|r| max_rep.is_none_or(|m| r["rep"].as_u64().unwrap_or(u64::MAX) <= m))
        .collect()
}

/// One invariance check: the two requests of the file, then either the two answers identical (threads) or the
/// listed fields of the matched rows equal (or different).
pub fn check_invariance(adapter: Adapter, name: &str) -> Result<(), String> {
    let all = load_sim("invariance", "cases").map_err(load_err)?;
    let check = array(&all, "checks")
        .iter()
        .find(|c| c["name"].as_str() == Some(name))
        .ok_or(format!("no invariance check {name}"))?;
    let a = call(adapter, &format!("{name} (a)"), &check["a"])?;
    let b = call(adapter, &format!("{name} (b)"), &check["b"])?;
    let mut p = Problems::new();
    if check["kind"].as_str() == Some("answers_identical") {
        let (a, b) = (strip_threads(a), strip_threads(b));
        p.eq(
            &format!("{name}: the two answers are equal as JSON values"),
            a == b,
            true,
        );
        return p.finish(name);
    }
    let want_equal = check["relation"].as_str() == Some("equal");
    let eta_len = check
        .get("eta_len")
        .and_then(Value::as_u64)
        .map(|n| n as usize);
    let fields: Vec<&str> = array(check, "fields")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut matched = 0usize;
    let mut different = 0usize;
    for table in array(check, "tables").iter().filter_map(Value::as_str) {
        let ra = restricted(array(&a, table), check);
        let rb = restricted(array(&b, table), check);
        p.eq(&format!("{name}: rows of {table}"), ra.len(), rb.len());
        let index: std::collections::BTreeMap<String, &Value> =
            rb.iter().map(|r| (row_key(table, r), *r)).collect();
        for row in ra {
            let Some(other) = index.get(&row_key(table, row)) else {
                p.fail(format!(
                    "{name}: {table} row {} has no partner",
                    row_key(table, row)
                ));
                continue;
            };
            matched += 1;
            for f in &fields {
                let (Some(x), Some(y)) = (row.get(*f), other.get(*f)) else {
                    continue;
                };
                let cut = |v: &Value| -> Value {
                    match (f, v.as_array(), eta_len) {
                        (&"eta", Some(arr), Some(n)) => {
                            Value::Array(arr.iter().take(n).cloned().collect())
                        }
                        _ => v.clone(),
                    }
                };
                if cut(x) != cut(y) {
                    different += 1;
                    if want_equal {
                        p.fail(format!(
                            "{name}: {table} row {} field {f} differs: {x} and {y}",
                            row_key(table, row)
                        ));
                    }
                }
            }
        }
    }
    p.eq(
        &format!("{name}: some rows were matched"),
        matched > 0,
        true,
    );
    if want_equal {
        p.compared += 1;
    } else {
        p.eq(
            &format!("{name}: at least one value differs"),
            different > 0,
            true,
        );
    }
    if name == "reverse_random_effects" {
        // the reversed run itself, against its reference (tolerance form)
        let tol = all["tol_exact"].as_f64().unwrap_or(TOL_EXACT);
        let rows = array(&b, "subjects");
        for e in array(&all, "reversed_run_reference") {
            let Some(row) = rows
                .iter()
                .find(|r| r["rep"] == e["rep"] && r["id"] == e["id"])
            else {
                p.fail(format!(
                    "{name}: no subject {} in replicate {} of the reversed run",
                    e["id"], e["rep"]
                ));
                continue;
            };
            let scale = numbers(&e["scale"]).unwrap_or_default();
            let eta = numbers(&e["eta"]).unwrap_or_default();
            for k in 0..eta.len() {
                p.close(
                    &format!(
                        "{name}: reversed run rep {} subject {} eta[{k}]",
                        e["rep"], e["id"]
                    ),
                    row["eta"].get(k).and_then(num),
                    eta[k],
                    scale[k],
                    tol,
                );
            }
        }
    }
    p.finish(name)
}

// ---------------------------------------------------------------- SIM-REF: refusals and accepted inputs

fn row(name: &str) -> Result<Value, String> {
    error_rows()?
        .into_iter()
        .find(|r| r["case"].as_str() == Some(name))
        .ok_or(format!("no row {name} in errors.json"))
}

/// A refused row: the code (where the file states one), the words in the message, the words that must not
/// be there, and the control (the same request with the cause removed) not refused.
pub fn check_refused(adapter: Adapter, name: &str) -> Result<(), String> {
    let r = row(name)?;
    if r["expect"].as_str() != Some("refused") || !is_constructible(&r) {
        return Err(format!("{name}: not a constructible refused row"));
    }
    let error = match adapter(&r["request"]) {
        Ok(answer) => {
            return Err(format!(
                "{name}: the request must be refused, got an answer ({} bytes)",
                answer.to_string().len()
            ));
        }
        Err(e) if e.code == "not_built" => {
            return Err(format!("{name}: the engine is not built (not_built): {e}"));
        }
        Err(e) => e,
    };
    let want = &r["error"];
    let mut p = Problems::new();
    if let Some(code) = want["code"].as_str() {
        p.eq(
            &format!("{name}: the code of the refusal"),
            error.code.as_str(),
            code,
        );
    }
    for w in array(want, "words").iter().filter_map(Value::as_str) {
        p.compared += 1;
        if !error.message.contains(w) {
            p.fail(format!(
                "{name}: the message must contain `{w}`: {}",
                error.message
            ));
        }
    }
    for w in array(want, "must_not_contain")
        .iter()
        .filter_map(Value::as_str)
    {
        p.compared += 1;
        if error.message.contains(w) {
            p.fail(format!(
                "{name}: the message must not contain `{w}`: {}",
                error.message
            ));
        }
    }
    if let Some(control) = r.get("control").filter(|c| !c.is_null()) {
        p.compared += 1;
        if let Err(e) = adapter(&control["request"]) {
            p.fail(format!(
                "{name}: the control ({}) must not be refused: {e}",
                control["change"].as_str().unwrap_or("?")
            ));
        }
    }
    p.finish(name)
}

/// An accepted row: not refused, and the counts the row states.
pub fn check_accepted(adapter: Adapter, name: &str) -> Result<(), String> {
    let r = row(name)?;
    if r["expect"].as_str() != Some("accepted") {
        return Err(format!("{name}: not an accepted row"));
    }
    let answer = call(adapter, name, &r["request"])?;
    let mut p = Problems::new();
    p.compared += 1;
    if let Some(expected) = r.get("expect_counts").and_then(Value::as_object) {
        for (key, e) in expected {
            p.eq(
                &format!("{name}: counts.{key}"),
                answer["counts"].get(key).and_then(Value::as_u64),
                e.as_u64(),
            );
        }
    }
    p.finish(name)
}

// ---------------------------------------------------------------- the exact input of a study request

/// The exact-part input of a study request (for the consistency test of the case files).
pub fn study_input(request: &Value) -> Result<ExactInput, String> {
    ExactInput::from_value(request)
}
