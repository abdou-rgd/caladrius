//! A checked study: the call of SIM-MOD-05 after every refusal of section 7, ready to evaluate cells.
//!
//! The unit of work is the cell (replicate, subject). [`Study::cell`] is a pure function of the study, the
//! replicate and the subject (SIM-DET-01): no state, no clock, no other subject. A caller with any number of
//! threads fills an array of cells in the order of its choice and [`Study::assemble`] builds the tables
//! in index order (`index = replicate * subjects + subject`), so the bits do not depend on the grouping.

use std::collections::BTreeMap;

use caladrius_models::{ModelId, ModelInput};

use super::error::{SimError, SimErrorCode};
use super::input::{Design, ErrorKind, NegativeRule, RowKind, SimInput};
use super::omega;
use super::rng::{LabelKey, Purpose, Seed, Stream};

/// The replicate sits in the low 24 bits of the counter word c1 (SIM-RNG-03).
pub const MAX_REPLICATES: u64 = 1 << 24;

/// The version of the generator written in every result (SIM-DET-03).
pub const RNG_VERSION: &str = "philox4x32-10/as241/v1";

/// The default probabilities of the summary (SIM-MOD-05).
pub const DEFAULT_QUANTILES: [f64; 3] = [0.05, 0.5, 0.95];

/// A parameter of the model: its typical value and, if it has one, the index of its random effect.
#[derive(Debug, Clone)]
struct ParamPlan {
    name: String,
    theta: f64,
    effect: Option<usize>,
}

/// One subject of the design after checking.
#[derive(Debug, Clone)]
pub(super) struct SubjectPlan {
    pub(super) label: String,
    key: LabelKey,
    pub(super) dose: f64,
    pub(super) dose_time: f64,
    /// The observation times as the design holds them.
    pub(super) times: Vec<f64>,
    /// The times since the dose, the times at which the model is evaluated.
    relative: Vec<f64>,
    /// PRED: the model at the typical values (every eta = 0), which does not depend on the replicate.
    pub(super) pred: Vec<f64>,
}

/// The draws and the results of one subject in one replicate.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// The replicate, from 0.
    pub replicate: u64,
    /// The subject, in design order, from 0.
    pub subject: usize,
    /// The random effects eta = L z, in the order of `random_effects`.
    pub eta: Vec<f64>,
    /// The individual parameters, all of them, by name.
    pub p: BTreeMap<String, f64>,
    /// IPRED at each observation row.
    pub ipred: Vec<f64>,
    /// sigma at each observation row.
    pub sigma: Vec<f64>,
    /// The standard normal z of each observation row.
    pub eps: Vec<f64>,
    /// y before the negative rule.
    pub y_raw: Vec<f64>,
    /// y after the negative rule, before the BLQ replacement.
    pub y: Vec<f64>,
    /// 1 on a BLQ row, else 0.
    pub cens: Vec<u8>,
    /// The value written to the dataset: y, or the LLOQ on a BLQ row.
    pub dv: Vec<f64>,
    /// Rows with sigma = 0.
    pub n_zero_sigma: usize,
    /// Rows with raw y below 0.
    pub n_negative: usize,
    /// Rows below the LLOQ.
    pub n_blq: usize,
}

/// A checked study.
#[derive(Debug, Clone)]
pub struct Study {
    pub(super) input: SimInput,
    pub(super) seed: Seed,
    pub(super) model: ModelId,
    params: Vec<ParamPlan>,
    q: usize,
    l: Vec<f64>,
    error: ErrorKind,
    a: f64,
    b: f64,
    pub(super) negative: NegativeRule,
    pub(super) lloq: Option<f64>,
    pub(super) quantiles: Vec<f64>,
    pub(super) replicates: u64,
    pub(super) subjects: Vec<SubjectPlan>,
    pub(super) n_dv_ignored: usize,
}

impl Study {
    /// Checks the call (section 7) and prepares the evaluation. Nothing is drawn here: a refusal stops the
    /// run before any output.
    pub fn new(input: &SimInput) -> Result<Study, SimError> {
        Study::build(input, true)
    }

