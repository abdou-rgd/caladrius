//! Unit tests of task T-149: `objective = ml` (`specs/fit.md` section 13). The worked examples
//! ML1 and ML3 on the five-point F data, the refusals of FIT-ML-14 and FIT-ML-15, the options and
//! their wording, and the stepper (progress and cancel) for the likelihood. The 468 cases of the
//! oracle are in `tests/oracle_fit_ml.rs`.

use std::collections::BTreeMap;

use serde_json::json;

use crate::*;

const T: [f64; 5] = [0.5, 1.0, 2.0, 4.0, 8.0];
const Y: [f64; 5] = [9.31, 7.92, 6.85, 4.31, 2.11];

fn f_data(error_model: ErrorModel) -> FitInput {
    FitInput {
        model: ModelId::IvBolus,
        dose: 100.0,
        time: T.to_vec(),
        conc: Y.to_vec(),
        weighting: Weighting::Uniform,
        initial: BTreeMap::from([("v".to_string(), 12.0), ("k".to_string(), 0.15)]),
        options: FitOptions::maximum_likelihood(error_model),
    }
}

#[track_caller]
fn near(r: &FitResult, name: &str, expected: f64, relative: f64) {
    let x = r
        .get(name)
        .unwrap_or_else(|| panic!("{name} is not calculated"));
    assert!(
        (x - expected).abs() <= relative * expected.abs().max(1e-300),
        "{name}: expected {expected}, got {x}"
    );
}

fn refusal(input: &FitInput) -> FitError {
    match run(input) {
        Err(e) => e,
        Ok(r) => panic!("expected a refusal, got {:?}", r.status()),
    }
}

// ---------------------------------------------------------------- the worked examples

#[test]
fn ml1_additive_needs_no_iteration_and_matches_the_hand_values() {
    let r = run(&f_data(ErrorModel::Additive)).unwrap();
    assert_eq!(r.status(), FitStatus::Converged);
    assert_eq!(r.trace().len(), 1, "converged at the start (FIT-ML-08)");
    near(&r, "estimate.v", 9.9186407, 1e-6);
    near(&r, "estimate.k", 0.20405768, 1e-6);
    near(&r, "estimate.error_a", 0.19778278, 1e-6);
    near(&r, "se.v", 0.19198610, 1e-6);
    near(&r, "se.error_a", 0.062544406, 1e-6);
    near(&r, "correlation.v.k", -0.72359196, 1e-6);
    assert!(r.get("correlation.k.error_a").unwrap().abs() < 1e-8);
    assert!((r.get("m2ll").unwrap() - -2.0164739).abs() < 1e-7);
    assert!((r.get("aic_ml").unwrap() - 3.9835261).abs() < 1e-7);
    assert!((r.get("bic_ml").unwrap() - 2.8118398).abs() < 1e-7);
    assert_eq!(r.get("df"), Some(2.0));
    assert!((r.get("iwres_sum_sq").unwrap() - 5.0).abs() < 1e-9);
    let first = r.observations()[0];
    assert!((first.iwres.unwrap() - 1.04101).abs() < 1e-5);
    assert_eq!(first.sigma, r.get("estimate.error_a"));
}

#[test]
fn ml3_both_combined_forms_collapse_onto_the_proportional_model() {
    for model in [ErrorModel::Combined, ErrorModel::CombinedSd] {
        let r = run(&f_data(model)).unwrap();
        assert_eq!(r.status(), FitStatus::AtBound, "{model:?}");
        assert!(
            (r.get("m2ll").unwrap() - -3.1899419082).abs() < 1e-7,
            "{model:?}"
        );
        near(&r, "estimate.v", 10.079808, 1e-6);
        near(&r, "estimate.error_b", 0.032551269, 1e-6);
        // The component on its bound is exactly 0, without statistics; both still count in P + Q.
        assert_eq!(r.get("estimate.error_a"), Some(0.0), "{model:?}");
        assert_eq!(r.get("se.error_a"), None);
        assert_eq!(r.get("cv_percent.error_a"), None);
        assert!(r.get("se.error_b").is_some());
        assert_eq!(r.get("p"), Some(2.0));
        assert_eq!(r.get("q"), Some(2.0));
        assert_eq!(r.get("df"), Some(1.0));
        let m2ll = r.get("m2ll").unwrap();
        assert!((r.get("aic_ml").unwrap() - (m2ll + 8.0)).abs() < 1e-9);
        assert!(
            r.flags()
                .iter()
                .any(|f| matches!(f, FitFlag::AtBound { parameter, .. } if parameter == "error_a"))
        );
    }
}

