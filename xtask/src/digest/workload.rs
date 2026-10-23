//! The workload of `cargo xtask digest`: what the engine is asked, in a fixed order.
//!
//! Every request is an engine command (`AGENTS.md` golden rule 4), so the numbers are the ones the
//! CLI, the MCP server, the desktop application and the browser page would give:
//!
//! - `examples`: the example of every command of the registry, in the order of the registry (the
//!   script the engine tests run: import, NCA, fits, simulation, plots, comparison, export);
//! - `nca/<case>`: each NCA case of `oracle/` (`data.import` of its dataset, `nca.run` with the
//!   options and the route of the case, every subject);
//! - `fit/<case>`: each reference fit of `oracle/expected/fit/` (`data.import` of the observations
//!   the case fits, then `fit.run` for every subject with the start values, weighting and options of
//!   the case), the Theoph and Indometh fits among them;
//! - `model/<case>`: each value case of the one- and two-compartment models and each
//!   multiple-dosing case (`model.simulate`, with the regimen carried in the parameters);
//! - `refuse/<suite>/<id>`: each input of the two error suites that JSON can carry, which the engine
//!   must refuse (the message is part of the digest);
//! - `plots/theoph`: a fit of a Theoph subject and its plots, linear and logarithmic, exported as SVG;
//! - `replay/<case>`: `project.replay` of a project file that this build made and sealed (an import,
//!   an NCA, a fit, a simulation, an edit of one cell and the NCA again): as it was saved, with one
//!   number of its history changed, with one stored result changed, with its seals taken out (a
//!   project saved before seals) and with the parameters of its import left out. Every machine that
//!   runs the digest replays the file its own build made, and the one that runs the workload in
//!   WebAssembly replays the file the native build made, so the report, which holds every seal
//!   stored and recomputed, says whether a seal made on one machine is the seal another computes.
//!
//! The cases are the ones `cargo xtask conformance` compares to the oracle; the digest does not
//! compare anything, it only says whether two machines return the same bits for them.

use std::fs;

use caladrius_digest::{Session, Step, Workload};
use caladrius_engine::Engine;
use caladrius_models::{ModelId, Regimen};
use caladrius_testkit::oracle::oracle_dir;
use caladrius_testkit::{
    FitCase, ModelCase, OracleError, Pk2ErrorCase, Pk2Kind, list_cases, list_fit_cases,
    list_md_cases, list_model_cases, list_pk2_cases, load_case, load_fit_case, load_md_case,
    load_md_errors, load_model_case, load_pk2_case, load_pk2_errors,
};
use serde_json::{Value, json};

use crate::conformance::step3::{fit_input, model_input, regimen_of};
use crate::conformance::{options_of, route_of};
use crate::error::{Result, XtaskError};

fn oracle_error(e: OracleError) -> XtaskError {
    XtaskError::new(e.to_string())
}

fn json_error(e: serde_json::Error) -> XtaskError {
    XtaskError::new(format!("cannot write a request as JSON: {e}"))
}

fn step(id: &str, params: Value) -> Step {
    Step {
        id: id.to_owned(),
        params,
    }
}

/// The whole workload, in a fixed order.
pub fn build() -> Result<Workload> {
    let mut sessions = vec![examples()];
    sessions.extend(nca_sessions()?);
    sessions.extend(fit_sessions()?);
    sessions.extend(model_sessions()?);
    sessions.extend(refusal_sessions()?);
    sessions.push(plots()?);
    sessions.extend(replay_sessions()?);
    Ok(Workload { sessions })
}

/// The example of every command, one after the other on the same engine.
fn examples() -> Session {
    Session {
        name: "examples".to_owned(),
        steps: caladrius_engine::describe()
            .into_iter()
            .map(|command| step(&command.id, command.example))
            .collect(),
    }
}

fn read_oracle_file(relative: &str) -> Result<String> {
    let path = oracle_dir().join(relative);
    fs::read_to_string(&path).map_err(|e| XtaskError::io("read", &path, &e))
}

