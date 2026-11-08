#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: helpers panic on purpose

//! Cross-checks the three-compartment oracle (task T-142) against independent arithmetic
//! (`AGENTS.md` section 5, source 3), so that the expected files are tested by something other than
//! the R script that wrote them:
//!
//! - the files load, are complete and have the expected shape (counts per kind, every id, every set);
//! - the naive double-precision textbook forms of `caladrius_testkit::naive3c` reproduce the
//!   concentrations, AUC and AUMC of the cases where those forms keep their digits;
//! - the scalars satisfy the identities of `specs/models.md` MOD-3C-03, 04, 07, 15, 16;
//! - worked example Q1 is reproduced at the printed digits;
//! - the derivative grids at the well-conditioned base point agree with central differences of the
//!   naive implementation;
//! - the multiple-dosing cases agree with a naive superposition (regular and schedule) and with a
//!   naive sum over past doses (steady state);
//! - the error suite loads and covers every group.
//!
//! This tests the oracle and the testkit, not the engine.

use std::collections::BTreeMap;

use caladrius_testkit::naive3c::conc_auc_aumc;
use caladrius_testkit::{
    Pk3Case, Pk3Kind, RegimenKind, list_pk3_cases, list_pk3_md_cases, load_pk3_case,
    load_pk3_errors, load_pk3_md_case,
};

fn all_cases() -> Vec<Pk3Case> {
    list_pk3_cases()
        .unwrap()
        .iter()
        .map(|n| load_pk3_case(n).unwrap())
        .collect()
}

fn of_kind(kind: Pk3Kind) -> Vec<Pk3Case> {
    all_cases().into_iter().filter(|c| c.kind == kind).collect()
}

fn close(what: &str, actual: f64, expected: f64, rel: f64) {
    let allowed = if expected == 0.0 {
        0.0
    } else {
        rel * expected.abs()
    };
    assert!(
        (actual - expected).abs() <= allowed,
        "{what}: independent value {actual:e}, oracle {expected:e}"
    );
}

fn scalar(c: &Pk3Case, name: &str) -> Option<f64> {
    c.case
        .expected
        .get("scalar", name)
        .flatten()
        .or_else(|| c.case.expected.get("scalar_ill", name).flatten())
        .or_else(|| c.case.expected.get("scalar_abs", name).flatten())
}

const IDS: [&str; 6] = [
    "pk3.iv_bolus",
    "pk3.iv_infusion",
    "pk3.oral_1",
    "pk3.oral_1_lag",
    "pk3.oral_0",
    "pk3.oral_0_lag",
];

const SCALARS: [&str; 38] = [
    "alpha",
    "beta",
    "gamma",
    "k10",
    "k12",
    "k21",
    "k13",
    "k31",
    "cl",
    "vc",
    "q2",
    "vp2",
    "q3",
    "vp3",
    "a",
    "b",
    "c",
    "w_alpha",
    "w_beta",
    "w_gamma",
    "half_life",
    "half_life_alpha",
    "half_life_beta",
    "half_life_gamma",
    "vss",
    "vz",
    "v_extrap",
    "auc_inf",
    "aumc_inf",
    "mrt_system",
    "mrt",
    "k21_ge_k31",
    "k10",
    "k12",
    "k21",
    "k13",
    "k31",
    "vc",
];