#[test]
fn the_squared_iwres_sum_to_n_and_the_least_squares_forms_are_kept() {
    let r = run(&f_data(ErrorModel::Proportional)).unwrap();
    assert_eq!(r.status(), FitStatus::Converged);
    assert!((r.get("iwres_sum_sq").unwrap() - 5.0).abs() < 1e-8);
    let wrss: f64 = r
        .observations()
        .iter()
        .map(|o| o.residual * o.residual)
        .sum();
    near(&r, "wrss", wrss, 1e-12);
    near(&r, "aic", 5.0 * libm::log(wrss) + 4.0, 1e-12);
    // The ML estimate is not the least-squares one for a proportional error.
    assert!(r.get("start.v").is_some());
    assert_ne!(r.get("start.v"), r.get("estimate.v"));
    // The trace carries −2LL and the decrement; the last row is the result.
    let last = r.trace().last().unwrap();
    assert_eq!(last.m2ll, r.get("m2ll"));
    assert!(last.decrement.unwrap() <= 1e-10);
    assert!(r.trace().iter().all(|row| row.m2ll.is_some()));
}

// ---------------------------------------------------------------- refusals

#[test]
fn the_error_model_is_required_and_only_for_ml() {
    let mut i = f_data(ErrorModel::Additive);
    i.options.error_model = None;
    assert!(matches!(
        refusal(&i),
        FitError::InvalidOption { option, .. } if option == "error_model"
    ));
    let mut i = f_data(ErrorModel::Additive);
    i.options.objective = Objective::Wls;
    assert!(matches!(
        refusal(&i),
        FitError::InvalidOption { option, .. } if option == "error_model"
    ));
    let mut i = f_data(ErrorModel::Additive);
    i.options.objective = Objective::Wls;
    i.options.error_model = None;
    i.options.error_init = Some(vec![0.1]);
    assert!(matches!(
        refusal(&i),
        FitError::InvalidOption { option, .. } if option == "error_init"
    ));
}

#[test]
fn a_weighting_is_refused_with_ml() {
    let mut i = f_data(ErrorModel::Proportional);
    i.weighting = Weighting::InvY2;
    match refusal(&i) {
        FitError::InvalidOption { option, reason } => {
            assert_eq!(option, "weighting");
            assert!(reason.contains("error model"), "{reason}");
        }
        e => panic!("{e:?}"),
    }
}

#[test]
fn error_init_must_match_the_model_and_be_positive() {
    for (model, init) in [
        (ErrorModel::Additive, vec![0.1, 0.2]),
        (ErrorModel::Combined, vec![0.1]),
        (ErrorModel::Proportional, vec![0.0]),
        (ErrorModel::Proportional, vec![-0.1]),
        (ErrorModel::Additive, vec![f64::NAN]),
        (ErrorModel::Additive, vec![f64::INFINITY]),
    ] {
        let mut i = f_data(model);
        i.options.error_init = Some(init.clone());
        assert!(
            matches!(refusal(&i), FitError::InvalidOption { option, .. } if option == "error_init"),
            "{model:?} {init:?}"
        );
    }
    let mut i = f_data(ErrorModel::Combined);
    i.options.error_init = Some(vec![0.1, 0.05]);
    assert!(run(&i).is_ok());
}

