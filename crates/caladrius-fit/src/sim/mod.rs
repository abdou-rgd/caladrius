//! The population simulator: reproducible draws from a population model (`specs/simulation.md`, rule ids
//! `SIM-…` are cited in the code and the tests).
//!
//! Given theta, Omega, an error model, a design and a seed it returns simulated random effects, individual
//! parameters, predictions and observations, the same bits on 1 thread, on N threads and in WebAssembly:
//!
//! - [`rng`]: Philox4x32-10 on integers, the address of a stream (seed, replicate, purpose, label) and the
//!   uniform of a word (SIM-RNG-01 to -04);
//! - [`normal`]: one normal per uniform, Wichura's AS 241 (SIM-RNG-05);
//! - [`omega`]: Omega and its Cholesky factor, eta = L z (SIM-RNG-07);
//! - [`Study`]: the checked call. [`Study::cell`] is a pure function of one (replicate, subject);
//!   [`Study::assemble`] builds the tables in index order, so a caller may compute the cells on any number of
//!   threads in any order (SIM-DET-01). [`Study::cell_with_draws`] is the test entry point that takes the
//!   standard normals z^eta and z^eps and bypasses the generator (SIM-ORA-03).
//!
//! The library has no thread, no clock, no file and no global state. Every transcendental function comes
//! from `libm` (golden rule 10).

mod error;
mod input;
mod normal;
mod omega;
mod result;
mod rng;
mod sha256;
mod study;

#[cfg(test)]
mod tests;

pub use error::{SimError, SimErrorCode};
pub use input::{Design, DesignSubject, ErrorKind, NegativeRule, RowKind, SimInput, TableRow};
pub use normal::as241;
pub use omega::OmegaShape;
pub use result::{
    Counts, DatasetRow, ObservationRow, SimResult, SubjectRow, SummaryRow, UsedOptions, quantile7,
};
pub use rng::{LabelKey, Purpose, Seed, Stream, philox4x32_10, uniform};
pub use study::{Cell, DEFAULT_QUANTILES, MAX_REPLICATES, RNG_VERSION, Study};

/// Checks the call and runs it on the calling thread.
pub fn simulate(input: &SimInput) -> Result<SimResult, SimError> {
    Study::new(input)?.simulate()
}
