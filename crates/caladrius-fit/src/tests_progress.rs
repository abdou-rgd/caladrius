//! Unit tests of task T-143: the stepper (`FitRun`) and its observer change no number. Results are
//! compared field by field with `to_bits()` on every float, never with a tolerance: one loop, the
//! same sequence of floating-point operations (golden rule 10).

use std::collections::BTreeMap;

use crate::*;

/// Deterministic multiplicative noise, so that the data are not exactly on the model.
const NOISE: [f64; 8] = [1.04, 0.97, 1.02, 0.98, 1.03, 0.99, 1.05, 0.96];

fn noisy(model: ModelId, dose: f64, params: &[(&str, f64)], time: &[f64]) -> Vec<f64> {
    let params: BTreeMap<String, f64> = params.iter().map(|(n, v)| (n.to_string(), *v)).collect();
    let exact = model.predict(dose, &params, time).unwrap();
    exact
        .iter()
        .zip(NOISE.iter().cycle())
        .map(|(c, e)| c * e)
        .collect()
}

fn initial(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(n, v)| (n.to_string(), *v)).collect()
}

const TIMES: [f64; 10] = [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 24.0];

fn input(
    model: ModelId,
    truth: &[(&str, f64)],
    start: &[(&str, f64)],
    weighting: Weighting,
    options: FitOptions,
) -> FitInput {
    FitInput {
        model,
        dose: 100.0,
        time: TIMES.to_vec(),
        conc: noisy(model, 100.0, truth, &TIMES),
        weighting,
        initial: initial(start),
        options,
    }
}

/// Subject 1 of `oracle/data/fit/synthetic_infusion.csv`, as in `tests_stopping.rs`.
fn infusion_subject_1() -> FitInput {
    FitInput {
        model: ModelId::IvInfusion,
        dose: 100.0,
        time: vec![
            0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0,
        ],
        conc: vec![
            1.841, 3.418, 4.44, 5.785, 5.718, 4.244, 3.088, 1.817, 1.182, 0.3836, 0.1278, 0.01712,
        ],
        weighting: Weighting::InvY,
        initial: initial(&[("v", 16.2), ("k", 0.229)]),
        options: FitOptions {
            derivatives: Derivatives::ForwardDifference,
            increment: 0.001,
            convergence: 1e-10,
            max_iterations: 200,
            fixed: BTreeMap::from([("dur".to_string(), 2.0)]),
            ..FitOptions::default()
        },
    }
}

/// The cases: every model family used by the crate's tests, every weighting family, and the
/// endings `converged`, `max_iterations` and a failure.
fn cases() -> Vec<(&'static str, FitInput)> {
    let mut spec_limit = input(
        ModelId::IvBolus,
        &[("v", 10.0), ("k", 0.2)],
        &[("v", 12.0), ("k", 0.15)],
        Weighting::Uniform,
        FitOptions {
            derivatives: Derivatives::Analytic,
            convergence: 1e-12,
            max_iterations: 1,
            ..FitOptions::default()
        },
    );
    spec_limit.dose = 100.0;
    vec![
        (
            "pk1.iv_bolus uniform, analytic, converged",
            input(
                ModelId::IvBolus,
                &[("v", 10.0), ("k", 0.2)],
                &[("v", 12.0), ("k", 0.15)],
                Weighting::Uniform,
                FitOptions::default(),
            ),
        ),
        ("pk1.iv_bolus, the iteration limit (1)", spec_limit),
        (
            "pk1.iv_bolus 1/yhat, reference conventions",
            input(
                ModelId::IvBolus,
                &[("v", 10.0), ("k", 0.2)],
                &[("v", 12.0), ("k", 0.15)],
                Weighting::InvYhat,
                FitOptions::reference_conventions(),
            ),
        ),
        (
            "pk1.iv_bolus 1/yhat2, relative offset",
            input(
                ModelId::IvBolus,
                &[("v", 10.0), ("k", 0.2)],
                &[("v", 12.0), ("k", 0.15)],
                Weighting::InvYhat2,
                FitOptions {
                    criterion: Criterion::RelativeOffset,
                    ..FitOptions::default()
                },
            ),
        ),
        (
            "pk1.oral_1 1/y, forward differences",
            input(
                ModelId::Oral1,
                &[("v", 10.0), ("k", 0.2), ("ka", 1.0)],
                &[("v", 8.0), ("k", 0.3), ("ka", 0.7)],
                Weighting::InvY,
                FitOptions {
                    derivatives: Derivatives::ForwardDifference,
                    ..FitOptions::default()
                },
            ),
        ),
        (
            "pk1.oral_1_lag 1/yhat2, bounds",
            input(
                ModelId::Oral1Lag,
                &[("v", 10.0), ("k", 0.2), ("ka", 1.0), ("tlag", 0.25)],
                &[("v", 8.0), ("k", 0.15), ("ka", 0.7), ("tlag", 0.1)],
                Weighting::InvYhat2,
                FitOptions {
                    bounds: BTreeMap::from([(
                        "k".to_string(),
                        Bounds {
                            lower: None,
                            upper: Some(0.19),
                        },
                    )]),
                    ..FitOptions::default()
                },
            ),
        ),
        (
            "pk1.iv_infusion, dur fixed, derivative floor",
            infusion_subject_1(),
        ),
        (
            "pk2.iv_bolus uniform",
            input(
                ModelId::Pk2IvBolus,
                &[("k10", 0.2), ("k12", 0.4), ("k21", 0.5), ("vc", 10.0)],
                &[("k10", 0.25), ("k12", 0.3), ("k21", 0.4), ("vc", 9.0)],
                Weighting::Uniform,
                FitOptions::default(),
            ),
        ),
        (
            "pk1.oral_1_lag, lag after every observation (singular)",
            singular_case(),
        ),
        (
            "pk1.oral_1, far start, 1/yhat (no decrease, rollback)",
            far_start_case(),
        ),
    ]
}