#[test]
fn n_must_exceed_p_plus_q() {
    let mut i = f_data(ErrorModel::Combined);
    i.time.truncate(4);
    i.conc.truncate(4);
    match refusal(&i) {
        FitError::TooFewObservations {
            n,
            p,
            q,
            error_model,
        } => {
            assert_eq!((n, p, q), (4, 2, Some(2)));
            assert_eq!(error_model.as_deref(), Some("combined"));
        }
        e => panic!("{e:?}"),
    }
    // Four observations are enough for the one-parameter error models.
    let mut i = f_data(ErrorModel::Additive);
    i.time.truncate(4);
    i.conc.truncate(4);
    assert!(run(&i).is_ok());
}

#[test]
fn an_exact_fit_has_no_maximum_likelihood() {
    for model in [
        ErrorModel::Additive,
        ErrorModel::Proportional,
        ErrorModel::Combined,
        ErrorModel::CombinedSd,
    ] {
        let mut i = f_data(model);
        i.conc = T.iter().map(|t| 10.0 * libm::exp(-0.2 * t)).collect();
        assert_eq!(refusal(&i), FitError::ExactFit, "{model:?}");
    }
}

fn oral_with_zero_at_time_zero(model: ErrorModel) -> FitInput {
    let time = vec![0.0, 0.25, 0.5, 1.0, 2.0, 4.0, 6.0, 8.0, 12.0];
    let conc = vec![0.0, 3.1, 5.2, 6.1, 5.4, 3.7, 2.4, 1.6, 0.7];
    FitInput {
        model: ModelId::Oral1,
        dose: 100.0,
        time,
        conc,
        weighting: Weighting::Uniform,
        initial: BTreeMap::from([
            ("v".to_string(), 12.0),
            ("k".to_string(), 0.2),
            ("ka".to_string(), 1.5),
        ]),
        options: FitOptions::maximum_likelihood(model),
    }
}

#[test]
fn a_zero_prediction_cannot_carry_a_proportional_error() {
    match refusal(&oral_with_zero_at_time_zero(ErrorModel::Proportional)) {
        FitError::NonPositiveSigma { index, time, .. } => {
            assert_eq!(index, 0);
            assert_eq!(time, 0.0);
        }
        e => panic!("{e:?}"),
    }
    // The additive error has no such problem.
    assert!(run(&oral_with_zero_at_time_zero(ErrorModel::Additive)).is_ok());
}

#[test]
fn an_observed_zero_where_the_prediction_is_zero_makes_the_combined_likelihood_unbounded() {
    for model in [ErrorModel::Combined, ErrorModel::CombinedSd] {
        match refusal(&oral_with_zero_at_time_zero(model)) {
            FitError::UnboundedLikelihood { indices, times, .. } => {
                assert_eq!(indices, vec![0]);
                assert_eq!(times, vec![0.0]);
            }
            e => panic!("{model:?}: {e:?}"),
        }
    }
}

// ---------------------------------------------------------------- options

#[test]
fn least_squares_options_serialize_as_before() {
    let v = serde_json::to_value(FitOptions::default()).unwrap();
    for key in ["objective", "error_model", "error_init", "information"] {
        assert!(v.get(key).is_none(), "{key}");
    }
    let back: FitOptions = serde_json::from_value(v).unwrap();
    assert_eq!(back, FitOptions::default());
}

#[test]
fn ml_options_round_trip_and_default_to_500_iterations() {
    let o = FitOptions::maximum_likelihood(ErrorModel::CombinedSd);
    assert_eq!(o.max_iterations, 500);
    let v = serde_json::to_value(&o).unwrap();
    assert_eq!(v["objective"], "ml");
    assert_eq!(v["error_model"], "combined_sd");
    let back: FitOptions = serde_json::from_value(v).unwrap();
    assert_eq!(back, o);
    // A json without max_iterations gets the limit of the objective.
    let o: FitOptions =
        serde_json::from_value(json!({"objective": "ml", "error_model": "additive"})).unwrap();
    assert_eq!(o.max_iterations, 500);
    let o: FitOptions = serde_json::from_value(json!({})).unwrap();
    assert_eq!(o.max_iterations, FitOptions::default().max_iterations);
}

