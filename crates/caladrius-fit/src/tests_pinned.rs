//! Tests of task T-143 (revision): the loop of `FitRun` is pinned bit for bit against the loop it
//! replaced. The constants below were printed by `caladrius_fit::run` at commit 0ef234c, where the
//! minimisation was still inline in `engine.rs`, through `FitResult::trace()`; nothing here was
//! computed by the stepper. A state field that `FitRun` reset or forgot between two iterations
//! moves a bit of a row and fails here, where the tests that compare the stepper with itself
//! cannot see it (golden rule 10).

use std::collections::BTreeMap;

use crate::*;

/// One trace row: iteration, WRSS, estimates (v, k, ka), step, lambda, halvings, relative decrease;
/// every float as `to_bits()`.
type Row = (usize, u64, [u64; 3], Option<u64>, u64, usize, Option<u64>);

const NOISE: [f64; 8] = [1.04, 0.97, 1.02, 0.98, 1.03, 0.99, 1.05, 0.96];
const TIMES: [f64; 10] = [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 24.0];

fn oral_1(start: [f64; 3], weighting: Weighting, options: FitOptions) -> FitInput {
    let truth = BTreeMap::from([
        ("v".to_string(), 10.0),
        ("k".to_string(), 0.2),
        ("ka".to_string(), 1.0),
    ]);
    let exact = ModelId::Oral1.predict(100.0, &truth, &TIMES).unwrap();
    FitInput {
        model: ModelId::Oral1,
        dose: 100.0,
        time: TIMES.to_vec(),
        conc: exact
            .iter()
            .zip(NOISE.iter().cycle())
            .map(|(c, e)| c * e)
            .collect(),
        weighting,
        initial: BTreeMap::from([
            ("v".to_string(), start[0]),
            ("k".to_string(), start[1]),
            ("ka".to_string(), start[2]),
        ]),
        options,
    }
}

/// Far start with 1/yhat weights and the reference conventions: the predicted-value weights of
/// FIT-CNV-05, damping up to 1e4, the rollback to the best iterate.
fn far() -> FitInput {
    oral_1(
        [1e4, 5.0, 0.001],
        Weighting::InvYhat,
        FitOptions::reference_conventions(),
    )
}

/// Start (3, 3, 3), uniform weights, default options: damping lambda > 0 on rows 1 to 8, back to
/// pure Gauss-Newton (lambda 0) from row 9 to convergence.
fn damped() -> FitInput {
    oral_1([3.0, 3.0, 3.0], Weighting::Uniform, FitOptions::default())
}

const FAR_ROWS: [Row; 5] = [
    (0, 0x4196b10e182709d2, [0x40c3880000000000, 0x4014000000000000, 0x3f50624dd2f1a9fc], None, 0x0000000000000000, 0, None),
    (1, 0x4193998759b47e92, [0x4184e042ca626856, 0x3ed4f8b588e368f0, 0x401d59c4d1214088], Some(0x3f60000000000000), 0x3f50624dd2f1a9fc, 9, Some(0x3e81863a1f515c7c)),
    (2, 0x417ae2b58655a1c1, [0x416cd58688f35d68, 0x3ed4f8b588e368f0, 0x40346609533ff5ba], Some(0x3f70000000000000), 0x40c3880000000000, 8, Some(0x3ebda698424c6671)),
    (3, 0x4167f54a3e850cd7, [0x4159b2eb1ccc7409, 0x3ed4f8b588e368f0, 0x406e32d5c10c0e4b], Some(0x3f50000000000000), 0x408f400000000000, 10, Some(0x3ecbceef39b7093d)),
    (4, 0x4167da99cd925d09, [0x3f847ae147ae147b, 0x3ed4f8b588e368f0, 0x3e112e0be826d695], Some(0x3ff0000000000000), 0x4059000000000000, 0, Some(0x3edd7bae4e7155d3)),
];
const FAR_FINAL: [u64; 4] = [0x3f847ae147ae147b, 0x3ed4f8b588e368f0, 0x3e112e0be826d695, 0x4167da99cd925d09];