#[test]
fn the_oracle_has_its_expected_shape() {
    let values = of_kind(Pk3Kind::Values);
    let derivs = of_kind(Pk3Kind::Derivatives);
    let conds = of_kind(Pk3Kind::Conditioning);
    let convs = of_kind(Pk3Kind::Conversion);
    assert!(values.len() >= 150, "{} value cases", values.len());
    assert!(derivs.len() >= 60, "{} derivative cases", derivs.len());
    assert!(conds.len() >= 15, "{} conditioning cases", conds.len());
    assert!(convs.len() >= 30, "{} conversion cases", convs.len());
    assert!(
        list_pk3_md_cases().unwrap().len() >= 60,
        "multiple-dosing cases"
    );
    for model in IDS {
        for set in ["clearance", "micro", "macro"] {
            assert!(
                values
                    .iter()
                    .any(|c| c.case.model == model && c.parameterisation == set),
                "{model} in the {set} set"
            );
        }
        assert!(
            derivs.iter().filter(|c| c.case.model == model).count() >= 8,
            "derivatives of {model}"
        );
    }
    for c in &values {
        for key in &c.case.time_keys {
            for q in ["conc", "auc", "aumc"] {
                assert!(
                    c.case.expected.get(key, q).flatten().is_some(),
                    "{} {key} {q}",
                    c.case.name
                );
            }
        }
        for name in SCALARS {
            assert!(
                scalar(c, name).is_some(),
                "{} lacks the scalar {name}",
                c.case.name
            );
        }
        assert!(c.case.times.contains(&0.0), "{}: t = 0", c.case.name);
        assert!(
            c.case.times.iter().any(|&t| t < 0.0),
            "{}: a time before the dose",
            c.case.name
        );
    }
}

/// The naive textbook forms reproduce the cases where they keep their digits.
#[test]
fn the_naive_forms_reproduce_the_well_conditioned_cases() {
    let mut checked = 0;
    for c in of_kind(Pk3Kind::Values) {
        if !c.textbook_double_ok || c.parameterisation == "macro" || c.case.dose <= 0.0 {
            continue;
        }
        for (i, &t) in c.case.times.iter().enumerate() {
            let key = &c.case.time_keys[i];
            let Some([conc, auc, aumc]) = conc_auc_aumc(
                &c.case.model,
                &c.parameterisation,
                &c.case.parameters,
                c.case.dose,
                t,
            ) else {
                panic!("{}: the naive implementation refuses the case", c.case.name)
            };
            // far in the tail the cancellation of the naive forms loses digits: compare relative to
            // the largest value of the case
            let big = |q: &str| {
                c.case
                    .time_keys
                    .iter()
                    .filter_map(|k| c.case.expected.get(k, q).flatten())
                    .fold(0.0_f64, |m, x| m.max(x.abs()))
            };
            for (q, v) in [("conc", conc), ("auc", auc), ("aumc", aumc)] {
                let want = c.case.expected.get(key, q).flatten().unwrap();
                assert!(
                    (v - want).abs() <= 1e-8 * want.abs() + 1e-11 * big(q),
                    "{} {key} {q}: naive {v:e}, oracle {want:e}",
                    c.case.name
                );
            }
        }
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} textbook cases were checked");
}

#[test]
fn the_scalars_satisfy_the_identities_of_the_specification() {
    for c in all_cases() {
        if c.kind == Pk3Kind::Derivatives || c.kind == Pk3Kind::Free {
            continue;
        }
        let name = c.case.name.as_str();
        let g = |n: &str| scalar(&c, n).unwrap_or_else(|| panic!("{name}: no scalar {n}"));
        if c.case.dose == 0.0 || c.scalar_ill.iter().any(|n| n == "vss") {
            continue;
        }
        let (al, be, ga) = (g("alpha"), g("beta"), g("gamma"));
        assert!(
            al > be && be >= ga && ga > 0.0,
            "{name}: order of exponents"
        );
        close(
            &format!("{name} sum of weights"),
            g("w_alpha") + g("w_beta") + g("w_gamma"),
            1.0,
            1e-12,
        );
        close(&format!("{name} vz"), g("vz"), g("cl") / ga, 1e-12);
        close(
            &format!("{name} vss"),
            g("vss"),
            g("vc") * (1.0 + g("k12") / g("k21") + g("k13") / g("k31")),
            1e-11,
        );
        close(
            &format!("{name} auc_inf"),
            g("auc_inf"),
            c.case.dose / g("cl"),
            1e-12,
        );
        close(
            &format!("{name} mrt_system"),
            g("mrt_system"),
            g("vss") / g("cl"),
            1e-11,
        );
        close(
            &format!("{name} half_life"),
            g("half_life"),
            std::f64::consts::LN_2 / ga,
            1e-12,
        );
        // the exponents are the roots of the characteristic cubic
        let s = g("k10") + g("k12") + g("k13") + g("k21") + g("k31");
        close(&format!("{name} sum of exponents"), al + be + ga, s, 1e-11);
        close(
            &format!("{name} product of exponents"),
            al * be * ga,
            g("k10") * g("k21") * g("k31"),
            1e-11,
        );
        let flag = g("k21_ge_k31");
        assert!(
            (flag == 1.0) == (g("k21") >= g("k31")),
            "{name}: k21_ge_k31 is {flag}"
        );
    }
}

