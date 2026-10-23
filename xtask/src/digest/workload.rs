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
//! - `plots/theoph`: a fit of a Theoph subject and its plots, linear and logarithmic, exported as SVG.
//!
//! The cases are the ones `cargo xtask conformance` compares to the oracle; the digest does not
//! compare anything, it only says whether two machines return the same bits for them.

use std::fs;

use caladrius_digest::{Session, Step, Workload};
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
