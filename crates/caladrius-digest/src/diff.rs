//! The comparison of two canonical dumps (the text of [`crate::Run::text`], one JSON document per
//! step): which steps differ, how many numbers are not the same double and by how much, and where
//! a list is longer on one side. It is what the digest of two platforms cannot say: that they differ
//! is one bit of information, how they differ is this report.

use serde_json::{Number, Value};

/// How many examples of each kind the report keeps.
const KEPT: usize = 8;

/// What differs between two dumps.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Diff {
    /// Steps in the left and in the right dump.
    pub steps: (usize, usize),
    /// Steps whose text is not the same on both sides.
    pub steps_differing: usize,
    /// Numbers found at the same place on both sides.
    pub numbers: usize,
    /// Of those, the numbers that are not the same double (a different last bit counts).
    pub numbers_differing: usize,
    /// The largest relative difference, `|a - b| / max(|a|, |b|)`, among the numbers that are doubles
    /// on both sides (a count that changed, such as a number of halvings, is not one of them), and
    /// where it is.
    pub largest: Option<(f64, String)>,
    /// How many differences between doubles fall in each class of relative size: up to each bound of
    /// [`SIZES`], then above the last. Rounding alone is in the first classes; a minimization that took
    /// another path is in the last ones.
    pub sizes: [usize; 6],
    /// Texts that differ.
    pub texts_differing: usize,
    /// Places where the shape differs: a list with another length, a key on one side only, a value of
    /// another kind. The first few are kept.
    pub shape: Vec<String>,
    /// How many such places there are.
    pub shape_total: usize,
    /// The first differing numbers: where, left, right.
    pub first: Vec<String>,
}

/// Upper bounds of the classes of relative difference counted in [`Diff::sizes`].
pub const SIZES: [f64; 5] = [1e-15, 1e-12, 1e-9, 1e-6, 1e-3];

impl Diff {
    /// True when the two dumps say the same thing.
    pub fn identical(&self) -> bool {
        self.steps.0 == self.steps.1
            && self.steps_differing == 0
            && self.numbers_differing == 0
            && self.texts_differing == 0
            && self.shape_total == 0
    }

    /// The report, a few lines of text.
    pub fn report(&self) -> String {
        let mut out = Vec::new();
        out.push(if self.steps.0 == self.steps.1 {
            format!(
                "steps: {} on each side, {} differ",
                self.steps.0, self.steps_differing
            )
        } else {
            format!(
                "steps: {} on the left, {} on the right, {} differ",
                self.steps.0, self.steps.1, self.steps_differing
            )
        });
        let share = if self.numbers == 0 {
            0.0
        } else {
            100.0 * self.numbers_differing as f64 / self.numbers as f64
        };
        out.push(format!(
            "numbers: {} compared, {} differ ({share:.1} %)",
            self.numbers, self.numbers_differing
        ));
        if let Some((relative, at)) = &self.largest {
            out.push(format!(
                "largest relative difference between doubles: {relative:.3e} at {at}"
            ));
        }
        if self.sizes.iter().any(|&n| n > 0) {
            let mut classes = Vec::new();
            for (i, count) in self.sizes.iter().enumerate() {
                classes.push(match SIZES.get(i) {
                    Some(bound) => format!("up to {bound:.0e}: {count}"),
                    None => format!("above: {count}"),
                });
            }
            out.push(format!(
                "differences between doubles by relative size: {}",
                classes.join(", ")
            ));
        }
        out.push(format!("texts that differ: {}", self.texts_differing));
        out.push(format!(
            "differences of shape (a list of another length, a key on one side only): {}",
            self.shape_total
        ));
        for line in &self.shape {
            out.push(format!("  {line}"));
        }
        if self.shape_total > self.shape.len() {
            out.push(format!(
                "  ... and {} more",
                self.shape_total - self.shape.len()
            ));
        }
        if !self.first.is_empty() {
            out.push("first numbers that differ:".to_owned());
            for line in &self.first {
                out.push(format!("  {line}"));
            }
        }
        out.join("\n")
    }

    fn note_shape(&mut self, line: String) {
        self.shape_total += 1;
        if self.shape.len() < KEPT {
            self.shape.push(line);
        }
    }
}