    /// The study of the test entry point of SIM-ORA-03 (with [`Study::cell_with_draws`]): the same checks as
    /// [`Study::new`] except the order of the observation times, because the exact part is evaluated
    /// observation row by observation row and its cases may repeat a time to place two draws on one
    /// prediction. Decision taken meanwhile (2026-11-08), reversible by Abdou: `specs/simulation.md` gives
    /// the entry point no rule on times, and `duplicate_times` is a refusal of a design (SIM-REF-04).
    pub fn for_draws(input: &SimInput) -> Result<Study, SimError> {
        Study::build(input, false)
    }

    fn build(input: &SimInput, strict_times: bool) -> Result<Study, SimError> {
        let seed = input.seed.ok_or_else(missing_seed)?;
        // The design first (SIM-REF-04, `label_key_collision`): it is the cheapest check and the one a
        // user meets most.
        let (raw, n_dv_ignored) = plan_design(&input.design, strict_times)?;
        let model = parse_model(&input.model)?;
        let names = parameter_names(model);

        let lloq = match input.lloq {
            None => None,
            Some(x) if x.is_finite() && x > 0.0 => Some(x),
            Some(x) => {
                return Err(SimError::invalid(format!(
                    "option lloq {x} must be a finite number > 0"
                )));
            }
        };
        let quantiles = match &input.quantiles {
            None => DEFAULT_QUANTILES.to_vec(),
            Some(list) => {
                if list.iter().any(|p| !(0.0..=1.0).contains(p)) {
                    return Err(SimError::invalid(
                        "option quantiles takes probabilities from 0 to 1",
                    ));
                }
                list.clone()
            }
        };
        if input.replicates == 0 {
            return Err(SimError::invalid("replicates must be an integer from 1"));
        }
        if input.replicates > MAX_REPLICATES {
            return Err(SimError::new(
                SimErrorCode::SizeLimit,
                format!(
                    "replicates {} exceed {MAX_REPLICATES}, the replicate field holds {MAX_REPLICATES} values",
                    input.replicates
                ),
            ));
        }
        let cells = usize::try_from(input.replicates)
            .ok()
            .and_then(|r| r.checked_mul(raw.len()));
        if cells.is_none() {
            return Err(SimError::new(
                SimErrorCode::SizeLimit,
                "the simulation has more (replicate, subject) cells than this target can index; reduce subjects or replicates, or run in pieces: the replicates 0..K-1 and K..2K-1 of a seed can be run separately and are the same as in one run",
            ));
        }

        // The error model.
        let mut values = BTreeMap::new();
        for name in input.error_model.needs() {
            let given = if *name == "a" { input.a } else { input.b };
            let x = given.ok_or_else(|| {
                SimError::invalid(format!(
                    "error parameter {name} is missing for {}",
                    input.error_model.name()
                ))
            })?;
            if !(x.is_finite() && x >= 0.0) {
                return Err(SimError::invalid(format!(
                    "error parameter {name} must be finite and not negative"
                )));
            }
            values.insert(*name, x);
        }

        // The random effects and theta.
        if input.random_effects.is_empty() {
            return Err(SimError::invalid(
                "random_effects is empty: name at least one parameter",
            ));
        }
        for (i, name) in input.random_effects.iter().enumerate() {
            if input.random_effects.iter().take(i).any(|n| n == name) {
                return Err(SimError::invalid(format!(
                    "random_effects names {name} twice"
                )));
            }
            // Decision taken meanwhile (2026-11-08), reversible by Abdou (OS-03): no random effect on a lag time
            // or a duration, as in the fit (SIM-MOD-03).
            if name == "tlag" || name == "dur" {
                return Err(SimError::invalid(format!(
                    "random_effects cannot name {name}: a lag time or a duration has no random effect in this version"
                )));
            }
            if !names.iter().any(|n| n == name) {
                return Err(SimError::invalid(format!(
                    "random_effects names {name}, which {} does not have",
                    input.model
                )));
            }
        }
        if let Some(extra) = input.theta.keys().find(|k| !names.contains(k)) {
            return Err(SimError::invalid(format!(
                "theta has {extra}, which {} does not have",
                input.model
            )));
        }
        let mut params = Vec::with_capacity(names.len());
        for name in &names {
            let theta = input
                .theta
                .get(name)
                .copied()
                .ok_or_else(|| SimError::invalid(format!("theta is missing {name}")))?;
            if !theta.is_finite() {
                return Err(SimError::invalid(format!(
                    "theta {name} is not a finite number"
                )));
            }
            let effect = input.random_effects.iter().position(|n| n == name);
            if effect.is_some() && theta <= 0.0 {
                return Err(SimError::invalid(format!(
                    "theta {name} must be > 0 because it has a random effect"
                )));
            }
            params.push(ParamPlan {
                name: name.clone(),
                theta,
                effect,
            });
        }

        // Omega and its factor.
        let q = input.random_effects.len();
        let expected = input.omega_shape.count(q);
        if input.omega.len() != expected {
            return Err(SimError::invalid(format!(
                "omega has {} numbers; a {} omega of {q} random effects needs {expected}",
                input.omega.len(),
                input.omega_shape.name()
            )));
        }
        if input.omega.iter().any(|x| !x.is_finite()) {
            return Err(SimError::invalid("omega has a number that is not finite"));
        }
        let matrix = omega::matrix(input.omega_shape, &input.omega, q)
            .ok_or_else(|| SimError::invalid("omega does not fit the random effects"))?;
        for (i, name) in input.random_effects.iter().enumerate() {
            if matrix.get(i * q + i).is_some_and(|v| *v < 0.0) {
                return Err(SimError::invalid(format!(
                    "omega: the variance of random effect {name} is negative"
                )));
            }
        }
        let l = omega::cholesky(&matrix, q).map_err(|(i, s)| {
            let name = input.random_effects.get(i).map_or("?", String::as_str);
            SimError::new(
                SimErrorCode::OmegaNotPositiveDefinite,
                format!(
                    "omega is not positive definite: the Cholesky pivot of random effect {name} is {s} (it must be positive); remove the random effect or correct the variances and covariances (a random effect without variance is the same as no random effect)"
                ),
            )
        })?;

        // PRED at the typical values, once per subject.
        let typical: BTreeMap<String, f64> =
            params.iter().map(|p| (p.name.clone(), p.theta)).collect();
        let mut subjects = Vec::with_capacity(raw.len());
        for r in raw {
            let relative: Vec<f64> = r.times.iter().map(|t| t - r.dose_time).collect();
            let pred = predict(model, &typical, r.dose, &relative).map_err(|e| {
                SimError::new(
                    SimErrorCode::ModelRefused,
                    format!("{e} (in subject {}, at the typical values)", r.label),
                )
            })?;
            subjects.push(SubjectPlan {
                key: LabelKey::of(&r.label),
                label: r.label,
                dose: r.dose,
                dose_time: r.dose_time,
                times: r.times,
                relative,
                pred,
            });
        }

        Ok(Study {
            input: input.clone(),
            seed,
            model,
            params,
            q,
            l,
            error: input.error_model,
            a: values.get("a").copied().unwrap_or(0.0),
            b: values.get("b").copied().unwrap_or(0.0),
            negative: input.negative,
            lloq,
            quantiles,
            replicates: input.replicates,
            subjects,
            n_dv_ignored,
        })
    }