#[test]
fn an_unknown_objective_or_error_model_is_named() {
    let e = serde_json::from_value::<FitOptions>(json!({"objective": "reml"})).unwrap_err();
    let text = e.to_string();
    assert!(
        text.contains("objective") && text.contains("reml"),
        "{text}"
    );
    let e =
        serde_json::from_value::<FitOptions>(json!({"objective": "ml", "error_model": "poisson"}))
            .unwrap_err();
    let text = e.to_string();
    assert!(
        text.contains("error_model") && text.contains("poisson"),
        "{text}"
    );
    let e = serde_json::from_value::<FitOptions>(json!({"information": "observed"})).unwrap_err();
    assert!(e.to_string().contains("information"), "{e}");
}

// ---------------------------------------------------------------- the stepper

type Seen = (u32, f64, Vec<String>, Vec<f64>);

#[derive(Default)]
struct Recorder {
    first: Vec<Seen>,
    seen: Vec<Seen>,
}

impl Observer for Recorder {
    fn before_first_iteration(&mut self, p: &Progress<'_>) -> Control {
        self.first.push((
            p.iteration,
            p.objective,
            p.names.to_vec(),
            p.values.to_vec(),
        ));
        Control::Continue
    }
    fn after_iteration(&mut self, p: &Progress<'_>) -> Control {
        self.seen.push((
            p.iteration,
            p.objective,
            p.names.to_vec(),
            p.values.to_vec(),
        ));
        Control::Continue
    }
}

struct CancelAfter(u32);

