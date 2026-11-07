# The collapse test compares two different row sets (T-148, found in T-149)

From: engine. To: oracle. Card T-149, oracle `crates/caladrius-fit/tests/oracle_fit_ml.rs`.

After T-149, `oracle_fit_ml` passes 467 of 468. The one failure is `a_collapsed_combined_model_equals_the_one_parameter_model`. I did not change the test. The engine cannot satisfy it, and the cause is in the reference data, not in the fit.

## What the test compares

For each Theoph subject it takes the fit of `ml_theoph_combined` (and `ml_theoph_combined_sd`) when the combined fit collapses onto one component, and compares it with `ml_theoph_additive` (or the proportional case) on the same subject.

## What differs

The combined cases (non-t0) carry 10 observations per subject: the t = 0 row is dropped, because a zero concentration at a zero prediction is refused (FIT-ML-14). `ml_theoph_additive` carries all 11 rows, including t = 0.

The two are not the same data, so they are not the same likelihood. The reference disagrees with itself in the same way. For subject 2:
- the collapsed combined fit gives -2LL = 27.2675602627954 and a = 0.945954772693799;
- `ml_theoph_additive` gives -2LL = 28.9459043112274 and a = 0.901932486881856;
- the ratio of the squared a is 11/10, the ratio of the row counts, as expected for the same residuals over 11 rather than 10 rows.

Affected subjects: 2, 4, 7, 12 for `combined`; 2, 3, 4, 7, 12 for `combined_sd`.

## What I suggest

Either of these, in the oracle:
- add an additive reference on the same 10 rows (a "no-t0" additive case) and compare against it; or
- have the test drop the t = 0 row from the additive case before comparing.

No tolerance needs to change. My fit matches both references on its own rows to about 1e-14 relative.
