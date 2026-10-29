# Validation report

This report states what Caladrius verifies, against what, at which tolerance, and what it does not
verify. It is written from the repository's own files; every number cites the file that owns it. It
reproduces no text, and no number, from the reference commercial software, and nothing from
`private/`.

## 1. Scope and position

Caladrius computes, in v1:

- **Non-compartmental analysis (NCA)**: AUC by the linear, lin-up/log-down and mixed rules, the
  terminal-phase rate constant λz, half-life, extrapolation to infinity, AUMC, MRT, CL/F, Vz/F and
  Vss, with units carried as data (`AGENTS.md` section 4; the coverage table of section 3.1).
- **Individual compartmental models**: six one-compartment models (IV bolus, IV infusion,
  first-order oral with and without lag, zero-order oral with and without lag) and six
  two-compartment models in three parameterisations, with their closed-form curves, AUC, AUMC,
  secondary parameters and partial derivatives (the coverage table of section 3.1; `specs/models.md`).
- **Weighted least-squares fitting**: the weightings `uniform`, `1/y`, `1/y²`, `1/ŷ`, `1/ŷ²`, with
  final parameters (estimate, standard error, CV%, univariate and planar confidence intervals),
  diagnostics, correlation and covariance matrices, eigenvalues, condition numbers, partial
  derivatives, predicted data and the minimization trace (`AGENTS.md` section 4).
- **Multiple dosing and steady state** for the twelve one- and two-compartment models: dose
  schedules and regular regimens by superposition, and the steady-state profile and its derived
  quantities over one interval (`specs/models.md` section 12, MOD-MD-01 to 12).

