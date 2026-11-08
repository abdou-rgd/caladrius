//! Loaders and an independent second implementation for the population-simulation oracle (task T-158,
//! `specs/simulation.md`, SIM-ORA-*): `oracle/expected/sim/`.
//!
//! Files (produced by `oracle/scripts/sim_oracle.R`, which runs `sim_gen.R`, `sim_transform.R`, `sim_exact.R`,
//! `sim_study.R`, `sim_dist.R`, `sim_errors.R` and `sim_extras.R`):
//!
//! - `gen/kat.json`: the three known-answer vectors of Philox4x32-10 and the blocks of SX1 (SIM-ORA-01 item 1);
//! - `gen/streams.json`: the words, uniforms and normals of streams (SIM-ORA-01 items 2 and 3);
//! - `transform/grid.json`: the normal transform against the inverse at 256 bits (SIM-ORA-02);
//! - `exact/<case>.json`: the exact part with the draws given (SIM-ORA-03);
//! - `study/<case>.json`: whole studies, the tables of SIM-OUT-01, the summary of SIM-SUM-01 and the dataset
//!   of SIM-OUT-02 (SIM-ORA-03 and -05, SIM-DET-02 workload);
//! - `dist/sx3.json`: the distribution of the draws with stated bounds (SIM-ORA-04);
//! - `quantile/cases.json`: the quantile of type 7 (SIM-SUM-01);
//! - `invariance/cases.json`: pairs of requests whose answers must agree (SIM-ORA-05);
//! - `roundtrip/<design>.json`: the simulate-then-fit calls (SIM-ORA-06);
//! - `errors.json`: refusals as rows, each with a control where one exists (SIM-REF-*).
//!
//! **Doubles that must be bit-exact are exchanged as hexadecimal floats** (R's `%a`), parsed by
//! [`parse_hex_float`], and compared as parsed values. Decimal strings (30 or 50 digits) are read with Rust's
//! correctly rounded parser. [`number`] reads a JSON number, a decimal string or a hexadecimal string.
//!
//! The second implementation ([`philox4x32_10`], [`stream_word`], [`as241`], [`cholesky`], [`replay_subject`],
//! [`quantile7`]) is written from `specs/simulation.md` and shares no code with the R scripts or with any engine:
//! the consistency test (`tests/sim_consistency.rs`) uses it to check the case files, so that the expected
//! values are tested by something other than the script that wrote them.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

use crate::oracle::{OracleError, oracle_dir};
use crate::pop::sha256_hex;
use crate::step3::{json, read};

/// Relative bound on the normal transform away from the central region (SIM-ORA-02, open item OS-05): the
/// measured maximum on the R double implementation is 7.0e-16 (including the logarithm moved by one unit in
/// the last place), so the margin is about 14.
pub const TOL_Z: f64 = 1e-14;
/// Relative-to-scale bound on every quantity after `exp` or `ln` in the exact part (SIM-ORA-03): the measured
/// maximum of a correct double implementation against 256 bits is 9.5e-16 over the exact cases and 4.9e-16 over
/// 2994 study rows, so the margin is more than 1000.
pub const TOL_EXACT: f64 = 1e-12;
/// Bound on the statistics of SIM-ORA-04 against the recorded ones: the measured effect of a sequential sum
/// and of a logarithm moved by one unit in the last place is below 1e-13.
pub const TOL_STAT: f64 = 1e-9;

/// `oracle/expected/sim/`.
pub fn sim_dir() -> PathBuf {
    oracle_dir().join("expected").join("sim")
}

/// Names (without `.json`) of the files of a subdirectory of `oracle/expected/sim/`, sorted in byte order,
/// without `measure`.
pub fn list_sim_cases(sub: &str) -> Result<Vec<String>, OracleError> {
    let dir = sim_dir().join(sub);
    let entries = std::fs::read_dir(&dir).map_err(|e| OracleError::Io {
        path: dir.display().to_string(),
        message: e.to_string(),
    })?;
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter_map(|n| n.strip_suffix(".json").map(str::to_owned))
        .filter(|n| n != "measure")
        .collect();
    names.sort();
    Ok(names)
}