/// Compares two dumps step by step. The error is a line that is not JSON.
pub fn compare(left: &str, right: &str) -> Result<Diff, String> {
    let mut diff = Diff {
        steps: (left.lines().count(), right.lines().count()),
        ..Diff::default()
    };
    for (index, (a, b)) in left.lines().zip(right.lines()).enumerate() {
        let parse = |text: &str, side: &str| {
            serde_json::from_str::<Value>(text)
                .map_err(|e| format!("{side} dump, line {}: {e}", index + 1))
        };
        let (value_a, value_b) = (parse(a, "left")?, parse(b, "right")?);
        if a != b {
            diff.steps_differing += 1;
        }
        let mut path = step_name(&value_a);
        walk(&value_a, &value_b, &mut path, &mut diff);
    }
    Ok(diff)
}

/// `session#step id`, from the two fields every step carries.
fn step_name(step: &Value) -> String {
    let text = |key: &str| step.get(key).and_then(Value::as_str).unwrap_or("?");
    format!("{} {}", text("at"), text("id"))
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a text",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

fn walk(left: &Value, right: &Value, path: &mut String, diff: &mut Diff) {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => number(a, b, path, diff),
        (Value::String(a), Value::String(b)) => {
            if a != b {
                diff.texts_differing += 1;
            }
        }
        (Value::Bool(a), Value::Bool(b)) if a == b => {}
        (Value::Null, Value::Null) => {}
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                diff.note_shape(format!(
                    "{path}: {} entries on the left, {} on the right",
                    a.len(),
                    b.len()
                ));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                let keep = path.len();
                path.push_str(&format!("/{i}"));
                walk(x, y, path, diff);
                path.truncate(keep);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            for (key, x) in a {
                let keep = path.len();
                path.push_str(&format!("/{key}"));
                match b.get(key) {
                    Some(y) => walk(x, y, path, diff),
                    None => diff.note_shape(format!("{path}: on the left only")),
                }
                path.truncate(keep);
            }
            for key in b.keys().filter(|key| !a.contains_key(*key)) {
                diff.note_shape(format!("{path}/{key}: on the right only"));
            }
        }
        _ => diff.note_shape(format!(
            "{path}: {} on the left, {} on the right",
            kind(left),
            kind(right)
        )),
    }
}