fn singular_case() -> FitInput {
    // The lag is later than every observation: the model is zero there and every column of the
    // Jacobian is zero.
    input(
        ModelId::Oral1Lag,
        &[("v", 10.0), ("k", 0.2), ("ka", 1.0), ("tlag", 0.25)],
        &[("v", 8.0), ("k", 0.3), ("ka", 0.7), ("tlag", 100.0)],
        Weighting::Uniform,
        FitOptions::default(),
    )
}

fn far_start_case() -> FitInput {
    input(
        ModelId::Oral1,
        &[("v", 10.0), ("k", 0.2), ("ka", 1.0)],
        &[("v", 1e4), ("k", 5.0), ("ka", 0.001)],
        Weighting::InvYhat,
        FitOptions::reference_conventions(),
    )
}

fn same_f64(a: f64, b: f64, what: &str) {
    assert_eq!(a.to_bits(), b.to_bits(), "{what}: {a} against {b}");
}

fn same_opt(a: Option<f64>, b: Option<f64>, what: &str) {
    assert_eq!(
        a.map(f64::to_bits),
        b.map(f64::to_bits),
        "{what}: {a:?} against {b:?}"
    );
}

fn same_slice(a: &[f64], b: &[f64], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: length");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        same_f64(*x, *y, &format!("{what}[{i}]"));
    }
}

fn same_row(a: &TraceRow, b: &TraceRow, what: &str) {
    assert_eq!(a.iteration, b.iteration, "{what}: iteration");
    same_f64(a.wrss, b.wrss, &format!("{what}: wrss"));
    same_slice(&a.estimates, &b.estimates, &format!("{what}: estimates"));
    same_opt(a.step, b.step, &format!("{what}: step"));
    same_f64(a.lambda, b.lambda, &format!("{what}: lambda"));
    assert_eq!(a.halvings, b.halvings, "{what}: halvings");
    same_opt(
        a.relative_decrease,
        b.relative_decrease,
        &format!("{what}: relative_decrease"),
    );
}

fn same_rows(a: &[TraceRow], b: &[TraceRow], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: number of trace rows");
    for (x, y) in a.iter().zip(b) {
        same_row(x, y, &format!("{what}: row {}", x.iteration));
    }
}

