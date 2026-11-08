//! Unit tests of the population objective that the oracle cannot reach: the injected failure of the
//! inner problem (POP-INN-03 item 4), the chain rule of the one-compartment model in (v, cl), the
//! reading of a table in the event layout, and the closed form of a linear model.

use std::collections::BTreeMap;

use caladrius_models::ModelId;

use super::data;
use super::structure::{Structure, SubjectModel};
use super::*;

fn subject(id: &str, time: &[f64], y: &[f64]) -> SubjectInput {
    SubjectInput {
        id: id.to_string(),
        dose: 1.0,
        time: time.to_vec(),
        y: y.iter().map(|x| Some(*x)).collect(),
        duration: None,
    }
}

fn linear_request() -> PopRequest {
    PopRequest {
        model: Some("identity_linear".to_string()),
        initial: [("theta1".to_string(), 1.0), ("theta2".to_string(), 0.5)]
            .into_iter()
            .collect(),
        random_effects: Some(vec!["e1".to_string(), "e2".to_string()]),
        omega: Some(vec![vec![0.3, 0.05], vec![0.05, 0.2]]),
        omega_shape: Some("block".to_string()),
        error_model: Some("additive".to_string()),
        error_init: Some(vec![0.4]),
        model_options: ModelOptions {
            z_powers: Some(vec![0, 1]),
        },
        options: PopOptions {
            evaluate_only: true,
            ..PopOptions::default()
        },
        subjects: Some(vec![
            subject("S1", &[0.5, 1.0, 2.0], &[1.3, 1.4, 2.3]),
            subject("S2", &[0.5, 1.5, 3.0], &[0.9, 1.6, 2.2]),
            subject("S3", &[1.0, 2.0, 4.0], &[1.8, 2.1, 3.4]),
        ]),
        ..PopRequest::default()
    }
}

#[test]
fn an_inner_problem_that_fails_is_refused_with_the_subject_named() {
    let request = linear_request();
    assert!(run(&request).is_ok());
    let error = run_with(&request, &|id| id == "S3").unwrap_err();
    assert_eq!(error.code, PopErrorCode::InnerFailed);
    assert_eq!(error.code.id(), "inner_failed");
    assert_eq!(
        error.message,
        "the inner problem of subject S3 did not converge at the starting values; check the starting values, the model and the data of that subject"
    );
}

#[test]
fn foce_i_on_a_linear_model_with_additive_error_is_the_marginal_likelihood() {
    // With ŷ linear in η and σ constant, the linearisation is exact: OFV_i = n ln 2π + ln|C| +
    // (y − Xθ)ᵀC⁻¹(y − Xθ), C = ZΩZᵀ + a²I, whatever η̂ is.
    let request = linear_request();
    let answer = run(&request).unwrap();
    let omega = [[0.3, 0.05], [0.05, 0.2]];
    let a2 = 0.16;
    let mut total = 0.0;
    for (s, out) in request
        .subjects
        .as_ref()
        .unwrap()
        .iter()
        .zip(&answer.subjects)
    {
        let n = s.time.len();
        let mut c = Mat::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                let zi = [1.0, s.time[i]];
                let zj = [1.0, s.time[j]];
                let mut x = 0.0;
                for k in 0..2 {
                    for l in 0..2 {
                        x += zi[k] * omega[k][l] * zj[l];
                    }
                }
                if i == j {
                    x += a2;
                }
                c.set(i, j, x);
            }
        }
        let e: Vec<f64> = (0..n)
            .map(|j| s.y[j].unwrap() - (1.0 + 0.5 * s.time[j]))
            .collect();
        let l = dense::cholesky(&c).unwrap();
        let w = dense::forward(&l, &e);
        let log_det: f64 = (0..n).map(|i| 2.0 * libm::log(l.at(i, i))).sum();
        let want = n as f64 * libm::log(std::f64::consts::TAU)
            + log_det
            + w.iter().map(|x| x * x).sum::<f64>();
        assert!((out.ofv - want).abs() < 1e-11, "{} {}", out.ofv, want);
        total += want;
        // The mode of a linear-Gaussian model: η̂ = ΩZᵀC⁻¹e.
        let ci_e = dense::solve(&l, &e);
        let eta = out.eta_hat.as_ref().unwrap();
        for k in 0..2 {
            let mut x = 0.0;
            for (t, ce) in s.time.iter().zip(&ci_e) {
                let zj = [1.0, *t];
                for l2 in 0..2 {
                    x += omega[k][l2] * zj[l2] * ce;
                }
            }
            assert!((eta[k] - x).abs() < 1e-12, "{} {}", eta[k], x);
        }
    }
    assert!((answer.ofv - total).abs() < 1e-10);
    let no_2pi = run(&PopRequest {
        options: PopOptions {
            evaluate_only: true,
            ofv_convention: Some("no_2pi".to_string()),
            ..PopOptions::default()
        },
        ..request
    })
    .unwrap();
    assert!((answer.ofv - no_2pi.ofv - 9.0 * libm::log(std::f64::consts::TAU)).abs() < 1e-10);
}