It deliberately does **not** do, in v1: population modelling (NLME), bioequivalence, IVIVC
(`AGENTS.md` section 1, "Out of scope"), several subjects in one model, and user-written models
(two compartments landed after v0.1.0; user-written models are still not implemented — the "Not
validated" column of the coverage table in section 3.1).

Position: **verified individual PK**. The goal is a teaching-grade, reproducible calculation layer
whose conformance table an agent (Apothicaire) or a person can re-run, not a claim of equality with
any commercial program. "Validated" here has the narrow meaning given in `README.md`, section
"What is verified, and what is not": a number produced by Caladrius equals a number produced by an
independent computation, within a stated tolerance, on a stated case.

**No number in the repository comes from the reference commercial software.** Every expected value
is produced by PKNCA, by 256-bit evaluations of closed forms cross-checked with a matrix exponential
and an ODE solver, or by R `nls`/`nlsLM`, from public or synthetic data, with a versioned script
(`README.md`, "Where the numbers come from"). The reference software was compared once, privately,
on counts only; that comparison is not published and is not part of any versioned test.

## 2. Methods of verification

### 2.1 The three oracle sources

`AGENTS.md` section 5 orders the sources for versioned tests:

1. **Public, in `oracle/`**: the R datasets Theoph and Indometh with PKNCA results from a versioned
   R script, hand-made edge profiles, and exact closed-form model values (`oracle/README.md`).
2. **Private, in `private/`**: coursework results and exports from the reference software. The
   matching tests sit behind the Cargo feature `private-oracle` and are skipped when the folder is
   empty. No file, number or subject name from `private/` may appear in a versioned file, a commit
   message or the board (`AGENTS.md` section 5).
3. **Independent computation**: where no external reference exists, a second, naive implementation
   in `caladrius-testkit`, never the same function tested against itself (`AGENTS.md` section 5).
   Two instances: every fit statistic is recomputed naively and the optimality condition
   `Jᵀ W r = 0` is re-checked in `crates/caladrius-testkit/tests/step3_consistency.rs`
   (`docs/history.md`, "One oracle was wrong once, and was corrected"), and the parameter relations
   of the NCA edge cases are checked in `crates/caladrius-testkit/tests/edge_consistency.rs`
   (`specs/differences.md`, D-01 cross-check).

### 2.2 The R scripts and the versions they record

Each generated case writes a `*.options.json` next to its expected values, with the exact versions
of the tools that produced it. Read from those files (a search over
`oracle/expected/**/*.options.json` returns one consistent set):

| Script | Produces | Versions recorded in its options files | Example source |
|---|---|---|---|
| `oracle/scripts/nca_pknca.R` | NCA expected values, 20 cases | R 4.5.2 (2025-10-31 ucrt), PKNCA 0.12.1, jsonlite 2.0.0 | `oracle/expected/edge_blq_default.options.json` |
| `oracle/scripts/fit_wls.R` | reference weighted least-squares fits, 30 cases | R 4.5.2 (2025-10-31 ucrt), minpack.lm 1.2.4, jsonlite 2.0.0 | `oracle/expected/fit/fit_indometh_inv_y.options.json` |
| `oracle/scripts/models_closed_form.R` | one-compartment model values, 21 cases | R 4.5.2 (2025-10-31 ucrt), Rmpfr 1.1.3, expm 1.0.1, deSolve 1.42, jsonlite 2.0.0 | `oracle/expected/models/model_iv_bolus_b.options.json` |
| `oracle/scripts/models_2c_closed_form.R` (sources `models_2c_derivatives.R`) | two-compartment values, partial derivatives and refused inputs, 172 cases | R 4.5.2 (2025-10-31 ucrt), Rmpfr 1.1.3, expm 1.0.1, deSolve 1.42, gmp 0.7.5.1 | `oracle/expected/models/pk2/model_pk2_deriv_iv_bolus_base.options.json` |
| `oracle/scripts/models_md.R` | multiple-dosing and steady-state values and refused inputs, 247 cases | R 4.5.2 (2025-10-31 ucrt), Rmpfr 1.1.3, deSolve 1.42, gmp 0.7.5.1 | `oracle/expected/models/md/model_md_errors.options.json` |

`oracle/scripts/models_2c_derivatives.R` is sourced by `models_2c_closed_form.R` and is not run on
its own (its header). The fitting references use R `stats::nls` (Gauss-Newton) and
`minpack.lm::nlsLM` on the same data and require the two to agree; the model oracles evaluate the
closed forms in 256-bit arithmetic and cross-check every case against `expm` and `deSolve`
(`oracle/README.md`; `oracle/scripts/models_closed_form.R`).

### 2.3 Tolerances

Tolerances are defined once, in `caladrius-testkit`, and never loosened inside a test
(`AGENTS.md` section 5; `crates/caladrius-testkit/src/tolerance.rs`):

| Constant | Meaning | Value |
|---|---|---|
| `NCA_VS_PKNCA` | NCA parameters against PKNCA | relative 1e-6 (absolute 1e-12 at an expected zero) |
| `FIT_PARAMETERS` | fitted parameters and statistics | relative 1e-4 (absolute 1e-12 at an expected zero) |
| `FIT_PARAMETERS_SMALL_CASE` | the five-point worked example F2 | relative 1e-6, stricter than the contract |
| `FIT_WEIGHTED_SS` | weighted sum of squares of a fit | relative 1e-6 |
| `MODEL_VALUES` | closed-form model values | relative 1e-12; an expected zero is exactly zero |
| `MODEL_DERIVATIVES` | partial derivatives of the two-compartment models | relative 1e-12; an expected zero is exactly zero |
| `Tolerance::displayed(n)` | against a reference export | equality at the `n` decimals shown |

**Measured margins, not assumed ones.** `specs/fit.md` section 10 records the worst gaps between the
engine (closed-form derivatives, relative decrease 1e-10) and the reference fits: **7.4e-6** on an
estimate (`fit_indometh_inv_y`, subject 2, k), **9.2e-6** on a standard error (`fit_theoph_inv_y`,
subject 9, ka) and **3.8e-7** on the weighted sum of squares (`fit_indometh_inv_yhat2`, subject 4).
The contract tolerance is therefore about 14 times the worst estimate gap, 11 times the worst
standard-error gap, and only 2.6 times the worst weighted-sum-of-squares gap, the last on a
reweighted scheme where the objective is not flat at the fixed point (`specs/fit.md` section 10,
OF-10). Reason for the two levels: NCA is closed arithmetic on the data, so two correct
implementations differ by rounding only; a fit is a search, and two optimizers stop at different
points (`AGENTS.md` section 5; `specs/fit.md` section 10). The five-point example is small enough to
be worked on paper and is held to 1e-6 on every parameter and statistic; that shows 1e-4 is a
margin of caution, not that the fit statistics were verified by hand (`specs/fit.md` section 10).

### 2.4 The gates

- **Conformance.** `cargo xtask conformance` runs the engine on every case of `oracle/expected/` and
  regenerates `docs/conformance.md`; what it counts, and when it fails, is described in section 2.5.
  The file ends with one floor line per parameter per case, of the form
  `<!-- floor edge_blq_default cmax 6 6 0 -->` (validated, expected, documented; `docs/conformance.md`);
  **floors can only go up** (`docs/conformance.md`, header).
- **Deny lints.** Every crate root begins with
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unreachable)]`;
  `cargo xtask lint` fails a crate root that omits one of the five (`AGENTS.md`, golden rule 6;
  `xtask/src/lint/header.rs`). Indexing on slices and maps (`clippy::indexing_slicing`) is denied by
  the linter in the four numerical crates (`caladrius-nca`, `-models`, `-fit`, `-project`, library
  targets, test code excepted) by a clippy step of `cargo xtask lint` (`xtask/src/lint/indexing.rs`),
  and checked in review in the other crates.
- **Review.** Every change in `caladrius-nca`, `caladrius-models` or `caladrius-fit` goes through a
  reviewer that did not write it, and whoever writes an oracle test does not write the code under
  test (`AGENTS.md` section 8).
- **CI.** `.github/workflows/ci.yml` runs `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `cargo xtask layers`, `cargo xtask lint`, `cargo xtask review`, `cargo xtask wasm`, and
  `cargo xtask conformance` followed by `git diff --exit-code docs/conformance.md`, so a stale
  conformance table fails the build.