/// One file `oracle/expected/sim/<sub>/<name>.json` as a JSON value.
pub fn load_sim(sub: &str, name: &str) -> Result<Value, OracleError> {
    json(&sim_dir().join(sub).join(format!("{name}.json")))
}

/// A file directly under `oracle/expected/sim/` (`errors.json`).
pub fn load_sim_top(name: &str) -> Result<Value, OracleError> {
    json(&sim_dir().join(format!("{name}.json")))
}

/// The text of `oracle/expected/sim/<sub>/<name>.json`, for the hash check.
pub fn read_sim_text(sub: &str, name: &str) -> Result<String, OracleError> {
    read(&sim_dir().join(sub).join(format!("{name}.json")))
}

// ---------------------------------------------------------------- numbers

/// A power of two as a double, for -1022 <= e <= 1023 (exact).
fn pow2(e: i32) -> f64 {
    f64::from_bits(((1023 + i64::from(e)) as u64) << 52)
}

/// A hexadecimal float as R writes it (`%a`: `0x1.3333333333333p-4`, `-0x1p+0`, `0x0p+0`), exactly, or `None`
/// when the text is not one or has more than 53 significant bits.
pub fn parse_hex_float(text: &str) -> Option<f64> {
    let s = text.trim();
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let rest = rest
        .strip_prefix("0x")
        .or_else(|| rest.strip_prefix("0X"))?;
    let (mant_text, exp_text) = match rest.split_once(['p', 'P']) {
        Some((m, e)) => (m, e),
        None => (rest, "0"),
    };
    let exp: i32 = exp_text.parse().ok()?;
    let (int_text, frac_text) = mant_text.split_once('.').unwrap_or((mant_text, ""));
    if int_text.is_empty() && frac_text.is_empty() {
        return None;
    }
    let mut mant: u64 = 0;
    for c in int_text.chars().chain(frac_text.chars()) {
        let d = c.to_digit(16)?;
        if mant >> 56 != 0 {
            return None;
        }
        mant = mant * 16 + u64::from(d);
    }
    if mant >= 1u64 << 53 {
        return None;
    }
    let mut x = mant as f64;
    let mut e = exp.checked_sub(4 * i32::try_from(frac_text.len()).ok()?)?;
    while e > 1000 {
        x *= pow2(1000);
        e -= 1000;
    }
    while e < -1000 {
        x *= pow2(-1000);
        e += 1000;
    }
    x *= pow2(e);
    Some(if neg { -x } else { x })
}

/// The 16 hexadecimal digits of the bit pattern of a double.
pub fn bits_hex(x: f64) -> String {
    format!("{:016x}", x.to_bits())
}

