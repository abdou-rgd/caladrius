//! Three-compartment models through the commands (task T-145): the schemas list the `pk3.*` ids,
//! `model.simulate`, `fit.evaluate` and `fit.run` work on a pk3 profile, `fit.initial_estimates`
//! refuses pk3 with the way out, and the refusals of the model (MOD-3C-06, 08, 26) reach the
//! caller as the sentence of the model. Parameters and expected values are the public ones of
//! `oracle/expected/models/pk3/` (clearance set cl 3, vc 8, q2 9.5, vp2 9.5, q3 6.3, vp3 31.5,
//! dose 100).
//!
//! The tests of a refusal never pin the code of the model error (`DerivedOutOfRange` or
//! `DegenerateExponents`, which the spec moves for some macro-path refusals): they compare the
//! sentence of the command with the sentence of `caladrius_models::run`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::schema::validate;
use crate::*;

fn run(engine: &mut Engine, id: &str, params: Value) -> Value {
    match engine.execute(id, params) {
        Ok(v) => v,
        Err(e) => panic!("{id} failed: {e}"),
    }
}

fn refuse(engine: &mut Engine, id: &str, params: Value) -> CommandError {
    match engine.execute(id, params) {
        Ok(v) => panic!("{id} should have failed, gave {v}"),
        Err(e) => e,
    }
}

fn numbers(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs()
}

/// The public clearance set of the oracle base cases.
fn clearance() -> Value {
    json!({ "cl": 3, "vc": 8, "q2": 9.5, "vp2": 9.5, "q3": 6.3, "vp3": 31.5 })
}

/// Times of a profile that sees the three phases (alpha 3, beta 0.5, gamma 0.05 per hour).
const TIMES: [f64; 20] = [
    0.05, 0.1, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 36.0, 48.0,
    72.0, 96.0, 120.0,
];

/// A simulated pk3 profile with a small deterministic alternating error (1 %), as a worksheet.
fn engine_with_profile(model: &str, params: Value, dose: f64) -> Engine {
    let mut e = Engine::new();
    let sim = run(
        &mut e,
        "model.simulate",
        json!({ "model": model, "dose": dose, "params": params, "times": TIMES }),
    );
    let mut csv = String::from("Subject,Time (h),Conc (mg/L),Dose (mg)\n");
    for (i, (t, c)) in TIMES.iter().zip(numbers(&sim["conc"])).enumerate() {
        let noise = if i % 2 == 0 { 1.01 } else { 0.99 };
        csv.push_str(&format!("A,{t},{},{dose}\n", c * noise));
    }
    run(&mut e, "data.import", json!({ "name": "pk3", "csv": csv }));
    e
}

#[test]
fn every_pk3_id_is_in_the_schemas_with_the_parameter_sets_in_a_short_description() {
    for id in [
        "fit.run",
        "fit.evaluate",
        "fit.initial_estimates",
        "model.simulate",
    ] {
        let info = describe().into_iter().find(|c| c.id == id).unwrap();
        let schema = &info.params_schema;
        let text = schema.to_string();
        for m in caladrius_models::ModelId::THREE_COMPARTMENT {
            assert!(text.contains(m.id()), "{id} lacks {}", m.id());
        }
        // The three sets of MOD-3C-02, in the names of the spec and the oracle.
        for set in [
            "cl,vc,q2,vp2,q3,vp3",
            "k10,k12,k21,k13,k31,vc",
            "a,b,c,alpha,beta,gamma",
        ] {
            assert!(text.contains(set), "{id}: parameter set {set}");
        }
        let mut params = info.example.clone();
        params["model"] = json!("pk3.oral_1_lag");
        if let Err(why) = validate::check(schema, schema, &params) {
            panic!("{id}: {why}");
        }
        params["model"] = json!("pk3.bogus");
        assert!(validate::check(schema, schema, &params).is_err(), "{id}");
    }
}