fn nca_sessions() -> Result<Vec<Session>> {
    let mut out = Vec::new();
    for name in list_cases().map_err(oracle_error)? {
        let case = load_case(&name).map_err(oracle_error)?;
        let csv = read_oracle_file(&case.options.data_file)?;
        let route = serde_json::to_value(route_of(&case)?).map_err(json_error)?;
        let options = serde_json::to_value(options_of(&case)?).map_err(json_error)?;
        out.push(Session {
            name: format!("nca/{name}"),
            steps: vec![
                step(
                    "data.import",
                    json!({ "name": case.options.dataset, "csv": csv }),
                ),
                step(
                    "nca.run",
                    json!({ "worksheet": 1, "route": route, "options": options }),
                ),
            ],
        });
    }
    Ok(out)
}

/// The observations a fit case fits, as the CSV `data.import` reads: one row per sample.
fn fit_csv(case: &FitCase, subjects: &[&String]) -> String {
    let mut csv = String::from("subject,time,conc,dose\n");
    for profile in &case.dataset.profiles {
        let label = profile.subject.to_string();
        if !subjects.contains(&&label) {
            continue;
        }
        let (time, conc) = case.fitted_observations(profile);
        for (t, c) in time.iter().zip(&conc) {
            csv.push_str(&format!("{label},{t},{c},{}\n", profile.dose));
        }
    }
    csv
}

fn fit_sessions() -> Result<Vec<Session>> {
    let mut out = Vec::new();
    for name in list_fit_cases().map_err(oracle_error)? {
        let case = load_fit_case(&name).map_err(oracle_error)?;
        // The subjects that have reference results, then those the iterated scheme has no fixed
        // point for: the second kind runs to the iteration limit, which is worth fingerprinting.
        let subjects: Vec<&String> = case.subjects.iter().chain(&case.no_fixed_point).collect();
        let mut steps = vec![step(
            "data.import",
            json!({ "name": case.dataset.name, "csv": fit_csv(&case, &subjects) }),
        )];
        for subject in subjects {
            let input = fit_input(&case, subject)?;
            steps.push(step(
                "fit.run",
                json!({
                    "worksheet": 1,
                    "subject": subject,
                    "model": input.model,
                    "weighting": input.weighting,
                    "initial": input.initial,
                    "options": input.options,
                }),
            ));
        }
        out.push(Session {
            name: format!("fit/{name}"),
            steps,
        });
    }
    Ok(out)
}

/// The `model.simulate` parameters of a model case, with its regimen when it has one.
fn simulate_params(case: &ModelCase, regimen: Option<&Regimen>) -> Result<Value> {
    match ModelId::from_id(&case.model) {
        Some(model) => {
            let input = model_input(case, model);
            let input = match regimen {
                Some(regimen) => input.with_regimen(regimen),
                None => input,
            };
            serde_json::to_value(input).map_err(json_error)
        }
        // A model id the engine does not know is still a request: the engine refuses it.
        None => Ok(json!({
            "model": case.model,
            "dose": case.dose,
            "params": case.parameters,
            "times": case.times,
        })),
    }
}

fn simulate_session(name: String, params: Value) -> Session {
    Session {
        name,
        steps: vec![step("model.simulate", params)],
    }
}

fn model_sessions() -> Result<Vec<Session>> {
    let mut out = Vec::new();
    for name in list_model_cases().map_err(oracle_error)? {
        let case = load_model_case(&name).map_err(oracle_error)?;
        out.push(simulate_session(
            format!("model/{name}"),
            simulate_params(&case, None)?,
        ));
    }
    for name in list_pk2_cases().map_err(oracle_error)? {
        let loaded = load_pk2_case(&name).map_err(oracle_error)?;
        // The derivative cases test the Jacobian, which no command returns on its own.
        if loaded.kind == Pk2Kind::Values {
            out.push(simulate_session(
                format!("model/{name}"),
                simulate_params(&loaded.case, None)?,
            ));
        }
    }
    for name in list_md_cases().map_err(oracle_error)? {
        let loaded = load_md_case(&name).map_err(oracle_error)?;
        let regimen = regimen_of(&loaded).map_err(XtaskError::new)?;
        out.push(simulate_session(
            format!("model/{name}"),
            simulate_params(&loaded.case, Some(&regimen))?,
        ));
    }
    Ok(out)
}