    /// The number of subjects.
    pub fn subject_count(&self) -> usize {
        self.subjects.len()
    }

    /// The number of replicates.
    pub fn replicates(&self) -> u64 {
        self.replicates
    }

    /// The number of cells, replicates times subjects (checked to fit in `usize` by [`Study::new`]).
    pub fn cell_count(&self) -> usize {
        usize::try_from(self.replicates)
            .unwrap_or(usize::MAX)
            .saturating_mul(self.subjects.len())
    }

    /// The label of subject `subject`.
    pub fn label(&self, subject: usize) -> Option<&str> {
        self.subjects.get(subject).map(|s| s.label.as_str())
    }

    /// The lower Cholesky factor of Omega, row by row (q rows of q numbers).
    pub fn factor_rows(&self) -> Vec<Vec<f64>> {
        if self.q == 0 {
            return Vec::new();
        }
        self.l.chunks(self.q).map(<[f64]>::to_vec).collect()
    }

    /// PRED of subject `subject` at each observation row.
    pub fn pred(&self, subject: usize) -> Option<&[f64]> {
        self.subjects.get(subject).map(|s| s.pred.as_slice())
    }

    /// The cell of a flat index `replicate * subjects + subject`.
    pub fn cell_at(&self, index: usize) -> Result<Cell, SimError> {
        let m = self.subjects.len().max(1);
        self.cell((index / m) as u64, index % m)
    }

