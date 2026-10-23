//! Seals: the hashed text, the data a command reads, the time, and the stamps of files.

use super::*;
use serde_json::json;

fn failure(code: &str) -> Result<Value, CommandError> {
    Err(CommandError::new(code, "a message that is not hashed"))
}

#[test]
fn the_sealed_text_is_the_canonical_text_of_the_command_the_parameters_and_the_outcome() {
    let params = json!({ "b": [1, 2.5], "a": "x" });
    let ok: Result<Value, CommandError> = Ok(json!({ "z": 1, "y": [true, null] }));
    let text = sealed_text("nca.run", &params, &ok);
    assert_eq!(
        text,
        r#"{"command":"nca.run","format":"caladrius-seal-1","outcome":{"ok":true,"result":{"y":[true,null],"z":1}},"params":{"a":"x","b":[1,2.5]}}"#
    );
    // The order written by hand is the canonical one.
    let document = json!({
        "command": "nca.run", "format": TAG, "params": params,
        "outcome": { "ok": true, "result": { "z": 1, "y": [true, null] } },
    });
    assert_eq!(text, canonical_json(&document).0);
    let err = failure("nca_error");
    let document = json!({
        "command": "nca.run", "format": TAG, "params": params,
        "outcome": { "ok": false, "error": "nca_error" },
    });
    assert_eq!(
        sealed_text("nca.run", &params, &err),
        canonical_json(&document).0
    );
}

#[test]
fn a_failure_is_sealed_by_its_code_and_not_by_its_message() {
    let params = json!({});
    let one = seal("x.y", &params, &failure("a_code"), None, false);
    let other_message = Err(CommandError::new("a_code", "another wording"));
    let two = seal("x.y", &params, &other_message, None, false);
    let other_code = seal("x.y", &params, &failure("b_code"), None, false);
    assert_eq!(one.sha256, two.sha256);
    assert_ne!(one.sha256, other_code.sha256);
}

#[test]
fn one_changed_bit_changes_the_seal_and_the_order_of_keys_does_not() {
    let params = json!({ "dose": 100, "route": "iv_bolus" });
    let reordered = json!({ "route": "iv_bolus", "dose": 100 });
    let result = |x: f64| -> Result<Value, CommandError> { Ok(json!({ "cmax": x })) };
    let a = seal("nca.run", &params, &result(0.1 + 0.2), None, false);
    let b = seal("nca.run", &reordered, &result(0.1 + 0.2), None, false);
    let c = seal("nca.run", &params, &result(0.3), None, false);
    assert_eq!(a.sha256, b.sha256);
    assert_ne!(
        a.sha256, c.sha256,
        "the last bit of a double is part of the seal"
    );
    let other_command = seal("nca.runs", &params, &result(0.1 + 0.2), None, false);
    assert_ne!(a.sha256, other_command.sha256);
    let other_params = seal(
        "nca.run",
        &json!({ "dose": 101 }),
        &result(0.1 + 0.2),
        None,
        false,
    );
    assert_ne!(a.sha256, other_params.sha256);
}

#[test]
fn the_provenance_is_next_to_the_hash_and_not_in_it() {
    let params = json!({});
    let outcome: Result<Value, CommandError> = Ok(json!({ "n": 1 }));
    let with = seal(
        "project.describe",
        &params,
        &outcome,
        Some("f".repeat(64)),
        true,
    );
    let without = seal("project.describe", &params, &outcome, None, false);
    assert_eq!(with.sha256, without.sha256);
    assert_eq!(with.engine.as_deref(), Some(ENGINE_VERSION));
    assert_eq!(with.commit.as_deref(), GIT_COMMIT);
    assert_eq!(with.input, Some("f".repeat(64)));
    assert_eq!(
        (without.engine, without.commit, without.input),
        (None, None, None)
    );
    assert_eq!(with.format, FORMAT);
    assert_eq!(with.short().len(), SHORT);
    assert!(with.sha256.starts_with(with.short()));
}

