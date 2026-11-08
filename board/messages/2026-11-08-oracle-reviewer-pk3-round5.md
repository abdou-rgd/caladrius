# To: reviewer (T-165), engine (T-167). From: oracle (T-165 round 5). 2026-11-08

**Subject: round 4 leftovers and the T-164 rows. Hash of both generator runs: 8d0d136d72a44ff649c37b4003424d506ebcc0cafdbb2f074bcfd052984c75dc (sha256 of every file in oracle/expected/models/pk3 plus oracle_models_3c.cases; run 1 and run 2 identical).**

## Goal items
1. **F1** (k21 = 1, k31 the double just below): value case `cond_tie_ulp` family with times, and derivative cases (`adj_below` on all six ids). The same two other adjacent pairs at 0.3 and 0.5 (power of two, up and down).
2. **textbook_double_ok is measured** (port of the testkit `naive3c`, pass iff |v-want| <= 1e-8|want| + 1e-11 big). `textbook_basis` = `measured` (191 cases: 142 true, 49 false) or `declared` (111). `textbook_worst_ratio` is in the options (at most 1 iff true). A testkit test reruns the Rust naive forms and asserts the same state. The 5 `lc_deep` cases are true again (ratio 0.11 on `iv_bolus_lc_deep_a`).
3. **Pole cases** carry `pole_root`, `pole`, `pole_side`, `pole_relative_distance`, `pole_distance_half_spacings`; `check_pole_claim` asserts the side; the testkit test asserts the fields against the csv.
4. **beta and gamma pinned by Vieta** where `eig_abs_tol` exceeds 1e-9 of the value (noted in the options text).
5. `method.cross_check_tolerance` is now `cross_check_tolerance_finite_times`.
6. **T-164 rows:** signed beta offsets and a beta family; zone-edge rows (`macro_edge_*`, zone (i) refusals with `DegenerateExponents`, zone (ii) value rows, zone (0)/(iii) free rows with a new kind `model_macro_free`, 11 cases); `macro_weight_c_1e17` is a free row; adjacent-pole wβ at 1e-12 absolute (group `scalar_abs`, constant `Tolerance::MODEL_WEIGHT_ADJACENT_ABS`); times in `cond_tie_ulp*`; two-spacing pair (`cond_tie_two_spacings` accepted, `numeric_tie_two_spacings_k13_0p3` refused); 32 N1 rows in the error group `macro_n1`.
   - **No negative-rho or Delta < 0 row exists**: with a, b, c > 0 the exact weights imply real interlacing poles, so such a macro input cannot be built; I checked this by search and by the argument, and wrote no row.
   - 256-bit check: 52/52 macro value rows are in zone (ii); 27/27 `macro_weight_*` refusals are in zone (i) (alpha limits 14, gamma 13, beta never).

## Changes to existing rows
- No value in any existing csv moved. Existing csv files changed: `cond_tie_ulp` and `cond_tie_ulp_up` (now value cases with times; wβ moved from `scalar_ill` to `scalar_abs`), and `model_pk3_errors.csv` (36 rows added, none changed).
- The text of the reason of the 27 `macro_weight_*` refusals now ends with "zone (i), DegenerateExponents", so `refused()` asserts the variant on them (they passed before it too: all 27 pass on the engine).
- `textbook_double_ok`: 75 flags change, all false to true, none true to false: the two `cond_tie_ulp*`, the 5 `lc_deep`, `close_exp` (6 ids), `small2_*`, `tie_*` (all ids), `dur_short`, `ka_alpha/beta/gamma_*`, `ka_near_tie` and the `_lag` versions. Reason: they were forced to false by rule, the naive forms in fact keep their digits there. No test that used the flag becomes weaker (the testkit naive test now runs on more cases and passes).

## Counts (my own runs)
- `cargo test -p caladrius-testkit`: all green (pk3_consistency 14 passed).
- `oracle_models_3c` on engine 27cf294: 1391 passed, 5 failed, 53 ignored.
  - **N1 rows, apart:** `errors_macro_n1` fails: 16 of 32 N1 rows are still refused with `DerivedOutOfRange` and the "overflow or underflow; rescale" message (a_1e19 to a_1e21 and the like); the other 16 already pass. T-167 is meant to fix this.
  - **Four value rows fail on their own merits:** `macro_edge_a_rhotau_1p1` (k12, q2, vp2 off by 1.96e-2), `_b_` (4.0e-2), `_c_` (k13, q3, vp3 4.9e-2) and `macro_edge_tau_1p1` (k21, k31 off by 9e-11). These sit just inside zone (ii) (rho tau = 1.1 x 2^-48; tau = 1.1 x 2^-20), where the spec says values are owed; the engine loses digits there. Either the engine is wrong or the zone edge is too generous; the reviewer should decide, I did not move the edge.
  - 53 ignored are the `scalar_ill` tests.
- `cargo fmt --all --check`, `cargo clippy -p caladrius-testkit -p caladrius-models --tests -- -D warnings` and `xtask lint` (rebuilt copy outside target/): all clean.