    /// One cell, drawn from the streams of (seed, replicate, purpose, label). A pure function of its
    /// arguments and the study.
    pub fn cell(&self, replicate: u64, subject: usize) -> Result<Cell, SimError> {
        let plan = self
            .subjects
            .get(subject)
            .ok_or_else(|| SimError::invalid(format!("subject index {subject} is out of range")))?;
        let rep = u32::try_from(replicate)
            .ok()
            .filter(|r| u64::from(*r) < MAX_REPLICATES)
            .ok_or_else(|| SimError::invalid(format!("replicate {replicate} is out of range")))?;
        let z_eta = Stream::new(self.seed, rep, Purpose::Eta, plan.key).normals(self.q);
        let z_eps =
            Stream::new(self.seed, rep, Purpose::Epsilon, plan.key).normals(plan.relative.len());
        self.evaluate(replicate, subject, plan, &z_eta, &z_eps)
    }

    /// The test entry point of SIM-ORA-03: one cell from given standard normals, bypassing the generator.
    pub fn cell_with_draws(
        &self,
        replicate: u64,
        subject: usize,
        z_eta: &[f64],
        z_eps: &[f64],
    ) -> Result<Cell, SimError> {
        let plan = self
            .subjects
            .get(subject)
            .ok_or_else(|| SimError::invalid(format!("subject index {subject} is out of range")))?;
        if z_eta.len() != self.q || z_eps.len() != plan.relative.len() {
            return Err(SimError::invalid(
                "the number of draws does not fit the random effects and the observation rows",
            ));
        }
        self.evaluate(replicate, subject, plan, z_eta, z_eps)
    }

    /// Steps 1 to 6 of SIM-ORD-01 for one subject, from given draws.
    fn evaluate(
        &self,
        replicate: u64,
        subject: usize,
        plan: &SubjectPlan,
        z_eta: &[f64],
        z_eps: &[f64],
    ) -> Result<Cell, SimError> {
        let non_finite = |what: &str, value: f64| {
            SimError::new(
                SimErrorCode::NonFiniteValue,
                format!(
                    "in subject {}, replicate {replicate}, with {what} = {value}: no table receives it",
                    plan.label
                ),
            )
        };
        let eta = omega::apply(&self.l, self.q, z_eta);
        let mut p = BTreeMap::new();
        for param in &self.params {
            let value = match param.effect {
                Some(k) => param.theta * libm::exp(eta.get(k).copied().unwrap_or(0.0)),
                None => param.theta,
            };
            if !value.is_finite() {
                return Err(non_finite(&param.name, value));
            }
            p.insert(param.name.clone(), value);
        }
        let ipred = predict(self.model, &p, plan.dose, &plan.relative).map_err(|e| {
            let with = p
                .iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect::<Vec<_>>()
                .join(", ");
            SimError::new(
                SimErrorCode::ModelRefused,
                format!(
                    "{e} (in subject {}, replicate {replicate}, with {with})",
                    plan.label
                ),
            )
        })?;
        let sigma: Vec<f64> = ipred
            .iter()
            .map(|&c| match self.error {
                ErrorKind::Additive => self.a,
                ErrorKind::Proportional => self.b * c.abs(),
                ErrorKind::Combined => (self.a * self.a + self.b * self.b * c * c).sqrt(),
                ErrorKind::CombinedSd => self.a + self.b * c.abs(),
            })
            .collect();
        let y_raw: Vec<f64> = ipred
            .iter()
            .zip(&sigma)
            .zip(z_eps)
            .map(|((c, s), z)| c + s * z)
            .collect();
        for (what, list) in [("ipred", &ipred), ("sigma", &sigma), ("y", &y_raw)] {
            if let Some(bad) = list.iter().find(|v| !v.is_finite()) {
                return Err(non_finite(what, *bad));
            }
        }
        let mut y = y_raw.clone();
        let mut n_negative = 0;
        for (value, raw) in y.iter_mut().zip(&y_raw) {
            if *raw < 0.0 {
                n_negative += 1;
                if self.negative == NegativeRule::Zero {
                    *value = 0.0;
                }
            }
        }
        // BLQ: strictly below the limit, after the negative rule (SIM-ORD-05).
        let cens: Vec<u8> = y
            .iter()
            .map(|&v| u8::from(self.lloq.is_some_and(|limit| v < limit)))
            .collect();
        let dv: Vec<f64> = y
            .iter()
            .zip(&cens)
            .map(|(&v, &c)| match (c, self.lloq) {
                (1, Some(limit)) => limit,
                _ => v,
            })
            .collect();
        Ok(Cell {
            replicate,
            subject,
            eta,
            p,
            n_zero_sigma: sigma.iter().filter(|s| **s == 0.0).count(),
            n_blq: cens.iter().filter(|c| **c == 1).count(),
            ipred,
            sigma,
            eps: z_eps.to_vec(),
            y_raw,
            y,
            cens,
            dv,
            n_negative,
        })
    }
}

