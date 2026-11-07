//! The measure mode of the wave 3 brief (the same contract for the three prototypes), the part that
//! needs no browser: the plan of the drags, the cells the answer says the screen should show, the
//! comparison with what the screen shows, and the JSON. The loop that drives the page is in `app.rs`.
//!
//!  1. Load nca50.csv (50 subjects of 20 points).
//!  2. 100 drags. Drag k takes subject 1 + k % 50 and sets the λz start so that the range holds the
//!     last 3, 4, 5 or 6 points (`CYCLE[k % 4]`). Choosing the subject comes first and is not timed;
//!     the timed part is the handler a pointer drag of the start handle calls (`set_range_start`) up
//!     to the frame that shows the new result.
//!  3. `input_to_paint_ms`: handler called -> `nca.run` and `plot.scene` answered -> state rendered ->
//!     the first frame after it (a `requestAnimationFrame` callback, then a message, which runs after
//!     the paint). `frame_ms`: the time between that frame and the one before it.
//!  4. Every result cell on screen is compared with the value formatted from the answer.
//!  5. Wait 3 seconds (the memory is sampled then), then exit; a browser page stays open.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::results::{adj_text, all_cells, count, headline_cells, number};
use crate::session::State;

pub const DRAGS: usize = 100;
pub const CYCLE: [usize; 4] = [3, 4, 5, 6];

/// The JSON of the measure, with exactly the keys of the brief.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub stack: &'static str,
    pub version: String,
    pub first_frame_ms: f64,
    pub drags: usize,
    pub input_to_paint_ms: Vec<f64>,
    pub frame_ms: Vec<f64>,
    pub cells_checked: usize,
    pub cells_equal: usize,
    pub errors: Vec<String>,
}

impl Report {
    pub fn new(version: &str, first_frame_ms: f64) -> Self {
        Report {
            stack: "leptos",
            version: version.to_owned(),
            first_frame_ms,
            drags: DRAGS,
            input_to_paint_ms: Vec::new(),
            frame_ms: Vec::new(),
            cells_checked: 0,
            cells_equal: 0,
            errors: Vec::new(),
        }
    }

    pub fn json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            format!("{{\"errors\":[\"the report could not be written: {e}\"]}}")
        })
    }
}

/// The subject of drag `k`.
pub fn subject_of(k: usize) -> String {
    format!("{}", 1 + k % 50)
}

/// The time the start handle is dragged to in drag `k`: the n-th sample from the end, n from the cycle.
pub fn start_of(k: usize, samples: &[f64]) -> Result<f64, String> {
    let n = CYCLE.get(k % CYCLE.len()).copied().unwrap_or(3);
    samples
        .len()
        .checked_sub(n)
        .and_then(|i| samples.get(i))
        .copied()
        .ok_or_else(|| {
            format!(
                "drag {k}: subject {} has fewer than {n} samples",
                subject_of(k)
            )
        })
}

/// What every cell on screen should say, by its `data-cell` key, formatted from the answer.
pub fn expected_cells(state: &State) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let (Some(subject), Some(ok)) = (state.subject_result(), state.ok()) else {
        return out;
    };
    let units = state.units();
    for (prefix, cells) in [
        ("h", headline_cells(subject, ok, &units, true)),
        ("a", all_cells(subject, ok, &units, true)),
    ] {
        for c in cells {
            out.insert(format!("{prefix}:{}", c.key), c.text);
            out.insert(format!("{prefix}-unit:{}", c.key), c.unit);
        }
    }
    for (i, c) in ok.lambda_z_candidates.iter().enumerate() {
        out.insert(format!("c{i}:n"), count(c.n_points));
        out.insert(format!("c{i}:from"), number(c.time_first));
        out.insert(format!("c{i}:to"), number(c.time_last));
        out.insert(format!("c{i}:lambda_z"), number(c.lambda_z));
        out.insert(format!("c{i}:adj"), adj_text(c));
    }
    out
}

/// Compares the cells of the document, as `(data-cell key, text)`, with the answer.
pub fn compare_cells(
    expected: &BTreeMap<String, String>,
    shown: &[(String, String)],
) -> (usize, usize, Vec<String>) {
    let mut equal = 0;
    let mut errors = Vec::new();
    for (key, got) in shown {
        let got = got.trim();
        match expected.get(key) {
            Some(want) if got == want.trim() => equal += 1,
            other => errors.push(format!(
                "cell {key}: shows \"{got}\", the answer formats to \"{}\"",
                other.map_or("(none)", String::as_str)
            )),
        }
    }
    (shown.len(), equal, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drags_follow_the_contract_of_the_brief() {
        assert_eq!(subject_of(0), "1");
        assert_eq!(subject_of(49), "50");
        assert_eq!(subject_of(50), "1");
        assert_eq!(subject_of(99), "50");
        let samples: Vec<f64> = (0..10).map(f64::from).collect();
        assert_eq!(start_of(0, &samples), Ok(7.0)); // last 3
        assert_eq!(start_of(1, &samples), Ok(6.0)); // last 4
        assert_eq!(start_of(2, &samples), Ok(5.0));
        assert_eq!(start_of(3, &samples), Ok(4.0)); // last 6
        assert_eq!(start_of(4, &samples), Ok(7.0)); // the cycle starts again
        assert!(start_of(0, &[1.0, 2.0]).is_err());
    }

    #[test]
    fn the_report_has_exactly_the_keys_of_the_brief() {
        let json: serde_json::Value =
            serde_json::from_str(&Report::new("v", 1.5).json()).unwrap_or_default();
        let mut keys: Vec<&str> = json
            .as_object()
            .map(|o| o.keys().map(String::as_str).collect())
            .unwrap_or_default();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "cells_checked",
                "cells_equal",
                "drags",
                "errors",
                "first_frame_ms",
                "frame_ms",
                "input_to_paint_ms",
                "stack",
                "version"
            ]
        );
        assert_eq!(json["stack"], "leptos");
    }

    #[test]
    fn a_cell_that_differs_is_reported_with_both_texts() {
        let expected: BTreeMap<String, String> = [("h:cmax".to_owned(), "6.3".to_owned())].into();
        let shown = [
            ("h:cmax".to_owned(), " 6.3 ".to_owned()),
            ("h:tmax".to_owned(), "1".to_owned()),
        ];
        let (checked, equal, errors) = compare_cells(&expected, &shown);
        assert_eq!((checked, equal), (2, 1));
        assert!(
            errors
                .first()
                .is_some_and(|e| e.contains("h:tmax") && e.contains("(none)"))
        );
    }
}
