//! `cargo xtask conformance`: runs `caladrius-nca` on every oracle case of `oracle/expected/` and
//! writes `docs/conformance.md`, the validated parameters per case (AGENTS.md section 5).
//!
//! A value is validated when it is within `Tolerance::NCA_VS_PKNCA` of the expected one, or when
//! it is expected as not available and the engine computes the parameter and reports it as not
//! available (a parameter the engine does not compute never matches). The file records a floor per
//! parameter per case: the number of validated values and the number of expected rows. A run
//! fails, and leaves the file unchanged, when a parameter validates fewer values, loses expected
//! rows, gains unvalidated rows, or has disappeared; when a floor line is malformed; or when a
//! non-empty previous file holds no floor at all. So floors only go up (golden rule 2). The file
//! is written to a temporary file then renamed.
//!
//! A value covered by a documented difference (`documented_differences` in the case's options file,
//! `specs/differences.md`) is neither validated nor a failure: it is counted in its own column and
//! left out of the validated and expected totals. A difference cannot hide a regression: moving a
//! row from validated to documented lowers the validated floor and fails the run.
//!
//! A value the oracle writes but whose tolerance is an open question (the `scalar_ill` rows of the
//! three-compartment cases, held by Q-024) is *held*: counted in its own column, in neither the
//! validated nor the expected totals, and never a failure. The floors count it with the other rows
//! the oracle has, so that a held row cannot be dropped silently.
//!
//! A value of a free-zone case of the three-compartment oracle (`Pk3Kind::Free`, zones (0) and (iii)
//! of MOD-3C-08) that the engine does not produce because it refused the macro input, which the
//! spec allows, is *refused*: counted in its own column, in neither the validated nor the expected
//! totals, and not a failure either. The floors count it with the other rows, so that a case the
//! engine used to accept and now refuses fails on its validated floor, and a refused row cannot be
//! dropped silently.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use caladrius_nca::{
    AucMethod, BlqAction, BlqPolicy, LambdaZOptions, MissingPolicy, NcaInput, NcaOptions,
    NcaResult, NegativePolicy, Route, StartPolicy, TmaxTie, run as run_nca,
};
use caladrius_testkit::oracle::{BlqRule, NaRule};
use caladrius_testkit::{OracleCase, Tolerance, list_cases, load_case};

pub(crate) mod step3;

use crate::console;
use crate::error::{Result, XtaskError};
use crate::workspace;

/// Output file, relative to the workspace root.
const OUTPUT: &str = "docs/conformance.md";
/// Marker of a floor line in the output file:
/// `<!-- floor <case> <parameter> <validated> <expected> <documented> [<held> [<refused>]] -->` (the documented
/// count is optional when reading, for files written before documented differences existed; the
/// held count is written only when it is not zero or when a refused count follows, and the refused
/// count `<refused>` after it only when it is not zero).
const FLOOR_MARK: &str = "<!-- floor ";
/// Closing of a floor line.
const FLOOR_END: &str = "-->";

/// Validated, expected, documented and held counts of one parameter. `expected` leaves out the
/// rows covered by a documented difference, which are counted in `documented` instead, and the
/// rows held by an open question, which are counted in `held`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Count {
    validated: usize,
    expected: usize,
    documented: usize,
    held: usize,
    /// Rows of a free-zone case the engine refused (an outcome the spec allows): not compared.
    refused: usize,
}

impl Count {
    fn plus(self, other: Count) -> Count {
        Count {
            validated: self.validated + other.validated,
            expected: self.expected + other.expected,
            documented: self.documented + other.documented,
            held: self.held + other.held,
            refused: self.refused + other.refused,
        }
    }

    /// Every row the oracle has for this parameter, whatever became of it.
    fn rows(self) -> usize {
        self.expected + self.documented + self.held + self.refused
    }
}

/// The three kinds of oracle case (`oracle/README.md`), in the order of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// NCA against PKNCA (`oracle/expected/*.csv`).
    Nca,
    /// Closed-form model values (`oracle/expected/models/`).
    Model,
    /// Weighted least-squares fits (`oracle/expected/fit/`).
    Fit,
}

impl Kind {
    const ALL: [Kind; 3] = [Kind::Nca, Kind::Model, Kind::Fit];

    fn title(self) -> &'static str {
        match self {
            Kind::Nca => "NCA against PKNCA (relative error 1e-6)",
            Kind::Model => {
                "Models against exact closed-form values and derivatives (relative error 1e-12), and the inputs they must refuse"
            }
            Kind::Fit => {
                "Weighted least-squares fits against reference fits (1e-4 on estimates and statistics, 1e-6 on the residual sum of squares)"
            }
        }
    }

    /// Columns of the summary table that describe the case.
    fn headers(self) -> &'static [&'static str] {
        match self {
            Kind::Nca => &["route", "AUC method"],
            Kind::Model => &["model"],
            Kind::Fit => &["model", "weighting", "subjects"],
        }
    }

    /// What a "parameter" of the per-case table is.
    fn parameter_title(self) -> &'static str {
        match self {
            Kind::Nca => "parameter",
            Kind::Model => "quantity",
            Kind::Fit => "group",
        }
    }

    /// The plural of `parameter_title`, for the summary table header.
    fn parameter_plural(self) -> &'static str {
        match self {
            Kind::Nca => "parameters",
            Kind::Model => "quantities",
            Kind::Fit => "groups",
        }
    }
}

