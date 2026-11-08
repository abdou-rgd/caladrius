#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: helpers panic on purpose

//! Cross-checks the population-simulation oracle (task T-158, `specs/simulation.md` SIM-ORA-*) against the
//! reference implementation of the testkit (`sim_ref`, written without the R scripts), so that the expected
//! files are tested by something other than the code that wrote them:
//!
//! - every check of `sim_check` that the engine tests run passes on the reference (KAT and blocks of SX1,
//!   streams, the AS 241 grid, 22 exact cases, 9 studies, SX3 statistics, type 7 quantiles, 14 invariance
//!   checks, every refusal row with its control, every accepted row);
//! - the hexadecimal-float parser round-trips, and the uniforms in the stream files are the ones of the words;
//! - the label-key collision pair really collides (SHA-256, first 8 bytes) and the two labels are different;
//! - the sealed design of the round trip is the file whose hash the case states;
//! - the manifest lists every file with its SHA-256, and the combined hash is the hash of the list.
//!
//! This tests the oracle and the testkit, not the engine.

use caladrius_testkit::pop::sha256_hex;
use caladrius_testkit::sim::{
    self, bits_hex, load_sim, load_sim_top, parse_hex_float, word_uniform,
};
use caladrius_testkit::sim_check as check;
use caladrius_testkit::sim_ref::reference_simulation;

