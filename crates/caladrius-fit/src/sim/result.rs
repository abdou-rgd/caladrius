//! The tables returned (SIM-OUT-01, -02), the summary over replicates (SIM-SUM-01) and the canonical text
//! that is hashed for SIM-DET-02.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use super::error::SimError;
use super::input::SimInput;
use super::study::{Cell, RNG_VERSION, Study};

/// One row of the subject table: a (replicate, subject).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubjectRow {
    /// The replicate.
    pub rep: u64,
    /// The label.
    pub id: String,
    /// The random effects, in the order of `random_effects`.
    pub eta: Vec<f64>,
    /// The individual parameters P_k, all of them.
    pub p: BTreeMap<String, f64>,
}

/// One row of the observation table: a (replicate, subject, observation row).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObservationRow {
    /// The replicate.
    pub rep: u64,
    /// The label.
    pub id: String,
    /// The observation row of the subject, from 1.
    pub obs: usize,
    /// The time as the design holds it.
    pub time: f64,
    /// The prediction at the typical values.
    pub pred: f64,
    /// The individual prediction.
    pub ipred: f64,
    /// The standard deviation of the observation.
    pub sigma: f64,
    /// The standard normal of the residual error.
    pub eps: f64,
    /// y before the negative rule.
    pub y_raw: f64,
    /// y after the negative rule.
    pub y: f64,
    /// 1 on a BLQ row.
    pub cens: u8,
    /// The value written to the dataset.
    pub dv: f64,
}

/// One row of the dataset of SIM-OUT-02, the event layout of `specs/data.md`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DatasetRow {
    /// `{rep}:{label}`: each replicate's subjects are distinct subjects of the dataset.
    pub subject: String,
    /// The original label, kept in a column that no role of the dataset reader reads.
    pub label: String,
    /// The time.
    pub time: f64,
    /// `dose` or `obs`.
    pub event: &'static str,
    /// The amount of a dose row.
    pub amt: Option<f64>,
    /// The value of an observation row (the limit on a BLQ row).
    pub dv: Option<f64>,
    /// 1 on a censored observation row, 0 otherwise; empty on a dose row.
    pub cens: Option<u8>,
}

/// The summary of one observation row of one subject over the replicates (SIM-SUM-01).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SummaryRow {
    /// The label.
    pub id: String,
    /// The observation row, from 1.
    pub obs: usize,
    /// The time.
    pub time: f64,
    /// The quantiles of y (type 7), one per probability.
    pub y: Vec<f64>,
    /// The quantiles of ipred (type 7), one per probability.
    pub ipred: Vec<f64>,
    /// The share of replicates in which the row is BLQ.
    pub blq_fraction: f64,
}

/// The counts of a run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Counts {
    /// Subjects.
    pub subjects: usize,
    /// Replicates.
    pub replicates: u64,
    /// Observation rows over all replicates.
    pub rows: usize,
    /// Rows of the design whose value cell was filled and ignored.
    pub n_dv_ignored: usize,
    /// Rows with sigma = 0.
    pub n_zero_sigma: usize,
    /// Rows with raw y below 0.
    pub n_negative: usize,
    /// Rows below the LLOQ.
    pub n_blq: usize,
    /// `n_negative` per subject label.
    pub n_negative_by_subject: BTreeMap<String, usize>,
    /// `n_blq` per subject label.
    pub n_blq_by_subject: BTreeMap<String, usize>,
}

/// The options as they were used, defaults included.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsedOptions {
    /// `keep` or `zero`.
    pub negative: &'static str,
    /// The limit, if any.
    pub lloq: Option<f64>,
    /// The probabilities of the summary.
    pub quantiles: Vec<f64>,
}

/// A whole simulation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SimResult {
    /// The version of the generator (`philox4x32-10/as241/v1`, SIM-DET-03).
    pub rng: &'static str,
    /// The model id as given.
    pub model: String,
    /// The seed as a decimal string, whatever its input form (SIM-RNG-02).
    pub seed: String,
    /// The options used.
    pub options: UsedOptions,
    /// The call, repeated so that the run can be replayed and sealed (SIM-MOD-05).
    pub input: SimInput,
    /// The subject table.
    pub subjects: Vec<SubjectRow>,
    /// The observation table.
    pub observations: Vec<ObservationRow>,
    /// The counts.
    pub counts: Counts,
    /// The summary over replicates.
    pub summary: Vec<SummaryRow>,
    /// The simulated dataset, all replicates in one table.
    pub dataset: Vec<DatasetRow>,
}

