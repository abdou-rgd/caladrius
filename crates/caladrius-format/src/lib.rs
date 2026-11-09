#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unreachable
)]
//! Layer L1: display formatting, numbers for the eye and names for parameters. Never rounds a
//! stored value.
//!
//! How numbers are shown (significant digits, decimal mark) is a setting of the person
//! ([`Display`]). Two ways to give it: [`set_display`] sets it once on the current thread and the
//! plain functions ([`number`], [`fit_quality`], [`exact`]) read it, which is what the egui
//! interface does at the start of every frame; and the `_with` functions ([`number_with`],
//! [`fit_quality_with`], [`exact_with`]) take it as an argument, for a caller that has no
//! thread-local setting to rely on (a page that calls the WebAssembly build from several places).
//! The plain function and the `_with` function give the same text for the same [`Display`]. The
//! default is four significant digits and a decimal point.
//!
//! The crate depends on `libm` alone and compiles to WebAssembly: the egui interface, the browser
//! page and the desktop webview format a number with this code and no other.

mod labels;

pub use labels::{parameter_label, parameter_unit, plain, with_labels, words};

use std::cell::Cell;

/// How numbers are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display {
    /// Significant digits of [`number`].
    pub digits: u8,
    /// A decimal comma instead of a point.
    pub comma: bool,
}

impl Default for Display {
    fn default() -> Self {
        Display {
            digits: 4,
            comma: false,
        }
    }
}

thread_local! {
    static DISPLAY: Cell<Display> = const { Cell::new(Display { digits: 4, comma: false }) };
}

/// Sets how numbers are shown on this thread (the UI thread).
pub fn set_display(display: Display) {
    DISPLAY.with(|d| d.set(display));
}

/// How numbers are shown now on this thread.
pub fn display() -> Display {
    DISPLAY.with(Cell::get)
}

/// Writes the decimal mark the person chose.
fn mark(text: String, display: Display) -> String {
    if display.comma {
        text.replace('.', ",")
    } else {
        text
    }
}

/// A number the person typed: `3,25` and `3.25` both read as 3.25, a typographic minus as a
/// minus. `None` for text that is not a finite number.
pub fn parse_number(text: &str) -> Option<f64> {
    let cleaned: String = text
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            ',' => '.',
            '\u{2212}' => '-',
            other => other,
        })
        .collect();
    cleaned.parse::<f64>().ok().filter(|x| x.is_finite())
}

/// A number with the chosen number of significant digits (four by default), scientific notation
/// outside 1e-3 to 1e5. `-` for a value that is not finite.
pub fn number(x: f64) -> String {
    number_with(x, display())
}

/// [`number`] with the display given instead of read from the thread.
pub fn number_with(x: f64, display: Display) -> String {
    if !x.is_finite() {
        return "-".to_owned();
    }
    if x == 0.0 {
        return "0".to_owned();
    }
    let digits = usize::from(display.digits.clamp(2, 10));
    let magnitude = libm::floor(libm::log10(x.abs()));
    if !(-3.0..5.0).contains(&magnitude) {
        return mark(format!("{x:.prec$e}", prec = digits - 1), display);
    }
    let decimals = ((digits as f64 - 1.0) - magnitude).clamp(0.0, 12.0) as usize;
    let text = format!("{x:.decimals$}");
    // Drop trailing zeros after a decimal point, but keep the digits that carry information.
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    };
    mark(text, display)
}

/// A goodness of fit: up to five decimals, enough to tell 0.9999 from 1.
pub fn fit_quality(x: f64) -> String {
    fit_quality_with(x, display())
}

/// [`fit_quality`] with the display given instead of read from the thread.
pub fn fit_quality_with(x: f64, display: Display) -> String {
    if !x.is_finite() {
        return "-".to_owned();
    }
    let text = format!("{x:.5}");
    mark(
        text.trim_end_matches('0').trim_end_matches('.').to_owned(),
        display,
    )
}