/// Worked example Q1 of `specs/models.md` section 14 at the printed digits.
#[test]
fn worked_example_q1_is_reproduced() {
    let c = load_pk3_case("model_pk3_iv_bolus_base").unwrap();
    let get = |t: f64| {
        let key = c
            .case
            .time_keys
            .iter()
            .zip(&c.case.times)
            .find(|(_, x)| **x == t)
            .map(|(k, _)| k.clone())
            .unwrap();
        c.case.expected.get(&key, "conc").flatten().unwrap()
    };
    close("Q1 C(1)", get(1.0), 2.759_806_823_685, 5e-12);
    close("Q1 C(12)", get(12.0), 0.740_531_047_822, 5e-12);
    for (n, v) in [
        ("alpha", 3.0),
        ("beta", 0.5),
        ("gamma", 0.05),
        ("vss", 49.0),
        ("vz", 60.0),
    ] {
        close(&format!("Q1 {n}"), scalar(&c, n).unwrap(), v, 1e-12);
    }
}

/// At the exact tie k21 = k31 the middle exponent is k21 and its weight is exactly 0.
#[test]
fn the_exact_tie_is_structural() {
    let c = load_pk3_case("model_pk3_iv_bolus_tie_exact").unwrap();
    assert_eq!(scalar(&c, "w_beta"), Some(0.0));
    assert_eq!(scalar(&c, "beta"), scalar(&c, "k21"));
    assert_eq!(scalar(&c, "k21_ge_k31"), Some(1.0));
}

/// The three parameterisations of one point give one curve (the conversion cases).
#[test]
fn the_three_sets_of_a_conversion_point_agree() {
    let convs = of_kind(Pk3Kind::Conversion);
    let mut groups: BTreeMap<String, Vec<&Pk3Case>> = BTreeMap::new();
    for c in &convs {
        let point = c
            .case
            .name
            .trim_end_matches("_clearance")
            .trim_end_matches("_micro")
            .trim_end_matches("_macro")
            .to_string();
        groups.entry(point).or_default().push(c);
    }
    assert!(groups.len() >= 10, "{} conversion points", groups.len());
    for (point, cs) in groups {
        assert!(cs.len() >= 2, "{point}: fewer than two sets");
        for pair in cs.windows(2) {
            for n in ["alpha", "k10", "k21", "k31", "cl", "vc", "a", "w_alpha"] {
                let (x, y) = (scalar(pair[0], n).unwrap(), scalar(pair[1], n).unwrap());
                // the macro set does not label the peripheral compartments (MOD-3C-05): k21 and k31
                // may come back exchanged, and the macro input is rounded to 12 digits
                let other = match n {
                    "k21" => scalar(pair[1], "k31").unwrap(),
                    "k31" => scalar(pair[1], "k21").unwrap(),
                    _ => y,
                };
                let ok = |v: f64| (x - v).abs() <= 1e-8 * x.abs();
                assert!(ok(y) || ok(other), "{point} {n}: {x:e} against {y:e}");
            }
        }
    }
}