impl Study {
    /// Runs every cell in index order on the calling thread and assembles the result.
    pub fn simulate(&self) -> Result<SimResult, SimError> {
        self.refuse_if_draws_only()?;
        let cells = (0..self.cell_count())
            .map(|index| self.cell_at(index))
            .collect::<Result<Vec<_>, _>>()?;
        self.assemble(&cells)
    }

    /// Builds the tables and the summary from the cells of every (replicate, subject), given in index order
    /// (`index = replicate * subjects + subject`, SIM-DET-01). The result does not depend on how the cells
    /// were computed.
    pub fn assemble(&self, cells: &[Cell]) -> Result<SimResult, SimError> {
        let m = self.subjects.len();
        if cells.len() != self.cell_count() {
            return Err(SimError::invalid(format!(
                "assembly needs {} cells, got {}",
                self.cell_count(),
                cells.len()
            )));
        }
        for (index, cell) in cells.iter().enumerate() {
            if cell.replicate != (index / m.max(1)) as u64 || cell.subject != index % m.max(1) {
                return Err(SimError::invalid(
                    "the cells are not in the order replicate, then subject",
                ));
            }
        }
        let mut subjects = Vec::with_capacity(cells.len());
        let mut observations = Vec::new();
        let mut dataset = Vec::new();
        let (mut n_zero, mut n_neg, mut n_blq, mut rows) = (0, 0, 0, 0);
        let mut neg_by: BTreeMap<String, usize> = BTreeMap::new();
        let mut blq_by: BTreeMap<String, usize> = BTreeMap::new();
        for cell in cells {
            let plan = self.subjects.get(cell.subject).ok_or_else(|| {
                SimError::invalid("a cell names a subject that is not in the design")
            })?;
            let column = format!("{}:{}", cell.replicate, plan.label);
            subjects.push(SubjectRow {
                rep: cell.replicate,
                id: plan.label.clone(),
                eta: cell.eta.clone(),
                p: cell.p.clone(),
            });
            dataset.push(DatasetRow {
                subject: column.clone(),
                label: plan.label.clone(),
                time: plan.dose_time,
                event: "dose",
                amt: Some(plan.dose),
                dv: None,
                cens: None,
            });
            for (j, time) in plan.times.iter().enumerate() {
                let at = |v: &[f64]| v.get(j).copied().unwrap_or(f64::NAN);
                let cens = cell.cens.get(j).copied().unwrap_or(0);
                let dv = at(&cell.dv);
                observations.push(ObservationRow {
                    rep: cell.replicate,
                    id: plan.label.clone(),
                    obs: j + 1,
                    time: *time,
                    pred: at(&plan.pred),
                    ipred: at(&cell.ipred),
                    sigma: at(&cell.sigma),
                    eps: at(&cell.eps),
                    y_raw: at(&cell.y_raw),
                    y: at(&cell.y),
                    cens,
                    dv,
                });
                dataset.push(DatasetRow {
                    subject: column.clone(),
                    label: plan.label.clone(),
                    time: *time,
                    event: "obs",
                    amt: None,
                    dv: Some(dv),
                    cens: Some(cens),
                });
            }
            n_zero += cell.n_zero_sigma;
            n_neg += cell.n_negative;
            n_blq += cell.n_blq;
            rows += plan.times.len();
            *neg_by.entry(plan.label.clone()).or_default() += cell.n_negative;
            *blq_by.entry(plan.label.clone()).or_default() += cell.n_blq;
        }
        let reps = usize::try_from(self.replicates).unwrap_or(usize::MAX);
        let mut summary = Vec::new();
        for (s, plan) in self.subjects.iter().enumerate() {
            for (j, time) in plan.times.iter().enumerate() {
                let column = |pick: &dyn Fn(&Cell) -> f64| -> Vec<f64> {
                    (0..reps)
                        .filter_map(|r| cells.get(r * m + s))
                        .map(pick)
                        .collect()
                };
                let ys = column(&|c| c.y.get(j).copied().unwrap_or(f64::NAN));
                let ips = column(&|c| c.ipred.get(j).copied().unwrap_or(f64::NAN));
                let blq = (0..reps)
                    .filter_map(|r| cells.get(r * m + s))
                    .filter(|c| c.cens.get(j) == Some(&1))
                    .count();
                let quantiles = |x: &[f64]| -> Vec<f64> {
                    self.quantiles
                        .iter()
                        .filter_map(|&p| quantile7(x, p))
                        .collect()
                };
                summary.push(SummaryRow {
                    id: plan.label.clone(),
                    obs: j + 1,
                    time: *time,
                    y: quantiles(&ys),
                    ipred: quantiles(&ips),
                    blq_fraction: blq as f64 / self.replicates as f64,
                });
            }
        }
        Ok(SimResult {
            rng: RNG_VERSION,
            model: self.input.model.clone(),
            seed: self.seed.text(),
            options: UsedOptions {
                negative: self.negative.name(),
                lloq: self.lloq,
                quantiles: self.quantiles.clone(),
            },
            input: self.input.clone(),
            subjects,
            observations,
            counts: Counts {
                subjects: m,
                replicates: self.replicates,
                rows,
                n_dv_ignored: self.n_dv_ignored,
                n_zero_sigma: n_zero,
                n_negative: n_neg,
                n_blq,
                n_negative_by_subject: neg_by,
                n_blq_by_subject: blq_by,
            },
            summary,
            dataset,
        })
    }
}