/// Every field of a result, every float by its bits.
#[track_caller]
fn same_result(a: &FitResult, b: &FitResult, what: &str) {
    assert_eq!(a.status(), b.status(), "{what}: status");
    assert_eq!(a.parameters(), b.parameters(), "{what}: parameters");
    assert_eq!(a.derivatives(), b.derivatives(), "{what}: derivatives");
    // Flags carry floats; `Debug` prints each with the digits that round-trip it, the sign of a
    // zero and NaN included.
    assert_eq!(
        format!("{:?}", a.flags()),
        format!("{:?}", b.flags()),
        "{what}: flags"
    );
    assert_eq!(a.values().len(), b.values().len(), "{what}: values");
    for ((na, xa), (nb, xb)) in a.values().iter().zip(b.values()) {
        assert_eq!(na, nb, "{what}: value names");
        same_f64(*xa, *xb, &format!("{what}: {na}"));
    }
    assert_eq!(
        a.observations().len(),
        b.observations().len(),
        "{what}: observations"
    );
    for (i, (x, y)) in a.observations().iter().zip(b.observations()).enumerate() {
        for (field, u, v) in [
            ("time", x.time, y.time),
            ("observed", x.observed, y.observed),
            ("predicted", x.predicted, y.predicted),
            ("residual", x.residual, y.residual),
            (
                "weighted_residual",
                x.weighted_residual,
                y.weighted_residual,
            ),
            ("weight", x.weight, y.weight),
        ] {
            same_f64(u, v, &format!("{what}: observation {i} {field}"));
        }
    }
    assert_eq!(a.curve().len(), b.curve().len(), "{what}: curve");
    for (i, (x, y)) in a.curve().iter().zip(b.curve()).enumerate() {
        same_f64(x.time, y.time, &format!("{what}: curve {i} time"));
        same_f64(x.conc, y.conc, &format!("{what}: curve {i} conc"));
    }
    assert_eq!(a.partials().len(), b.partials().len(), "{what}: partials");
    for (j, (x, y)) in a.partials().iter().zip(b.partials()).enumerate() {
        same_slice(x, y, &format!("{what}: partials {j}"));
    }
    same_rows(a.trace(), b.trace(), what);
}

type Seen = (u32, f64, Vec<String>, Vec<f64>);

/// Records everything it is offered and never cancels.
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

/// Cancels after iteration `n` (0: before the first).
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

fn done(outcome: Outcome) -> FitResult {
    match outcome {
        Outcome::Done(r) => r,
        Outcome::Cancelled {
            after_iteration, ..
        } => {
            panic!("cancelled after {after_iteration} by an observer that never cancels")
        }
    }
}

/// The reference: `run`, which is `solve`.
fn reference(i: &FitInput) -> FitResult {
    run(i).unwrap()
}

#[test]
fn the_cases_cover_the_iteration_limit_and_failures() {
    let statuses: Vec<FitStatus> = cases().iter().map(|(_, i)| reference(i).status()).collect();
    for wanted in [
        FitStatus::Converged,
        FitStatus::MaxIterations,
        FitStatus::NoDecrease,
        FitStatus::Singular,
    ] {
        assert!(statuses.contains(&wanted), "no case ends {wanted:?}");
    }
}

#[test]
fn a_recording_observer_changes_nothing() {
    for (name, i) in cases() {
        let expected = reference(&i);
        // Owned model.
        let owned = done(
            FitRun::new(i.model, &i)
                .unwrap()
                .run(&mut Recorder::default()),
        );
        same_result(&owned, &expected, name);
        // Lent model, through the trait object `run_model` takes.
        let dynamic: &dyn FitModel = &i.model;
        let lent = done(
            FitRun::new(dynamic, &i)
                .unwrap()
                .run(&mut Recorder::default()),
        );
        same_result(&lent, &expected, name);
    }
}

#[test]
fn a_loop_of_step_equals_solve() {
    for (name, i) in cases() {
        let expected = reference(&i);
        let mut fit = FitRun::new(i.model, &i).unwrap();
        let mut committed = 0usize;
        let result = loop {
            match fit.step() {
                Step::Progress(p) => {
                    committed += 1;
                    assert_eq!(p.iteration as usize, committed, "{name}");
                }
                Step::Finished(r) => break r,
            }
        };
        same_result(&result, &expected, name);
        assert_eq!(committed, expected.trace().len() - 1, "{name}");
        // Asking again gives the same result again.
        let Step::Finished(again) = fit.step() else {
            panic!("{name}: a finished run stepped");
        };
        same_result(&again, &expected, name);
    }
}