const DAMPED_ROWS: [Row; 19] = [
    (0, 0x40715861287864d5, [0x4008000000000000, 0x4008000000000000, 0x4008000000000000], None, 0x0000000000000000, 0, None),
    (1, 0x40674e6295f70c1d, [0x400e4e1361e7162d, 0x4008b0820dfd6e2b, 0x3ec92a737110e454], Some(0x3ff0000000000000), 0x3f50624dd2f1a9fc, 0, Some(0x3fdf430544aa84dc)),
    (2, 0x40674de05beb7259, [0x3ff79b861ddb2834, 0x3ff261051522e77d, 0x3ed443d89a441756], Some(0x3f70000000000000), 0x408f400000000000, 8, Some(0x3f165a3ca8e4bd9d)),
    (3, 0x40674b00124b0055, [0x3fe4f4cdb7525cd1, 0x3fe120857870a842, 0x3edf893d3bdb2f46], Some(0x3f70000000000000), 0x4059000000000000, 8, Some(0x3f3f9c2008eb902f)),
    (4, 0x40673a7b7779c464, [0x3fd0a5bcdc6f5b18, 0x3fd17927ee437de3, 0x3ee9462bb6882da2], Some(0x3f70000000000000), 0x4024000000000000, 8, Some(0x3f66c16a5ea9427e)),
    (5, 0x4066f097356cdbe9, [0x3fb6b0ebf2b52234, 0x3fd29083656cf3ad, 0x3ef4f86a52c38336], Some(0x3f80000000000000), 0x3ff0000000000000, 7, Some(0x3f89c4d518f9955b)),
    (6, 0x40645ee2d1926d60, [0x3f887d0c47bef4e8, 0x3fe5f08cb3ee70e7, 0x3f03902df37047de], Some(0x3f90000000000000), 0x3fb999999999999a, 6, Some(0x3fc024b86dbe9979)),
    (7, 0x4053175d64c6c6d8, [0x3f45f2edcb92ad80, 0x3ffcff2d7fe2a5ca, 0x3f130f55842bf018], Some(0x3fb0000000000000), 0x3f847ae147ae147b, 4, Some(0x3ff224fa413032f9)),
    (8, 0x404e808034057ea0, [0x3f40b7d4f30f327d, 0x400d5cdd7284c2a6, 0x3f1a761f0fac0ec4], Some(0x3ff0000000000000), 0x3f50624dd2f1a9fc, 0, Some(0x3fd01d97b7134e93)),
    (9, 0x404e576227653e7a, [0x3fa28218fd1a771d, 0x400a6fb939e57565, 0x3f7d42ed51e65635], Some(0x3fc0000000000000), 0x0000000000000000, 3, Some(0x3f75aebc8622a4b6)),
    (10, 0x404bb93e6d85f5da, [0x3fbc0666271f6a94, 0x40060779e944c1e0, 0x3f9512fdd83a5bf8], Some(0x3fd0000000000000), 0x0000000000000000, 2, Some(0x3fb82c12c9c2857b)),
    (11, 0x4047b9af9eb8b646, [0x3fd1ab24e39a5ef0, 0x40006bd9ab4429ff, 0x3fa7eccdae33ea1e], Some(0x3fe0000000000000), 0x0000000000000000, 1, Some(0x3fc5922c39454c33)),
    (12, 0x4040bffe8717c40d, [0x3fe6200373ab377f, 0x3ff6141eda148e0e, 0x3fb93c50334cb5c4], Some(0x3ff0000000000000), 0x0000000000000000, 0, Some(0x3fdaa6f369690d1a)),
    (13, 0x401dfcfbb3312e18, [0x3ff39a8e24cd2cca, 0x3ff299a51972c591, 0x3fc2cbf496129164], Some(0x3ff0000000000000), 0x0000000000000000, 0, Some(0x400bbf50d90c2909)),
    (14, 0x3feeb9e06586c321, [0x3ffbc094f3fdcb3e, 0x3ff07b86a13c8187, 0x3fc77fc8379aeb95], Some(0x3ff0000000000000), 0x0000000000000000, 0, Some(0x401b3b4668ab4aec)),
    (15, 0x3fc22abef297f2d5, [0x3fff813987967c80, 0x3fefd10fe7f522dd, 0x3fc94c8fa19f1b4f], Some(0x3ff0000000000000), 0x0000000000000000, 0, Some(0x40170fa060985599)),
    (16, 0x3fc070a94e4ebee5, [0x4000037011e70c36, 0x3fefb08c710e851c, 0x3fc984b091025538], Some(0x3ff0000000000000), 0x0000000000000000, 0, Some(0x3fbae401aceb3d56)),
    (17, 0x3fc0708888ffa27a, [0x4000049a09661216, 0x3fefafe10e1c7849, 0x3fc985854994b6de], Some(0x3ff0000000000000), 0x0000000000000000, 0, Some(0x3effe4fc0c9c5a97)),
    (18, 0x3fc0708888fb9d2c, [0x4000049b2f6b626d, 0x3fefafdf0bdc49e1, 0x3fc9858615ace6bc], Some(0x3ff0000000000000), 0x0000000000000000, 0, Some(0x3dcf4e41134a5b42)),
];
const DAMPED_FINAL: [u64; 4] = [0x4000049b2f6b626d, 0x3fefafdf0bdc49e1, 0x3fc9858615ace6bc, 0x3fc0708888fb9d2c];

fn bits(x: f64) -> u64 {
    x.to_bits()
}

#[track_caller]
fn pinned(rows: &[TraceRow], expected: &[Row], what: &str) {
    assert_eq!(rows.len(), expected.len(), "{what}: number of rows");
    for (row, e) in rows.iter().zip(expected) {
        let at = format!("{what}: row {}", e.0);
        assert_eq!(row.iteration, e.0, "{at}: iteration");
        assert_eq!(bits(row.wrss), e.1, "{at}: wrss");
        let estimates: Vec<u64> = row.estimates.iter().copied().map(bits).collect();
        assert_eq!(estimates, e.2.to_vec(), "{at}: estimates");
        assert_eq!(row.step.map(bits), e.3, "{at}: step");
        assert_eq!(bits(row.lambda), e.4, "{at}: lambda");
        assert_eq!(row.halvings, e.5, "{at}: halvings");
        assert_eq!(
            row.relative_decrease.map(bits),
            e.6,
            "{at}: relative_decrease"
        );
    }
}