/// A number from JSON: a JSON number, a decimal string (30 or 50 digits are read correctly rounded), or a
/// hexadecimal float string.
pub fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let t = s.trim();
            if t.contains("0x") || t.contains("0X") {
                parse_hex_float(t)
            } else {
                t.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

/// An array of [`number`]s.
pub fn numbers(v: &Value) -> Option<Vec<f64>> {
    v.as_array()?.iter().map(number).collect()
}

/// `|actual - expected| <= tol * max(|expected|, scale)`; an expected value of exactly 0 with no scale
/// requires an actual value of exactly 0.
pub fn within(actual: f64, expected: f64, scale: f64, tol: f64) -> bool {
    if !actual.is_finite() || !expected.is_finite() {
        return false;
    }
    (actual - expected).abs() <= tol * expected.abs().max(scale)
}

// ---------------------------------------------------------------- Philox4x32-10, SIM-RNG-01 to -04

const PHILOX_M0: u32 = 0xD251_1F53;
const PHILOX_M1: u32 = 0xCD9E_8D57;
const PHILOX_W0: u32 = 0x9E37_79B9;
const PHILOX_W1: u32 = 0xBB67_AE85;

fn mulhilo(a: u32, b: u32) -> (u32, u32) {
    let p = u64::from(a) * u64::from(b);
    ((p >> 32) as u32, p as u32)
}

/// Philox4x32-10: ten rounds of the counter `c` under the key `k`.
pub fn philox4x32_10(c: [u32; 4], k: [u32; 2]) -> [u32; 4] {
    let (mut c0, mut c1, mut c2, mut c3) = (c[0], c[1], c[2], c[3]);
    for round in 0..10u32 {
        let kk0 = k[0].wrapping_add(round.wrapping_mul(PHILOX_W0));
        let kk1 = k[1].wrapping_add(round.wrapping_mul(PHILOX_W1));
        let (hi0, lo0) = mulhilo(PHILOX_M0, c0);
        let (hi1, lo1) = mulhilo(PHILOX_M1, c2);
        let n0 = hi1 ^ c1 ^ kk0;
        let n2 = hi0 ^ c3 ^ kk1;
        c0 = n0;
        c1 = lo1;
        c2 = n2;
        c3 = lo0;
    }
    [c0, c1, c2, c3]
}

/// The key of a seed written as a decimal string: (seed mod 2^32, floor(seed / 2^32)); `None` when the string
/// is not a number below 2^64.
pub fn seed_key(seed: &str) -> Option<[u32; 2]> {
    if seed.is_empty() || !seed.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let s: u64 = seed.parse().ok()?;
    Some([s as u32, (s >> 32) as u32])
}

/// The 64-bit stream key of a label: the first 8 bytes of the SHA-256 of its UTF-8 bytes, as (c2, c3), the two
/// halves of a big-endian integer. Also returns the 16 hexadecimal digits.
pub fn label_key(label: &str) -> ([u32; 2], String) {
    let hex = sha256_hex(label.as_bytes());
    let head = hex.get(..16).unwrap_or("0000000000000000").to_owned();
    let c2 = u32::from_str_radix(head.get(..8).unwrap_or("0"), 16).unwrap_or(0);
    let c3 = u32::from_str_radix(head.get(8..).unwrap_or("0"), 16).unwrap_or(0);
    ([c2, c3], head)
}

/// Word `n` of the stream (seed key, replicate `r`, purpose `p`, label key): block `n / 2`, half `n % 2`,
/// `x = r_{2h} + 2^32 r_{2h+1}` (SIM-RNG-03).
pub fn stream_word(key: [u32; 2], r: u32, p: u32, lkey: [u32; 2], n: u64) -> u64 {
    let block = (n / 2) as u32;
    let half = (n % 2) as usize;
    let out = philox4x32_10([block, (p << 24) | r, lkey[0], lkey[1]], key);
    u64::from(out[2 * half]) | (u64::from(out[2 * half + 1]) << 32)
}

/// The uniform of a word (SIM-RNG-04): `(top 52 bits + 0.5) * 2^-52`.
pub fn word_uniform(word: u64) -> f64 {
    ((word >> 12) as f64 + 0.5) * pow2(-52)
}

const AS_A: [&str; 8] = [
    "3.387132872796366608",
    "133.14166789178437745",
    "1971.5909503065514427",
    "13731.693765509461125",
    "45921.953931549871457",
    "67265.770927008700853",
    "33430.575583588128105",
    "2509.0809287301226727",
];
const AS_B: [&str; 8] = [
    "1",
    "42.313330701600911252",
    "687.1870074920579083",
    "5394.1960214247511077",
    "21213.794301586595867",
    "39307.89580009271061",
    "28729.085735721942674",
    "5226.495278852854561",
];
const AS_C: [&str; 8] = [
    "1.42343711074968357734",
    "4.6303378461565452959",
    "5.7694972214606914055",
    "3.64784832476320460504",
    "1.27045825245236838258",
    "0.24178072517745061177",
    "0.0227238449892691845833",
    "7.7454501427834140764e-4",
];
const AS_D: [&str; 8] = [
    "1",
    "2.05319162663775882187",
    "1.6763848301838038494",
    "0.68976733498510000455",
    "0.14810397642748007459",
    "0.0151986665636164571966",
    "5.475938084995344946e-4",
    "1.05075007164441684324e-9",
];
const AS_E: [&str; 8] = [
    "6.6579046435011037772",
    "5.4637849111641143699",
    "1.7848265399172913358",
    "0.29656057182850489123",
    "0.026532189526576123093",
    "0.0012426609473880784386",
    "2.71155556874348757815e-5",
    "2.01033439929228813265e-7",
];
const AS_F: [&str; 8] = [
    "1",
    "0.59983220655588793769",
    "0.13692988092273580531",
    "0.0148753612908506148525",
    "7.868691311456132591e-4",
    "1.8463183175100546818e-5",
    "1.4215117583164458887e-7",
    "2.04426310338993978564e-15",
];

fn coefficients(text: &[&str; 8]) -> [f64; 8] {
    let mut c = [0.0; 8];
    for (slot, t) in c.iter_mut().zip(text) {
        *slot = t.parse::<f64>().unwrap_or(f64::NAN);
    }
    c
}

/// Horner from the highest degree down, every product and sum rounded separately.
fn poly(x: f64, cf: &[f64; 8]) -> f64 {
    let mut r = cf[7];
    for i in (0..7).rev() {
        r = r * x + cf[i];
    }
    r
}

/// Wichura's AS 241 (PPND16) as SIM-RNG-05 states it, with the platform's logarithm: central region
/// `|u - 0.5| <= 0.425` bit for bit, the tails within `TOL_Z`.
pub fn as241(u: f64) -> f64 {
    let q = u - 0.5;
    if q.abs() <= 0.425 {
        let rho = 0.180625 - q * q;
        return q * poly(rho, &coefficients(&AS_A)) / poly(rho, &coefficients(&AS_B));
    }
    let pt = if q < 0.0 { u } else { 1.0 - u };
    let t = (-pt.ln()).sqrt();
    let v = if t <= 5.0 {
        let tp = t - 1.6;
        poly(tp, &coefficients(&AS_C)) / poly(tp, &coefficients(&AS_D))
    } else {
        let tp = t - 5.0;
        poly(tp, &coefficients(&AS_E)) / poly(tp, &coefficients(&AS_F))
    };
    if q < 0.0 { -v } else { v }
}

/// True when `u` is in the central region of [`as241`] (the result is then bit-exact).
pub fn as241_is_central(u: f64) -> bool {
    (u - 0.5).abs() <= 0.425
}

/// The first `n` normals of a stream.
pub fn stream_normals(seed: &str, r: u32, p: u32, label: &str, n: usize) -> Option<Vec<f64>> {
    let key = seed_key(seed)?;
    let (lkey, _) = label_key(label);
    Some(
        (0..n as u64)
            .map(|i| as241(word_uniform(stream_word(key, r, p, lkey, i))))
            .collect(),
    )
}

// ---------------------------------------------------------------- Omega, eta (SIM-RNG-07)

/// The Omega matrix from its entries: the lower triangle by rows (`block`) or the diagonal (`diagonal`).
#[allow(clippy::needless_range_loop)]
pub fn omega_matrix(shape: &str, entries: &[f64], q: usize) -> Result<Vec<Vec<f64>>, String> {
    let mut m = vec![vec![0.0; q]; q];
    match shape {
        "diagonal" => {
            if entries.len() != q {
                return Err(format!(
                    "{} entries for a diagonal omega of {q}",
                    entries.len()
                ));
            }
            for i in 0..q {
                m[i][i] = entries[i];
            }
        }
        "block" => {
            if entries.len() != q * (q + 1) / 2 {
                return Err(format!(
                    "{} entries for a block omega of {q}",
                    entries.len()
                ));
            }
            let mut k = 0;
            for i in 0..q {
                for j in 0..=i {
                    m[i][j] = entries[k];
                    m[j][i] = entries[k];
                    k += 1;
                }
            }
        }
        other => return Err(format!("omega shape `{other}`")),
    }
    Ok(m)
}

/// Cholesky-Banachiewicz row by row; the sum of the products is accumulated from 0 left to right and
/// subtracted once. `Err((index, pivot))` when a pivot is not finite or not positive.
#[allow(clippy::needless_range_loop)]
pub fn cholesky(om: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, (usize, f64)> {
    let q = om.len();
    let mut l = vec![vec![0.0; q]; q];
    for i in 0..q {
        for j in 0..=i {
            let mut t = 0.0;
            for k in 0..j {
                t += l[i][k] * l[j][k];
            }
            let s = om[i][j] - t;
            if i == j {
                if !(s.is_finite() && s > 0.0) {
                    return Err((i, s));
                }
                l[i][i] = s.sqrt();
            } else {
                l[i][j] = s / l[j][j];
            }
        }
    }
    Ok(l)
}

// ---------------------------------------------------------------- the model, the error model, the rules

/// Concentration of one of the five models of the oracle at the times `t` (zero before the dose, MOD-GEN-02).
pub fn naive_conc(
    model: &str,
    p: &BTreeMap<String, f64>,
    dose: f64,
    t: &[f64],
) -> Result<Vec<f64>, String> {
    let get = |k: &str| -> Result<f64, String> {
        p.get(k)
            .copied()
            .ok_or_else(|| format!("parameter {k} missing for {model}"))
    };
    let oral = |s: f64| -> Result<f64, String> {
        let (cl, v, ka) = (get("cl")?, get("v")?, get("ka")?);
        let k = cl / v;
        Ok(dose * ka / (v * (ka - k)) * ((-k * s).exp() - (-ka * s).exp()))
    };
    let mut out = Vec::with_capacity(t.len());
    for &time in t {
        let c = if time < 0.0 {
            0.0
        } else {
            match model {
                "pk1.iv_bolus" => dose / get("v")? * (-(get("cl")? / get("v")?) * time).exp(),
                "pk1.iv_infusion" => {
                    let (cl, v, dur) = (get("cl")?, get("v")?, get("dur")?);
                    let k = cl / v;
                    let r0 = dose / dur;
                    if time <= dur {
                        r0 / cl * (1.0 - (-k * time).exp())
                    } else {
                        r0 / cl * (1.0 - (-k * dur).exp()) * (-k * (time - dur)).exp()
                    }
                }
                "pk1.oral_1" => oral(time)?,
                "pk1.oral_1_lag" => {
                    let s = time - get("tlag")?;
                    if s > 0.0 { oral(s)? } else { 0.0 }
                }
                "pk2.iv_bolus" => {
                    let (cl, vc, q, vp) = (get("cl")?, get("vc")?, get("q")?, get("vp")?);
                    let (k10, k12, k21) = (cl / vc, q / vc, q / vp);
                    let s = k10 + k12 + k21;
                    let r = (s * s - 4.0 * k10 * k21).sqrt();
                    let al = (s + r) / 2.0;
                    let be = k10 * k21 / al;
                    let wa = (al - k21) / (al - be);
                    let wb = (k21 - be) / (al - be);
                    dose / vc * (wa * (-al * time).exp() + wb * (-be * time).exp())
                }
                other => return Err(format!("model {other} is not in the oracle")),
            }
        };
        out.push(c);
    }
    Ok(out)
}

/// The inputs of the exact part, read from a case file or from a request.
#[derive(Debug, Clone)]
pub struct ExactInput {
    pub model: String,
    pub theta: BTreeMap<String, f64>,
    pub random_effects: Vec<String>,
    pub omega_shape: String,
    pub omega: Vec<f64>,
    pub error_model: String,
    pub xi: BTreeMap<String, f64>,
    pub dose: f64,
    pub negative_zero: bool,
    pub lloq: Option<f64>,
}

fn map_of(v: &Value) -> Result<BTreeMap<String, f64>, String> {
    let obj = v.as_object().ok_or("an object of numbers is expected")?;
    obj.iter()
        .map(|(k, x)| {
            number(x)
                .map(|n| (k.clone(), n))
                .ok_or(format!("{k} is not a number"))
        })
        .collect()
}

impl ExactInput {
    /// From a case of `exact/` (the dose, `negative` and `lloq` at the top level) or a study request (the dose in
    /// the first subject, `negative` and `lloq` in `options`).
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let str_of = |k: &str| -> Result<String, String> {
            v.get(k)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(format!("{k} missing"))
        };
        let options = v.get("options");
        let opt = |k: &str| -> Option<&Value> {
            v.get(k)
                .filter(|x| !x.is_null())
                .or_else(|| options.and_then(|o| o.get(k)).filter(|x| !x.is_null()))
        };
        let dose = match v.get("dose") {
            Some(d) => number(d).ok_or("dose")?,
            None => {
                let s = v
                    .get("design")
                    .and_then(|d| d.get("subjects"))
                    .and_then(|s| s.get(0))
                    .and_then(|s| s.get("dose"))
                    .ok_or("dose missing")?;
                number(s).ok_or("dose")?
            }
        };
        Ok(ExactInput {
            model: str_of("model")?,
            theta: map_of(v.get("theta").ok_or("theta missing")?)?,
            random_effects: v
                .get("random_effects")
                .and_then(Value::as_array)
                .ok_or("random_effects missing")?
                .iter()
                .map(|x| x.as_str().map(str::to_owned).ok_or("random effect name"))
                .collect::<Result<_, _>>()?,
            omega_shape: str_of("omega_shape")?,
            omega: numbers(v.get("omega").ok_or("omega missing")?).ok_or("omega entries")?,
            error_model: str_of("error_model")?,
            xi: map_of(v.get("xi").ok_or("xi missing")?)?,
            dose,
            negative_zero: opt("negative").and_then(Value::as_str) == Some("zero"),
            lloq: opt("lloq").and_then(number),
        })
    }

    /// The Cholesky factor of Omega.
    pub fn factor(&self) -> Result<Vec<Vec<f64>>, String> {
        let m = omega_matrix(&self.omega_shape, &self.omega, self.random_effects.len())?;
        cholesky(&m).map_err(|(i, s)| format!("pivot {i} is {s}"))
    }
}

/// One subject of the exact part, in double precision.
#[derive(Debug, Clone)]
pub struct SubjectReplay {
    pub eta: Vec<f64>,
    pub eta_scale: Vec<f64>,
    pub p: BTreeMap<String, f64>,
    pub pred: Vec<f64>,
    pub ipred: Vec<f64>,
    pub sigma: Vec<f64>,
    pub y_raw: Vec<f64>,
    pub y_scale: Vec<f64>,
    pub y: Vec<f64>,
    pub cens: Vec<u8>,
    pub dv: Vec<f64>,
    pub n_zero_sigma: usize,
    pub n_negative: usize,
    pub n_blq: usize,
}

/// The exact part of one subject: `eta = L z`, `P = theta exp(eta)`, `PRED`, `IPRED`, `sigma`, `y_raw`, the
/// negative rule, then the BLQ rule (strictly below the LLOQ), then `dv` (SIM-ORD-01 to -05).
pub fn replay_subject(
    input: &ExactInput,
    l: &[Vec<f64>],
    times: &[f64],
    z_eta: &[f64],
    z_eps: &[f64],
) -> Result<SubjectReplay, String> {
    let q = input.random_effects.len();
    if z_eta.len() != q || z_eps.len() != times.len() {
        return Err("the number of draws does not fit the model and the times".to_owned());
    }
    let mut eta = vec![0.0; q];
    let mut eta_scale = vec![0.0; q];
    for i in 0..q {
        let (mut acc, mut sc) = (0.0, 0.0);
        for j in 0..=i {
            let term = l[i][j] * z_eta[j];
            acc += term;
            sc += term.abs();
        }
        eta[i] = acc;
        eta_scale[i] = sc;
    }
    let mut p = input.theta.clone();
    for (k, name) in input.random_effects.iter().enumerate() {
        let base = input
            .theta
            .get(name)
            .ok_or(format!("no theta for {name}"))?;
        p.insert(name.clone(), base * eta[k].exp());
    }
    let ipred = naive_conc(&input.model, &p, input.dose, times)?;
    let pred = naive_conc(&input.model, &input.theta, input.dose, times)?;
    let a = input.xi.get("a").copied().unwrap_or(0.0);
    let b = input.xi.get("b").copied().unwrap_or(0.0);
    let sigma: Vec<f64> = ipred
        .iter()
        .map(|&c| match input.error_model.as_str() {
            "additive" => a,
            "proportional" => b * c.abs(),
            "combined" => (a * a + b * b * c * c).sqrt(),
            _ => a + b * c.abs(),
        })
        .collect();
    let y_raw: Vec<f64> = ipred
        .iter()
        .zip(&sigma)
        .zip(z_eps)
        .map(|((c, s), z)| c + s * z)
        .collect();
    let y_scale: Vec<f64> = ipred
        .iter()
        .zip(&sigma)
        .zip(z_eps)
        .map(|((c, s), z)| c.abs() + s * z.abs())
        .collect();
    let mut y = y_raw.clone();
    let mut n_negative = 0;
    for (yi, raw) in y.iter_mut().zip(&y_raw) {
        if *raw < 0.0 {
            n_negative += 1;
            if input.negative_zero {
                *yi = 0.0;
            }
        }
    }
    let cens: Vec<u8> = y
        .iter()
        .map(|&v| u8::from(input.lloq.is_some_and(|lq| v < lq)))
        .collect();
    let dv: Vec<f64> = y
        .iter()
        .zip(&cens)
        .map(|(&v, &c)| match (c, input.lloq) {
            (1, Some(lq)) => lq,
            _ => v,
        })
        .collect();
    Ok(SubjectReplay {
        eta,
        eta_scale,
        p,
        pred,
        ipred,
        n_zero_sigma: sigma.iter().filter(|s| **s == 0.0).count(),
        sigma,
        y_raw,
        y_scale,
        y,
        n_negative,
        n_blq: cens.iter().filter(|c| **c == 1).count(),
        cens,
        dv,
    })
}

// ---------------------------------------------------------------- summaries

/// The quantile of type 7 (SIM-SUM-01), in the form `(1 - g) x_lo + g x_hi`; `None` for no value or a
/// probability outside [0, 1].
pub fn quantile7(x: &[f64], p: f64) -> Option<f64> {
    if x.is_empty() || !(0.0..=1.0).contains(&p) {
        return None;
    }
    let mut s = x.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len() as f64;
    let h = 1.0 + (n - 1.0) * p;
    let lo = h.floor();
    let hi = h.ceil();
    let g = h - lo;
    Some((1.0 - g) * s[lo as usize - 1] + g * s[hi as usize - 1])
}

/// Sample moments by sequential sums (SIM-ORA-04): mean, variance with divisor N - 1, skewness and excess
/// kurtosis from the central moments with divisor N.
pub fn moments(z: &[f64]) -> (f64, f64, f64, f64) {
    let n = z.len() as f64;
    let mean = z.iter().sum::<f64>() / n;
    let (mut s2, mut s3, mut s4) = (0.0, 0.0, 0.0);
    for v in z {
        let d = v - mean;
        let d2 = d * d;
        s2 += d2;
        s3 += d2 * d;
        s4 += d2 * d2;
    }
    let (m2, m3, m4) = (s2 / n, s3 / n, s4 / n);
    (
        mean,
        m2 * n / (n - 1.0),
        m3 / (m2 * m2.sqrt()),
        m4 / (m2 * m2) - 3.0,
    )
}

/// Pearson correlation.
pub fn correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx) * (a - mx);
        syy += (b - my) * (b - my);
    }
    sxy / (sxx * syy).sqrt()
}