/// A number the way it was typed: the shortest text that reads back as the same value, with the
/// decimal mark the person chose.
pub fn exact(x: f64) -> String {
    exact_with(x, display())
}

/// [`exact`] with the display given instead of read from the thread.
pub fn exact_with(x: f64, display: Display) -> String {
    mark(format!("{x}"), display)
}

/// As [`exact`], always with a decimal point: for comparing with what was typed.
pub fn exact_point(x: f64) -> String {
    format!("{x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMA: Display = Display {
        digits: 3,
        comma: true,
    };

    #[test]
    fn numbers_show_four_significant_digits() {
        for (x, text) in [
            (0.0, "0"),
            (3.971, "3.971"),
            (12.3456, "12.35"),
            (123.456, "123.5"),
            (1234.56, "1235"),
            (99999.0, "99999"),
            (100000.0, "1.000e5"),
            (0.0123456, "0.01235"),
            (0.000123456, "1.235e-4"),
            (-2.5, "-2.5"),
            (2.0, "2"),
            (0.5, "0.5"),
        ] {
            assert_eq!(number(x), text, "{x}");
        }
        assert_eq!(number(f64::NAN), "-");
        assert_eq!(number(f64::INFINITY), "-");
    }

    #[test]
    fn exact_powers_of_ten_land_in_their_own_decade() {
        for (x, text) in [
            (0.001, "0.001"),
            (0.01, "0.01"),
            (0.1, "0.1"),
            (1.0, "1"),
            (10.0, "10"),
            (1000.0, "1000"),
            (10000.0, "10000"),
            (1e-4, "1.000e-4"),
            (1e5, "1.000e5"),
        ] {
            assert_eq!(number(x), text, "{x}");
        }
    }

    #[test]
    fn the_explicit_display_gives_the_text_of_the_thread_setting() {
        let values = [
            0.0,
            3.971,
            -12.3456,
            1234.56,
            1e5,
            1.2e-7,
            0.99994,
            0.1 + 0.2,
        ];
        for display in [Display::default(), COMMA] {
            set_display(display);
            for x in values {
                assert_eq!(number(x), number_with(x, display), "{x}");
                assert_eq!(fit_quality(x), fit_quality_with(x, display), "{x}");
                assert_eq!(exact(x), exact_with(x, display), "{x}");
            }
        }
        set_display(Display::default());
    }

    #[test]
    fn the_explicit_display_sets_the_digits_and_the_decimal_mark() {
        assert_eq!(number_with(12.3456, COMMA), "12,3");
        assert_eq!(number_with(1.2e-7, COMMA), "1,20e-7");
        assert_eq!(number_with(12.3456, Display::default()), "12.35");
        assert_eq!(fit_quality_with(0.99994, COMMA), "0,99994");
        assert_eq!(exact_with(2.5, COMMA), "2,5");
        assert_eq!(exact_point(2.5), "2.5");
        // The explicit form does not read the thread, whatever it was set to.
        set_display(COMMA);
        assert_eq!(number_with(12.3456, Display::default()), "12.35");
        set_display(Display::default());
    }

    #[test]
    fn a_fit_quality_keeps_the_digits_that_matter() {
        assert_eq!(fit_quality(0.99994), "0.99994");
        assert_eq!(fit_quality(0.999999), "1");
        assert_eq!(fit_quality(0.9), "0.9");
        assert_eq!(fit_quality(f64::NAN), "-");
    }

    #[test]
    fn the_exact_text_reads_back() {
        for x in [0.1 + 0.2, 1.0 / 3.0, 1e-300, 123456789.123456] {
            assert_eq!(parse_number(&exact_point(x)), Some(x));
            assert_eq!(exact(x).parse::<f64>().ok(), Some(x));
        }
    }

    #[test]
    fn typed_numbers_take_a_comma_or_a_point() {
        assert_eq!(parse_number("3,25"), Some(3.25));
        assert_eq!(parse_number(" 3.25 "), Some(3.25));
        assert_eq!(parse_number("\u{2212}2"), Some(-2.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
    }
}