/// The sentence of SIM-REF-01.
fn missing_seed() -> SimError {
    SimError::new(
        SimErrorCode::MissingSeed,
        "a simulation needs a seed: give an integer from 0 to 18446744073709551615 (a seed above 9007199254740991 must be written as a decimal string)",
    )
}

/// The model of an id of SIM-MOD-02: the `pk1.*` and the `pk2.*` ids (clearance set). A `pk3.*` id is
/// refused until section 14 of `specs/models.md` is on the same branch as this engine.
fn parse_model(id: &str) -> Result<ModelId, SimError> {
    if id.starts_with("pk3.") {
        return Err(SimError::invalid(format!(
            "model not available in this version: {id}"
        )));
    }
    ModelId::from_id(id)
        .ok_or_else(|| SimError::invalid(format!("model {id} is not a model of this version")))
}

/// The parameters of a model in the clearance set (`cl, v` or `cl, vc, q, vp`), then its input parameters.
fn parameter_names(model: ModelId) -> Vec<String> {
    let disposition: &[&str] = if model.compartments() == 2 {
        &["cl", "vc", "q", "vp"]
    } else {
        &["cl", "v"]
    };
    disposition
        .iter()
        .chain(model.input_parameters())
        .map(|s| (*s).to_owned())
        .collect()
}

/// The concentrations of the model at the parameters `p`, a dose and the times since the dose.
fn predict(
    model: ModelId,
    p: &BTreeMap<String, f64>,
    dose: f64,
    times: &[f64],
) -> Result<Vec<f64>, caladrius_models::ModelError> {
    let output = caladrius_models::run(&ModelInput {
        model,
        dose,
        params: p.clone(),
        times: times.to_vec(),
    })?;
    Ok(output.conc().to_vec())
}

/// A subject of the design before the model is involved.
struct RawSubject {
    label: String,
    dose: f64,
    dose_time: f64,
    times: Vec<f64>,
}