/// The outcome of one oracle case.
#[derive(Debug, Clone, PartialEq)]
struct CaseReport {
    kind: Kind,
    name: String,
    /// The cells of the kind's summary table after the case name (`Kind::headers`).
    details: Vec<String>,
    /// Per parameter, in the order of the case's parameter list.
    parameters: Vec<(String, Count)>,
    /// Subjects the engine refused, with the error message.
    errors: Vec<String>,
}

impl CaseReport {
    fn total(&self) -> Count {
        self.parameters
            .iter()
            .fold(Count::default(), |acc, (_, c)| acc.plus(*c))
    }
}

fn percent(count: Count) -> f64 {
    if count.expected == 0 {
        100.0
    } else {
        100.0 * count.validated as f64 / count.expected as f64
    }
}

fn blq_action(rule: BlqRule) -> BlqAction {
    match rule {
        BlqRule::Keep => BlqAction::Keep,
        BlqRule::Drop => BlqAction::Drop,
        BlqRule::Set(x) => BlqAction::Set(x),
    }
}

/// The engine options matching the PKNCA options recorded with the case, and the engine hints the
/// case file carries for what PKNCA has no option for (start and negative-value policies).
pub(crate) fn options_of(case: &OracleCase) -> Result<NcaOptions> {
    let pk = &case.options.pknca_options;
    let unknown = |what: &str, value: &str| {
        XtaskError::new(format!(
            "{}: unknown {what} {value:?} in the oracle options",
            case.name
        ))
    };
    let incomplete = || {
        XtaskError::new(format!(
            "{}: the BLQ rule of the oracle options names neither first/middle/last nor before/after Tmax completely",
            case.name
        ))
    };
    let auc_method = match pk.auc_method.as_str() {
        "linear" => AucMethod::Linear,
        "lin up/log down" => AucMethod::LinUpLogDown,
        other => return Err(unknown("AUC method", other)),
    };
    let missing = match pk.conc_na {
        NaRule::Drop => MissingPolicy::Drop,
        NaRule::Replace(x) => MissingPolicy::Replace(x),
    };
    let rule = &pk.conc_blq;
    let blq = match (
        rule.first,
        rule.middle,
        rule.last,
        rule.before_tmax,
        rule.after_tmax,
    ) {
        (Some(first), Some(middle), Some(last), None, None) => BlqPolicy::Position {
            first: blq_action(first),
            middle: blq_action(middle),
            last: blq_action(last),
        },
        (None, None, None, Some(before), Some(after)) => BlqPolicy::Tmax {
            before: blq_action(before),
            after: blq_action(after),
        },
        _ => return Err(incomplete()),
    };
    let hints = &case.options.engine;
    let start = match hints.start.as_deref() {
        None => StartPolicy::default(),
        Some("none") => StartPolicy::None,
        Some("zero") => StartPolicy::Zero,
        Some("c0") => StartPolicy::C0,
        Some(other) => return Err(unknown("start policy", other)),
    };
    let negative = match hints.negative.as_deref() {
        None => NegativePolicy::default(),
        Some("error") => NegativePolicy::Error,
        Some("allow") => NegativePolicy::Allow,
        Some("set_zero") => NegativePolicy::SetZero,
        Some(other) => return Err(unknown("negative-value policy", other)),
    };
    Ok(NcaOptions {
        auc_method,
        missing,
        blq,
        start,
        negative,
        tmax_tie: if pk.first_tmax {
            TmaxTie::First
        } else {
            TmaxTie::Last
        },
        lambda_z: LambdaZOptions {
            min_points: usize::try_from(pk.min_hl_points).unwrap_or(usize::MAX),
            allow_tmax: pk.allow_tmax_in_half_life,
            adj_r_squared_factor: pk.adj_r_squared_factor,
        },
        ..NcaOptions::default()
    })
}

pub(crate) fn route_of(case: &OracleCase) -> Result<Route> {
    match (case.options.route.as_str(), case.options.infusion_duration) {
        ("extravascular", _) => Ok(Route::Extravascular),
        ("iv_bolus", _) => Ok(Route::IvBolus),
        ("iv_infusion", Some(duration)) => Ok(Route::IvInfusion { duration }),
        ("iv_infusion", None) => Err(XtaskError::new(format!(
            "{}: route iv_infusion without an infusion_duration in the oracle options",
            case.name
        ))),
        (other, _) => Err(XtaskError::new(format!(
            "{}: unknown route {other:?} in the oracle options",
            case.name
        ))),
    }
}

/// Whether one expected row is reproduced. `result` is the engine's result for the subject, `None`
/// when the engine refused the subject. A value expected as not available only matches when the
/// engine computes the parameter (`parameter(name)` is `Some`) and reports it as not available.
fn row_validated(expected: Option<f64>, result: Option<&NcaResult>, name: &str) -> bool {
    let Some(result) = result else {
        return false;
    };
    match (expected, result.parameter(name)) {
        (_, None) => false,
        (Some(e), Some(p)) => p
            .value()
            .is_some_and(|a| Tolerance::NCA_VS_PKNCA.accepts(a, e)),
        (None, Some(p)) => p.value().is_none(),
    }
}