/// The derivative grids at the base point agree with central differences of the naive forms.
#[test]
fn the_base_derivatives_agree_with_finite_differences() {
    let mut checked = 0;
    for c in of_kind(Pk3Kind::Derivatives) {
        if !(c.case.name.ends_with("_base") || c.case.name.ends_with("_base_micro")) {
            continue;
        }
        let model = c.case.model.as_str();
        for (group, pname, value) in c.case.expected.iter() {
            let Some(p) = pname.strip_prefix("d_") else {
                continue;
            };
            let Some(want) = value else { continue };
            let Some(i) = c.case.time_keys.iter().position(|k| k == group) else {
                continue;
            };
            let t = c.case.times[i];
            let Some(x) = c.case.parameters.get(p).copied() else {
                continue;
            };
            let h = 1e-6 * x.abs();
            let at = |v: f64| {
                let mut q = c.case.parameters.clone();
                q.insert(p.to_string(), v);
                conc_auc_aumc(model, &c.parameterisation, &q, c.case.dose, t)
                    .map(|r| r[0])
                    .unwrap()
            };
            let fd = (at(x + h) - at(x - h)) / (2.0 * h);
            let scale = c
                .case
                .expected
                .iter()
                .filter(|(_, n, _)| *n == pname)
                .filter_map(|(_, _, v)| v)
                .fold(0.0_f64, |m, v| m.max(v.abs()));
            // a kink of the profile (the end of the input, the lag) inside the step is the only
            // legitimate disagreement, and those entries are not written
            assert!(
                (fd - want).abs() <= 1e-5 * scale,
                "{} {group} {pname}: finite difference {fd:e}, oracle {want:e}",
                c.case.name
            );
            checked += 1;
        }
    }
    assert!(checked >= 200, "only {checked} derivative entries checked");
}

fn sum_doses(model: &str, set: &str, c: &caladrius_testkit::MdCase, t: f64) -> [f64; 2] {
    let mut p = c.case.parameters.clone();
    let mut doses: Vec<(f64, f64, Option<f64>)> = Vec::new();
    match c.regimen.kind {
        RegimenKind::Schedule => {
            doses.extend(c.regimen.records.iter().map(|r| (r.time, r.dose, r.dur)));
        }
        RegimenKind::Regular => {
            let (tau, n) = (c.regimen.tau.unwrap(), c.regimen.n_doses.unwrap());
            doses.extend((0..n).map(|j| (f64::from(j) * tau, c.case.dose, p.get("dur").copied())));
        }
        RegimenKind::SteadyState => {
            let tau = c.regimen.tau.unwrap();
            // 400 past doses: far more than the slowest exponent needs for the textbook cases
            doses.extend(
                (0..400).map(|j| (-f64::from(j) * tau, c.case.dose, p.get("dur").copied())),
            );
        }
    }
    let (mut conc, mut auc) = (0.0, 0.0);
    for (time, dose, dur) in doses {
        if let Some(d) = dur {
            p.insert("dur".to_string(), d);
        }
        let r = conc_auc_aumc(model, set, &p, dose, t - time).unwrap();
        conc += r[0];
        if c.regimen.kind != RegimenKind::SteadyState {
            auc += r[1];
        }
    }
    [conc, auc]
}

/// Regular regimens and schedules are naive sums of single-dose functions; a steady state is a naive
/// sum over 400 earlier doses.
#[test]
fn the_multiple_dosing_cases_agree_with_a_naive_superposition() {
    let mut checked = 0;
    for name in list_pk3_md_cases().unwrap() {
        let c = load_pk3_md_case(&name).unwrap();
        if !c.textbook_double_ok {
            continue;
        }
        let model = c.case.model.as_str();
        for (i, &t) in c.case.times.iter().enumerate() {
            let key = &c.case.time_keys[i];
            let [conc, auc] = sum_doses(model, &c.parameterisation, &c, t);
            let want = c.case.expected.get(key, "conc").flatten().unwrap();
            let big = c
                .case
                .time_keys
                .iter()
                .filter_map(|k| c.case.expected.get(k, "conc").flatten())
                .fold(0.0_f64, |m, x| m.max(x.abs()));
            assert!(
                (conc - want).abs() <= 1e-8 * want.abs().max(1e-6 * big),
                "{name} {key}: naive {conc:e}, oracle {want:e}"
            );
            if c.regimen.kind != RegimenKind::SteadyState {
                let want = c.case.expected.get(key, "auc").flatten().unwrap();
                assert!(
                    (auc - want).abs() <= 1e-8 * want.abs(),
                    "{name} {key}: naive AUC {auc:e}, oracle {want:e}"
                );
            }
        }
        checked += 1;
    }
    assert!(
        checked >= 20,
        "only {checked} multiple-dosing cases checked"
    );
}