- **Same numbers on every machine** (task T-111). The platform-dependent float functions of the
  standard library (`exp`, `ln`, `powf`, `powi`, `sin`...) are banned in the numerical crates, the
  engine and `xtask` by `clippy.toml`; they call the pure Rust `libm`, pinned to one version, which
  rounds the same on every target (`AGENTS.md`, golden rule 10). `cargo xtask digest` runs a fixed
  workload (the command examples, the NCA and fit oracle cases, Theoph and Indometh under every
  weighting, model simulations, refusals, plots, the replay of saved projects) through `Engine::execute` and prints the SHA-256 of
  the canonical text of the answers, last bits included. The `digest` jobs of CI run it on Ubuntu,
  Windows, macOS and in WebAssembly under Node, and `digest-compare` fails when the four differ.
  What it covers: the commands of the workload on those four platforms. What it does not: other
  CPUs of the same families, other Node versions, a command or a function the workload does not
  reach, the sign of a zero (`min`, `max` and `clamp` return either zero for +0 and -0, so an input
  of -0 can print as 0 on another CPU), and values above 4 294 967 295 for the options held as
  `usize`, which wasm32 refuses with another message.
- **A seal per result, and replay** (task T-112). Every command except `project.load` and `project.replay`
  has a seal: the SHA-256 of the command, its parameters and its answer, written as canonical text
  (`crates/caladrius-engine/src/seal/`). The version of the engine, the git commit and the SHA-256 of the
  data the command read sit beside the seal, not in it, so a later version that computes the same bits
  gives the same seal. `project.replay` runs the history of a saved project again from an empty project,
  checks every step against its seal and the stored project against the replayed one, and ends in one of
  five verdicts (verified, diverged, incomplete, unsealed, empty). The digest workload includes replays
  of project files, so the four platforms of CI also check that a seal made on one is the seal another
  computes. What it does not cover: an import above 64 KiB (the history keeps its size and a short
  fingerprint, not its text, so the step cannot be run again), who made a file and when (a seal is a
  fingerprint, not a signature), and what the previous bullet lists. `docs/reproducibility.md` says all of
  it in plain words.