fn number(a: &Number, b: &Number, path: &str, diff: &mut Diff) {
    diff.numbers += 1;
    let same = if a.is_f64() && b.is_f64() {
        a.as_f64().map(f64::to_bits) == b.as_f64().map(f64::to_bits)
    } else {
        a == b
    };
    if same {
        return;
    }
    diff.numbers_differing += 1;
    if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64())
        && a.is_f64()
        && b.is_f64()
    {
        let scale = x.abs().max(y.abs());
        let relative = if scale > 0.0 {
            (x - y).abs() / scale
        } else {
            0.0
        };
        let class = SIZES
            .iter()
            .position(|bound| relative <= *bound)
            .unwrap_or(SIZES.len());
        if let Some(count) = diff.sizes.get_mut(class) {
            *count += 1;
        }
        if diff
            .largest
            .as_ref()
            .is_none_or(|(best, _)| relative > *best)
        {
            diff.largest = Some((relative, path.to_owned()));
        }
    }
    if diff.first.len() < KEPT {
        diff.first
            .push(format!("{path}: {a} on the left, {b} on the right"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(session: &str, n: usize, result: &str) -> String {
        format!(r#"{{"at":"{session}#{n}","id":"x.y","ok":true,"result":{result}}}"#)
    }

    #[test]
    fn the_same_dump_is_identical() {
        let dump = format!(
            "{}\n{}\n",
            step("a", 0, r#"{"v":[1,2.5]}"#),
            step("a", 1, "null")
        );
        let diff = compare(&dump, &dump).unwrap();
        assert!(diff.identical());
        assert_eq!(diff.steps, (2, 2));
        assert_eq!(diff.numbers, 2);
        assert_eq!(diff.numbers_differing, 0);
    }

    #[test]
    fn a_last_bit_is_a_difference_and_its_size_is_reported() {
        let left = step("s", 3, r#"{"auc":[1.0,2.0],"k":0.3}"#);
        let right = step("s", 3, r#"{"auc":[1.0,2.0000000000000004],"k":0.3}"#);
        let diff = compare(&left, &right).unwrap();
        assert!(!diff.identical());
        assert_eq!(diff.numbers, 3);
        assert_eq!(diff.numbers_differing, 1);
        assert_eq!(diff.steps_differing, 1);
        let (relative, at) = diff.largest.clone().unwrap_or_default();
        assert!(relative > 1e-16 && relative < 3e-16, "{relative}");
        assert_eq!(at, "s#3 x.y/result/auc/1");
        assert_eq!(diff.shape_total, 0);
    }

    #[test]
    fn a_list_with_another_length_is_a_difference_of_shape() {
        let left = step("fit/indometh", 1, r#"{"trace":[1,2,3,4,5,6]}"#);
        let right = step("fit/indometh", 1, r#"{"trace":[1,2,3,4,5]}"#);
        let diff = compare(&left, &right).unwrap();
        assert_eq!(diff.shape_total, 1);
        assert_eq!(
            diff.shape,
            ["fit/indometh#1 x.y/result/trace: 6 entries on the left, 5 on the right"]
        );
        assert_eq!(diff.numbers, 5);
        assert_eq!(diff.numbers_differing, 0);
        assert!(!diff.identical());
    }

    #[test]
    fn keys_kinds_texts_and_steps_are_compared_too() {
        let left = format!(
            "{}\n{}\n",
            step("a", 0, r#"{"p":1,"q":"x"}"#),
            step("a", 1, "2")
        );
        let right = format!("{}\n", step("a", 0, r#"{"q":"y","r":true}"#));
        let diff = compare(&left, &right).unwrap();
        assert_eq!(diff.steps, (2, 1));
        assert_eq!(diff.texts_differing, 1);
        assert_eq!(diff.shape_total, 2);
        assert!(
            diff.report().contains("on the left only"),
            "{}",
            diff.report()
        );
        let kinds = compare(&step("a", 0, "1"), &step("a", 0, r#""1""#)).unwrap();
        assert_eq!(kinds.shape_total, 1);
    }

    #[test]
    fn the_report_names_the_counts() {
        let left = step("s", 0, "[1.5,2.5,3.5,4.5]");
        let right = step("s", 0, "[1.5,2.5,3.5,4.500000000000001]");
        let report = compare(&left, &right).unwrap().report();
        assert!(
            report.contains("numbers: 4 compared, 1 differ (25.0 %)"),
            "{report}"
        );
        assert!(report.contains("largest relative difference"), "{report}");
    }

    #[test]
    fn the_rounding_alone_is_told_apart_from_a_path_that_split() {
        let rounded = step("a", 0, r#"{"x":1.0,"z":3.0}"#);
        let rounded_other = step("a", 0, r#"{"x":1.0000000000000002,"z":3.000000003}"#);
        let split = step("b", 0, r#"{"trace":[1.0,2.0],"y":1.0}"#);
        let split_other = step("b", 0, r#"{"trace":[1.5],"y":2.0}"#);
        let left = format!(
            "{rounded}
{split}
"
        );
        let right = format!(
            "{rounded_other}
{split_other}
"
        );
        let diff = compare(&left, &right).unwrap();
        assert_eq!(diff.sizes, [1, 0, 1, 0, 0, 2]);
        let (largest, at) = diff.largest.clone().unwrap_or_default();
        assert!((largest - 0.5).abs() < 1e-12, "{largest}");
        assert_eq!(at, "b#0 x.y/result/y");
        assert!(
            diff.report().contains("up to 1e-15: 1, up to 1e-12: 0"),
            "{}",
            diff.report()
        );
    }

    #[test]
    fn a_changed_count_is_a_difference_but_not_a_relative_one() {
        let left = step("s", 0, r#"{"halvings":0,"x":1.5}"#);
        let right = step("s", 0, r#"{"halvings":1,"x":1.5000000000000002}"#);
        let diff = compare(&left, &right).unwrap();
        assert_eq!(diff.numbers_differing, 2);
        let (relative, at) = diff.largest.clone().unwrap_or_default();
        assert!(relative < 2e-16, "{relative}");
        assert_eq!(at, "s#0 x.y/result/x");
    }

    #[test]
    fn a_line_that_is_not_json_is_an_error() {
        assert!(compare("{", "{}").is_err());
    }
}