#[test]
fn a_pk3_model_is_simulated_in_the_three_parameter_sets() {
    let mut e = Engine::new();
    // Oracle `model_pk3_iv_bolus_base` (and its micro and macro forms): C(1), C(3), AUC(0, 1).
    let sets = [
        clearance(),
        json!({ "k10": 0.375, "k12": 1.1875, "k21": 1, "k13": 0.7875, "k31": 0.2, "vc": 8 }),
        json!({ "a": 9.491525423728813, "b": 1.6666666666666667, "c": 1.34180790960452,
                "alpha": 3, "beta": 0.5, "gamma": 0.05 }),
    ];
    for params in sets {
        let r = run(
            &mut e,
            "model.simulate",
            json!({ "model": "pk3.iv_bolus", "dose": 100, "params": params, "times": [0, 1, 3] }),
        );
        let conc = numbers(&r["conc"]);
        let auc = numbers(&r["auc"]);
        assert!(close(conc[0], 12.5, 1e-12), "{conc:?}");
        assert!(close(conc[1], 2.759_806_823_684_508, 1e-12), "{conc:?}");
        assert!(close(conc[2], 1.5279597181672522, 1e-12), "{conc:?}");
        assert!(close(auc[1], 5.626_702_746_356_785, 1e-12), "{auc:?}");
        let secondary = &r["secondary"];
        assert!(close(secondary["alpha"].as_f64().unwrap(), 3.0, 1e-12));
        assert!(close(secondary["beta"].as_f64().unwrap(), 0.5, 1e-12));
        assert!(close(secondary["gamma"].as_f64().unwrap(), 0.05, 1e-12));
        assert!(close(secondary["vss"].as_f64().unwrap(), 49.0, 1e-12));
        for name in ["w_alpha", "half_life", "auc_inf", "k21_ge_k31"] {
            assert!(secondary[name].as_f64().is_some(), "{name}");
        }
    }
}

#[test]
fn every_pk3_id_is_simulated_against_the_oracle() {
    let mut e = Engine::new();
    // (model, extra parameters, [C(1), C(3), AUC(0, 1), AUC(0, 24)]) from the oracle base cases.
    let cases = [
        (
            "pk3.iv_infusion",
            json!({ "dur": 2 }),
            [
                2.8133513731783926,
                1.9321887624236225,
                1.7658995069418255,
                24.832442718941486,
            ],
        ),
        (
            "pk3.oral_1",
            json!({ "ka": 1.5 }),
            [
                3.6144495617995007,
                1.8136318136390714,
                3.2170697051571175,
                24.97168655977034,
            ],
        ),
        (
            "pk3.oral_0_lag",
            json!({ "dur": 2, "tlag": 0.5 }),
            [
                1.9289055541530233,
                2.4179649411248776,
                0.560_482_664_624_015_5,
                24.617232544036636,
            ],
        ),
    ];
    for (model, extra, want) in cases {
        let mut params = clearance();
        for (k, v) in extra.as_object().unwrap() {
            params[k] = v.clone();
        }
        let r = run(
            &mut e,
            "model.simulate",
            json!({ "model": model, "dose": 100, "params": params, "times": [1, 3, 24] }),
        );
        let conc = numbers(&r["conc"]);
        let auc = numbers(&r["auc"]);
        let got = [conc[0], conc[1], auc[0], auc[2]];
        for (g, w) in got.iter().zip(want) {
            assert!(close(*g, w, 1e-12), "{model}: {got:?} against {want:?}");
        }
    }
    // A stored pk3 simulation exports its table like any other.
    let mut params = clearance();
    params["dur"] = json!(2);
    let s = run(
        &mut e,
        "model.simulate",
        json!({ "model": "pk3.oral_0", "dose": 100, "params": params,
                "grid": { "start": 0, "end": 24, "points": 25 }, "store": true }),
    );
    let table = run(
        &mut e,
        "export.table",
        json!({ "table": "simulation", "analysis": s["analysis"] }),
    );
    assert_eq!(table["rows"].as_array().map(Vec::len), Some(25));
}