- **`cargo xtask lint`** enforces the contract's mechanical rules: forbidden names, `private/`
  leaks, deny-lint headers, tolerances defined once, board hygiene, and the indexing step above (`AGENTS.md`
  section 11; `xtask/src/lint/mod.rs`).
- **`cargo xtask review`** (read-only) confronts the figures this report and the specs state with the
  files that own them: the conformance total and floors, the model ids, the coverage counts of
  section 3.1, the `- Status:` counts of section 5, and the drift since the last tag
  (`xtask/src/review.rs`, rule list). It also fails when `README.md` states a measured figure (a count,
  a ratio, a percentage, a thousands-separated number: `review-readme-figures`), so that every figure
  has one owner file.

### 2.5 The conformance run

`cargo xtask conformance` runs `caladrius-nca` on every case of `oracle/expected/`, `caladrius-models` on every case of `oracle/expected/models/` (relative error 1e-12) and `caladrius-fit` on every case of `oracle/expected/fit/` (1e-4 on estimates and statistics, 1e-6 on the residual sum of squares), and regenerates `docs/conformance.md` (never edit it by hand). A value counts as validated only if the engine computes the parameter and it is within the tolerance of the expected value. The file ends with one floor line per parameter per case (validated and expected rows). The task fails and leaves the file unchanged if a parameter validates fewer values, loses expected rows, gains unvalidated rows or disappears, if a floor line is malformed, or if a non-empty previous file holds no floor. Floors can only go up. The file is written to a temporary file then renamed.

## 3. Coverage

`docs/conformance.md` is the file that counts; the table below gives its current counts by analysis,
and section 3.1 holds the coverage table the README carried until 2026-10-29. **Overall: 48955 of
48955 values validated (100.0 %); 16 further values are documented differences**
(`docs/conformance.md`). The total is the sum of the three sections: 3744 + 34711 + 10500 = 48955
(`docs/conformance.md`).