/// The chi-square statistic of the counts of 100 equal bins of [0, 1).
pub fn chisq_bins(u: &[f64], bins: usize) -> f64 {
    let mut counts = vec![0u64; bins];
    for v in u {
        let k = ((v * bins as f64).floor() as usize).min(bins - 1);
        counts[k] += 1;
    }
    let e = u.len() as f64 / bins as f64;
    counts
        .iter()
        .map(|&o| (o as f64 - e) * (o as f64 - e) / e)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_floats_read_back_to_the_bit_patterns() {
        assert_eq!(parse_hex_float("0x1p+0"), Some(1.0));
        assert_eq!(parse_hex_float("-0x1.8p+1"), Some(-3.0));
        assert_eq!(parse_hex_float("0x0p+0"), Some(0.0));
        assert_eq!(
            parse_hex_float("0x1.3333333333333p-4").map(f64::to_bits),
            Some(0.075f64.to_bits())
        );
        assert_eq!(
            parse_hex_float("0x1p-1074").map(f64::to_bits),
            Some(1),
            "the smallest denormal"
        );
        assert_eq!(parse_hex_float("0x1.fffffffffffffp+1023"), Some(f64::MAX));
        assert_eq!(parse_hex_float("0x1p"), None);
        assert_eq!(parse_hex_float("1.5"), None);
    }

    #[test]
    fn the_known_answer_vector_of_zero() {
        assert_eq!(
            philox4x32_10([0; 4], [0; 2]),
            [0x6627e8d5, 0xe169c58d, 0xbc57ac4c, 0x9b00dbd8]
        );
    }
}