/// The subjects of a design in order of first appearance, with the refusals of SIM-REF-04 and
/// `label_key_collision`; and the number of observation rows whose value cell was filled.
fn plan_design(design: &Design, strict_times: bool) -> Result<(Vec<RawSubject>, usize), SimError> {
    let mut ignored = 0;
    let subjects: Vec<RawSubject> = match design {
        Design::Subjects(list) => {
            for (i, s) in list.iter().enumerate() {
                if list.iter().take(i).any(|o| o.label == s.label) {
                    return Err(SimError::invalid(format!(
                        "subject {} appears twice in the design: give each subject once",
                        s.label
                    )));
                }
            }
            list.iter()
                .map(|s| RawSubject {
                    label: s.label.clone(),
                    dose: s.dose,
                    dose_time: s.dose_time,
                    times: s.times.clone(),
                })
                .collect()
        }
        Design::Grid { times, dose, m } => {
            if *m < 1 {
                return Err(SimError::invalid("m must be an integer from 1"));
            }
            (1..=*m)
                .map(|i| RawSubject {
                    label: i.to_string(),
                    dose: *dose,
                    dose_time: 0.0,
                    times: times.clone(),
                })
                .collect()
        }
        Design::Table(rows) => {
            // A label in two separate blocks is one subject whose rows are gathered in table order.
            let mut order: Vec<&str> = Vec::new();
            let mut by: BTreeMap<&str, Vec<&super::input::TableRow>> = BTreeMap::new();
            for row in rows {
                let entry = by.entry(row.subject.as_str()).or_default();
                if entry.is_empty() {
                    order.push(row.subject.as_str());
                }
                entry.push(row);
            }
            let mut out = Vec::new();
            for id in order {
                let mut dose = None;
                let mut times = Vec::new();
                for row in by.get(id).map_or(&[][..], Vec::as_slice) {
                    if !row.time.is_finite() {
                        return Err(SimError::invalid(format!(
                            "a time in subject {id} is not a finite number"
                        )));
                    }
                    match row.kind {
                        RowKind::Dose => {
                            if dose.is_some() {
                                return Err(SimError::new(
                                    SimErrorCode::MoreThanOneDose,
                                    format!(
                                        "subject {id} has more than one dose row: multiple dosing is not simulated in this version"
                                    ),
                                ));
                            }
                            let amt = row.amt.ok_or_else(|| {
                                SimError::new(
                                    SimErrorCode::DoseWithoutAmount,
                                    format!("the dose row of subject {id} has no amount"),
                                )
                            })?;
                            dose = Some((amt, row.time));
                        }
                        RowKind::Observation => {
                            if row.amt.is_some() {
                                return Err(SimError::new(
                                    SimErrorCode::AmountOnObservation,
                                    format!(
                                        "a row of subject {id} is an observation but has an amount"
                                    ),
                                ));
                            }
                            if row.dv_given {
                                ignored += 1;
                            }
                            times.push(row.time);
                        }
                    }
                }
                let (dose, dose_time) =
                    dose.ok_or_else(|| SimError::invalid(format!("subject {id} has no dose row")))?;
                out.push(RawSubject {
                    label: id.to_owned(),
                    dose,
                    dose_time,
                    times,
                });
            }
            out
        }
    };
    if subjects.is_empty() {
        return Err(SimError::invalid("the design has no subject"));
    }
    for s in &subjects {
        if !s.dose.is_finite() {
            return Err(SimError::invalid(format!(
                "the dose of subject {} is not a finite number",
                s.label
            )));
        }
        if !s.dose_time.is_finite() || s.times.iter().any(|t| !t.is_finite()) {
            return Err(SimError::invalid(format!(
                "a time in subject {} is not a finite number",
                s.label
            )));
        }
        check_times(&s.label, &s.times, strict_times)?;
    }
    let mut seen: BTreeMap<LabelKey, &str> = BTreeMap::new();
    for s in &subjects {
        let key = LabelKey::of(&s.label);
        if let Some(other) = seen.insert(key, &s.label) {
            return Err(SimError::new(
                SimErrorCode::LabelKeyCollision,
                format!(
                    "subjects {other} and {} have the same 64-bit stream key ({}); rename one",
                    s.label,
                    key.hex()
                ),
            ));
        }
    }
    Ok((subjects, ignored))
}

/// The observation times of one subject: at least one and, when `strict`, not going backwards and no two
/// equal (DAT-ORD-01).
fn check_times(label: &str, times: &[f64], strict: bool) -> Result<(), SimError> {
    if times.is_empty() {
        return Err(SimError::new(
            SimErrorCode::SubjectWithoutObservation,
            format!("subject {label} has no observation row: nothing would be simulated for it"),
        ));
    }
    for w in times.windows(2).filter(|_| strict) {
        let (earlier, later) = match w {
            [a, b] => (*a, *b),
            _ => continue,
        };
        if later < earlier {
            return Err(SimError::new(
                SimErrorCode::TimeNotIncreasing,
                format!("time goes backwards in subject {label}: {later} after {earlier}"),
            ));
        }
        if later == earlier {
            return Err(SimError::new(
                SimErrorCode::DuplicateTimes,
                format!("subject {label} has two observations at time {earlier}"),
            ));
        }
    }
    Ok(())
}
