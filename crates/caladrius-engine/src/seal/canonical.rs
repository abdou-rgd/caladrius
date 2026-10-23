//! Canonical JSON: one text for one value, whatever built it.
//!
//! Objects list their keys in byte order, there is no space, a string is written the way
//! `serde_json` writes it, and a number is the shortest text that reads back as the same number
//! (what `serde_json` writes for an `f64`, so two doubles that differ in the last bit give two
//! different texts, and `-0.0` differs from `0.0`). The seals of the commands and the digest of
//! `cargo xtask digest` hash this text.

use serde_json::Value;

/// The canonical text of `value`, and how many numbers it holds.
pub fn canonical_json(value: &Value) -> (String, usize) {
    let mut out = String::new();
    let mut numbers = 0;
    write(value, &mut out, &mut numbers);
    (out, numbers)
}

/// Appends the canonical text of `value` to `out`.
pub(crate) fn write_value(value: &Value, out: &mut String) {
    let mut numbers = 0;
    write(value, out, &mut numbers);
}

/// Appends `s` as a JSON string to `out`.
pub(crate) fn write_string(s: &str, out: &mut String) {
    string(s, out);
}

fn write(value: &Value, out: &mut String, numbers: &mut usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => {
            *numbers += 1;
            out.push_str(&n.to_string());
        }
        Value::String(s) => string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(item, out, numbers);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                string(key, out);
                out.push(':');
                if let Some(item) = map.get(key) {
                    write(item, out, numbers);
                }
            }
            out.push('}');
        }
    }
}

fn string(s: &str, out: &mut String) {
    match serde_json::to_string(s) {
        Ok(text) => out.push_str(&text),
        // Not reachable: a `str` always serializes. Keep the output well formed anyway.
        Err(_) => out.push_str("\"\""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_are_sorted_and_nothing_is_spaced() {
        let (text, numbers) =
            canonical_json(&json!({ "b": [1, 2.5, "x"], "a": { "z": null, "y": true } }));
        assert_eq!(text, r#"{"a":{"y":true,"z":null},"b":[1,2.5,"x"]}"#);
        assert_eq!(numbers, 2);
    }

    #[test]
    fn the_text_of_a_number_tells_the_last_bit() {
        let a = 0.1_f64 + 0.2;
        let b = 0.3_f64;
        assert_ne!(a.to_bits(), b.to_bits());
        let (ta, _) = canonical_json(&json!(a));
        let (tb, _) = canonical_json(&json!(b));
        assert_ne!(ta, tb);
        assert_eq!(ta.parse::<f64>().ok().map(f64::to_bits), Some(a.to_bits()));
        let (zero, _) = canonical_json(&json!(0.0_f64));
        let (negative_zero, _) = canonical_json(&json!(-0.0_f64));
        assert_ne!(zero, negative_zero);
    }

    #[test]
    fn strings_are_escaped_and_other_text_is_kept() {
        let (text, _) = canonical_json(&json!("a\"b\\c\n\u{3bb}z"));
        assert_eq!(text, "\"a\\\"b\\\\c\\n\u{3bb}z\"");
    }

    #[test]
    fn the_same_value_built_in_another_order_has_the_same_text() {
        let mut first = serde_json::Map::new();
        first.insert("one".into(), json!(1));
        first.insert("two".into(), json!(2));
        let mut second = serde_json::Map::new();
        second.insert("two".into(), json!(2));
        second.insert("one".into(), json!(1));
        assert_eq!(
            canonical_json(&Value::Object(first)),
            canonical_json(&Value::Object(second))
        );
    }
}
