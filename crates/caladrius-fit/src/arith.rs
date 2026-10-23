//! Powers by multiplication, the same on every machine.
//!
//! `f64::powi` is lowered by the compiler: a few products in an optimized build when the exponent is
//! known, otherwise a call into the runtime (compiler-rt's `__powidf2`, or the C runtime's `pow` on
//! windows-msvc, which has no such call), and the last bit is not promised to agree between them. A
//! product is an IEEE operation and rounds the same everywhere, so the fit uses these two (T-111;
//! `clippy.toml` bans `powi`).

/// `x * x`.
pub(crate) fn sq(x: f64) -> f64 {
    x * x
}

/// `x` to the integer power `n`, by repeated squaring (the products of `__powidf2` and of LLVM's
/// expansion of a constant exponent); a negative power is the reciprocal of the positive one.
pub(crate) fn powi(x: f64, n: i32) -> f64 {
    let (mut base, mut rest, mut power) = (x, n.unsigned_abs(), 1.0);
    while rest > 0 {
        if rest & 1 == 1 {
            power *= base;
        }
        rest >>= 1;
        if rest > 0 {
            base *= base;
        }
    }
    if n < 0 { 1.0 / power } else { power }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_powers_are_the_plain_products() {
        let x = 1.1_f64;
        assert_eq!(powi(x, 0), 1.0);
        assert_eq!(powi(x, 1), x);
        assert_eq!(powi(x, 2), x * x);
        assert_eq!(sq(x), x * x);
        assert_eq!(powi(x, 3), x * (x * x));
        assert_eq!(powi(x, 7), x * (x * x) * ((x * x) * (x * x)));
    }

    #[test]
    fn a_negative_power_is_the_reciprocal() {
        assert_eq!(powi(2.0, -2), 0.25);
        assert_eq!(powi(10.0, -3), 1e-3);
        assert_eq!(powi(0.0, 0), 1.0);
        assert!(powi(0.0, -1).is_infinite());
        assert_eq!(powi(-2.0, 3), -8.0);
    }

    #[test]
    fn the_most_negative_exponent_is_handled() {
        assert_eq!(powi(1.0, i32::MIN), 1.0);
        assert_eq!(powi(1.0, i32::MAX), 1.0);
    }
}