/// Whether a refusal case can be written as JSON (it has no NaN or infinity, which JSON cannot hold).
fn is_json_safe(case: &Pk2ErrorCase) -> bool {
    case.dose.is_finite()
        && case.parameters.values().all(|v| v.is_finite())
        && case.times.iter().all(|t| t.is_finite())
}

fn refusal_sessions() -> Result<Vec<Session>> {
    let mut out = Vec::new();
    let suites = [
        ("pk2", load_pk2_errors().map_err(oracle_error)?),
        ("md", load_md_errors().map_err(oracle_error)?),
    ];
    for (suite, cases) in suites {
        for case in cases.iter().filter(|case| is_json_safe(case)) {
            out.push(simulate_session(
                format!("refuse/{suite}/{}", case.id),
                json!({
                    "model": case.model,
                    "dose": case.dose,
                    "params": case.parameters,
                    "times": case.times,
                }),
            ));
        }
    }
    Ok(out)
}

/// A fit of one Theoph subject and every kind of plot made from the worksheet and from the fit,
/// exported as SVG: the axes, ticks and labels of `export.plot` depend on logarithms and powers.
fn plots() -> Result<Session> {
    let csv = read_oracle_file("data/theoph.csv")?;
    let plot = |params: Value| step("plot.xy", params);
    let export = |analysis: u32| step("export.plot", json!({ "analysis": analysis }));
    // Objects are numbered in the order they are made, worksheets and analyses together: the worksheet
    // is 1, the fit 2, the plots 3 to 9.
    let steps = vec![
        step("data.import", json!({ "name": "theoph", "csv": csv })),
        step(
            "fit.run",
            json!({
                "worksheet": 1, "subject": "1", "model": "pk1.oral_1",
                "initial": { "v": 30.0, "k": 0.1, "ka": 1.5 },
            }),
        ),
        plot(json!({ "worksheet": 1, "scale": "both", "layout": "per_subject" })),
        plot(json!({
            "worksheet": 1, "scale": "log", "layout": "overlay", "subjects": ["1", "2", "3"]
        })),
        plot(json!({ "fit": 2, "view": "profile", "scale": "both" })),
        plot(json!({ "fit": 2, "view": "observed_predicted", "scale": "log" })),
        plot(json!({ "fit": 2, "view": "residuals_time", "scale": "linear" })),
        plot(json!({ "fit": 2, "view": "weighted_residuals_time", "scale": "linear" })),
        plot(json!({
            "worksheet": 1, "scale": "linear", "layout": "overlay", "title": "Theoph, every subject"
        })),
        export(3),
        export(4),
        export(5),
        export(6),
        export(7),
        export(8),
        export(9),
    ];
    Ok(Session {
        name: "plots/theoph".to_owned(),
        steps,
    })
}