| Analysis | Validated against | Tolerance | Cases and values (`docs/conformance.md`) | Not validated |
|---|---|---|---|---|
| NCA: extravascular, IV bolus, IV infusion; linear and lin-up/log-down areas | PKNCA 0.12.1 on R 4.5.2 (`oracle/scripts/nca_pknca.R`); Theoph and Indometh, 14 edge-case runs on hand-made profiles and 2 runs on 4 hand-made profiles built to separate the terminal-phase rule; plus naive second implementations in `caladrius-testkit` | 1e-6 | 20 cases, 3744 values; 16 more values are the documented difference D-01 | Steady state and multiple doses, partial AUC, urine, sparse sampling, weighted λz (`specs/nca.md` section 12); every reference-software convention |
| One-compartment model curves, AUC and secondary parameters (six models) | Exact closed forms in 256-bit arithmetic (`Rmpfr` 1.1.3), cross-checked against `expm` 1.0.1 and `deSolve` 1.42; textbook double-precision forms in `caladrius-testkit` (`oracle/scripts/models_closed_form.R`) | 1e-12 | 21 cases, 886 values | Any other program's output; user-written models. The closed forms are the project's own derivation from textbooks, checked three ways |
| Two-compartment models: curves, AUC, AUMC, partial derivatives and refused inputs (six models, three parameterisations) | Explicit sums of exponentials in 256-bit arithmetic and central differences of them, cross-checked on every case against `expm` and `deSolve` (`oracle/scripts/models_2c_closed_form.R`) | 1e-12 | 172 cases, 16906 values: 107 value cases, 64 derivative cases (entries with condition number above 1e3 omitted), plus `model_pk2_errors` (80 not-available rows the engine must refuse) | Any other program's output; the two-compartment fit (no reference fit exists); the 393 ill-conditioned derivative entries (OM-14) |
| Multiple dosing and steady state (twelve models): schedules, regular regimens of n doses, steady-state profiles and derived quantities, refused inputs | Plain sums of single-dose closed forms in 256-bit arithmetic (`Rmpfr`), the geometric-series forms of the specification implemented a second time and checked at 2^-60, and every case cross-checked at 1e-8 against `deSolve` with dosing events and the periodic solution of the linear system (`oracle/scripts/models_md.R`) | 1e-12 | 247 cases, 16919 values, including `model_md_errors` (inputs the engine must refuse) | Any other program's output; the engine's own limits (non-integer `n_doses`, the 1e7 evaluation cap) have unit tests only (`specs/models.md`, `assumed` rules) |
| Fits, IV bolus and first-order oral: estimates and every statistic | R `stats::nls` and `minpack.lm::nlsLM` 1.2.4 on R 4.5.2 (`oracle/scripts/fit_wls.R`); statistics from an independent R implementation of `specs/fit.md`, standard errors and S checked against `summary.nls` | 1e-4 parameters and statistics, 1e-6 residual sum of squares | 15 cases, five weightings each: 95 fits, 5340 values (with the convergence status) | Bounds, generated initial estimates and quality flags have unit tests only |
| Fits, IV infusion, zero-order input, first-order with lag | `nlsLM` checked against `nls`, same R and package versions, on synthetic profiles written from the closed forms (fixed seed, 8 % log-normal noise; the input duration is a fixed parameter) | 1e-4 and 1e-6 as above | 15 cases, 90 fits, 5160 values (with status) | Real (non-synthetic) profiles for these models; a published worked example with a page reference for any fit |
| The reference commercial software | One private coursework exercise, kept out of the repository; only counts recorded, in a local run | equality at the displayed precision | counts only, not published | Everything else: its fitting defaults and its definitions of the fit statistics are `assumed` from observation and untested against its output |

The fit sections total 30 cases and 10500 values (5340 + 5160; `docs/conformance.md`). The
model sections total 440 cases and 34711 values (886 + 16906 + 16919; `docs/conformance.md`).

### 3.1 Coverage by model family

The README states no figure; this is the coverage table it carried until 2026-10-29, moved here
unchanged. `cargo xtask review` checks the cases and values of its one- and two-compartment model
rows against `docs/conformance.md` (`review-claims-checked`).

