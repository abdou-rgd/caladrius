//! The generator: Philox4x32-10, the address of a stream, the word and the uniform (SIM-RNG-01 to -04).
//!
//! Integers only: 32-bit words, 64-bit products, additions modulo 2^32. The one conversion to a double is
//! the exact `m as f64` of a 52-bit integer, then a multiplication by 2^-52 (`f64::EPSILON`, never `powi`,
//! never a fused multiply-add). The same bits on every target, WebAssembly included.

use super::normal::as241;
use super::sha256::{hex, sha256};

const M0: u32 = 0xD251_1F53;
const M1: u32 = 0xCD9E_8D57;
const W0: u32 = 0x9E37_79B9;
const W1: u32 = 0xBB67_AE85;

/// The 64-bit product of two 32-bit words, split as (high, low).
fn mulhilo(a: u32, b: u32) -> (u32, u32) {
    let product = u64::from(a) * u64::from(b);
    ((product >> 32) as u32, product as u32)
}

/// Philox4x32 with 10 rounds: a counter (c0, c1, c2, c3) and a key (k0, k1) to four output words
/// (SIM-RNG-01). Round r uses the key increased by r times (W0, W1) modulo 2^32.
pub fn philox4x32_10(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let [mut c0, mut c1, mut c2, mut c3] = counter;
    let [mut k0, mut k1] = key;
    for _ in 0..10 {
        let (hi0, lo0) = mulhilo(M0, c0);
        let (hi1, lo1) = mulhilo(M1, c2);
        [c0, c1, c2, c3] = [hi1 ^ c1 ^ k0, lo1, hi0 ^ c3 ^ k1, lo0];
        k0 = k0.wrapping_add(W0);
        k1 = k1.wrapping_add(W1);
    }
    [c0, c1, c2, c3]
}

/// The seed: an integer 0 <= s < 2^64 (SIM-RNG-02). It is held, written and sealed as a decimal string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Seed(pub u64);

impl Seed {
    /// The key (k0, k1) = (s mod 2^32, floor(s / 2^32)).
    pub(super) fn key(self) -> [u32; 2] {
        [self.0 as u32, (self.0 >> 32) as u32]
    }
}

/// The 64-bit stream key of a subject label: the first 8 bytes of the SHA-256 of its UTF-8 bytes, read as
/// a big-endian integer, split as (c2, c3) (SIM-RNG-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LabelKey {
    pub(super) c2: u32,
    pub(super) c3: u32,
}

impl LabelKey {
    pub fn of(label: &str) -> LabelKey {
        let digest = sha256(label.as_bytes());
        let word = |at: usize| {
            digest
                .get(at..at + 4)
                .map_or(0, |b| b.iter().fold(0u32, |a, &x| (a << 8) | u32::from(x)))
        };
        LabelKey {
            c2: word(0),
            c3: word(4),
        }
    }

    /// The 16 hexadecimal digits of the key (for the message of `label_key_collision`).
    pub fn hex(self) -> String {
        let mut bytes = self.c2.to_be_bytes().to_vec();
        bytes.extend_from_slice(&self.c3.to_be_bytes());
        hex(&bytes)
    }
}

/// What a stream draws (SIM-RNG-03): the purpose is the top 8 bits of the counter word c1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// The random effects (p = 1).
    Eta,
    /// The residual errors (p = 2).
    Epsilon,
}

impl Purpose {
    fn number(self) -> u32 {
        match self {
            Purpose::Eta => 1,
            Purpose::Epsilon => 2,
        }
    }
}

/// The stream of (seed, replicate, purpose, label): a counter-based sequence of 64-bit words with no state.
#[derive(Debug, Clone, Copy)]
pub struct Stream {
    key: [u32; 2],
    c1: u32,
    label: LabelKey,
}

impl Stream {
    pub fn new(seed: Seed, replicate: u32, purpose: Purpose, label: LabelKey) -> Stream {
        Stream {
            key: seed.key(),
            c1: (purpose.number() << 24) | replicate,
            label,
        }
    }

    fn block(&self, index: u32) -> [u32; 4] {
        philox4x32_10([index, self.c1, self.label.c2, self.label.c3], self.key)
    }

    /// Word `n`: block n / 2, half n mod 2, x = r_{2h} + 2^32 r_{2h+1}. `None` past the 2^33 words of a stream.
    pub fn word(&self, n: u64) -> Option<u64> {
        let index = u32::try_from(n / 2).ok()?;
        Some(half(self.block(index), n % 2 == 1))
    }

    /// The first `count` normals z_0, z_1, ... of the stream (one per word, SIM-RNG-05, -06).
    pub fn normals(&self, count: usize) -> Vec<f64> {
        let mut out = Vec::with_capacity(count);
        let mut index = 0u32;
        while out.len() < count {
            let r = self.block(index);
            out.push(as241(uniform(half(r, false))));
            if out.len() < count {
                out.push(as241(uniform(half(r, true))));
            }
            index = index.wrapping_add(1);
        }
        out
    }
}

/// Half `second` of a block as a 64-bit word.
fn half(r: [u32; 4], second: bool) -> u64 {
    let [r0, r1, r2, r3] = r;
    let (low, high) = if second { (r2, r3) } else { (r0, r1) };
    u64::from(low) | (u64::from(high) << 32)
}

/// The uniform of a word (SIM-RNG-04): u = (m + 0.5) 2^-52 with m the top 52 bits. Exact in double precision,
/// in [2^-53, 1 - 2^-53], never 0 or 1.
pub fn uniform(word: u64) -> f64 {
    ((word >> 12) as f64 + 0.5) * f64::EPSILON
}