/// The steady-state scalars obey D/CL identities.
#[test]
fn the_steady_state_scalars_obey_the_area_identities() {
    let mut seen = 0;
    for name in list_pk3_md_cases().unwrap() {
        let c = load_pk3_md_case(&name).unwrap();
        if c.regimen.kind != RegimenKind::SteadyState {
            continue;
        }
        let g = |n: &str| c.case.expected.get("scalar", n).flatten();
        let (cmax, cmin) = (g("cmax_ss").unwrap(), g("cmin_ss").unwrap());
        let tau = c.regimen.tau.unwrap();
        assert!(cmax >= cmin && cmin > 0.0, "{name}");
        close(
            &format!("{name} cav"),
            g("cav_ss").unwrap(),
            g("auc_tau_ss").unwrap() / tau,
            1e-12,
        );
        let last = c
            .case
            .time_keys
            .iter()
            .zip(&c.case.times)
            .find(|(_, t)| **t == tau)
            .map(|(k, _)| k.clone())
            .unwrap();
        close(
            &format!("{name} area over one interval"),
            c.case.expected.get(&last, "auc").flatten().unwrap(),
            g("auc_tau_ss").unwrap(),
            1e-12,
        );
        seen += 1;
    }
    assert!(seen >= 20, "{seen} steady-state cases");
}

#[test]
fn the_error_suite_covers_every_group() {
    let errors = load_pk3_errors().unwrap();
    for group in [
        "domain",
        "one_compartment",
        "macro",
        "macro_n1",
        "sets",
        "numeric",
        "times",
    ] {
        assert!(
            errors.iter().filter(|e| e.group == group).count() >= 3,
            "group {group}"
        );
    }
    assert!(errors.len() >= 100, "{} error cases", errors.len());
    for e in &errors {
        assert!(e.model.starts_with("pk3."), "{}", e.id);
        assert!(
            !e.message_contains.is_empty() && e.message_contains.iter().all(|w| !w.is_empty()),
            "{}: the message words",
            e.id
        );
    }
}

// ---------------------------------------------------------------- T-165

/// The textbook flag is MEASURED (T-165): for a case whose basis is `measured`, the flag is true exactly
/// when the naive forms of this crate pass at the tolerance of
/// `the_naive_forms_reproduce_the_well_conditioned_cases`, and the worst ratio written in the options
/// is at most 1 exactly then.
#[test]
fn the_textbook_flag_is_what_the_naive_forms_do() {
    let mut measured = 0;
    let mut disagreements = Vec::new();
    for c in of_kind(Pk3Kind::Values) {
        match c.textbook_basis.as_deref() {
            Some("measured") => {}
            Some("declared") | None => continue,
            Some(other) => panic!("{}: textbook_basis {other:?}", c.case.name),
        }
        measured += 1;
        let ratio = c
            .textbook_worst_ratio
            .unwrap_or_else(|| panic!("{}: a measured flag has its worst ratio", c.case.name));
        assert_eq!(
            c.textbook_double_ok,
            ratio <= 1.0,
            "{}: flag {} but worst ratio {ratio}",
            c.case.name,
            c.textbook_double_ok
        );
        let mut worst = 0.0_f64;
        for (i, &t) in c.case.times.iter().enumerate() {
            let key = &c.case.time_keys[i];
            let naive = conc_auc_aumc(
                &c.case.model,
                &c.parameterisation,
                &c.case.parameters,
                c.case.dose,
                t,
            );
            for (j, q) in ["conc", "auc", "aumc"].into_iter().enumerate() {
                let want = c.case.expected.get(key, q).flatten().unwrap();
                let big = c
                    .case
                    .time_keys
                    .iter()
                    .filter_map(|k| c.case.expected.get(k, q).flatten())
                    .fold(0.0_f64, |m, x| m.max(x.abs()));
                let allowed = 1e-8 * want.abs() + 1e-11 * big;
                let v = naive.map_or(f64::NAN, |a| a[j]);
                let diff = (v - want).abs();
                let r = if !v.is_finite() {
                    f64::INFINITY
                } else if allowed > 0.0 {
                    diff / allowed
                } else if diff > 0.0 {
                    f64::INFINITY
                } else {
                    0.0
                };
                worst = worst.max(r);
            }
        }
        if (worst <= 1.0) != c.textbook_double_ok {
            disagreements.push(format!(
                "{}: flag {}, R ratio {ratio}, Rust ratio {worst}",
                c.case.name, c.textbook_double_ok
            ));
        }
    }
    assert!(measured >= 100, "only {measured} measured flags");
    assert!(
        disagreements.is_empty(),
        "the flag and the naive forms of the testkit disagree:\n{}",
        disagreements.join("\n")
    );
}