#[test]
fn the_build_knows_its_version_and_usually_its_commit() {
    assert!(!ENGINE_VERSION.is_empty());
    if let Some(commit) = GIT_COMMIT {
        assert!(commit.len() >= 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}

#[test]
fn the_echo_of_the_history_is_hashed_without_times_and_provenance() {
    let entry = |engine: &str, at: &str| {
        json!({
            "seq": 1, "command": "x", "params": {}, "ok": true, "error": null,
            "changed_project": false,
            "seal": { "format": 1, "sha256": "ab", "engine": engine, "commit": "c", "input": "i" },
            "at": at,
        })
    };
    let params = json!({});
    let save = |engine: &str, at: &str| -> Result<Value, CommandError> {
        Ok(json!({ "file_name": "p.json", "project": { "history": [entry(engine, at)] } }))
    };
    let a = sealed_text(
        "project.save",
        &params,
        &save("0.1.0", "2026-01-01T00:00:00Z"),
    );
    let b = sealed_text(
        "project.save",
        &params,
        &save("0.2.0", "2027-01-01T00:00:00Z"),
    );
    assert_eq!(a, b);
    assert!(a.contains(r#""sha256":"ab""#) && a.contains(r#""input":"i""#));
    assert!(!a.contains("2026") && !a.contains("0.1.0"));
    let list = |engine: &str, at: &str| -> Result<Value, CommandError> {
        Ok(json!({ "total": 1, "entries": [entry(engine, at)] }))
    };
    assert_eq!(
        sealed_text("history.list", &params, &list("0.1.0", "a")),
        sealed_text("history.list", &params, &list("0.2.0", "b"))
    );
    // Any other command keeps whatever its result holds.
    let kept = sealed_text("data.describe", &params, &list("0.1.0", "a"));
    assert!(kept.contains("0.1.0") && kept.contains("\"at\":\"a\""));
}

#[test]
fn project_load_and_project_replay_carry_no_seal() {
    assert!(!is_sealed("project.load"));
    assert!(!is_sealed("project.replay"));
    assert!(is_sealed("project.save"));
    assert!(is_sealed("nca.run"));
    assert!(is_sealed("no.such.command"));
}

#[test]
fn the_time_is_written_as_rfc_3339_in_utc() {
    assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339(1_000_000_000), "2001-09-09T01:46:40Z");
    assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(rfc3339(951_868_799), "2000-02-29T23:59:59Z");
    assert_eq!(rfc3339(4_102_444_800), "2100-01-01T00:00:00Z");
    assert_eq!(rfc3339(1_793_000_000), "2026-10-26T07:33:20Z");
    assert_eq!(rfc3339(-1), "1969-12-31T23:59:59Z");
    assert_eq!(rfc3339(-86_400 * 365), "1969-01-01T00:00:00Z");
    // The extremes do not panic or wrap.
    assert!(rfc3339(i64::MAX).ends_with('Z'));
    assert!(rfc3339(i64::MIN).ends_with('Z'));
}

fn sample_seal() -> Seal {
    Seal {
        format: FORMAT,
        sha256: "a".repeat(64),
        engine: Some("0.3.2".to_owned()),
        commit: Some("b".repeat(40)),
        input: Some("c".repeat(64)),
    }
}

#[test]
fn a_stamp_is_one_line_of_words() {
    let line = stamp_line("export.table", &sample_seal());
    assert_eq!(
        line,
        format!(
            "caladrius engine=0.3.2 commit={} command=export.table seal={} input={}",
            "b".repeat(40),
            "a".repeat(64),
            "c".repeat(64)
        )
    );
    let bare = Seal {
        engine: None,
        commit: None,
        input: None,
        ..sample_seal()
    };
    assert_eq!(
        stamp_line("plot.xy", &bare),
        format!("caladrius command=plot.xy seal={}", "a".repeat(64))
    );
    let odd = Seal {
        engine: Some("1 0--rc".to_owned()),
        ..sample_seal()
    };
    assert!(stamp_line("x", &odd).starts_with("caladrius engine=1_0-rc commit="));
}

#[test]
fn a_csv_with_a_stamp_reads_back_without_it() {
    let line = stamp_line("export.table", &sample_seal());
    let csv = "time,conc\n0,1\n1,2\n";
    let stamped = csv_with_stamp(csv, &line);
    assert!(stamped.starts_with("# caladrius engine=0.3.2"));
    assert_eq!(strip_stamp(&stamped), csv);
    // A file that has no stamp is returned as it is, byte order mark included.
    assert_eq!(strip_stamp(csv), csv);
    assert_eq!(strip_stamp("\u{feff}time,conc\n"), "\u{feff}time,conc\n");
    // A byte order mark in front of the stamp goes with it; two stamps go; another comment stays.
    assert_eq!(strip_stamp(&format!("\u{feff}{stamped}")), csv);
    assert_eq!(strip_stamp(&format!("# {line}\n{stamped}")), csv);
    assert_eq!(
        strip_stamp("# a note\ntime,conc\n"),
        "# a note\ntime,conc\n"
    );
    assert_eq!(strip_stamp("# caladrius only"), "");
    assert_eq!(strip_stamp("# caladrius a\r\ntime\r\n"), "time\r\n");
    assert_eq!(strip_stamp(""), "");
}

#[test]
fn an_svg_with_a_stamp_still_starts_with_its_root() {
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\"><g/></svg>";
    let stamped = svg_with_stamp(svg, "caladrius seal=ab--cd");
    assert!(stamped.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\">"));
    assert!(stamped.contains("<!-- caladrius seal=ab-cd -->"));
    assert!(stamped.ends_with("<g/></svg>"));
    assert_eq!(svg_with_stamp("not svg", "x"), "not svg");
    // The comment is well formed: it holds no double dash before its end.
    let inside = stamped
        .split("<!--")
        .nth(1)
        .and_then(|t| t.split("-->").next());
    assert!(inside.is_some_and(|t| !t.contains("--")));
}

#[test]
fn a_json_object_gets_the_seal_unless_it_has_one() {
    let sealed = json_with_seal(json!({ "a": 1 }), &sample_seal());
    assert_eq!(sealed["a"], 1);
    assert_eq!(sealed["seal"]["sha256"], "a".repeat(64));
    let kept = json_with_seal(json!({ "seal": "mine" }), &sample_seal());
    assert_eq!(kept["seal"], "mine");
    assert_eq!(json_with_seal(json!([1]), &sample_seal()), json!([1]));
}
