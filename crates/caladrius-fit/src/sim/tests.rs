//! Unit tests of the simulator. The oracle (`tests/oracle_sim.rs`) holds the independent checks; these pin
//! the known-answer vectors, the layout and the properties of SIM-DET-01.

use std::collections::BTreeMap;

use super::rng::{LabelKey, Purpose, Stream, philox4x32_10, uniform};
use super::sha256::{hex, sha256};
use super::*;

#[test]
fn sha256_nist_vectors() {
    assert_eq!(
        hex(&sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex(&sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hex(&sha256(
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        )),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    // A message of 64 bytes and one of 55, 56 and 119 bytes: the padding edges.
    for (len, want) in [
        (
            55,
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
        ),
        (
            56,
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
        ),
        (
            64,
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
        ),
    ] {
        let text = vec![b'a'; len];
        assert_eq!(hex(&sha256(&text)), want, "length {len}");
    }
}

#[test]
fn philox_known_answer_vectors() {
    assert_eq!(
        philox4x32_10([0, 0, 0, 0], [0, 0]),
        [0x6627e8d5, 0xe169c58d, 0xbc57ac4c, 0x9b00dbd8]
    );
    assert_eq!(
        philox4x32_10([u32::MAX; 4], [u32::MAX; 2]),
        [0x408f276d, 0x41c83b0e, 0xa20bc7c6, 0x6d5451fd]
    );
    assert_eq!(
        philox4x32_10(
            [0x243f6a88, 0x85a308d3, 0x13198a2e, 0x03707344],
            [0xa4093822, 0x299f31d0]
        ),
        [0xd16cfe09, 0x94fdcceb, 0x5001e420, 0x24126ea1]
    );
}

#[test]
fn the_layout_of_a_stream() {
    // SX1: seed 20261108, label "1", purpose eta, replicate 0: counter (0, 1<<24, 6b86b273, ff34fce1).
    let key = LabelKey::of("1");
    assert_eq!((key.c2, key.c3), (0x6b86b273, 0xff34fce1));
    assert_eq!(key.hex(), "6b86b273ff34fce1");
    let stream = Stream::new(Seed(20261108), 0, Purpose::Eta, key);
    let block0 = philox4x32_10([0, 1 << 24, 0x6b86b273, 0xff34fce1], [0x013528f4, 0]);
    assert_eq!(block0, [0xbc2fc714, 0x04bc16cb, 0x0f6e941d, 0xb0671d12]);
    assert_eq!(
        stream.word(0),
        Some(u64::from(block0[0]) | (u64::from(block0[1]) << 32))
    );
    assert_eq!(
        stream.word(1),
        Some(u64::from(block0[2]) | (u64::from(block0[3]) << 32))
    );
    let block1 = philox4x32_10([1, 1 << 24, 0x6b86b273, 0xff34fce1], [0x013528f4, 0]);
    assert_eq!(
        stream.word(2),
        Some(u64::from(block1[0]) | (u64::from(block1[1]) << 32))
    );
    assert_eq!(stream.word(1 << 33), None);
}

#[test]
fn the_uniform_is_exact_and_inside_the_open_interval() {
    assert_eq!(uniform(0), 0.5 * f64::EPSILON);
    assert_eq!(uniform(u64::MAX), (4503599627370495.0 + 0.5) * f64::EPSILON);
    assert!(uniform(u64::MAX) < 1.0);
    assert_eq!(1.0 - uniform(u64::MAX), 0.5 * f64::EPSILON);
    // The low 12 bits of the word are discarded.
    assert_eq!(uniform(0xfff), uniform(0));
    assert!(uniform(0x1000) > uniform(0xfff));
}

#[test]
fn the_normals_of_a_stream_are_the_transform_of_its_words() {
    let stream = Stream::new(Seed(7), 3, Purpose::Epsilon, LabelKey::of("abc"));
    let z = stream.normals(5);
    for (n, zn) in z.iter().enumerate() {
        let word = stream.word(n as u64).expect("word");
        assert_eq!(*zn, super::normal::as241(uniform(word)), "word {n}");
    }
}

#[test]
fn the_transform_at_the_edges() {
    use super::normal::as241;
    let lowest = as241(0.5 * f64::EPSILON);
    assert!((lowest + 8.209_536_151_601_386).abs() < 1e-14, "{lowest}");
    assert_eq!(as241(0.5), 0.0);
    // Odd symmetry away from the centre, where 1 - u is exact.
    for u in [0.03125, 0.0625, 0.125, 0.375] {
        assert_eq!(as241(u), -as241(1.0 - u), "u = {u}");
    }
}

#[test]
fn cholesky_of_a_block_omega() {
    let omega = [0.09, 0.03, 0.04];
    let m = super::omega::matrix(OmegaShape::Block, &omega, 2).expect("matrix");
    assert_eq!(m, vec![0.09, 0.03, 0.03, 0.04]);
    let l = super::omega::cholesky(&m, 2).expect("factor");
    assert_eq!(l.first().copied(), Some(0.09_f64.sqrt()));
    assert_eq!(l.get(1).copied(), Some(0.0));
    assert_eq!(l.get(2).copied(), Some(0.03 / 0.09_f64.sqrt()));
    // eta = L z, accumulated left to right.
    let eta = super::omega::apply(&l, 2, &[1.5, -0.5]);
    let l21 = 0.03 / 0.09_f64.sqrt();
    assert_eq!(eta.first().copied(), Some(0.0 + 0.09_f64.sqrt() * 1.5));
    assert_eq!(
        eta.get(1).copied(),
        Some(0.0 + l21 * 1.5 + l.get(3).copied().unwrap_or(0.0) * -0.5)
    );
    // A pivot that is not positive is named.
    let bad = super::omega::matrix(OmegaShape::Block, &[1.0, 2.0, 1.0], 2).expect("matrix");
    assert_eq!(super::omega::cholesky(&bad, 2).map_err(|(i, _)| i), Err(1));
}

fn bolus(m: u64, replicates: u64) -> SimInput {
    SimInput {
        model: "pk1.iv_bolus".to_owned(),
        theta: BTreeMap::from([("cl".to_owned(), 5.0), ("v".to_owned(), 50.0)]),
        random_effects: vec!["cl".to_owned(), "v".to_owned()],
        omega_shape: OmegaShape::Block,
        omega: vec![0.09, 0.03, 0.04],
        error_model: ErrorKind::Proportional,
        a: None,
        b: Some(0.1),
        design: Design::Grid {
            times: vec![0.5, 1.0, 2.0, 4.0, 8.0, 12.0],
            dose: 100.0,
            m,
        },
        seed: Some(Seed(20261108)),
        replicates,
        negative: NegativeRule::Keep,
        lloq: None,
        quantiles: None,
    }
}

#[test]
fn a_cell_is_a_pure_function_of_its_address() {
    let study = Study::new(&bolus(5, 4)).expect("study");
    let a = study.cell(2, 3).expect("cell");
    let b = study.cell(2, 3).expect("cell");
    assert_eq!(a, b);
    // The order of evaluation does not matter, and the flat index is replicate * subjects + subject.
    let reversed: Vec<Cell> = (0..study.cell_count())
        .rev()
        .map(|i| study.cell_at(i).expect("cell"))
        .rev()
        .collect();
    let forward: Vec<Cell> = (0..study.cell_count())
        .map(|i| study.cell_at(i).expect("cell"))
        .collect();
    assert_eq!(reversed, forward);
    assert_eq!(study.cell_at(2 * 5 + 3).expect("cell"), a);
    let one = study.assemble(&forward).expect("result");
    assert_eq!(one, study.simulate().expect("result"));
}

#[test]
fn a_subject_does_not_depend_on_the_others_or_on_the_replicate_count() {
    let big = Study::new(&bolus(8, 6)).expect("study");
    let small = Study::new(&bolus(3, 2)).expect("study");
    // Subjects "1".."3" are the same subjects in both; replicates 0 and 1 are the same in both.
    for rep in 0..2 {
        for subject in 0..3 {
            assert_eq!(
                big.cell(rep, subject).expect("cell"),
                small.cell(rep, subject).expect("cell")
            );
        }
    }
}

#[test]
fn the_pinned_canonical_text_of_a_small_study() {
    // The canonical text is what SIM-DET-02 hashes; this pins its SHA-256 for the design of SX2 (three
    // subjects, times 0.5 to 12, pk1.iv_bolus, proportional 10 %, seed 20261108, 20 replicates). Any change
    // of the generator, of the transform, of the model code or of the order of operations moves it, and the
    // change must be deliberate (SIM-DET-03).
    let result = simulate(&bolus(3, 20)).expect("result");
    let text = result.canonical_text();
    assert_eq!(
        text,
        simulate(&bolus(3, 20)).expect("result").canonical_text()
    );
    assert_eq!(hex(&sha256(text.as_bytes())), PINNED_SX2_20);
}

/// SHA-256 of the canonical text of `bolus(3, 20)`; see the test above.
const PINNED_SX2_20: &str = "c9b3ce1437b0e3a07a0a1b3cf647d9a30f82f0ce4ff066544b45b7d92c95a983";