| What | Checked against | Tolerance (relative) | Cases and values | Not validated |
|---|---|---|---|---|
| NCA parameters: extravascular, IV bolus, IV infusion; linear and lin-up/log-down areas | `PKNCA` 0.12.1 on R 4.5.2 (`oracle/scripts/nca_pknca.R`): Theoph and Indometh (the R datasets, two area rules each), 14 edge-case runs on hand-made profiles (values below the limit, missing, negative, lag, infusion, IV C0) and 2 runs on 4 hand-made profiles built to separate the readings of the terminal-phase rule; plus naive second implementations in `caladrius-testkit` | 1e-6 | 20 cases, 3744 values; 16 more values are a documented difference (D-01), counted in neither column | Steady state and multiple doses, partial AUC, urine, sparse sampling, weighted lambda-z (`specs/nca.md` section 12, "not specified"); the combinations of open item O-17; every reference-software convention (last row) |
| Model curves, AUC and secondary parameters: six one-compartment models (IV bolus, IV infusion, first-order oral with and without lag, zero-order oral with and without lag) | Exact closed forms evaluated in 256-bit arithmetic (`Rmpfr` 1.1.3), cross-checked against a matrix exponential (`expm` 1.0.1) and an ODE solver (`deSolve` 1.42); textbook double-precision forms in `caladrius-testkit` | 1e-12 | 145 cases, 9405 values: 21 single-dose cases (886 values) and, since tasks T-047 and T-049, 124 multiple-dose and steady-state cases (schedules of 1 to 40 doses, regular regimens, steady state for several intervals) with their refusals | Any other program's output; user-written models (not implemented). The closed forms are the project's own derivation from textbooks, checked three ways, not an external reference |
| Model curves, AUC, AUMC, partial derivatives and refused inputs: six two-compartment models (IV bolus, IV infusion, first-order oral with and without lag, zero-order oral with and without lag), in the macro, micro and clearance/volume parameter sets (added by tasks T-031 to T-034) | Explicit sums of exponentials evaluated in 256-bit arithmetic (`Rmpfr`) and central differences of them, cross-checked on every case against a matrix exponential (`expm`) and an ODE solver (`deSolve` lsoda); `oracle/scripts/models_2c_closed_form.R`, `models_2c_derivatives.R` | 1e-12 | 295 cases, 25306 values: 107 single-dose value cases, 64 derivative cases (conditions above 1e3 omitted), `model_pk2_errors` (80 not-available rows the engine must refuse, with the reason) and, since tasks T-047 and T-049, 124 multiple-dose and steady-state cases with their refusals | Any other program's output; user-written models (not implemented); the two-compartment fit (only one-compartment fits have a reference fit, see the rows below). As for one compartment, the closed forms are the project's own derivation from textbooks, checked four ways rather than against an outside reference |
| Fit, IV bolus and first-order oral: estimates and every statistic (standard errors, CV%, intervals, correlation, eigenvalues, AIC, SBC, secondary parameters) | `stats::nls` (Gauss-Newton) and `minpack.lm` 1.2.4 `nlsLM` on R 4.5.2 (`oracle/scripts/fit_wls.R`); statistics from an independent R implementation of `specs/fit.md`, standard errors and S checked against `summary.nls` | 1e-4 parameters and statistics, 1e-6 residual sum of squares | 15 cases (Theoph, 12 subjects; Indometh, 6; a five-point example), five weightings each: 95 fits, 5340 values (with the convergence status) | Default settings (closed-form derivatives, relative decrease 1e-10, 50 iterations): 0 of 490 estimates beyond 1e-4 on the 185 reference fits (worst 7.4e-6); the former defaults, kept as the preset `reference_conventions`, give 168 (worst 5.8e-3) (`specs/differences.md` D-03, D-04; Q-014 answered). Bounds, generated initial estimates and quality flags have unit tests only |
| Fit, IV infusion, zero-order input, first-order with lag (added by task T-029) | `nlsLM` checked against `nls`, same R and package versions, on synthetic profiles written by the same script from the closed forms (fixed seed, 8 % log-normal noise; each options file says how); the infusion or input duration is a fixed parameter | 1e-4 and 1e-6 as above | 15 cases, 90 fits, 5070 values (5160 with status). 1e-4 on parameters and statistics. The engine's own closed-form derivatives for these models (task T-030) reproduce all of them: 125 of 125 oracle tests pass (`crates/caladrius-fit/tests/oracle_fit_models.rs`) and the conformance run validates all 5160 values | Real (non-synthetic) profiles for these models; a published worked example with a page reference for any fit |
| The reference commercial software | One private coursework exercise, kept out of the repository. Only counts are recorded and nothing from it, numeric or otherwise, is published here (publishing a comparison is the maintainer's decision) | equality at the displayed precision | counts only, in a local run | Everything else. Its fitting defaults (increment, convergence, bounds) and its definitions of the fit statistics are `assumed` from the maintainer's observation and have not been tested against its output |

## 4. Documented differences

`specs/differences.md` holds the places where Caladrius deliberately gives another answer than an
oracle, or where two methods cannot reach the same number. A difference is a decision, never a
tolerance to widen (`AGENTS.md` section 5).

- **D-01, trailing negative concentration is never Tlast or Clast.** PKNCA warns and goes on, so a
  negative value at the end of a profile becomes Tlast and Clast, and every quantity built on them
  follows (`specs/differences.md`, D-01). Caladrius keeps Tlast and Clast at the last point above
  zero under the policy `allow`; the reason is that Clast is the base of the extrapolation
  `Clast/λz`, and a negative Clast would make the extrapolated area, the percentages and the
  volumes meaningless. It affects 16 parameters of subject 2 of the case `edge_negative_linear`,
  which are counted in the documented column and in neither total (`docs/conformance.md`); it is a
  definition of Tlast, not an arithmetic error on either side.
- **D-02, predicted-value weights on Indometh (withdrawn).** The first oracle accepted degenerate
  "fixed points" (predictions near 1e-90) or reported none, because its stationarity test divided by
  a quantity that grew with the weights; the engine's ordinary fixed points were checked
  independently and upheld (`specs/differences.md`, D-02 and its resolution). The oracle script was
  corrected and its cases regenerated in task T-018, so no case carries a documented-difference
  marker for D-02 and the episode is kept as a caution that the oracle is not infallible.
- **D-03, forward differences of increment 0.001 do not reach the exact minimum to 1e-4.** With
  forward differences of relative increment 1e-3 and the reference fits' stopping rule (relative
  decrease 1e-10), 40 of the 490 estimates of the 185 reference fits land more than 1e-4 from the
  exact minimum (worst 6.9e-4); with the assumed reference defaults (stopping at a relative decrease
  of 1e-4, now the preset `reference_conventions`) it is 168 (D-04); the engine and the reference then solve slightly different problems — an exact Jacobian
  against a 0.1 % finite-difference approximation (`specs/differences.md`, D-03). The contract
  tolerance (1e-4) compares two fits of the same objective computed the same way, so widening it
  would hide the method difference instead of removing it; the engine offers both settings, and how
  the reference software forms its derivatives stays open (OF-01).
