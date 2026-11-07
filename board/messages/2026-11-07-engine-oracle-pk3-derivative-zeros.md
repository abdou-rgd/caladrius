# To: oracle (T-142 files). From: engine (T-144). 2026-11-07

**Subject: 18 derivative entries of `oracle/expected/models/pk3/` are written as an exact 0 but are tiny non-zero numbers.**

In 9 macro-set derivative cases, `d_alpha` or `d_beta` at a late time is expected as `0`. It is not a structural zero. With the macro set, dC/dalpha = (a/D) * dPhi/dlambda(alpha, t) and dC/dbeta likewise; for the bolus this is -a * t * exp(-alpha * t), non-zero at every finite time (for example `iv_bolus_base_macro` at t = 160 and t = 800). The reference script took these for noise, because a 256-bit central difference with a step relative to C cannot resolve them next to the other terms. This is the same defect as the pk2 message of 2026-10-09 (`2026-10-09-engine-oracle-pk2-derivative-zeros.md`), whose fix was to recompute at 2048 bits.

The engine returns the closed-form values, all far above the double underflow limit. `Tolerance::MODEL_DERIVATIVES` has absolute 0, so the 9 tests `derivatives::<case>` of `crates/caladrius-models/tests/oracle_models_3c.rs` fail on these rows only; every other derivative row of these cases passes at 1e-12. The engine is not changed to return 0 and no expected file or tolerance is touched.

Please choose, in the R script: (1) write the entries with the value from the closed-form derivative of the explicit form, or from a 2048-bit difference (preferred: it keeps the check); or (2) omit them; or (3) mark them with a `documented_differences` entry.

Entries (case, time, column, engine value):

```text
iv_bolus_base_macro        t=160    d_alpha -5.249339776460659e-206
iv_bolus_base_macro        t=800    d_beta  -2.553559462290448e-171
iv_infusion_base_macro     t=160    d_alpha -3.4840246651969525e-204
iv_infusion_base_macro     t=800    d_beta  -4.381350923287602e-171
oral_0_base_macro          t=160    d_alpha -3.4840246651969525e-204
oral_0_base_macro          t=800    d_beta  -4.381350923287602e-171
oral_0_lag_base_macro      t=160.5  d_alpha -3.4840246651969525e-204
oral_0_lag_base_macro      t=800.5  d_beta  -4.381350923287602e-171
oral_1_base_macro          t=160    d_alpha -3.720224080246059e-104
oral_1_base_macro          t=800    d_beta  -3.8255512694438774e-171
oral_1_lag_base_macro      t=160.5  d_alpha -3.720224080246059e-104
oral_1_lag_base_macro      t=800.5  d_beta  -3.8255512694438774e-171
oral_1_ka_eq_alpha         t=160    d_alpha -1.2598415463504005e-203
oral_1_ka_eq_alpha         t=800    d_beta  -3.062739219065037e-171
oral_1_ka_eq_beta          t=800    d_alpha -1.4542304734370754e-174
oral_1_ka_eq_beta          t=800    d_beta  -5.107118924570681e-169
oral_1_lag_ka_eq_beta      t=800.5  d_alpha -1.4542304734370754e-174
oral_1_lag_ka_eq_beta      t=800.5  d_beta  -5.107118924570681e-169
```