impl Observer for CancelAfter {
    fn before_first_iteration(&mut self, _: &Progress<'_>) -> Control {
        if self.0 == 0 {
            Control::Cancel
        } else {
            Control::Continue
        }
    }
    fn after_iteration(&mut self, p: &Progress<'_>) -> Control {
        if p.iteration >= self.0 {
            Control::Cancel
        } else {
            Control::Continue
        }
    }
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn the_observer_sees_the_likelihood_and_the_error_parameters() {
    let i = f_data(ErrorModel::Proportional);
    let expected = run(&i).unwrap();
    assert!(expected.trace().len() > 2, "the proportional fit iterates");
    let mut rec = Recorder::default();
    let Outcome::Done(result) = FitRun::new(i.model, &i).unwrap().run(&mut rec) else {
        panic!("never cancelled");
    };
    // Watching changes no number.
    assert_eq!(
        bits(&Vec::from_iter(result.values().values().copied())),
        bits(&Vec::from_iter(expected.values().values().copied()))
    );
    assert_eq!(result.trace().len(), expected.trace().len());
    // Iteration 0 first, then 1, 2, ... with −2LL and the θ then the error parameter.
    let rows = expected.trace();
    assert_eq!(rec.first.len(), 1);
    assert_eq!(rec.first[0].0, 0);
    assert_eq!(rec.first[0].1.to_bits(), rows[0].m2ll.unwrap().to_bits());
    assert_eq!(rec.first[0].2, vec!["v", "k", "error_b"]);
    assert_eq!(rec.seen.len() + 1, rows.len());
    for (k, (iteration, objective, names, values)) in rec.seen.iter().enumerate() {
        assert_eq!(*iteration as usize, k + 1);
        assert_eq!(objective.to_bits(), rows[k + 1].m2ll.unwrap().to_bits());
        assert_eq!(names, &["v", "k", "error_b"]);
        assert_eq!(bits(values), bits(&rows[k + 1].estimates));
    }
}

#[test]
fn a_loop_of_step_equals_run_for_ml() {
    for model in [
        ErrorModel::Additive,
        ErrorModel::Proportional,
        ErrorModel::Combined,
    ] {
        let i = f_data(model);
        let expected = run(&i).unwrap();
        let mut fit = FitRun::new(i.model, &i).unwrap();
        let mut committed = 0usize;
        let result = loop {
            match fit.step() {
                Step::Progress(p) => {
                    committed += 1;
                    assert_eq!(p.iteration as usize, committed);
                }
                Step::Finished(r) => break r,
            }
        };
        assert_eq!(committed, expected.trace().len() - 1, "{model:?}");
        assert_eq!(result.status(), expected.status(), "{model:?}");
        for (name, x) in expected.values() {
            assert_eq!(
                result.get(name).map(f64::to_bits),
                Some(x.to_bits()),
                "{name}"
            );
        }
        // Asking again gives the same result again.
        let Step::Finished(again) = fit.step() else {
            panic!("a finished run stepped");
        };
        assert_eq!(
            again.get("m2ll").map(f64::to_bits),
            expected.get("m2ll").map(f64::to_bits)
        );
    }
}

#[test]
fn a_cancel_returns_the_ml_trace_up_to_that_iteration() {
    let i = f_data(ErrorModel::Proportional);
    let expected = run(&i).unwrap();
    let last = u32::try_from(expected.trace().len() - 1).unwrap();
    for n in [0, 1, last - 1] {
        match FitRun::new(i.model, &i).unwrap().run(&mut CancelAfter(n)) {
            Outcome::Cancelled {
                after_iteration,
                trace,
            } => {
                assert_eq!(after_iteration, n);
                assert_eq!(trace.len(), n as usize + 1);
                for (a, b) in trace.iter().zip(expected.trace()) {
                    assert_eq!(a.m2ll.map(f64::to_bits), b.m2ll.map(f64::to_bits));
                    assert_eq!(bits(&a.estimates), bits(&b.estimates));
                }
            }
            Outcome::Done(_) => panic!("n = {n} of {last} was not cancelled"),
        }
    }
}

#[test]
fn the_iteration_limit_ends_an_ml_fit_as_max_iterations() {
    let mut i = f_data(ErrorModel::Proportional);
    i.options.max_iterations = 1;
    let r = run(&i).unwrap();
    assert_eq!(r.status(), FitStatus::MaxIterations);
    assert_eq!(r.trace().len(), 2);
    assert!(
        r.flags()
            .iter()
            .any(|f| matches!(f, FitFlag::NotConverged { .. }))
    );
    assert!(r.get("m2ll").is_some());
    // FIT-ML-17, FIT-ML-18, FIT-CNV-03: the statistics are shown, with the mark, not hidden.
    for name in [
        "iwres_mean",
        "iwres_sd",
        "n_iwres_gt2",
        "se.v",
        "se.error_b",
        "cv_percent.v",
    ] {
        assert!(r.get(name).is_some(), "{name}");
    }
    // Secondary parameters: the delta method on the same covariance, no interval for ml.
    assert!(r.get("se.cl").is_some() && r.get("cv_percent.cl").is_some());
    assert!(r.get("ci_lo.cl").is_none() && r.get("ci_lo.v").is_none());
}

// ------------------------------------------------- the Newton polish (OF-19, FIT-ML-08)

#[test]
fn the_polish_is_marked_in_the_trace_and_follows_its_acceptance_rule() {
    let mut polished = 0;
    for model in [
        ErrorModel::Proportional,
        ErrorModel::Combined,
        ErrorModel::CombinedSd,
    ] {
        let r = run(&f_data(model)).unwrap();
        let rows = r.trace();
        // Iteration numbers go on one by one, polish rows included.
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.iteration, i, "{model:?}");
        }
        // The polish rows close the trace: a scoring row never follows one.
        let first = rows.iter().position(|row| row.polish).unwrap_or(rows.len());
        assert!(rows[first..].iter().all(|row| row.polish), "{model:?}");
        assert!(!rows[0].polish);
        // The rule: -2LL does not increase and the scaled score (decrement) decreases.
        for pair in rows[first.saturating_sub(1)..].windows(2) {
            let (before, after) = (&pair[0], &pair[1]);
            assert!(after.polish);
            assert!(after.m2ll.unwrap() <= before.m2ll.unwrap(), "{model:?}");
            assert!(
                after.decrement.unwrap() < before.decrement.unwrap(),
                "{model:?}"
            );
            assert_eq!(after.step, Some(1.0));
            assert_eq!(after.lambda, 0.0);
            assert_eq!(after.halvings, 0);
        }
        // The last scoring row met the test of FIT-ML-08.
        let last_scoring = rows[..first].last().unwrap();
        assert!(last_scoring.decrement.unwrap() <= 1e-10, "{model:?}");
        // The returned values are the last row's.
        let last = rows.last().unwrap();
        assert_eq!(
            r.get("estimate.v").unwrap().to_bits(),
            last.estimates[0].to_bits()
        );
        assert_eq!(
            r.get("estimate.k").unwrap().to_bits(),
            last.estimates[1].to_bits()
        );
        assert_eq!(r.get("m2ll").map(f64::to_bits), last.m2ll.map(f64::to_bits));
        polished += rows.iter().filter(|row| row.polish).count();
    }
    assert!(polished > 0, "the polish is exercised on the F data");
}