#[track_caller]
fn pinned_result(r: &FitResult, rows: &[Row], last: [u64; 4], what: &str) {
    pinned(r.trace(), rows, what);
    let got = [
        bits(r.get("estimate.v").unwrap()),
        bits(r.get("estimate.k").unwrap()),
        bits(r.get("estimate.ka").unwrap()),
        bits(r.get("wrss").unwrap()),
    ];
    assert_eq!(got, last, "{what}: final estimates and wrss");
}

fn done(outcome: Outcome) -> FitResult {
    match outcome {
        Outcome::Done(r) => r,
        other => panic!("not done: {other:?}"),
    }
}

struct Nobody;

impl Observer for Nobody {
    fn after_iteration(&mut self, _: &Progress<'_>) -> Control {
        Control::Continue
    }
}

/// Every route to a result gives the pinned trace: `run`, `run_model` through `&dyn FitModel`,
/// the stepper driven by hand from `start`, and `FitRun::run`.
fn all_routes(i: &FitInput, rows: &[Row], last: [u64; 4], what: &str) {
    pinned_result(&run(i).unwrap(), rows, last, &format!("{what} / run"));
    let dynamic: &dyn FitModel = &i.model;
    pinned_result(
        &run_model(dynamic, i).unwrap(),
        rows,
        last,
        &format!("{what} / run_model"),
    );
    let mut fit = start(i).unwrap();
    let result = loop {
        if let Step::Finished(r) = fit.step() {
            break r;
        }
    };
    pinned(fit.trace(), rows, &format!("{what} / trace of a hand-driven run"));
    pinned_result(&result, rows, last, &format!("{what} / step"));
    let watched = done(start(i).unwrap().run(&mut Nobody));
    pinned_result(&watched, rows, last, &format!("{what} / FitRun::run"));
}

#[test]
fn the_far_start_trace_is_the_one_of_the_old_loop() {
    all_routes(&far(), &FAR_ROWS, FAR_FINAL, "far start, 1/yhat");
}

#[test]
fn the_damped_trace_is_the_one_of_the_old_loop() {
    let positive = DAMPED_ROWS
        .iter()
        .filter(|r| f64::from_bits(r.4) > 0.0)
        .count();
    assert!(positive > 0, "the case must carry lambda > 0");
    all_routes(&damped(), &DAMPED_ROWS, DAMPED_FINAL, "damped start, uniform");
}

#[test]
fn start_generates_the_initial_estimates_as_run_did() {
    let mut i = damped();
    i.initial.clear();
    let generated = initial_estimates(i.model, i.dose, &i.time, &i.conc, &i.options.fixed).unwrap();
    let mut explicit = i.clone();
    explicit.initial = generated;
    let a = run(&i).unwrap();
    let b = run(&explicit).unwrap();
    assert_eq!(a.trace().len(), b.trace().len());
    for (x, y) in a.trace().iter().zip(b.trace()) {
        assert_eq!(bits(x.wrss), bits(y.wrss));
    }
    let fit = start(&i).unwrap();
    assert_eq!(fit.trace().len(), 1);
    assert_eq!(bits(fit.trace()[0].wrss), bits(a.trace()[0].wrss));
}

#[test]
fn trace_and_cancel_of_a_hand_driven_run() {
    let i = damped();
    let mut fit = start(&i).unwrap();
    // Iteration 0: one row, the initial estimates.
    pinned(fit.trace(), &DAMPED_ROWS[..1], "before the first step");
    for n in 1..=3 {
        assert!(matches!(fit.step(), Step::Progress(_)));
        assert_eq!(fit.trace().len(), n + 1);
    }
    pinned(fit.trace(), &DAMPED_ROWS[..4], "after three steps");
    match fit.cancel() {
        Outcome::Cancelled {
            after_iteration,
            trace,
        } => {
            assert_eq!(after_iteration, 3);
            pinned(&trace, &DAMPED_ROWS[..4], "cancelled");
        }
        other => panic!("{other:?}"),
    }
    // Before the first step.
    match start(&i).unwrap().cancel() {
        Outcome::Cancelled {
            after_iteration,
            trace,
        } => {
            assert_eq!(after_iteration, 0);
            pinned(&trace, &DAMPED_ROWS[..1], "cancelled at 0");
        }
        other => panic!("{other:?}"),
    }
    // After Finished the whole trace is still there.
    let mut fit = start(&i).unwrap();
    while !matches!(fit.step(), Step::Finished(_)) {}
    pinned(fit.trace(), &DAMPED_ROWS, "after Finished");
}
