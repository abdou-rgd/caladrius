#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code
//! SIM-DET-01: the simulation gives the same bits on 1 thread and on N threads (card T-159).
//!
//! The library has no thread (`crates/caladrius-fit/src/sim`): a cell, one (replicate, subject), is a pure
//! function of its address, and `Study::assemble` builds the tables in index order. The threads belong to the
//! caller, here an app: this test is the caller. It cuts the cells among N threads in two ways (strided and in
//! contiguous blocks), lets them finish in any order, assembles, and compares the canonical text with the
//! sequential run, bit for bit. The numbers of threads are chosen so that one divides the number of cells,
//! one does not, and one is larger than the number of cells (some threads have nothing to do).

use std::collections::BTreeMap;

use caladrius_fit::sim::{
    Cell, Design, ErrorKind, NegativeRule, OmegaShape, Seed, SimInput, Study, simulate,
};

fn input() -> SimInput {
    SimInput {
        model: "pk1.oral_1".to_owned(),
        theta: BTreeMap::from([
            ("cl".to_owned(), 5.0),
            ("v".to_owned(), 50.0),
            ("ka".to_owned(), 1.2),
        ]),
        random_effects: vec!["cl".to_owned(), "v".to_owned(), "ka".to_owned()],
        omega_shape: OmegaShape::Block,
        omega: vec![0.09, 0.03, 0.04, 0.01, 0.0, 0.16],
        error_model: ErrorKind::Combined,
        a: Some(0.2),
        b: Some(0.1),
        design: Design::Grid {
            times: vec![0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 12.0, 24.0],
            dose: 100.0,
            m: 7,
        },
        seed: Some(Seed(20_261_108)),
        replicates: 13,
        negative: NegativeRule::Zero,
        lloq: Some(0.05),
        quantiles: None,
    }
}

/// The cells computed by `threads` threads, each taking the indices `pick` gives it, put back in index order.
fn parallel(
    study: &Study,
    threads: usize,
    pick: impl Fn(usize, usize, usize) -> Vec<usize> + Sync,
) -> Vec<Cell> {
    let total = study.cell_count();
    let mut slots: Vec<Option<Cell>> = (0..total).map(|_| None).collect();
    let done: Vec<Vec<(usize, Cell)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let pick = &pick;
                scope.spawn(move || {
                    pick(t, threads, total)
                        .into_iter()
                        .map(|i| (i, study.cell_at(i).expect("cell")))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        // Joined in reverse: the order in which the threads finish does not matter.
        handles
            .into_iter()
            .rev()
            .map(|h| h.join().expect("thread"))
            .collect()
    });
    for (i, cell) in done.into_iter().flatten() {
        *slots.get_mut(i).expect("index") = Some(cell);
    }
    slots.into_iter().map(|c| c.expect("every cell")).collect()
}

#[test]
fn n_threads_give_the_bits_of_one_thread() {
    let input = input();
    let study = Study::new(&input).expect("study");
    let total = study.cell_count();
    assert_eq!(total, 7 * 13);
    let sequential = simulate(&input).expect("sequential");
    let want = sequential.canonical_text();
    // 7 divides 91, 3 and 4 do not, 128 is larger than the number of cells.
    for threads in [1, 3, 4, 7, 128] {
        let strided = parallel(&study, threads, |t, n, total| {
            (t..total).step_by(n).collect()
        });
        let blocks = parallel(&study, threads, |t, n, total| {
            let size = total.div_ceil(n);
            (t * size..((t + 1) * size).min(total)).collect()
        });
        for (name, cells) in [("strided", strided), ("blocks", blocks)] {
            let result = study.assemble(&cells).expect("assemble");
            assert_eq!(result, sequential, "{name} on {threads} threads");
            assert_eq!(
                result.canonical_text(),
                want,
                "{name} on {threads} threads: canonical text"
            );
        }
    }
}

#[test]
fn a_missing_or_misplaced_cell_is_refused_not_guessed() {
    let input = input();
    let study = Study::new(&input).expect("study");
    let mut cells: Vec<Cell> = (0..study.cell_count())
        .map(|i| study.cell_at(i).expect("cell"))
        .collect();
    cells.swap(0, 1);
    assert!(study.assemble(&cells).is_err(), "cells out of order");
    cells.swap(0, 1);
    cells.pop();
    assert!(study.assemble(&cells).is_err(), "a cell missing");
}