/// A refusal of the model reaches the caller as its sentence: the command's message is the
/// message of `caladrius_models::run` for the same input, under the code `model_error`.
fn assert_same_sentence(e: &mut Engine, what: &str, params: Value) {
    let model = params["model"].as_str().unwrap_or_default().to_owned();
    let input = caladrius_models::ModelInput {
        model: caladrius_models::ModelId::from_id(&model).unwrap(),
        dose: params["dose"].as_f64().unwrap_or(f64::NAN),
        params: params["params"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_f64().unwrap()))
            .collect::<BTreeMap<_, _>>(),
        times: numbers(&params["times"]),
    };
    let sentence = match caladrius_models::run(&input) {
        Ok(_) => panic!("{what}: the model accepted the input"),
        Err(err) => err.to_string(),
    };
    let got = refuse(e, "model.simulate", params);
    assert_eq!(got.code, "model_error", "{what}: {got}");
    assert_eq!(got.message, sentence, "{what}");
}

#[test]
fn a_pk3_refusal_reaches_the_caller_as_the_sentence_of_the_model() {
    let mut e = Engine::new();
    let base = |model: &str, dose: f64, params: Value| json!({ "model": model, "dose": dose, "params": params, "times": [1] });
    // MOD-3C-06: a parameter outside its domain is named.
    let mut bad = clearance();
    bad["cl"] = json!(0);
    let r = refuse(
        &mut e,
        "model.simulate",
        base("pk3.iv_bolus", 100.0, bad.clone()),
    );
    assert_eq!(r.code, "model_error");
    assert!(r.message.contains("cl"), "{r}");
    assert_same_sentence(&mut e, "cl = 0", base("pk3.iv_bolus", 100.0, bad));
    // A peripheral flow of zero is a smaller model: the sentence says which.
    let mut one = clearance();
    one["q3"] = json!(0);
    let r = refuse(
        &mut e,
        "model.simulate",
        base("pk3.iv_bolus", 100.0, one.clone()),
    );
    assert_eq!(r.code, "model_error");
    assert_same_sentence(&mut e, "q3 = 0", base("pk3.iv_bolus", 100.0, one));
    // Two sets mixed, and one- and two-compartment names, are refused naming the parameter.
    let mut mixed = clearance();
    mixed["k10"] = json!(0.2);
    let r = refuse(
        &mut e,
        "model.simulate",
        base("pk3.iv_bolus", 100.0, mixed.clone()),
    );
    assert!(r.message.contains("k10"), "{r}");
    assert_same_sentence(&mut e, "mixed sets", base("pk3.iv_bolus", 100.0, mixed));
    let v = base("pk3.iv_bolus", 100.0, json!({ "v": 10, "k": 0.2 }));
    assert_same_sentence(&mut e, "pk1 names", v);
    let pk2 = base(
        "pk3.iv_bolus",
        100.0,
        json!({ "cl": 2, "vc": 10, "q": 4, "vp": 8 }),
    );
    assert_same_sentence(&mut e, "pk2 names", pk2);
    // MOD-3C-02: the macro set needs a dose.
    let macro_set = json!({ "a": 9.491525423728813, "b": 1.6666666666666667,
                            "c": 1.34180790960452, "alpha": 3, "beta": 0.5, "gamma": 0.05 });
    let r = refuse(
        &mut e,
        "model.simulate",
        base("pk3.iv_bolus", 0.0, macro_set.clone()),
    );
    assert_eq!(r.code, "model_error");
    assert_same_sentence(
        &mut e,
        "macro without dose",
        base("pk3.iv_bolus", 0.0, macro_set),
    );
}

/// The error suite of the oracle (MOD-3C-06, 08, 26): every input that JSON can carry is refused
/// by `model.simulate` with the sentence of the model, and the code is `model_error`.
#[test]
fn every_input_of_the_pk3_error_suite_is_refused_with_the_sentence_of_the_model() {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "oracle",
        "expected",
        "models",
        "pk3",
        "model_pk3_errors.options.json",
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let suite: Value = serde_json::from_str(&text).unwrap();
    let cases = suite["cases"].as_array().unwrap();
    assert!(cases.len() > 200, "{} cases", cases.len());
    let mut e = Engine::new();
    let (mut compared, mut not_json) = (0, 0);
    for c in cases {
        let id = c["id"].as_str().unwrap_or("?");
        let params = &c["parameters"];
        // NaN and infinity are strings in the oracle: JSON cannot carry them.
        let finite = params.as_object().unwrap().values().all(|v| v.is_number())
            && c["dose"].is_number()
            && c["times"].as_array().unwrap().iter().all(Value::is_number);
        if !finite {
            not_json += 1;
            continue;
        }
        assert_same_sentence(
            &mut e,
            id,
            json!({ "model": c["model"], "dose": c["dose"], "params": params, "times": c["times"] }),
        );
        compared += 1;
    }
    assert!(compared > 150 && not_json > 0, "{compared} / {not_json}");
}