/// The quantile of type 7 (SIM-SUM-01): h = 1 + (n - 1) p, lo = floor h, hi = ceil h, g = h - lo,
/// Q = (1 - g) x_(lo) + g x_(hi), in exactly this form. `None` for no value or p outside [0, 1].
pub fn quantile7(x: &[f64], p: f64) -> Option<f64> {
    if x.is_empty() || !(0.0..=1.0).contains(&p) {
        return None;
    }
    let mut sorted = x.to_vec();
    sorted.sort_by(f64::total_cmp);
    let h = 1.0 + (sorted.len() as f64 - 1.0) * p;
    let (lo, hi) = (h.floor(), h.ceil());
    let g = h - lo;
    let at = |position: f64| sorted.get((position as usize).checked_sub(1)?).copied();
    Some((1.0 - g) * at(lo)? + g * at(hi)?)
}

impl SimResult {
    /// The canonical text of SIM-DET-02: the tables of SIM-OUT-01 and the summary, one row per line, numbers
    /// as the shortest decimal that reads back to the same double. This is what is hashed and compared on
    /// every target.
    pub fn canonical_text(&self) -> String {
        let num = |x: f64| format!("{x:?}");
        let list = |v: &[f64]| v.iter().map(|x| num(*x)).collect::<Vec<_>>().join(",");
        let mut out = String::new();
        let _ = writeln!(
            out,
            "caladrius-sim rng={} model={} seed={}",
            self.rng, self.model, self.seed
        );
        let _ = writeln!(
            out,
            "options negative={} lloq={} quantiles={}",
            self.options.negative,
            self.options.lloq.map_or_else(|| "none".to_owned(), num),
            list(&self.options.quantiles)
        );
        let c = &self.counts;
        let _ = writeln!(
            out,
            "counts subjects={} replicates={} rows={} n_dv_ignored={} n_zero_sigma={} n_negative={} n_blq={}",
            c.subjects, c.replicates, c.rows, c.n_dv_ignored, c.n_zero_sigma, c.n_negative, c.n_blq
        );
        let _ = writeln!(out, "subjects");
        for r in &self.subjects {
            let p =
                r.p.iter()
                    .map(|(k, v)| format!("{k}={}", num(*v)))
                    .collect::<Vec<_>>()
                    .join(",");
            let _ = writeln!(out, "{}|{}|{}|{p}", r.rep, r.id, list(&r.eta));
        }
        let _ = writeln!(out, "observations");
        for o in &self.observations {
            let _ = writeln!(
                out,
                "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                o.rep,
                o.id,
                o.obs,
                num(o.time),
                num(o.pred),
                num(o.ipred),
                num(o.sigma),
                num(o.eps),
                num(o.y_raw),
                num(o.y),
                o.cens,
                num(o.dv)
            );
        }
        let _ = writeln!(out, "summary");
        for s in &self.summary {
            let _ = writeln!(
                out,
                "{}|{}|{}|{}|{}|{}",
                s.id,
                s.obs,
                num(s.time),
                list(&s.y),
                list(&s.ipred),
                num(s.blq_fraction)
            );
        }
        out
    }
}