#[test]
fn only_the_likelihood_trace_has_polish_rows() {
    let ml = serde_json::to_value(run(&f_data(ErrorModel::Combined)).unwrap().trace()).unwrap();
    assert!(ml.to_string().contains("\"polish\":true"));
    let mut ls = f_data(ErrorModel::Additive);
    ls.options = FitOptions::default();
    let ls = serde_json::to_value(run(&ls).unwrap().trace()).unwrap();
    assert!(!ls.to_string().contains("polish"));
    // Absent means false when a trace is read back.
    let row: TraceRow = serde_json::from_value(json!({
        "iteration": 0, "wrss": 1.0, "estimates": [1.0], "step": null,
        "lambda": 0.0, "halvings": 0, "relative_decrease": null
    }))
    .unwrap();
    assert!(!row.polish);
}

// ------------------------------------- the unbounded likelihood by structure (FIT-ML-13c)

const ONE_COMPARTMENT: [ModelId; 6] = ModelId::ALL;

fn zero_at(model: ModelId, time0: f64) -> FitInput {
    let mut input = oral_with_zero_at_time_zero(ErrorModel::Combined);
    input.model = model;
    input.time[0] = time0;
    input.initial = BTreeMap::from([("v".to_string(), 12.0), ("k".to_string(), 0.2)]);
    for name in model.input_parameters() {
        let value = match *name {
            "ka" => 1.5,
            "dur" => 1.0,
            _ => 0.2,
        };
        input.initial.insert((*name).to_string(), value);
    }
    input
}

#[test]
fn a_model_is_silent_at_the_dose_time_when_its_input_is_not_a_bolus() {
    for model in ModelId::ALL.into_iter().chain(ModelId::TWO_COMPARTMENT) {
        let bolus = matches!(model, ModelId::IvBolus | ModelId::Pk2IvBolus);
        assert_eq!(model.silent_at_dose_time(), !bolus, "{}", model.id());
    }
}

#[test]
fn an_observed_zero_at_the_dose_time_is_unbounded_for_every_input_but_the_bolus() {
    for model in ONE_COMPARTMENT {
        for error_model in [ErrorModel::Combined, ErrorModel::CombinedSd] {
            let mut input = zero_at(model, 0.0);
            input.options = FitOptions::maximum_likelihood(error_model);
            let outcome = run(&input);
            if model == ModelId::IvBolus {
                // The prediction is D/V at t = 0: nothing is silent, nothing is refused so.
                assert!(
                    !matches!(outcome, Err(FitError::UnboundedLikelihood { .. })),
                    "{} {error_model:?}",
                    model.id()
                );
            } else {
                match outcome {
                    Err(FitError::UnboundedLikelihood { indices, times, .. }) => {
                        assert_eq!(indices, vec![0], "{}", model.id());
                        assert_eq!(times, vec![0.0]);
                    }
                    other => panic!(
                        "{} {error_model:?}: {:?}",
                        model.id(),
                        other.map(|r| r.status())
                    ),
                }
            }
        }
    }
}