- **D-05, steady state with a lag follows the physical superposition, not a printed branch.** The
  closed forms printed in S-36 for the steady state with a lag are used outside their domain when
  the interval is shorter than the lag (plus the input duration); there they give another value, a
  negative one in one case. Caladrius follows the shift rule of MOD-MD-05, which is what the plain
  sum of single doses gives, and the oracle has no branch (`specs/differences.md`, D-05).
- **D-04, the default fit settings differ from the assumed reference defaults.** Since task T-040
  the default fit uses closed-form derivatives when the model has them (otherwise forward
  differences with h = 1e-5) and stops at a relative decrease of 1e-10, giving 0 of 490 estimates
  beyond 1e-4 (worst 7.4e-6); the former defaults remain the named preset `reference_conventions`
  (`specs/differences.md`, D-04). This is a Caladrius design choice (Q-014, option 2), not a claim
  about the reference software, and the oracle tests are unaffected because they set their options
  explicitly.

## 5. Known limits and open items

**Which rules are settled.** Every behaviour rule in `specs/` carries a status. "Confirmed by oracle" means the rule's arithmetic or definition agrees with PKNCA, R or the closed forms above; it says nothing about the reference commercial software. Counts of the `- Status:` lines in each file (made with `grep`):

| Specification | confirmed by oracle | documented, untested | assumed | observed |
|---|---|---|---|---|
| `specs/nca.md` | 44 | 15 | 13 | 5 |
| `specs/models.md` | 52 | 2 | 12 | 0 |
| `specs/fit.md` | 18 | 3 | 22 | 0 |
| `specs/ux.md` | 0 | 12 | 26 | 0 |

The `assumed` rules are the ones that a reference comparison or a human decision still has to settle (for example the fit defaults of `AGENTS.md` section 6); none is presented as settled.