#[test]
fn a_request_without_evaluate_only_is_not_available_yet() {
    let mut request = linear_request();
    request.options.evaluate_only = false;
    assert_eq!(run(&request).unwrap_err().code, PopErrorCode::NotAvailable);
}

#[test]
fn the_columns_in_clearance_follow_the_chain_rule() {
    for (id, extra) in [
        ("pk1.iv_bolus", vec![]),
        ("pk1.oral_1", vec![("ka", 1.3)]),
        ("pk1.iv_infusion", vec![("dur", 0.7)]),
    ] {
        let model = ModelId::from_id(id).unwrap();
        let structure = Structure::Catalogue(model);
        let mut theta: BTreeMap<String, f64> = [("cl".to_string(), 2.0), ("v".to_string(), 10.0)]
            .into_iter()
            .collect();
        for (k, x) in extra {
            theta.insert(k.to_string(), x);
        }
        let mut effects = vec!["cl".to_string(), "v".to_string()];
        if theta.contains_key("ka") {
            effects.push("ka".to_string());
        }
        let times = [0.25, 0.5, 1.0, 2.0, 6.0, 12.0];
        let m = SubjectModel {
            structure: &structure,
            theta: &theta,
            random_effects: &effects,
            dose: 100.0,
            times: &times,
        };
        let eta: Vec<f64> = (0..effects.len()).map(|k| 0.1 - 0.15 * k as f64).collect();
        let ev = m.evaluate(&eta).unwrap();
        let h = 1e-6;
        for k in 0..effects.len() {
            let mut up = eta.clone();
            let mut down = eta.clone();
            up[k] += h;
            down[k] -= h;
            let (pu, pd) = (m.predict(&up).unwrap(), m.predict(&down).unwrap());
            for j in 0..times.len() {
                let fd = (pu[j] - pd[j]) / (2.0 * h);
                let scale = fd.abs().max(1e-3);
                assert!(
                    (ev.g[k][j] - fd).abs() / scale < 1e-6,
                    "{id} {} t={}: {} vs {fd}",
                    effects[k],
                    times[j],
                    ev.g[k][j]
                );
            }
        }
    }
}

#[test]
fn a_table_in_the_event_layout_is_read_on_the_time_since_the_dose() {
    let text = "ID,TIME,DV,AMT,EVID,MDV\n\
                A,-1,0.5,.,0,0\n\
                A,2,.,100,1,1\n\
                A,3,4.0,.,0,0\n\
                A,4,9.9,.,0,1\n\
                A,6,2.0,.,0,0\n\
                B,0,.,50,1,1\n\
                B,1,3.0,.,0,0\n";
    let request = PopRequest {
        dataset_file: Some("table.csv".to_string()),
        dataset_text: Some(text.to_string()),
        ..PopRequest::default()
    };
    let subjects = data::subjects(&request).unwrap();
    assert_eq!(subjects.len(), 2);
    let a = &subjects[0];
    assert_eq!((a.id.as_str(), a.dose), ("A", 100.0));
    assert_eq!(a.times, vec![1.0, 4.0]);
    assert_eq!(a.y, vec![4.0, 2.0]);
    assert_eq!(subjects[1].times, vec![1.0]);

    let excluded = PopRequest {
        options: PopOptions {
            exclude_subjects: vec!["A".to_string()],
            ..PopOptions::default()
        },
        ..request.clone()
    };
    assert_eq!(data::subjects(&excluded).unwrap().len(), 1);

    let unknown = PopRequest {
        options: PopOptions {
            exclude_subjects: vec!["Z".to_string()],
            ..PopOptions::default()
        },
        ..request
    };
    let error = data::subjects(&unknown).unwrap_err();
    assert_eq!(error.code, PopErrorCode::InvalidOption);
    assert!(error.message.contains("`Z`"));
}