#[test]
fn a_zero_after_the_dose_time_is_refused_only_through_a_lag() {
    for model in ONE_COMPARTMENT {
        let mut after = zero_at(model, 0.1);
        after.options = FitOptions::maximum_likelihood(ErrorModel::Combined);
        let lagged = model.lag_parameter().is_some();
        match run(&after) {
            // The first observation is a zero at 0.1 and tlag (no upper bound) can reach it.
            Err(FitError::UnboundedLikelihood {
                lag,
                indices,
                times,
                ..
            }) => {
                assert!(lagged && lag, "{}", model.id());
                assert_eq!((indices, times), (vec![0], vec![0.1]));
            }
            other => assert!(
                !lagged && !matches!(other, Err(FitError::UnboundedLikelihood { .. })),
                "{}",
                model.id()
            ),
        }
        for error_model in [ErrorModel::Additive, ErrorModel::Proportional] {
            let mut input = zero_at(model, 0.0);
            input.options = FitOptions::maximum_likelihood(error_model);
            assert!(
                !matches!(run(&input), Err(FitError::UnboundedLikelihood { .. })),
                "{} {error_model:?}",
                model.id()
            );
        }
    }
}

// ------------------------------------------ the lag rule (FIT-ML-13c, review of T-149, A1)

/// The reviewer's scenario: `pk1.oral_1_lag`, the first observation 0 at 0.25, start tlag 0.1.
fn lagged_zero(error_model: ErrorModel) -> FitInput {
    FitInput {
        model: ModelId::Oral1Lag,
        dose: 100.0,
        time: vec![0.25, 0.5, 1.0, 2.0, 4.0, 6.0, 8.0, 12.0],
        conc: vec![0.0, 4.2, 6.1, 5.4, 3.7, 2.4, 1.6, 0.7],
        weighting: Weighting::Uniform,
        initial: BTreeMap::from([
            ("v".to_string(), 12.0),
            ("k".to_string(), 0.2),
            ("ka".to_string(), 1.5),
            ("tlag".to_string(), 0.1),
        ]),
        options: FitOptions::maximum_likelihood(error_model),
    }
}

fn is_unbounded_by_lag(input: &FitInput) -> Option<(Vec<usize>, Vec<f64>)> {
    match run(input) {
        Err(FitError::UnboundedLikelihood {
            lag: true,
            indices,
            times,
            ..
        }) => Some((indices, times)),
        Err(FitError::UnboundedLikelihood { lag: false, .. }) => panic!("refused at the dose time"),
        _ => None,
    }
}

#[test]
fn a_leading_zero_that_tlag_can_reach_is_unbounded_under_three_error_models() {
    for error_model in [
        ErrorModel::Combined,
        ErrorModel::CombinedSd,
        ErrorModel::Proportional,
    ] {
        let input = lagged_zero(error_model);
        assert_eq!(
            is_unbounded_by_lag(&input),
            Some((vec![0], vec![0.25])),
            "{error_model:?}"
        );
        let message = run(&input).unwrap_err().to_string();
        for word in ["time 0.25", "lag", "Exclude", "tlag", "additive"] {
            assert!(message.contains(word), "{error_model:?}: {message}");
        }
    }
    // The additive error model has no such problem.
    assert_eq!(
        is_unbounded_by_lag(&lagged_zero(ErrorModel::Additive)),
        None
    );
}

#[test]
fn the_domain_of_tlag_decides_the_lag_rule() {
    let bounded = |lower: Option<f64>, upper: Option<f64>| {
        let mut input = lagged_zero(ErrorModel::Combined);
        input
            .initial
            .insert("tlag".to_string(), lower.unwrap_or(0.1));
        input
            .options
            .bounds
            .insert("tlag".to_string(), Bounds { lower, upper });
        is_unbounded_by_lag(&input).is_some()
    };
    // Upper bound at the time of the zero or above: the domain reaches it.
    assert!(bounded(None, Some(0.25)));
    assert!(bounded(None, Some(3.0)));
    // Upper bound below it: the prediction of that row stays above 0.
    assert!(!bounded(None, Some(0.2)));
    // The domain must also begin before the first nonzero observation (0.5).
    assert!(bounded(Some(0.2), Some(3.0)));
    assert!(!bounded(Some(0.5), Some(3.0)));
    // A fixed tlag is both bounds.
    let fixed = |c: f64| {
        let mut input = lagged_zero(ErrorModel::Combined);
        input.initial.remove("tlag");
        input.options.fixed.insert("tlag".to_string(), c);
        is_unbounded_by_lag(&input).is_some()
    };
    assert!(fixed(0.25));
    assert!(fixed(0.4));
    assert!(!fixed(0.2));
    // Fixed at or above the first nonzero observation: that row is predicted 0 too, and its
    // error then sends -2LL to +infinity, not -infinity, so this is not refused for this reason.
    assert!(!fixed(0.5));
    assert!(!fixed(3.0));
}