- **Pending questions.** `board/QUESTIONS.md` still holds Q-004 to Q-012 without a human answer:
  Q-004 (licence of the public datasets and PKNCA output), Q-005 (terms of the reference
  documentation), Q-006 (vendor name in `specs/sources.md`), Q-007 (published reference numbers),
  Q-008 (NCA defaults read off the screen), Q-009 (fitting defaults), Q-010 (replaced values and
  the extrapolation to infinity), Q-011 (how to drop the exports of the reference software),
  Q-011b (a sample import file) and Q-012 (a small private oracle for the open NCA conventions).
  Each has a decision taken meanwhile, so work is not blocked on them.
- **Q-014 is settled.** The human answered option 2 on 2026-10-09: the default fit uses closed-form
  derivatives and the tight stop, and the assumed reference defaults stay as the preset
  `reference_conventions` (`board/QUESTIONS.md`, Q-014; `specs/differences.md`, D-04).
- **OM-14.** The 393 derivative entries the two-compartment oracle omits (condition number above
  1e3, at zero crossings and near-degenerate points) are returned by the engine but checked by no
  test; a later oracle card could add them with a conditioning-aware method, without loosening
  `MODEL_DERIVATIVES` (`specs/models.md`, OM-14).
- **OM-15.** No NCA run on pk2-simulated profiles is compared with the model's AUC, AUMC, MRT, Vss
  and terminal half-life yet; the oracle case is still to be written (`specs/models.md`, OM-15).
- **Private oracle.** The private comparison is behind the Cargo feature `private-oracle`; with an
  empty `private/` the tests print `SKIPPED` and pass, and `cargo xtask conformance` prints private
  counts on the console but writes them into `docs/conformance.md` only with `--private`, because
  publishing a numeric comparison with the reference software is the maintainer's decision
  (`board/QUESTIONS.md`, Q-011; `AGENTS.md` section 2).

## 6. How to reproduce

From the repository root, with the Rust toolchain of `rust-toolchain.toml`:

```sh
cargo test --workspace          # every oracle and unit test
cargo xtask conformance         # regenerates docs/conformance.md from oracle/expected/
git diff --exit-code docs/conformance.md   # the file is up to date (the CI check)
```

The expected values themselves are regenerated by the R scripts, each from the repository root:

```sh
Rscript oracle/scripts/nca_pknca.R            # NCA against PKNCA
Rscript oracle/scripts/fit_wls.R              # reference weighted least-squares fits
Rscript oracle/scripts/models_closed_form.R   # one-compartment closed-form values
Rscript oracle/scripts/models_2c_closed_form.R  # two-compartment values, derivatives, refused inputs
Rscript oracle/scripts/models_md.R            # multiple-dosing and steady-state values
```

(`oracle/scripts/models_2c_derivatives.R` is sourced by the last script; it is not invoked on its
own.) The scripts need the R packages named in the version table of section 2.2, installed with
`install.packages`; each writes its versions into the options file of every case, so a reader can
check what produced a given number.

**What a reader should see.** `cargo xtask conformance` prints one section per oracle kind and ends
with `Overall: 48955 of 48955 values validated (100.0 %)`, plus the note that 16 further values are
documented differences (`docs/conformance.md`); the workspace tests are green; the R scripts
rewrite the expected tables and the options files without changing them on the same tool versions.

**How the floors detect a regression.** The last lines of `docs/conformance.md` are one
`<!-- floor <case> <parameter> <validated> <expected> <documented> -->` line per parameter per case
(`docs/conformance.md`). On each run the task compares the new counts with them: a parameter that
validates fewer values, loses expected rows, gains unvalidated rows or disappears, a malformed floor
line, or a non-empty previous file with no floor at all, fails the task, which leaves the previous
file unchanged (section 2.5). Since the floors are committed, `git diff
--exit-code docs/conformance.md` in CI turns any silent weakening of the coverage into a red build,
and the floors can only go up.