/// Distance of the double `x` from the double `y` in half-spacings of the doubles at `y` on the side
/// of `x` (positive doubles only; just below a power of two the spacing is half as wide).
fn half_spacings_between(x: f64, y: f64) -> f64 {
    let above = x > y;
    let biased = (y.to_bits() >> 52) & 0x7ff;
    let power_of_two = y.to_bits() & ((1_u64 << 52) - 1) == 0;
    // spacing of the doubles at y on the side of x: 2^(e-52), half as wide just below a power of two
    let exponent = if above || !power_of_two {
        biased - 52
    } else {
        biased - 53
    };
    let spacing = f64::from_bits(exponent << 52);
    (x - y).abs() / (spacing / 2.0)
}

/// A large-coupling case named `_pole_` claims that one root sits next to one pole (T-165): the root, the
/// pole, the side and the distance in half-spacings are in its options, and the values of the case
/// (the scalars of the csv) agree with them.
#[test]
fn the_pole_claims_are_asserted() {
    let mut seen = 0;
    let (mut above, mut below) = (0, 0);
    for c in all_cases() {
        let name = c.case.name.as_str();
        let claim = c.pole.as_ref();
        assert_eq!(
            claim.is_some(),
            name.contains("_pole_"),
            "{name}: exactly the _pole_ cases carry a pole claim"
        );
        let Some(claim) = claim else { continue };
        assert!(
            ["alpha", "beta", "gamma"].contains(&claim.root.as_str()),
            "{name}: root {}",
            claim.root
        );
        assert!(["k21", "k31"].contains(&claim.pole.as_str()), "{name}");
        assert!(["above", "below"].contains(&claim.side.as_str()), "{name}");
        let root = scalar(&c, &claim.root).unwrap();
        let pole = scalar(&c, &claim.pole).unwrap();
        assert_eq!(
            root > pole,
            claim.side == "above",
            "{name}: {} = {root:e} against {} = {pole:e}, the claim says {}",
            claim.root,
            claim.pole,
            claim.side
        );
        let relative = (root - pole).abs() / pole;
        assert!(
            relative <= claim.max_relative_distance * (1.0 + 1e-6),
            "{name}: relative distance {relative:e} above the promised {:e}",
            claim.max_relative_distance
        );
        let others = ["alpha", "beta", "gamma"]
            .iter()
            .filter(|n| **n != claim.root)
            .map(|n| (scalar(&c, n).unwrap() - pole).abs() / pole)
            .fold(f64::INFINITY, f64::min);
        assert!(
            others > relative,
            "{name}: another root is nearer to {}",
            claim.pole
        );
        let half = half_spacings_between(root, pole);
        assert!(
            (half - claim.half_spacings).abs() <= 2.0 + 1e-6 * claim.half_spacings,
            "{name}: {half} half-spacings in the csv, {} in the options",
            claim.half_spacings
        );
        if claim.side == "above" {
            above += 1;
        } else {
            below += 1;
        }
        seen += 1;
    }
    assert!(seen >= 4, "{seen} pole cases");
    assert!(above >= 2 && below >= 2, "{above} above, {below} below");
}