#[test]
fn a_zero_after_a_nonzero_observation_is_an_ordinary_observation() {
    let mut input = lagged_zero(ErrorModel::Combined);
    input.conc = vec![3.0, 0.0, 6.1, 5.4, 3.7, 2.4, 1.6, 0.7];
    assert_eq!(is_unbounded_by_lag(&input), None);
    // Several leading zeros are all named; the zero after the nonzero one is not.
    input.conc = vec![0.0, 0.0, 6.1, 0.0, 3.7, 2.4, 1.6, 0.7];
    assert_eq!(
        is_unbounded_by_lag(&input),
        Some((vec![0, 1], vec![0.25, 0.5]))
    );
    // Not a lagged route.
    let mut plain = lagged_zero(ErrorModel::Combined);
    plain.model = ModelId::Oral1;
    plain.initial.remove("tlag");
    assert_eq!(is_unbounded_by_lag(&plain), None);
}

#[test]
fn the_lag_rule_covers_the_two_compartment_routes() {
    let initial = BTreeMap::from([
        ("cl".to_string(), 2.0),
        ("vc".to_string(), 10.0),
        ("q".to_string(), 4.0),
        ("vp".to_string(), 8.0),
        ("ka".to_string(), 2.0),
        ("tlag".to_string(), 0.1),
    ]);
    let mut input = lagged_zero(ErrorModel::Combined);
    input.model = ModelId::Pk2Oral1Lag;
    input.initial = initial;
    // N must exceed P + Q (FIT-ML-14a): six parameters and two error parameters.
    input.time = vec![0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 24.0];
    input.conc = vec![0.0, 4.2, 6.1, 5.4, 4.6, 3.7, 2.4, 1.6, 0.7, 0.2];
    assert_eq!(is_unbounded_by_lag(&input), Some((vec![0], vec![0.25])));
    assert!(ModelId::Pk2Oral1Lag.silent_at_dose_time());
    for model in ModelId::ALL.into_iter().chain(ModelId::TWO_COMPARTMENT) {
        assert_eq!(
            model.lag_parameter().is_some(),
            matches!(
                model,
                ModelId::Oral1Lag | ModelId::Oral0Lag | ModelId::Pk2Oral1Lag | ModelId::Pk2Oral0Lag
            ),
            "{}",
            model.id()
        );
    }
}

#[test]
fn the_lag_refusal_serialises_with_its_flag_and_the_dose_time_one_without() {
    let lagged =
        serde_json::to_value(run(&lagged_zero(ErrorModel::Combined)).unwrap_err()).unwrap();
    assert_eq!(lagged["code"], "unbounded_likelihood");
    assert_eq!(lagged["lag"], true);
    let at_dose =
        serde_json::to_value(run(&oral_with_zero_at_time_zero(ErrorModel::Combined)).unwrap_err())
            .unwrap();
    assert_eq!(at_dose["code"], "unbounded_likelihood");
    assert!(at_dose.get("lag").is_none());
}

// ------------------------------------------------------------------ the exact-fit status

#[test]
fn the_exact_fit_status_has_its_own_name_and_message() {
    assert_eq!(
        serde_json::to_value(FitStatus::ExactFit).unwrap(),
        json!("exact_fit")
    );
    let back: FitStatus = serde_json::from_value(json!("exact_fit")).unwrap();
    assert_eq!(back, FitStatus::ExactFit);
    assert!(!FitStatus::ExactFit.message().is_empty());
    assert_ne!(
        FitStatus::ExactFit.message(),
        FitStatus::NonFinite.message()
    );
}
