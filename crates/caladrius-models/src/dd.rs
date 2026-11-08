//! Double-double arithmetic for the few places where a difference of nearly equal quantities has to
//! keep its digits (the discriminant of the reverse conversion, MOD-3C-04).
//!
//! A value is the unevaluated sum `hi + lo` with |lo| at most half a unit in the last place of
//! `hi`, so that it carries about 106 bits. Everything here is FMA-free: a product is split by
//! Veltkamp's method and summed by Dekker's, using only `+`, `-` and `*`, each rounded to nearest
//! (the evaluation order is written in the code and is the same on every platform, the wasm build
//! included). A result is not finite when a factor of a product exceeds MAX / (2^27 + 1), about
//! 1.34e300 (the split multiplies by 2^27 + 1); the caller checks for that.

/// The exact sum of two doubles: (s, e) with s = fl(a + b) and s + e = a + b (Knuth).
pub(crate) fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}

/// As `two_sum` when |a| >= |b| (or a = 0): three operations (Dekker).
fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    (s, b - (s - a))
}

/// Veltkamp's split: a = hi + lo with 26 bits in each part.
fn split(a: f64) -> (f64, f64) {
    let c = 134_217_729.0 * a;
    let hi = c - (c - a);
    (hi, a - hi)
}

/// The exact product of two doubles: (p, e) with p = fl(a·b) and p + e = a·b (Dekker), barring
/// overflow and underflow of the partial products.
pub(crate) fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    let (a_hi, a_lo) = split(a);
    let (b_hi, b_lo) = split(b);
    let e = ((a_hi * b_hi - p) + a_hi * b_lo + a_lo * b_hi) + a_lo * b_lo;
    (p, e)
}

/// A double-double number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Dd {
    pub hi: f64,
    pub lo: f64,
}

impl Dd {
    pub fn new(x: f64) -> Dd {
        Dd { hi: x, lo: 0.0 }
    }

    /// The exact sum a + b.
    pub fn sum(a: f64, b: f64) -> Dd {
        let (hi, lo) = two_sum(a, b);
        Dd { hi, lo }
    }

    /// The exact product a·b.
    pub fn product(a: f64, b: f64) -> Dd {
        let (hi, lo) = two_prod(a, b);
        Dd { hi, lo }
    }

    pub fn add(self, o: Dd) -> Dd {
        let (s, e) = two_sum(self.hi, o.hi);
        let (t, f) = two_sum(self.lo, o.lo);
        let (s, e) = fast_two_sum(s, e + t);
        let (hi, lo) = fast_two_sum(s, e + f);
        Dd { hi, lo }
    }

    pub fn add_f(self, x: f64) -> Dd {
        self.add(Dd::new(x))
    }

    pub fn neg(self) -> Dd {
        Dd {
            hi: -self.hi,
            lo: -self.lo,
        }
    }

    pub fn sub(self, o: Dd) -> Dd {
        self.add(o.neg())
    }

    pub fn mul(self, o: Dd) -> Dd {
        let (p, e) = two_prod(self.hi, o.hi);
        let (hi, lo) = fast_two_sum(p, e + (self.hi * o.lo + self.lo * o.hi));
        Dd { hi, lo }
    }

    /// Four times the value, exactly (a power of two), without the split of a product.
    pub fn times_four(self) -> Dd {
        Dd {
            hi: 4.0 * self.hi,
            lo: 4.0 * self.lo,
        }
    }

    pub fn mul_f(self, x: f64) -> Dd {
        let (p, e) = two_prod(self.hi, x);
        let (hi, lo) = fast_two_sum(p, e + self.lo * x);
        Dd { hi, lo }
    }

    /// The value rounded to a double.
    pub fn value(self) -> f64 {
        self.hi + self.lo
    }

    pub fn is_finite(self) -> bool {
        self.hi.is_finite() && self.lo.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_sum_and_two_prod_are_exact() {
        let (s, e) = two_sum(1.0, 1e-20);
        assert_eq!((s, e), (1.0, 1e-20));
        // (1 + 2^-30)^2 = 1 + 2^-29 + 2^-60: the last term is the error of the product
        let x = 1.0 + libm::pow(2.0, -30.0);
        let (p, e) = two_prod(x, x);
        assert_eq!(p, 1.0 + libm::pow(2.0, -29.0));
        assert_eq!(e, libm::pow(2.0, -60.0));
    }

    /// n1² − 4·n0 for two nearly equal roots keeps its digits where plain doubles lose them.
    #[test]
    fn a_discriminant_of_nearly_equal_roots_keeps_its_digits() {
        let (k21, k31) = (1.0 + libm::pow(2.0, -20.0), 1.0);
        let n1 = Dd::sum(k21, k31);
        let n0 = Dd::product(k21, k31);
        let delta = n1.mul(n1).sub(n0.mul_f(4.0));
        // (k21 − k31)² = 2^-40 exactly
        assert_eq!(delta.value(), libm::pow(2.0, -40.0));
    }

    #[test]
    fn the_sum_of_three_terms_is_exact() {
        let t = Dd::sum(1e16, 1.0).add_f(1.0);
        assert_eq!(t.hi, 1e16 + 2.0);
        assert_eq!(t.value(), 1e16 + 2.0);
        let big = Dd::sum(1e300, 1e-300);
        assert_eq!(big.lo, 1e-300);
    }
}