fn run_all(kind: &str, names: Vec<String>, f: impl Fn(&str) -> Result<(), String>) {
    assert!(!names.is_empty(), "{kind}: no case found");
    let failures: Vec<String> = names.iter().filter_map(|n| f(n).err()).collect();
    assert!(
        failures.is_empty(),
        "{kind}: {} of {} cases fail on the reference\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
    println!("{kind}: {} cases pass", names.len());
}

#[test]
fn philox_known_answers_and_blocks() {
    run_all("philox", check::kat_names().unwrap(), |n| {
        check::check_philox(&reference_simulation, n)
    });
}

#[test]
fn streams() {
    run_all("streams", check::stream_names().unwrap(), |n| {
        check::check_stream(&reference_simulation, n)
    });
}

#[test]
fn transform_grid() {
    check::check_transform_grid(&reference_simulation).unwrap();
}

#[test]
fn exact_cases() {
    let names = check::exact_names().unwrap();
    assert_eq!(names.len(), 22, "{names:?}");
    run_all("exact", names, |n| {
        check::check_exact(&reference_simulation, n)
    });
}

#[test]
fn studies() {
    let names = check::study_names().unwrap();
    assert_eq!(names.len(), 9, "{names:?}");
    run_all("study", names, |n| {
        check::check_study(&reference_simulation, n)
    });
}

#[test]
fn distribution_of_the_draws() {
    check::check_sx3_streams(&reference_simulation).unwrap();
    check::check_sx3_correlated_eta(&reference_simulation).unwrap();
}

#[test]
fn quantiles() {
    run_all("quantile", check::quantile_names().unwrap(), |n| {
        check::check_quantile(&reference_simulation, n)
    });
}

#[test]
fn invariance() {
    run_all("invariance", check::invariance_names().unwrap(), |n| {
        check::check_invariance(&reference_simulation, n)
    });
}

#[test]
fn refusals_with_controls() {
    run_all(
        "refused",
        check::error_row_names("refused", true).unwrap(),
        |n| check::check_refused(&reference_simulation, n),
    );
}

#[test]
fn accepted_inputs() {
    run_all(
        "accepted",
        check::error_row_names("accepted", true).unwrap(),
        |n| check::check_accepted(&reference_simulation, n),
    );
}

#[test]
fn rows_of_errors_json_are_well_formed() {
    let rows = check::error_rows().unwrap();
    assert!(rows.len() >= 60, "{} rows", rows.len());
    let mut seen = std::collections::BTreeSet::new();
    for r in &rows {
        let name = r["case"].as_str().unwrap();
        assert!(seen.insert(name.to_owned()), "duplicate row {name}");
        assert!(
            r["rule"].as_str().is_some_and(|s| s.starts_with("SIM-")),
            "{name}: rule"
        );
        assert!(
            r["description"].as_str().is_some_and(|s| !s.is_empty()),
            "{name}: description"
        );
        let expect = r["expect"].as_str().unwrap();
        assert!(
            expect == "refused" || expect == "accepted",
            "{name}: {expect}"
        );
        if r["constructible"].as_bool() == Some(false) {
            assert!(
                r["note"].as_str().is_some_and(|s| !s.is_empty()),
                "{name}: a sentence-only row needs a note"
            );
            continue;
        }
        assert!(r["request"].is_object(), "{name}: request");
        if expect == "refused" {
            assert!(
                r["error"]["words"]
                    .as_array()
                    .is_some_and(|w| !w.is_empty()),
                "{name}: words"
            );
            assert!(
                r["control"]["request"].is_object(),
                "{name}: a refusal needs a control"
            );
        }
    }
    let collision = rows
        .iter()
        .find(|r| r["case"].as_str() == Some("label_key_collision"))
        .expect("label_key_collision row");
    assert_ne!(
        collision["constructible"].as_bool(),
        Some(false),
        "the collision pair was found and the row is built"
    );
}

/// A hexadecimal float of a double, as R's `%a` writes it.
fn hex_float(x: f64) -> String {
    let bits = x.to_bits();
    let sign = if bits >> 63 == 1 { "-" } else { "" };
    let exp = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & ((1u64 << 52) - 1);
    if exp == 0 && frac == 0 {
        return format!("{sign}0x0p+0");
    }
    let (lead, e) = if exp == 0 {
        (0, -1022)
    } else {
        (1, exp - 1023)
    };
    let digits = format!("{frac:013x}");
    let digits = digits.trim_end_matches('0');
    let dot = if digits.is_empty() {
        String::new()
    } else {
        format!(".{digits}")
    };
    format!("{sign}0x{lead}{dot}p{e:+}")
}

#[test]
fn hex_floats_round_trip() {
    for x in [
        0.0,
        1.0,
        -1.0,
        0.1,
        1e-300,
        4.9e-324,
        1.7976931348623157e308,
        0.5 * (1.0 + f64::EPSILON),
        -123456.789,
    ] {
        let back = parse_hex_float(&hex_float(x)).unwrap();
        assert_eq!(back.to_bits(), x.to_bits(), "{x}");
    }
    assert_eq!(bits_hex(1.0), "3ff0000000000000");
}

#[test]
fn stream_uniforms_are_the_words() {
    let streams = load_sim("gen", "streams").unwrap();
    let mut n = 0;
    for c in streams["cases"].as_array().unwrap() {
        for w in c["words"].as_array().unwrap() {
            let word = u64::from_str_radix(w["word"].as_str().unwrap(), 16).unwrap();
            let u = parse_hex_float(w["u"].as_str().unwrap()).unwrap();
            assert_eq!(
                u.to_bits(),
                word_uniform(word).to_bits(),
                "{} word {}",
                c["name"],
                w["n"]
            );
            n += 1;
        }
    }
    assert!(n > 100, "{n} words");
}

#[test]
fn collision_pair_collides() {
    let rows = check::error_rows().unwrap();
    let row = rows
        .iter()
        .find(|r| r["case"].as_str() == Some("label_key_collision"))
        .unwrap();
    let subjects = row["request"]["design"]["subjects"].as_array().unwrap();
    let ids: Vec<&str> = subjects.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert!(ids.len() >= 2 && ids[0] != ids[1], "{ids:?}");
    let key = |id: &str| -> String { sha256_hex(id.as_bytes())[..16].to_owned() };
    assert_eq!(
        key(ids[0]),
        key(ids[1]),
        "the two labels must have the same first 8 bytes of SHA-256"
    );
    assert_eq!(key(ids[0]), "5f0dd73b3f5913b6");
}

#[test]
fn roundtrip_cases_are_well_formed() {
    let names = sim::list_sim_cases("roundtrip").unwrap();
    assert!(names.contains(&"accurate".to_owned()), "{names:?}");
    for n in &names {
        let c = load_sim("roundtrip", n).unwrap();
        assert_eq!(c["simulate"]["op"].as_str(), Some("simulate"), "{n}");
        assert!(
            c["fit"]["truth"].is_object() && c["fit"]["start_factors"].is_object(),
            "{n}"
        );
        let file = c["sealed_design_file"].as_str().unwrap();
        let path = sim::sim_dir().join("..").join("..").join("..").join(file);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            sha256_hex(&bytes),
            c["sealed_design_sha256"].as_str().unwrap(),
            "{n}: the sealed design file changed"
        );
        assert!(c["aggregate_checks"]["asserted"].is_boolean(), "{n}");
        assert!(
            c["pending"].as_str().is_some_and(|s| s.contains("T-154")),
            "{n}"
        );
    }
}

#[test]
fn manifest_matches_the_files() {
    let m = load_sim_top("manifest").unwrap();
    let dir = sim::sim_dir();
    let mut listed = Vec::new();
    let mut text = String::new();
    for f in m["files"].as_array().unwrap() {
        let path = f["path"].as_str().unwrap();
        let bytes = std::fs::read(dir.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let h = sha256_hex(&bytes);
        assert_eq!(h, f["sha256"].as_str().unwrap(), "{path}");
        text.push_str(&format!("{path}  {h}\n"));
        listed.push(path.to_owned());
    }
    assert_eq!(
        sha256_hex(text.as_bytes()),
        m["combined_sha256"].as_str().unwrap()
    );
    // every file on disk is listed
    let mut on_disk = Vec::new();
    for sub in [
        "gen",
        "transform",
        "exact",
        "study",
        "dist",
        "quantile",
        "invariance",
        "roundtrip",
    ] {
        for e in std::fs::read_dir(dir.join(sub)).unwrap() {
            let name = e.unwrap().file_name().into_string().unwrap();
            on_disk.push(format!("{sub}/{name}"));
        }
    }
    on_disk.push("errors.json".to_owned());
    on_disk.sort();
    listed.sort();
    assert_eq!(on_disk, listed);
}
