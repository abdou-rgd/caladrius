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
}