/// Runs the engine on every subject of `case` and counts the validated values per parameter.
fn evaluate(case: &OracleCase) -> Result<CaseReport> {
    let options = options_of(case)?;
    let route = route_of(case)?;
    let mut results = BTreeMap::new();
    let mut errors = Vec::new();
    for profile in &case.dataset.profiles {
        let input = NcaInput {
            time: profile.time.clone(),
            conc: profile.conc.clone(),
            dose: profile.dose,
            route,
            options: options.clone(),
        };
        match run_nca(&input) {
            Ok(result) => {
                results.insert(profile.subject.to_string(), result);
            }
            Err(e) => errors.push(format!("subject {}: {e}", profile.subject)),
        }
    }
    let mut counts: BTreeMap<&str, Count> = BTreeMap::new();
    for (subject, name, expected) in case.expected.iter() {
        let count = counts.entry(name).or_default();
        if case
            .options
            .documented_differences
            .iter()
            .any(|d| d.covers(subject, name))
        {
            count.documented += 1;
            continue;
        }
        let ok = row_validated(expected, results.get(subject), name);
        count.expected += 1;
        count.validated += usize::from(ok);
    }
    let mut seen = std::collections::BTreeSet::new();
    let parameters = case
        .options
        .parameters
        .iter()
        .filter(|p| seen.insert(p.as_str()))
        .map(|p| {
            (
                p.clone(),
                counts.get(p.as_str()).copied().unwrap_or_default(),
            )
        })
        .collect();
    Ok(CaseReport {
        kind: Kind::Nca,
        name: case.name.clone(),
        details: vec![
            case.options.route.clone(),
            case.options.pknca_options.auc_method.clone(),
        ],
        parameters,
        errors,
    })
}

/// Floors recorded in a previous output.
#[derive(Debug, Default, PartialEq, Eq)]
struct Floors {
    /// Per case and parameter: the validated and expected counts.
    parameters: BTreeMap<(String, String), Count>,
    /// Per case: the total validated count, from the first file format (one total per case). Still
    /// enforced, then replaced by per-parameter floors when the file is rewritten.
    legacy_totals: BTreeMap<String, usize>,
}

impl Floors {
    fn is_empty(&self) -> bool {
        self.parameters.is_empty() && self.legacy_totals.is_empty()
    }
}

/// Reads the floors of a previous output. A line that starts a floor but does not parse is an
/// error, and so is a non-empty text without any floor: a floor must never vanish silently.
fn parse_floors(text: &str) -> Result<Floors> {
    let mut floors = Floors::default();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with(FLOOR_MARK) {
            continue;
        }
        let bad = |why: &str| {
            XtaskError::new(format!(
                "{OUTPUT} line {}: malformed floor line ({why}): {line:?}; expected \
`{FLOOR_MARK}<case> <parameter> <validated> <expected> <documented> [<held> [<refused>]] {FLOOR_END}`. Fix or restore the file \
from version control; floors are not dropped silently",
                index + 1
            ))
        };
        let body = line
            .strip_prefix(FLOOR_MARK)
            .and_then(|rest| rest.strip_suffix(FLOOR_END))
            .ok_or_else(|| bad("missing closing marker"))?;
        let words: Vec<&str> = body.split_whitespace().collect();
        let number = |word: &str| {
            word.parse::<usize>()
                .map_err(|_| bad("count is not a whole number"))
        };
        match words.as_slice() {
            [case, parameter, validated, expected]
            | [case, parameter, validated, expected, _]
            | [case, parameter, validated, expected, _, _]
            | [case, parameter, validated, expected, _, _, _] => {
                let count = Count {
                    validated: number(validated)?,
                    expected: number(expected)?,
                    documented: match words.get(4) {
                        Some(word) => number(word)?,
                        None => 0,
                    },
                    held: match words.get(5) {
                        Some(word) => number(word)?,
                        None => 0,
                    },
                    refused: match words.get(6) {
                        Some(word) => number(word)?,
                        None => 0,
                    },
                };
                if count.validated > count.expected {
                    return Err(bad("validated is larger than expected"));
                }
                let key = ((*case).to_owned(), (*parameter).to_owned());
                if floors.parameters.insert(key, count).is_some() {
                    return Err(bad("duplicated floor"));
                }
            }
            [case, validated] => {
                if floors
                    .legacy_totals
                    .insert((*case).to_owned(), number(validated)?)
                    .is_some()
                {
                    return Err(bad("duplicated floor"));
                }
            }
            _ => return Err(bad("wrong number of fields")),
        }
    }
    if floors.is_empty() && !text.trim().is_empty() {
        return Err(XtaskError::new(format!(
            "{OUTPUT} is not empty but holds no floor line; refusing to regenerate it without \
floors. Restore the file from version control"
        )));
    }
    Ok(floors)
}

/// Every floor must still be met: the case and the parameter exist, the parameter validates at
/// least as many values, keeps at least as many expected rows, and has no more unvalidated rows.
fn check_floors(floors: &Floors, reports: &[CaseReport]) -> Result<()> {
    let mut drops = Vec::new();
    for ((case, parameter), floor) in &floors.parameters {
        let Some(report) = reports.iter().find(|r| &r.name == case) else {
            drops.push(format!("case {case} is no longer in oracle/"));
            continue;
        };
        let Some((_, count)) = report.parameters.iter().find(|(name, _)| name == parameter) else {
            drops.push(format!(
                "{case}: parameter {parameter} is no longer checked"
            ));
            continue;
        };
        if count.validated < floor.validated {
            drops.push(format!(
                "{case}: {parameter} validates {} values, floor is {}",
                count.validated, floor.validated
            ));
        }
        // Rows documented as differences or held by a question are still rows of the oracle.
        if count.rows() < floor.rows() {
            drops.push(format!(
                "{case}: {parameter} has {} expected rows, floor is {}",
                count.rows(),
                floor.rows()
            ));
        }
        let unvalidated = count.expected.saturating_sub(count.validated);
        let floor_unvalidated = floor.expected.saturating_sub(floor.validated);
        if unvalidated > floor_unvalidated {
            drops.push(format!(
                "{case}: {parameter} has {unvalidated} unvalidated rows, floor allows {floor_unvalidated}"
            ));
        }
    }
    for (case, &floor) in &floors.legacy_totals {
        match reports.iter().find(|r| &r.name == case) {
            None => drops.push(format!("case {case} is no longer in oracle/")),
            Some(report) => {
                let validated = report.total().validated;
                if validated < floor {
                    drops.push(format!(
                        "{case}: {validated} values validated, floor is {floor}"
                    ));
                }
            }
        }
    }
    drops.sort();
    drops.dedup();
    if drops.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::new(format!(
            "conformance went down; {OUTPUT} is left unchanged:\n  {}",
            drops.join("\n  ")
        )))
    }
}

