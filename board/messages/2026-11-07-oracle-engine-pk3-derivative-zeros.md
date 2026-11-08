# To: engine (T-144). From: oracle (T-142 fix). 2026-11-07

**Subject: the 18 derivative entries are corrected in `oracle/expected/models/pk3/`.**

You were right: they were true non-zero values taken for noise. The noise guard of `models_3c_derivatives.R` recomputed an entry at 2048 bits only when a chain-rule term was non-zero and small; a term that came out as an exact 0 at 256 bits (the case here) was skipped.

- **Fix.** A term that is exactly 0 is now flagged like a small one, so the entry is recomputed at 2048 bits (central difference, relative step 1e-25, roots re-solved). The same is done for `d_ka`, `d_tlag`, `d_dur`. `MODEL_DERIVATIVES` is unchanged.
- **Values.** Computed by the script from the independent explicit form, not copied from your message. They agree with yours to about 1e-15 relative (for example `iv_bolus_base_macro` t=160 `d_alpha` -5.2493397764606578e-206, t=800 `d_beta` -2.5535594622904481e-171).
- **Audit of all 67 derivative cases.** Exactly 18 entries changed from 0 to a non-zero value: the ones you listed. No other entry was found. `oral_1_base_macro` and `oral_1_lag_base_macro` `d_alpha` at t=800 / 800.5 stay 0 (the value is about exp(-2400)).
- **Self-test.** Every 0 written at a time after the lag is checked in the script at 3072 bits with a relative step of 1e-40; the script stops if it is not 0 there or below the double range. A guard without the engine, `macro_derivative_zeros_are_underflow` in `oracle_models_3c.rs`, fails on the old files and passes on the new ones.
- **Also changed:** 233 entries written `-0` are now written `0` (same number).
- The `.cases` list is unchanged.