#[test]
fn the_observer_sees_each_iteration_once_with_its_trace_row() {
    for (name, i) in cases() {
        let expected = reference(&i);
        let mut rec = Recorder::default();
        let _ = done(FitRun::new(i.model, &i).unwrap().run(&mut rec));
        let rows = expected.trace();
        // Before the first iteration: iteration 0, the first row.
        assert_eq!(rec.first.len(), 1, "{name}");
        let (it0, obj0, names0, values0) = &rec.first[0];
        assert_eq!(*it0, 0, "{name}");
        same_f64(*obj0, rows[0].wrss, name);
        same_slice(values0, &rows[0].estimates, name);
        assert_eq!(names0, expected.parameters(), "{name}");
        // After each committed iteration: 1, 2, ... in order, the wrss and estimates of its row.
        assert_eq!(rec.seen.len() + 1, rows.len(), "{name}");
        for (k, (it, obj, names, values)) in rec.seen.iter().enumerate() {
            assert_eq!(*it as usize, k + 1, "{name}: strictly increasing from 1");
            let row = &rows[k + 1];
            assert_eq!(row.iteration, k + 1, "{name}");
            same_f64(*obj, row.wrss, &format!("{name}: objective at {it}"));
            same_slice(values, &row.estimates, &format!("{name}: values at {it}"));
            assert_eq!(names, expected.parameters(), "{name}");
        }
    }
}

#[test]
fn a_cancel_returns_the_trace_up_to_that_iteration() {
    let mut cancels = 0;
    for (name, i) in cases() {
        let expected = reference(&i);
        let last = u32::try_from(expected.trace().len() - 1).unwrap();
        // Before the first iteration, after the first, and after the last but one.
        let mut ns = vec![0, 1u32.min(last), last.saturating_sub(1)];
        ns.dedup();
        for n in ns {
            match FitRun::new(i.model, &i).unwrap().run(&mut CancelAfter(n)) {
                Outcome::Cancelled {
                    after_iteration,
                    trace,
                } => {
                    cancels += 1;
                    assert_eq!(after_iteration, n, "{name}");
                    assert_eq!(trace.len(), n as usize + 1, "{name}: n = {n}");
                    same_rows(
                        &trace,
                        &expected.trace()[..=n as usize],
                        &format!("{name}: n = {n}"),
                    );
                }
                Outcome::Done(r) => {
                    panic!(
                        "{name}: n = {n} of {last} was not cancelled ({:?})",
                        r.status()
                    )
                }
            }
        }
    }
    assert!(cancels >= 15, "{cancels} cancels");
}

/// FIT-CNV-05 (T-143, review minor): a fit with predicted-value weights converges through the
/// contraction ratio ρ = √(decrease / previous decrease) < 1, not through a decrease of exactly 0.
/// With ε = 0.01 the test is met at iteration 3 although iteration 4 would still lower the WRSS (by
/// 2e-11 relative): a loop that forgot the previous decrease (reset it at every iteration) never
/// meets the test, goes on to iteration 4 and ends later with a longer trace.
#[test]
fn a_predicted_value_fit_converges_through_rho_below_one() {
    let (_, mut i) = cases()
        .into_iter()
        .find(|(n, _)| n.starts_with("pk1.iv_bolus 1/yhat, reference"))
        .unwrap();
    i.options.convergence = 1e-2;
    let r = reference(&i);
    assert_eq!(r.status(), FitStatus::Converged);
    let rows = r.trace();
    assert_eq!(rows.len(), 4, "converged at iteration 3");
    let last = rows[3].relative_decrease.unwrap();
    let prev = rows[2].relative_decrease.unwrap();
    assert!(last > 0.0, "not the relative == 0 shortcut");
    let rho = (last / prev).sqrt();
    assert!(rho < 1.0, "rho = {rho}");
    assert!(
        last <= 1e-2 * (1.0 - rho) * (1.0 - rho),
        "the FIT-CNV-05 test"
    );
    // The same fit with the usual ε needs the next iteration too: the criterion is what stops it.
    i.options.convergence = 1e-4;
    assert_eq!(reference(&i).trace().len(), 5);
}