#[test]
fn fit_initial_estimates_refuses_pk3_and_says_to_supply_them() {
    let mut e = engine_with_profile("pk3.iv_bolus", clearance(), 100.0);
    let r = refuse(
        &mut e,
        "fit.initial_estimates",
        json!({ "worksheet": 1, "model": "pk3.iv_bolus" }),
    );
    assert_eq!(r.code, "fit_error");
    assert!(
        r.message.contains("three-compartment") && r.message.contains("initial value"),
        "{r}"
    );
    // The same refusal reaches fit.run and fit.evaluate without initial values, and nothing is
    // stored.
    for id in ["fit.run", "fit.evaluate"] {
        let r = refuse(
            &mut e,
            id,
            json!({ "worksheet": 1, "model": "pk3.iv_bolus" }),
        );
        assert_eq!(r.code, "fit_error", "{id}");
        assert!(r.message.contains("initial"), "{id}: {r}");
    }
    assert!(e.project().analyses().is_empty());
}

#[test]
fn fit_evaluate_and_fit_run_work_on_a_pk3_profile() {
    let mut e = engine_with_profile("pk3.iv_bolus", clearance(), 100.0);
    let start = json!({ "cl": 3.2, "vc": 7.5, "q2": 9.0, "vp2": 10.0, "q3": 6.0, "vp3": 30.0 });
    let ev = run(
        &mut e,
        "fit.evaluate",
        json!({ "worksheet": 1, "model": "pk3.iv_bolus", "initial": start }),
    );
    let at_start = ev["wrss"].as_f64().unwrap_or(f64::NAN);
    assert!(at_start > 0.0 && at_start.is_finite(), "{ev}");
    assert_eq!(ev["n_observations"], 20);
    let fit = run(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "model": "pk3.iv_bolus", "initial": start }),
    );
    let ok = &fit["result"]["outcome"]["ok"];
    // The status is whatever the fit reached; the estimates and the table are what the command
    // owes for every pk3 parameter (a 1 % error on 20 points does not pin six parameters).
    assert!(ok["status"].is_string(), "{ok}");
    for name in ["cl", "vc", "q2", "vp2", "q3", "vp3"] {
        assert!(
            ok["values"][format!("estimate.{name}")].as_f64().is_some(),
            "{name}: {ok}"
        );
    }
    let table = run(
        &mut e,
        "export.table",
        json!({ "table": "fit.parameters", "analysis": fit["id"] }),
    );
    assert_eq!(table["rows"].as_array().map(Vec::len), Some(6));
}

#[test]
fn a_pk3_fit_of_the_micro_set_and_of_an_oral_model_runs() {
    let micro =
        json!({ "k10": 0.375, "k12": 1.1875, "k21": 1, "k13": 0.7875, "k31": 0.2, "vc": 8 });
    let mut e = engine_with_profile("pk3.iv_bolus", micro, 100.0);
    let fit = run(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "model": "pk3.iv_bolus",
                "initial": { "k10": 0.4, "k12": 1.1, "k21": 1.1, "k13": 0.8, "k31": 0.21, "vc": 7.8 } }),
    );
    let ok = &fit["result"]["outcome"]["ok"];
    assert!(ok["values"]["estimate.k31"].as_f64().is_some(), "{ok}");
    let mut params = clearance();
    params["ka"] = json!(1.5);
    let mut e = engine_with_profile("pk3.oral_1", params, 100.0);
    let fit = run(
        &mut e,
        "fit.run",
        json!({ "worksheet": 1, "model": "pk3.oral_1",
                "initial": { "cl": 3.1, "vc": 7.8, "q2": 9.3, "vp2": 9.7, "q3": 6.2, "vp3": 31.0,
                             "ka": 1.4 } }),
    );
    let ok = &fit["result"]["outcome"]["ok"];
    assert!(ok["values"]["estimate.ka"].as_f64().is_some(), "{ok}");
}