/// Two doubles are neighbours when their bit patterns (both positive) differ by one.
fn adjacent(a: f64, b: f64) -> bool {
    a > 0.0 && b > 0.0 && a.to_bits().abs_diff(b.to_bits()) == 1
}

/// Adjacent poles (MOD-3C-22): w_beta is held to 1e-12 ABSOLUTE in the group `scalar_abs`, and only
/// there; no other value case has that group.
#[test]
fn adjacent_poles_have_the_absolute_group() {
    let mut adjacent_cases = 0;
    for c in of_kind(Pk3Kind::Values) {
        let name = c.case.name.as_str();
        let is_adjacent = match (
            c.case.expected.get("scalar", "k21").flatten(),
            c.case.expected.get("scalar", "k31").flatten(),
        ) {
            (Some(a), Some(b)) => adjacent(a, b),
            _ => false,
        };
        let abs_rows: Vec<_> = c
            .case
            .expected
            .iter()
            .filter(|(g, _, _)| *g == "scalar_abs")
            .collect();
        if is_adjacent {
            adjacent_cases += 1;
            assert_eq!(abs_rows.len(), 1, "{name}: w_beta alone is in scalar_abs");
            assert_eq!(abs_rows[0].1, "w_beta", "{name}");
            assert!(
                c.case.expected.get("scalar", "w_beta").is_none()
                    && c.case.expected.get("scalar_ill", "w_beta").is_none(),
                "{name}: w_beta is in one group only"
            );
            let w = abs_rows[0].2.unwrap();
            assert!((0.0..1.0).contains(&w), "{name}: w_beta {w}");
        } else {
            assert!(
                abs_rows.is_empty(),
                "{name}: scalar_abs without adjacent poles"
            );
        }
    }
    assert!(adjacent_cases >= 6, "{adjacent_cases} adjacent-pole cases");
}

/// The free-zone cases (MOD-3C-08 zones (0) and (iii)): macro input, a time grid, the macro-alone
/// scalars and the zone; no scalar built from the micro constants is written.
#[test]
fn the_free_zone_cases_have_their_shape() {
    let free = of_kind(Pk3Kind::Free);
    assert!(free.len() >= 10, "{} free cases", free.len());
    let (mut zero, mut iii) = (0, 0);
    for c in &free {
        let name = c.case.name.as_str();
        assert_eq!(c.parameterisation, "macro", "{name}");
        assert_eq!(c.case.model, "pk3.iv_bolus", "{name}");
        assert!(c.case.dose > 0.0, "{name}");
        match c.zone.as_deref() {
            Some("0") => zero += 1,
            Some("iii") => iii += 1,
            other => panic!("{name}: zone {other:?}"),
        }
        for q in [
            "alpha",
            "beta",
            "gamma",
            "w_alpha",
            "w_beta",
            "w_gamma",
            "a",
            "b",
            "c",
            "vc",
            "half_life",
        ] {
            assert!(scalar(c, q).is_some(), "{name}: no {q}");
        }
        for q in ["k10", "k12", "k13", "cl", "vss", "mrt"] {
            assert!(
                scalar(c, q).is_none(),
                "{name}: {q} has no contract in a free zone"
            );
        }
        let (a, b, cc) = (
            c.case.parameters["a"],
            c.case.parameters["b"],
            c.case.parameters["c"],
        );
        close(
            &format!("{name} vc"),
            scalar(c, "vc").unwrap(),
            c.case.dose / (a + b + cc),
            1e-12,
        );
        close(&format!("{name} a"), scalar(c, "a").unwrap(), a, 1e-14);
        let t0 = c.case.times.iter().position(|&t| t == 0.0).unwrap();
        close(
            &format!("{name} C(0)"),
            c.case
                .expected
                .get(&c.case.time_keys[t0], "conc")
                .flatten()
                .unwrap(),
            a + b + cc,
            1e-12,
        );
    }
    assert!(
        zero >= 1 && iii >= 5,
        "{zero} in zone (0), {iii} in zone (iii)"
    );
}