/// The Markdown document. Deterministic: no date, cases and parameters in a fixed order.
fn render(reports: &[CaseReport]) -> String {
    let mut out = String::new();
    let sum = |rs: &mut dyn Iterator<Item = &CaseReport>| {
        rs.fold(Count::default(), |acc, r| acc.plus(r.total()))
    };
    let all = sum(&mut reports.iter());
    out.push_str("# Conformance\n\n");
    out.push_str(
        "Generated by `cargo xtask conformance`; never edit by hand. Each value of `oracle/expected/` (NCA against PKNCA), `oracle/expected/models/` (closed-form model values; `pk2/` and `pk3/`: two- and three-compartment values, partial derivatives and the inputs the engine must refuse) and `oracle/expected/fit/` (reference weighted least-squares fits) is compared with the engine within the tolerance named for its kind below; a value expected as not available must be not available. For the fits, the entries of the per-case tables are the groups of the oracle tests (estimates, residual sum of squares, statistics, confidence limits, information criteria, secondary parameters) and the convergence status. A value covered by a documented difference (`specs/differences.md`) is neither validated nor a failure: it has its own column and is left out of the totals. A value held by an open question of `board/QUESTIONS.md` is the same: it is listed as held, and left out of the totals. The floors (last lines: validated, expected, documented and, when there are any, held rows per parameter per case) can only go up.\n\n",
    );
    out.push_str(&format!(
        "**Overall: {} of {} values validated ({:.1} %).**",
        all.validated,
        all.expected,
        percent(all)
    ));
    if all.documented > 0 {
        out.push_str(&format!(
            " {} further values are documented differences, neither validated nor failures.",
            all.documented
        ));
    }
    if all.held > 0 {
        out.push_str(&format!(
            " {} further values are held by an open question (column `held`), neither validated nor failures.",
            all.held
        ));
    }
    if all.refused > 0 {
        out.push_str(&format!(
            " {} further values are on free-zone cases that the engine refused (column `refused`), neither validated nor failures.",
            all.refused
        ));
    }
    out.push_str("\n\n");
    for kind in Kind::ALL {
        let of_kind: Vec<&CaseReport> = reports.iter().filter(|r| r.kind == kind).collect();
        if of_kind.is_empty() {
            continue;
        }
        let t = sum(&mut of_kind.iter().copied());
        out.push_str(&format!(
            "## {}\n\n{} of {} values validated ({:.1} %) on {} cases.\n\n",
            kind.title(),
            t.validated,
            t.expected,
            percent(t),
            of_kind.len()
        ));
        out.push_str(&format!(
            "| case | {} | {} fully validated | values validated | % | documented differences |\n",
            kind.headers().join(" | "),
            kind.parameter_plural()
        ));
        out.push_str(&format!(
            "|---|{}---|---|---|---|\n",
            "---|".repeat(kind.headers().len())
        ));
        for r in &of_kind {
            let t = r.total();
            let full = r
                .parameters
                .iter()
                .filter(|(_, c)| c.validated == c.expected)
                .count();
            out.push_str(&format!(
                "| {} | {} | {} / {} | {} / {} | {:.1} | {} |\n",
                r.name,
                r.details.join(" | "),
                full,
                r.parameters.len(),
                t.validated,
                t.expected,
                percent(t),
                t.documented
            ));
        }
        out.push('\n');
        let held: Vec<(&str, usize)> = of_kind
            .iter()
            .map(|r| (r.name.as_str(), r.total().held))
            .filter(|(_, n)| *n > 0)
            .collect();
        if !held.is_empty() {
            out.push_str(&format!(
                "{} values on {} cases are held by an open question (the `scalar_ill` rows of `oracle/expected/models/pk3/`, held by Q-024 in `board/QUESTIONS.md`: a condition number above 1e3 with respect to the parameters, so that the tolerance cannot be demanded of any double precision implementation). They are in none of the counts above, they are not failures, and the case tables below list them in a `held` column.\n\n",
                t.held,
                held.len()
            ));
            out.push_str("| case | held values |\n|---|---|\n");
            for (name, n) in &held {
                out.push_str(&format!("| {name} | {n} |\n"));
            }
            out.push('\n');
        }
        let refused: Vec<(&str, usize)> = of_kind
            .iter()
            .map(|r| (r.name.as_str(), r.total().refused))
            .filter(|(_, n)| *n > 0)
            .collect();
        if !refused.is_empty() {
            out.push_str(&format!(
                "{} values on {} cases are free-zone rows (zone (0) or (iii) of MOD-3C-08, `oracle/expected/models/pk3/`, options kind `model_macro_free`) whose macro input the engine refused with `DegenerateExponents`. The spec leaves the engine free to refuse them, and the oracle test `check_free` accepts the refusal, so they are not failures; there is no value to compare, so they are not validated either. They are in none of the counts above, and the case tables below list them in a `refused` column. When the engine accepts one of these inputs its rows are compared and move to the validated counts; the floors do not let them go back.\n\n",
                refused.iter().map(|(_, n)| n).sum::<usize>(),
                refused.len()
            ));
            out.push_str("| case | refused values |\n|---|---|\n");
            for (name, n) in &refused {
                out.push_str(&format!("| {name} | {n} |\n"));
            }
            out.push('\n');
        }
    }
    for r in reports {
        out.push_str(&format!("\n### {}\n\n", r.name));
        for e in &r.errors {
            out.push_str(&format!("- engine error: {e}\n"));
        }
        if !r.errors.is_empty() {
            out.push('\n');
        }
        let has_held = r.total().held > 0;
        let has_refused = r.total().refused > 0;
        let mut header = format!(
            "| {} | validated | documented differences",
            r.kind.parameter_title()
        );
        if has_held {
            header.push_str(" | held");
        }
        if has_refused {
            header.push_str(" | refused");
        }
        header.push_str(" | status |");
        out.push_str(&header);
        out.push_str("\n|---|---|---");
        out.push_str(&"|---".repeat(usize::from(has_held) + usize::from(has_refused)));
        out.push_str("|---|\n");
        for (name, c) in &r.parameters {
            let status = if c.validated == c.expected {
                "ok"
            } else {
                "FAILS"
            };
            let mut row = format!(
                "| `{name}` | {} / {} | {}",
                c.validated, c.expected, c.documented
            );
            if has_held {
                row.push_str(&format!(" | {}", c.held));
            }
            if has_refused {
                row.push_str(&format!(" | {}", c.refused));
            }
            out.push_str(&format!("{row} | {status} |\n"));
        }
    }
    out.push('\n');
    for r in reports {
        for (name, c) in &r.parameters {
            let held = match (c.held, c.refused) {
                (0, 0) => String::new(),
                (held, 0) => format!(" {held}"),
                (held, refused) => format!(" {held} {refused}"),
            };
            out.push_str(&format!(
                "{FLOOR_MARK}{} {name} {} {} {}{held} {FLOOR_END}\n",
                r.name, c.validated, c.expected, c.documented
            ));
        }
    }
    out
}