/// The Theoph profiles of some subjects, as a CSV: a short file keeps the saved project small.
fn theoph_subjects(subjects: &[&str]) -> Result<String> {
    let csv = read_oracle_file("data/theoph.csv")?;
    let mut out = String::new();
    for (index, line) in csv.lines().enumerate() {
        let subject = line.split(',').next().unwrap_or_default();
        if index == 0 || subjects.contains(&subject) {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(out)
}

/// The saved file of a short session, made by the engine of this build with no version or commit
/// in its seals (a commit that computes the same bits gives the same digest): an import, an NCA, a
/// fit, a simulation, an edit of one cell and the NCA again.
fn replay_file() -> Result<Value> {
    let mut engine = Engine::new();
    engine.set_provenance(false);
    let script = [
        step(
            "data.import",
            json!({ "name": "theoph", "csv": theoph_subjects(&["1", "2", "3"])? }),
        ),
        step(
            "nca.run",
            json!({ "worksheet": 1, "route": "extravascular" }),
        ),
        step(
            "fit.run",
            json!({
                "worksheet": 1, "subject": "1", "model": "pk1.oral_1",
                "initial": { "v": 30.0, "k": 0.1, "ka": 1.5 },
            }),
        ),
        step(
            "model.simulate",
            json!({
                "model": "pk1.iv_bolus", "dose": 100,
                "params": { "v": 10, "k": 0.1 }, "times": [0, 1, 2, 5, 10]
            }),
        ),
        step(
            "data.set_cell",
            json!({ "worksheet": 1, "column": "conc", "row": 3, "value": 4.5 }),
        ),
        step(
            "nca.run",
            json!({ "worksheet": 1, "route": "extravascular" }),
        ),
    ];
    for one in script {
        engine.execute(&one.id, one.params).map_err(|e| {
            XtaskError::new(format!(
                "{} failed while the replay file was made: {e}",
                one.id
            ))
        })?;
    }
    let bytes = engine
        .save_bytes()
        .map_err(|e| XtaskError::new(format!("the replay file cannot be saved: {e}")))?;
    serde_json::from_slice(&bytes).map_err(json_error)
}

/// The text of a CSV with the last digit of its first line of data changed: one number differs.
fn with_one_value_changed(csv: &str) -> String {
    let mut lines: Vec<String> = csv.lines().map(str::to_owned).collect();
    if let Some(line) = lines.get_mut(1) {
        if let Some((at, digit)) = line.char_indices().rev().find(|(_, c)| c.is_ascii_digit()) {
            let next = digit
                .to_digit(10)
                .and_then(|d| char::from_digit((d + 1) % 10, 10))
                .unwrap_or(digit);
            line.replace_range(at..at + digit.len_utf8(), &next.to_string());
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The parameters of the first `data.import` of the history of a saved file.
fn import_params(document: &mut Value) -> Option<&mut Value> {
    document
        .get_mut("history")?
        .as_array_mut()?
        .iter_mut()
        .find(|entry| entry["command"] == "data.import")?
        .get_mut("params")
}

/// A number of a stored worksheet: the fifth value of the concentration column.
fn stored_cell(document: &mut Value) -> Option<&mut Value> {
    document
        .get_mut("worksheets")?
        .get_mut(0)?
        .get_mut("columns")?
        .get_mut(2)?
        .get_mut("data")?
        .get_mut("number")?
        .get_mut(5)
}

/// The sessions that replay a project file, one for each way a replay can end.
fn replay_sessions() -> Result<Vec<Session>> {
    let saved = replay_file()?;
    let replay = |name: &str, document: Value| Session {
        name: format!("replay/{name}"),
        steps: vec![step("project.replay", json!({ "project": document }))],
    };

    // A project saved before seals: no step carries one.
    let mut before_seals = saved.clone();
    if let Some(history) = before_seals
        .get_mut("history")
        .and_then(Value::as_array_mut)
    {
        for entry in history {
            if let Some(map) = entry.as_object_mut() {
                map.remove("seal");
                map.remove("at");
            }
        }
    }
    // A number of the data changed in the history: the import is the first step that differs.
    let mut history_edited = saved.clone();
    if let Some(params) = import_params(&mut history_edited) {
        let csv = params["csv"].as_str().unwrap_or_default().to_owned();
        params["csv"] = json!(with_one_value_changed(&csv));
    }
    // A stored result changed: every step reproduces its seal and the project does not.
    let mut result_edited = saved.clone();
    if let Some(cell) = stored_cell(&mut result_edited) {
        let value = cell.as_f64().unwrap_or_default();
        *cell = json!(value + 0.5);
    }
    // The parameters of the import were too large to be kept: it cannot be run again, and the
    // steps that need its worksheet cannot be either.
    let mut params_not_kept = saved.clone();
    if let Some(params) = import_params(&mut params_not_kept) {
        *params = json!({ "omitted": true, "bytes": 70_000, "digest": "fnv1a64:0123456789abcdef" });
    }

    Ok(vec![
        replay("saved", saved),
        replay("history-edited", history_edited),
        replay("result-edited", result_edited),
        replay("before-seals", before_seals),
        replay("params-not-kept", params_not_kept),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session<'a>(workload: &'a Workload, name: &str) -> Option<&'a Session> {
        workload.sessions.iter().find(|s| s.name == name)
    }

    #[test]
    fn the_workload_is_the_same_every_time() {
        let first = build().unwrap();
        let second = build().unwrap();
        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
    }

    #[test]
    fn it_holds_the_examples_the_theoph_and_indometh_cases_and_the_models() {
        let workload = build().unwrap();
        let first = &workload.sessions[0];
        assert_eq!(first.name, "examples");
        assert_eq!(first.steps.len(), caladrius_engine::describe().len());
        for name in [
            "nca/theoph",
            "nca/indometh",
            "fit/fit_indometh_inv_yhat",
            "fit/fit_theoph_uniform",
            "plots/theoph",
            "replay/saved",
        ] {
            assert!(session(&workload, name).is_some(), "{name}");
        }
        let count = |prefix: &str| {
            workload
                .sessions
                .iter()
                .filter(|s| s.name.starts_with(prefix))
                .count()
        };
        assert!(count("nca/") >= 20, "{}", count("nca/"));
        assert!(count("fit/") >= 30, "{}", count("fit/"));
        assert!(
            count("model/model_pk2_") >= 50,
            "{}",
            count("model/model_pk2_")
        );
        assert!(
            count("model/model_md_") >= 100,
            "{}",
            count("model/model_md_")
        );
        assert!(count("refuse/") >= 50, "{}", count("refuse/"));
        assert_eq!(count("replay/"), 5);
        // Indometh: six subjects, five weightings, one fit per subject and weighting.
        let indometh = workload
            .sessions
            .iter()
            .filter(|s| s.name.starts_with("fit/fit_indometh_"))
            .map(|s| s.steps.iter().filter(|st| st.id == "fit.run").count())
            .sum::<usize>();
        assert!(indometh >= 30, "{indometh}");
    }

    #[test]
    fn the_replay_sessions_end_in_every_verdict_and_hold_the_seals_of_the_file() {
        let workload = build().unwrap();
        let run = caladrius_digest::run(&workload);
        let report = |name: &str| -> Value {
            let done = run
                .sessions()
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("{name} is in the workload"));
            let line = done.lines.first().unwrap();
            let doc: Value = serde_json::from_str(line).unwrap();
            assert_eq!(doc["ok"], true, "{name}: {line}");
            doc["result"].clone()
        };
        for (name, verdict) in [
            ("replay/saved", "verified"),
            ("replay/history-edited", "diverged"),
            ("replay/result-edited", "diverged"),
            ("replay/before-seals", "unsealed"),
            ("replay/params-not-kept", "incomplete"),
        ] {
            assert_eq!(report(name)["verdict"], verdict, "{name}");
        }
        // The file as saved: every step reproduces its seal, and the report carries them, 64
        // digits each, with no version or commit that would change the digest with every commit.
        let saved = report("replay/saved");
        let steps = saved["steps"].as_array().unwrap();
        assert!(steps.len() >= 6, "{}", steps.len());
        for one in steps {
            assert_eq!(one["status"], "match");
            assert_eq!(one["stored"].as_str().map(str::len), Some(64));
            assert_eq!(one["stored"], one["recomputed"]);
        }
        assert!(saved.get("engine").is_none() && saved["sealed_by"] == json!([]));
        // The history edit is found at the import; the result edit only in the stored project.
        assert_eq!(
            report("replay/history-edited")["first_divergence"]["command"],
            "data.import"
        );
        let edited = report("replay/result-edited");
        assert!(edited["first_divergence"].is_null());
        assert_eq!(edited["state"]["matches"], false);
        assert_eq!(
            report("replay/params-not-kept")["counts"]["not_replayable"],
            1
        );
    }

    #[test]
    fn the_engine_runs_every_request_except_the_inputs_it_must_refuse() {
        let workload = build().unwrap();
        let run = caladrius_digest::run(&workload);
        for done in run.sessions() {
            if done.name.starts_with("refuse/") {
                assert_eq!(
                    done.refused,
                    done.lines.len(),
                    "{} must be refused",
                    done.name
                );
            } else {
                let failed = done.lines.iter().find(|line| line.contains("\"ok\":false"));
                assert_eq!(done.refused, 0, "{} was refused: {failed:?}", done.name);
            }
        }
        assert!(run.steps() > 700, "{}", run.steps());
        assert!(run.numbers() > 100_000, "{}", run.numbers());
    }
}