pub fn run() -> Result<()> {
    let root = workspace::root()?;
    let oracle_error = |e: caladrius_testkit::OracleError| XtaskError::new(e.to_string());
    let mut reports = Vec::new();
    for name in list_cases().map_err(oracle_error)? {
        let case = load_case(&name).map_err(oracle_error)?;
        reports.push(evaluate(&case)?);
    }
    reports.extend(step3::model_reports()?);
    reports.extend(step3::fit_reports()?);
    check_unique_names(&reports)?;
    update_file(&root.join(OUTPUT), &reports)?;
    for r in &reports {
        let t = r.total();
        let mut documented = if t.documented > 0 {
            format!(", {} documented differences", t.documented)
        } else {
            String::new()
        };
        if t.held > 0 {
            documented.push_str(&format!(", {} held (Q-024)", t.held));
        }
        console::out(&format!(
            "{:<20} {:>4} / {:<4} values validated ({:.1} %){documented}",
            r.name,
            t.validated,
            t.expected,
            percent(t)
        ));
    }
    for kind in Kind::ALL {
        let t = reports
            .iter()
            .filter(|r| r.kind == kind)
            .fold(Count::default(), |acc, r| acc.plus(r.total()));
        console::out(&format!(
            "total {:<6} {:>5} / {:<5} values validated ({:.1} %)",
            match kind {
                Kind::Nca => "NCA",
                Kind::Model => "models",
                Kind::Fit => "fits",
            },
            t.validated,
            t.expected,
            percent(t)
        ));
    }
    console::out(&format!("conformance: wrote {OUTPUT}"));
    Ok(())
}

/// NCA, model and fit cases share one namespace of floors (floors are looked up by case name):
/// two cases with the same name would silently share their floors.
fn check_unique_names(reports: &[CaseReport]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    match reports.iter().find(|r| !seen.insert(r.name.as_str())) {
        None => Ok(()),
        Some(r) => Err(XtaskError::new(format!(
            "two oracle cases are called `{}` (NCA, model and fit cases share the floors' namespace); rename one",
            r.name
        ))),
    }
}

/// Checks the floors recorded in `path` against `reports`, then rewrites `path`. Any error leaves
/// the file as it was.
fn update_file(path: &Path, reports: &[CaseReport]) -> Result<()> {
    let previous = read_previous(path)?;
    check_floors(&parse_floors(&previous)?, reports)?;
    write_atomic(path, &render(reports))
}

/// Writes `text` to a temporary file next to `path`, then renames it over `path`, so a crash never
/// leaves a half-written file.
fn write_atomic(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| XtaskError::io("create", dir, &e))?;
    }
    let mut temp_name = path.as_os_str().to_owned();
    temp_name.push(".tmp");
    let temp = std::path::PathBuf::from(temp_name);
    fs::write(&temp, text).map_err(|e| XtaskError::io("write", &temp, &e))?;
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        XtaskError::io("rename onto", path, &e)
    })
}

/// The previous output, or an empty text when there is none yet.
fn read_previous(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(XtaskError::io("read", path, &e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(name: &str, validated: usize, expected: usize) -> (String, Count) {
        (
            name.to_owned(),
            Count {
                validated,
                expected,
                documented: 0,
                held: 0,
                refused: 0,
            },
        )
    }

    fn report_of(name: &str, parameters: Vec<(String, Count)>) -> CaseReport {
        CaseReport {
            kind: Kind::Nca,
            name: name.to_owned(),
            details: vec!["extravascular".to_owned(), "linear".to_owned()],
            parameters,
            errors: Vec::new(),
        }
    }

    fn report(name: &str, validated: usize, expected: usize) -> CaseReport {
        report_of(name, vec![param("cmax", validated, expected)])
    }

    /// A fresh scratch directory, removed by the caller.
    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "caladrius-xtask-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn case_names_are_unique_across_kinds() {
        let mut model = report("same", 1, 1);
        model.kind = Kind::Model;
        assert!(check_unique_names(&[report("a", 1, 1), report("b", 1, 1)]).is_ok());
        let err = check_unique_names(&[report("same", 1, 1), model]).unwrap_err();
        assert!(err.to_string().contains("`same`"), "{err}");
    }

    #[test]
    fn summary_headers_use_correct_plurals() {
        let mut model = report("m", 1, 1);
        model.kind = Kind::Model;
        let mut fit = report("f", 1, 1);
        fit.kind = Kind::Fit;
        let text = render(&[report("a", 1, 1), model, fit]);
        assert!(text.contains("| parameters fully validated |"));
        assert!(text.contains("| quantities fully validated |"));
        assert!(text.contains("| groups fully validated |"));
        assert!(!text.contains("quantitys"));
    }

    #[test]
    fn rendered_floors_are_read_back_per_parameter() {
        let text = render(&[
            report_of("a", vec![param("cmax", 3, 4), param("tmax", 2, 2)]),
            report("b", 2, 2),
        ]);
        let floors = parse_floors(&text).unwrap();
        let get = |case: &str, p: &str| floors.parameters.get(&(case.to_owned(), p.to_owned()));
        assert_eq!(
            get("a", "cmax"),
            Some(&Count {
                validated: 3,
                expected: 4,
                documented: 0,
                held: 0,
                refused: 0
            })
        );
        assert_eq!(
            get("a", "tmax"),
            Some(&Count {
                validated: 2,
                expected: 2,
                documented: 0,
                held: 0,
                refused: 0
            })
        );
        assert!(get("b", "cmax").is_some());
        assert!(floors.legacy_totals.is_empty());
        assert!(text.contains("<!-- floor a cmax 3 4 0 -->"));
        assert!(text.contains("**Overall: 7 of 8 values validated (87.5 %).**"));
        assert!(text.contains("| `cmax` | 3 / 4 | 0 | FAILS |"));
    }

    fn with_documented(
        name: &str,
        validated: usize,
        expected: usize,
        documented: usize,
    ) -> CaseReport {
        report_of(
            name,
            vec![(
                "cmax".to_owned(),
                Count {
                    validated,
                    expected,
                    documented,
                    held: 0,
                    refused: 0,
                },
            )],
        )
    }

    #[test]
    fn documented_differences_have_their_own_column_and_are_not_counted_elsewhere() {
        let text = render(&[with_documented("a", 3, 3, 2), report("b", 2, 2)]);
        // Neither validated nor expected: 5 of 5 values validated, 2 further documented.
        assert!(
            text.contains("**Overall: 5 of 5 values validated (100.0 %).** 2 further values"),
            "{text}"
        );
        assert!(text.contains("| documented differences |"), "{text}");
        assert!(text.contains("| 1 / 1 | 3 / 3 | 100.0 | 2 |"), "{text}");
        assert!(text.contains("| `cmax` | 3 / 3 | 2 | ok |"), "{text}");
        assert!(text.contains("<!-- floor a cmax 3 3 2 -->"), "{text}");
        let floors = parse_floors(&text).unwrap();
        let floor = floors
            .parameters
            .get(&("a".to_owned(), "cmax".to_owned()))
            .unwrap();
        assert_eq!(
            (floor.validated, floor.expected, floor.documented),
            (3, 3, 2)
        );
    }

    #[test]
    fn a_documented_difference_cannot_hide_a_regression_or_a_lost_row() {
        // Before: 4 of 5 validated, one failure. Documenting the failure is allowed.
        let floors = parse_floors(&render(&[report("a", 4, 5)])).unwrap();
        assert!(check_floors(&floors, &[with_documented("a", 4, 4, 1)]).is_ok());
        // Documenting a validated row lowers the validated count: refused.
        let err = check_floors(&floors, &[with_documented("a", 3, 3, 2)]).unwrap_err();
        assert!(
            err.to_string().contains("validates 3 values, floor is 4"),
            "{err}"
        );
        // Dropping rows from the oracle is still refused, documented or not.
        let err = check_floors(&floors, &[with_documented("a", 4, 4, 0)]).unwrap_err();
        assert!(
            err.to_string().contains("4 expected rows, floor is 5"),
            "{err}"
        );
        // A floor written before the column existed (four fields) still reads.
        let old = "<!-- floor a cmax 4 5 -->\n";
        let floors = parse_floors(old).unwrap();
        assert!(check_floors(&floors, &[with_documented("a", 4, 4, 1)]).is_ok());
    }

    #[test]
    fn floors_only_go_up() {
        let floors = parse_floors(&render(&[report("a", 3, 4)])).unwrap();
        assert!(check_floors(&floors, &[report("a", 3, 4)]).is_ok());
        assert!(check_floors(&floors, &[report("a", 4, 4)]).is_ok());
        let down = check_floors(&floors, &[report("a", 2, 4)]).unwrap_err();
        assert!(down.to_string().contains("floor is 3"), "{down}");
        let gone = check_floors(&floors, &[report("b", 1, 1)]).unwrap_err();
        assert!(gone.to_string().contains("no longer"), "{gone}");
        // A new case has no floor yet.
        assert!(check_floors(&floors, &[report("a", 3, 4), report("c", 0, 1)]).is_ok());
    }

    #[test]
    fn a_lost_parameter_is_not_hidden_by_a_gain_elsewhere() {
        let floors = parse_floors(&render(&[report_of(
            "a",
            vec![param("cmax", 2, 4), param("tmax", 1, 4)],
        )]))
        .unwrap();
        // The case total goes up (3 -> 5) but cmax went down.
        let now = report_of("a", vec![param("cmax", 1, 4), param("tmax", 4, 4)]);
        let err = check_floors(&floors, &[now]).unwrap_err();
        assert!(err.to_string().contains("cmax validates 1"), "{err}");
        // A parameter that is no longer checked is a drop.
        let missing = report_of("a", vec![param("tmax", 4, 4)]);
        let err = check_floors(&floors, &[missing]).unwrap_err();
        assert!(err.to_string().contains("cmax is no longer"), "{err}");
    }

    #[test]
    fn new_unvalidated_or_lost_rows_fail() {
        let floors = parse_floors(&render(&[report("a", 4, 4)])).unwrap();
        // Two oracle rows added that are not reproduced: the validated count still meets the floor.
        let err = check_floors(&floors, &[report("a", 4, 6)]).unwrap_err();
        assert!(err.to_string().contains("2 unvalidated rows"), "{err}");
        // Rows added and validated are fine.
        assert!(check_floors(&floors, &[report("a", 6, 6)]).is_ok());
        // Rows removed from the oracle are a drop.
        let err = check_floors(&floors, &[report("a", 3, 3)]).unwrap_err();
        assert!(err.to_string().contains("3 expected rows"), "{err}");
    }

    #[test]
    fn a_malformed_floor_line_is_an_error() {
        let good = render(&[report("a", 3, 4)]);
        for bad in [
            "<!-- floor a cmax 3 -->",
            "<!-- floor a cmax x 4 -->",
            "<!-- floor a cmax 5 4 -->",
            "<!-- floor a cmax 3 4",
            "<!-- floor a cmax 3 4 5 6 -->",
            "<!-- floor a cmax 3 4 x -->",
            "<!-- floor -->",
        ] {
            let text = format!("{good}{bad}\n");
            let err = parse_floors(&text).unwrap_err();
            assert!(
                err.to_string().contains("malformed floor line"),
                "{bad}: {err}"
            );
        }
        let twice = format!("{good}<!-- floor a cmax 3 4 -->\n");
        assert!(parse_floors(&twice).is_err());
    }

    #[test]
    fn a_non_empty_file_without_floors_is_an_error() {
        let err = parse_floors("# Conformance\n\nno floors here\n").unwrap_err();
        assert!(err.to_string().contains("holds no floor"), "{err}");
        // No file yet, or an empty one, is a first run.
        assert!(parse_floors("").unwrap().is_empty());
        assert!(parse_floors("\n \n").unwrap().is_empty());
    }

    #[test]
    fn first_format_totals_are_still_enforced() {
        let floors = parse_floors("<!-- floor a 3 -->\n").unwrap();
        assert_eq!(floors.legacy_totals.get("a"), Some(&3));
        assert!(check_floors(&floors, &[report("a", 3, 4)]).is_ok());
        assert!(check_floors(&floors, &[report("a", 2, 4)]).is_err());
        assert!(check_floors(&floors, &[report("b", 4, 4)]).is_err());
    }

    fn profile_result() -> NcaResult {
        let input = NcaInput {
            time: vec![0.0, 1.0, 2.0, 4.0, 6.0, 8.0],
            conc: vec![0.0, 8.0, 6.0, 3.0, 1.5, 0.75],
            // A missing dose: the dose-dependent parameters are not calculated (NCA-DAT-10).
            dose: f64::NAN,
            route: Route::Extravascular,
            options: NcaOptions::default(),
        };
        run_nca(&input).unwrap()
    }

    #[test]
    fn not_available_matches_only_a_computed_parameter() {
        let result = profile_result();
        // Computed and not calculated on both sides: validated.
        assert!(
            result
                .parameter("cl.obs")
                .is_some_and(|p| p.value().is_none())
        );
        assert!(row_validated(None, Some(&result), "cl.obs"));
        // The engine does not compute this name at all: never validated.
        assert!(result.parameter("no.such.parameter").is_none());
        assert!(!row_validated(None, Some(&result), "no.such.parameter"));
        assert!(!row_validated(
            Some(1.0),
            Some(&result),
            "no.such.parameter"
        ));
        // A refused subject validates nothing, not even a not-available row.
        assert!(!row_validated(None, None, "cl.obs"));
        // A computed value matches within tolerance only.
        let cmax = result.get("cmax").unwrap();
        assert!(row_validated(Some(cmax), Some(&result), "cmax"));
        assert!(!row_validated(Some(cmax * 1.001), Some(&result), "cmax"));
        // Expected not available but computed: not validated.
        assert!(!row_validated(None, Some(&result), "cmax"));
        // Expected a value but the engine says not calculated: not validated.
        assert!(!row_validated(Some(1.0), Some(&result), "cl.obs"));
    }

    #[test]
    fn the_file_is_written_atomically() {
        let dir = scratch("atomic");
        let path = dir.join("docs").join("conformance.md");
        write_atomic(&path, "first").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        write_atomic(&path, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert!(!dir.join("docs").join("conformance.md.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failing_gate_leaves_the_file_unchanged() {
        let dir = scratch("gate");
        let path = dir.join("conformance.md");
        update_file(&path, &[report("a", 3, 4)]).unwrap();
        let before = fs::read_to_string(&path).unwrap();
        assert!(update_file(&path, &[report("a", 2, 4)]).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        // A file with its floors stripped is refused, not regenerated.
        fs::write(&path, "# Conformance\n").unwrap();
        assert!(update_file(&path, &[report("a", 4, 4)]).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "# Conformance\n");
        // Going up rewrites the file, and a second run is byte-identical.
        fs::write(&path, &before).unwrap();
        update_file(&path, &[report("a", 4, 4)]).unwrap();
        let after = fs::read_to_string(&path).unwrap();
        update_file(&path, &[report("a", 4, 4)]).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), after);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_public_case_is_fully_validated_and_every_edge_case_runs() {
        for name in list_cases().unwrap() {
            let report = evaluate(&load_case(&name).unwrap()).unwrap();
            let total = report.total();
            assert!(report.errors.is_empty(), "{name}: {:?}", report.errors);
            // The edge cases (task T-012) record what the engine does not yet reproduce: their
            // floors go up as the engine improves, but they are not required to be complete.
            if !name.starts_with("edge_") {
                assert_eq!(total.validated, total.expected, "{name}");
            }
        }
    }

    fn with_held(name: &str, validated: usize, expected: usize, held: usize) -> CaseReport {
        report_of(
            name,
            vec![(
                "cmax".to_owned(),
                Count {
                    validated,
                    expected,
                    documented: 0,
                    held,
                    refused: 0,
                },
            )],
        )
    }

    #[test]
    fn held_rows_have_their_own_listing_and_are_counted_in_no_total() {
        let text = render(&[with_held("a", 3, 3, 2), report("b", 2, 2)]);
        assert!(
            text.contains(
                "**Overall: 5 of 5 values validated (100.0 %).** 2 further values are held"
            ),
            "{text}"
        );
        assert!(
            text.contains("2 values on 1 cases are held by an open question"),
            "{text}"
        );
        assert!(text.contains("| a | 2 |"), "{text}");
        assert!(text.contains("| `cmax` | 3 / 3 | 0 | 2 | ok |"), "{text}");
        // The case without held rows keeps the table it always had.
        assert!(text.contains("| `cmax` | 2 / 2 | 0 | ok |"), "{text}");
        assert!(text.contains("<!-- floor a cmax 3 3 0 2 -->"), "{text}");
        assert!(text.contains("<!-- floor b cmax 2 2 0 -->"), "{text}");
        let floors = parse_floors(&text).unwrap();
        let floor = floors
            .parameters
            .get(&("a".to_owned(), "cmax".to_owned()))
            .unwrap();
        assert_eq!(
            (
                floor.validated,
                floor.expected,
                floor.documented,
                floor.held
            ),
            (3, 3, 0, 2)
        );
    }

    #[test]
    fn a_held_row_cannot_be_dropped_and_may_become_an_expected_row() {
        let floors = parse_floors(&render(&[with_held("a", 3, 3, 2)])).unwrap();
        assert!(check_floors(&floors, &[with_held("a", 3, 3, 2)]).is_ok());
        // Q-024 answered: the two held rows become expected rows and validate.
        assert!(check_floors(&floors, &[with_held("a", 5, 5, 0)]).is_ok());
        // A held row dropped from the oracle is refused.
        let err = check_floors(&floors, &[with_held("a", 3, 3, 1)]).unwrap_err();
        assert!(
            err.to_string().contains("4 expected rows, floor is 5"),
            "{err}"
        );
    }

    fn with_refused(name: &str, validated: usize, expected: usize, refused: usize) -> CaseReport {
        report_of(
            name,
            vec![(
                "conc".to_owned(),
                Count {
                    validated,
                    expected,
                    documented: 0,
                    held: 0,
                    refused,
                },
            )],
        )
    }

    #[test]
    fn refused_rows_have_their_own_listing_and_are_counted_in_no_total() {
        let text = render(&[with_refused("a", 0, 0, 4), report("b", 2, 2)]);
        assert!(
            text.contains("**Overall: 2 of 2 values validated (100.0 %).**"),
            "{text}"
        );
        assert!(
            text.contains("4 further values are on free-zone cases that the engine refused"),
            "{text}"
        );
        assert!(text.contains("| a | 4 |"), "{text}");
        assert!(text.contains("| `conc` | 0 / 0 | 0 | 4 | ok |"), "{text}");
        assert!(text.contains("<!-- floor a conc 0 0 0 0 4 -->"), "{text}");
        let floors = parse_floors(&text).unwrap();
        let floor = floors
            .parameters
            .get(&("a".to_owned(), "conc".to_owned()))
            .unwrap();
        assert_eq!((floor.expected, floor.held, floor.refused), (0, 0, 4));
    }

    #[test]
    fn a_refused_row_cannot_be_dropped_and_may_become_a_validated_row() {
        let floors = parse_floors(&render(&[with_refused("a", 0, 0, 4)])).unwrap();
        assert!(check_floors(&floors, &[with_refused("a", 0, 0, 4)]).is_ok());
        // The engine accepts the input and the rows validate.
        assert!(check_floors(&floors, &[with_refused("a", 4, 4, 0)]).is_ok());
        // The rows are accepted but wrong: the floor on unvalidated rows (none) catches it.
        assert!(check_floors(&floors, &[with_refused("a", 3, 4, 0)]).is_err());
        // A refused row dropped from the oracle is refused.
        assert!(check_floors(&floors, &[with_refused("a", 0, 0, 3)]).is_err());
    }
}
