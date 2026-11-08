# Visual predictive check: simulate the design, bin, compare quantiles (VPC-*, card T-160)

Brick 4 of iteration 3 (T-139). `specs/simulation.md` simulates a population model and `specs/population.md` fits one; this file specifies the check that sets them side by side: **simulate R virtual studies of the observed design from the estimates of a fit, bin the independent variable, and in each bin compare the quantiles of the observed values with the band that the replicates give for the same quantiles.** Only numbers are specified (tables, section 7); no drawing. Written by the `reader` agent on 2026-11-08, in its own words, from the open sources of section 11 and from derivations (D). **Nothing in this file is implemented or exercised by an oracle.** Every rule is `assumed` (the sources are silent or disagree, and the reason is given) or `documented, untested` (a source states it and the arithmetic was checked in base R). The numbers of the worked examples (section 12) come from base R 4.5.2 scripts, seeded, kept outside the repository in the session scratchpad and named next to each example (`vx1_binning.R` to `vx6_refusals.R`, helpers in `vpclib.R`). They are hand checks for the future oracle, not results of the engine.

**Reading guide.** The rules answer the eleven items of the card in order: what is simulated (section 1), binning (2), statistics per bin (3), prediction correction (4), BLQ and censoring (5), stratification (6), outputs (7), determinism (8), testing (9), refusals (10); then the sources (11), the worked examples (12) and the open items for Abdou (13). Three choices are Caladrius's own and each has its reason: the VPC **draws no randomness of its own** (it calls the simulator, so the streams of SIM-RNG-03 are unchanged); **every default is a named option, and a transformation of the data (prediction correction, omitting rows) is never applied silently**; and **a statistic that cannot be computed is reported as not computed, with a flag, never filled in**.

Notation added to `specs/simulation.md` and `specs/population.md`:

| symbol | meaning |
|---|---|
| R, s | number of replicates and seed (SIM-RNG-02); replicate r = 0..R−1 |
| x_j | the independent variable of design row j: time after the subject's dose (VPC-BIN-09) |
| b | a bin, with left edge e_b and right edge e_{b+1}; k the number of bins |
| p, π | a probability; π the list of probabilities of the prediction interval |
| c, a | the band level and its tail probability a = (1 − c)/2 |
| Q(p; v) | the type-7 quantile of the values v (SIM-SUM-01) |
| PRED_j | the typical prediction (all η = 0) of design row j (POP-DIA-01) |
| lb | the lower bound of the dependent variable used by the correction (VPC-PC-03) |
| LLOQ | one number per run (SIM-ORD-05) |

## 1. What is simulated (card item 1)

### VPC-SIM-01 Simulation from the estimates, on the observed design
The VPC simulates R replicates with the simulator of `specs/simulation.md` at the estimates of the fit: θ̂, Ω̂ and the error parameters ξ̂ of POP-EST-04, no other value. The design is the table of the observed dataset read as SIM-MOD-04 item 1 reads it (subject labels, dose, route, observation times and covariates; the observed values are ignored by the simulation and used only by the statistics). The simulated table has one row per design row, in design order, so that row j of replicate r is the counterpart of observed row j. The simulation uses the **labels of the dataset**, so the draws of a subject do not depend on which other subjects are in the run (SIM-RNG-03).
- Status: `documented, untested`
- Sources: Karlsson and Holford 2008, slide 14 (the simulated data sets have the same structure as the original); Holford 2005 (simulate with the original data set as the template); H-VX2.

### VPC-SIM-02 Design rows: what is kept
A design row enters the VPC when it is an observation row of a subject used by the fit (VPC-SIM-03). Rows with a missing value (`DAT-CEN-*`) and rows before the dose (`before_dose`, POP-DAT-01) are **dropped from the VPC design** and counted (`n_missing`, `n_before_dose` in the header, VPC-OUT-06). **BLQ rows are kept**: they are observations, they count in the BLQ fraction (VPC-BLQ-03) and, depending on the option of VPC-BLQ-02, in the quantiles. Reason: the fit leaves BLQ rows out (POP-DAT-03) because it has no censored likelihood; a check of the data must not forget them, and the simulator already knows how to produce them (SIM-ORD-05).
- Status: `assumed` (the sources do not say which rows form the design; the reason is the one above)
- Sources: D; POP-DAT-01 to -03; DAT-CEN-02; Bergstrand and Karlsson 2009 (the BLQ data are part of the check).

### VPC-SIM-03 Subjects
The subjects are those used by the fit, minus the ones the user excludes by name. A subject whose rows are all BLQ **stays in** (it contributes to the BLQ fraction). A subject unknown to the fit is not added.
- Status: `assumed` (D: the check is of the fitted model on the data it was fitted to)
- Sources: D; POP-DAT-01.

### VPC-SIM-04 A fit that is not usable
A fit whose status is not converged (POP-EST-04) is **refused** (`fit_not_converged`) unless the call names the override `allow_unconverged_fit`; with the override the result carries the flag `unconverged_fit` in its header. Estimates with a non-positive-definite Ω̂ are refused as the simulator refuses them (SIM-REF-02).
- Status: `assumed` (a VPC of a failed fit is a picture of nothing; the sources do not discuss it)
- Sources: D; POP-EST-04; SIM-REF-02.

### VPC-SIM-05 Parameter uncertainty is out of v1
Replicates are simulated at the point estimates; the purpose p = 3 stays reserved (SIM-RNG-03), so a later rule that draws θ, Ω and ξ from the covariance of the fit moves no existing draw. Reasons for leaving it out: (1) the sources simulate at the final estimates; (2) the covariance of POP-COV-01 is on the log φ scale and its default is the sandwich estimator, and drawing from it needs a rule on how a non-positive-definite or missing covariance is handled, which no source gives; (3) a band that ignores the uncertainty of the estimates is narrower than one that includes it, which the header states (`parameter_uncertainty: none`) so a reader cannot take it for the other.
- Status: `assumed`
- Sources: OS-09; help pages of the `vpc` and `saemix` packages (simulation from the final estimates); Karlsson and Holford 2008, slide 12 (the confidence interval is about the simulated statistic).

### VPC-SIM-06 R, the seed and their limits
The seed is required with no default (SIM-RNG-02). The default R is 1000 (Bergstrand et al. 2011 used 1000; the help of `saemix` gives `nb.sim` = 1000; the help of nlmixr2plot gives 300). R is bounded by the size limits of OS-04 and by VPC-STAT-05.
- Status: `documented, untested` for the value 1000, `assumed` for its use as the default
- Sources: Bergstrand et al. 2011 (Methods, VPC); `saemix` help (`nb.sim`); nlmixr2plot help (`n`).

## 2. Binning (card item 2)

### VPC-BIN-01 The rules offered
Three rules, named by the option `bins`: **`nominal_time`** (one bin per distinct value of x), **`edges`** (the user gives the edges) and **`equal_count`** (about the same number of rows in each bin, VPC-BIN-06). `k` (number of bins) applies to `equal_count` only.
- Status: `documented, untested`
- Sources: Bergstrand et al. 2011 (Methods, VPC: one bin per protocol time point for a fixed design, about equal numbers of observations per bin otherwise); Karlsson and Holford 2008, slides 6 to 11; `saemix` help (`vpc.bin`, `bin.method`, `bin.breaks`).

### VPC-BIN-02 Default: `equal_count`, k = 10
The default is `equal_count` with k = 10. Reasons: it works for any design (fixed or sparse), it gives each bin enough rows, and ten is the number of bins of the `saemix` default. The `vpc` package's own default is a clustering method (Jenks) with an automatic number of bins: that is not offered, because its result is not a pure function of a rule that a port could implement from a paragraph (OV-02).
- Status: `assumed`
- Sources: `saemix` help (`vpc.bin` = 10); `vpc` help (`bins`, `n_bins`); D.

### VPC-BIN-03 Which bin a boundary value belongs to
A bin is **[e_b, e_{b+1})**: closed on the left, open on the right, except the last bin, which is **closed on both sides**. A value on an inner edge goes to the bin on its right; the maximum goes to the last bin. This is R's `findInterval(x, edges, rightmost.closed = TRUE)`. With user edges (VPC-BIN-08) a row whose x is below the first edge or above the last is **excluded from every statistic and counted** (`n_outside_bins`), never moved to an end bin.
- Status: `assumed` (no source says which side; the choice follows R's documentation of `findInterval`)
- Sources: R help of `findInterval` (the intervals, `rightmost.closed`); H-VX1.

### VPC-BIN-04 The x at which a bin is drawn
Each bin has: `x_mean` (mean of the x of its design rows), `x_middle` (the middle of its two edges) and the edges. The column `x` of the table is `x_mean` by default (option `bin_x` = `mean` | `middle`). `x_mean` is a sum taken **sequentially in plain double in the dataset order of the design rows** and divided by the count (VPC-DET-03). For `nominal_time` the three coincide.
- Status: `assumed` (the `vpc` help gives `bin_mid` with the values `mean` and `middle` and `mean` as default; the other sources are silent)
- Sources: `vpc` help (`bin_mid`); D; H-VX1, H-VX6.

### VPC-BIN-05 An empty bin, and a bin with few observations
A bin with no observation row has status `empty`: it is listed, its statistics are not computed (flag `empty`). A bin with fewer than `min_obs` observed values entering the quantiles (default 3) has status `sparse`: its statistics are computed and flagged `sparse`. A run with no bin at all is refused (VPC-REF-01). The value 3 is arbitrary and is an open item (OV-03).
- Status: `assumed`
- Sources: D; Bergstrand et al. 2011 (the choice of 2.5/97.5 or 5/95 percentiles depends on the amount of data).

### VPC-BIN-06 `equal_count`: the algorithm
Integer arithmetic only, so every port gives the same bins. Let n be the number of design rows (after VPC-SIM-02) of the stratum, v_1 < … < v_d the distinct x with their counts, C_j the cumulative count up to v_j.
1. If d ≤ k, use one bin per distinct value (as `nominal_time`).
2. Otherwise, for l = 1..k−1 choose the cut after the index j in 1..d−1 that minimises |C_j·k − l·n|, ties to the smaller j.
3. Merge equal cuts (there may be fewer than k bins). The left edges are the data value v_{j+1} after each cut, the first left edge is v_1, the last edge is v_d, and the last bin is closed (VPC-BIN-03).

Equal x values are never split between bins.
- Status: `assumed` (the sources say "about equal numbers"; the algorithm is this file's choice, with its reason: integer, deterministic, no floating point)
- Sources: Karlsson and Holford 2008, slides 6 to 11; D; H-VX1.

### VPC-BIN-07 `nominal_time`
One bin per distinct x, edges at the distinct values, the last one closed. It assumes the x values are the protocol times: **if the times are measured times that differ between subjects, the rule gives about one bin per row and the call is refused (`too_many_bins`, VPC-REF-01)** instead of silently drawing a point per observation. Whether a nominal-time column should exist is open (OV-14); `specs/data.md` has none.
- Status: `assumed`
- Sources: Bergstrand et al. 2011 (Methods, VPC); `specs/data.md`.

### VPC-BIN-08 `edges`
The user gives at least two strictly increasing finite edges. They are shared by all strata (VPC-STR-03). Duplicate or decreasing edges are refused (`bad_edges`).
- Status: `assumed`
- Sources: `saemix` help (`bin.breaks`); `vpc` help (`bins` given as a vector).

### VPC-BIN-09 The independent variable
x is the **time after the subject's dose**. Other variables (time since the first dose, a covariate, PRED) are not offered in v1 (OV-14).
- Status: `assumed` (one dose per subject, POP-DAT-01, makes time after dose well defined)
- Sources: D; POP-DAT-01.

## 3. The statistics per bin (card item 3)

### VPC-STAT-01 Observed quantiles
In each bin, for each probability p in π, the observed statistic is Q(p; v), type 7 in the form of SIM-SUM-01, on the observed values v of the rows of the bin that enter the quantiles (VPC-BLQ-02).
- Status: `documented, untested`
- Sources: Bergstrand et al. 2011 (Methods, VPC: percentiles of the observed data per bin); SIM-SUM-01; R help of `quantile`.

### VPC-STAT-02 The same quantile on each replicate
For each replicate r and each bin, the same quantile of the simulated values of the same design rows, with the same rule for BLQ rows (VPC-BLQ-02). The replicate-wise statistics are pure functions of replicate r alone (VPC-DET-02).
- Status: `documented, untested`
- Sources: Bergstrand et al. 2011 (Methods, VPC); Karlsson and Holford 2008, slide 3.

### VPC-STAT-03 The band across replicates
For each (bin, p), the band is the type-7 quantile **across the R replicate-wise statistics** at a = (1 − c)/2 and 1 − a, and at 0.5 (the simulated median): columns `sim_lo`, `sim_med`, `sim_hi`. A replicate for which the statistic is not defined (an empty bin in that replicate, or `censored_rank` giving a censored rank) is left out of the quantile and counted in `n_rep_defined`; if fewer than 2 replicates are defined the band is not computed (flag `band_undefined`).
- Status: `documented, untested`
- Sources: Bergstrand et al. 2011 (Methods, VPC: the 95% confidence interval from the 2.5 and 97.5 percentiles of the replicate-wise statistics); Karlsson and Holford 2008, slide 12; H-VX2.

### VPC-STAT-04 Defaults: π and the level
π = (0.05, 0.5, 0.95); c = 0.95. Reason: the median with the 5% and 95% percentiles is the interval of the sources (the 2.5/97.5 pair is the alternative for large data sets), and the 95% band is the one of both papers. The `vpc` help uses (0.05, 0.95) both for the interval and for the band, and always draws the median.
- Status: `documented, untested` for the values, `assumed` for the choice of 0.05/0.95 over 0.025/0.975
- Sources: Bergstrand et al. 2011 (Methods, VPC); Karlsson and Holford 2008, slide 3; `vpc` help (`pi`, `ci`); `saemix` help (`vpc.interval` = 0.95).

### VPC-STAT-05 R must allow the band level
The band at level c needs R·(1 − c) ≥ 2, so that each tail holds at least one replicate. The test is **R·(1 − c) ≥ 2·(1 − 10⁻¹²)**, not `ceil(2/(1 − c))`, because in double 2/(1 − 0.9) is 20.000000000000004 and the ceiling gives 21 where 20 is right (H-VX6). With c = 0.95 the smallest R is 40. A call below it is refused (`too_few_replicates`, VPC-REF-01).
- Status: `assumed` (D: the rule is this file's; the bound 2 is the least that puts a replicate in each tail)
- Sources: D; H-VX6.

### VPC-STAT-06 All rows in the bin, not one value per subject
The observed quantile is computed over **all observed rows in the bin**, as the replicate quantiles are. The option `per_subject` (one value per subject and bin, the mean of the subject's rows) is not offered in v1 (OV-06).
- Status: `assumed` (the sources compute a percentile of the observations in a bin and are silent on subjects)
- Sources: Karlsson and Holford 2008, slide 3; Bergstrand et al. 2011 (Methods, VPC); D.

### VPC-STAT-07 A statistic that cannot be computed
An empty bin (or a bin with no value entering the quantiles) gives no statistic: the cell is empty with its flag, never zero and never the neighbour's value.
- Status: `assumed`
- Sources: D.

## 4. Prediction correction (card item 4)

### VPC-PC-01 The normalisation
The option `prediction_correction = pc` replaces each observed and each simulated value Y of row j by

pcY_j = lb + (Y_j − lb)·(PREDbin − lb)/(PRED_j − lb)

where PRED_j is the typical prediction of row j (all η = 0, POP-DIA-01) and PREDbin the **median of the PRED_j of the design rows of the bin** (Bergstrand et al. 2011, Eq. 1). The correction is applied to the observed values and to every replicate, with the same PREDbin. The default is `none`.
- Status: `documented, untested`
- Sources: Bergstrand et al. 2011 (Methods, pcVPC, Eq. 1; the equation was read as an image from the full text); Karlsson and Holford 2008, slide 36 (the form OBS·PREDav,bin/PRED, which is lb = 0); H-VX3.

### VPC-PC-02 A named option that changes the data
The correction changes every value; it is **never applied silently**. The header of every table says `prediction_correction: none | pc`, and the table `vpc_obs` carries both `y` and `pcy`.
- Status: `assumed` (the card's requirement; the sources describe the correction as a choice)
- Sources: D; Bergstrand et al. 2011.

### VPC-PC-03 The lower bound
lb is the lower bound of the dependent variable, taken from the equation as the paper writes it: default 0 (a concentration), an option `lb` for another (for example a response with a floor of −1). A value at the floor stays at the floor, because Y − lb = 0. The paper's log form (Eq. 2: ln pcY = ln Y + (ln PREDbin − ln PRED)) is for a log-transformed DV and is **not offered** (`later`, OV-09).
- Status: `documented, untested` for the formula, `assumed` for the default lb = 0
- Sources: Bergstrand et al. 2011 (Eq. 1 and Eq. 2); `vpc` help (`pred_corr_lower_bnd`, default 0); H-VX3.

### VPC-PC-04 Which rows give PREDbin
PREDbin is the median (type 7) of PRED_j over **all design rows of the bin and stratum, BLQ rows included** (the typical prediction exists for every row; leaving BLQ rows out would make PREDbin depend on the data's censoring). The PRED_j come from the same estimates as the replicates.
- Status: `assumed` (no source says; the reason is the one above)
- Sources: D; Bergstrand et al. 2011.

### VPC-PC-05 A non-positive denominator
A row with PRED_j − lb ≤ 0 is **refused** (`non_positive_pred`), with the row and the value in the sentence. Rows are not dropped silently (a user may exclude subjects, VPC-SIM-03).
- Status: `assumed`
- Sources: D; H-VX3 (PRED = lb gives an infinite ratio).

### VPC-PC-06 Order of evaluation
The correction is evaluated ratio first: t = (PREDbin − lb)/(PRED_j − lb); pcY = lb + ((Y − lb)·t) (VPC-DET-03).
- Status: `assumed` (the order is fixed for bit equality)
- Sources: H-VX3.

### VPC-PC-07 Correction and BLQ
pcVPC with `blq_quantiles = censored_rank` is refused (`pc_with_censoring`): the correction moves the values and the LLOQ would have to move with them, and the source says the rank order is lost. With `omit` the BLQ rows are left out of the quantiles before the correction, as the paper advises.
- Status: `documented, untested` for the use of `omit`, `assumed` for the refusal
- Sources: Bergstrand et al. 2011 (Discussion: censored observations are omitted from the observed and the simulated data in a pcVPC; the rank order is lost after correction).

## 5. BLQ and censoring (card item 5)

### VPC-BLQ-01 The one LLOQ
The run has **one LLOQ** (SIM-ORD-05, OS-07). It comes from the option `lloq` or, if absent, from the `limit` of the BLQ rows of the dataset (DAT-CEN-02). Several different limits in the data are refused (`several_limits`); BLQ rows with no limit and no option are refused (`blq_without_limit`); an option that differs from the limit of the data is allowed and noted in the header. The simulated value is BLQ if and only if y < LLOQ, strictly (SIM-ORD-05).
- Status: `assumed`
- Sources: SIM-ORD-05; DAT-CEN-02; Bergstrand and Karlsson 2009 (a single quantification limit in the example).

### VPC-BLQ-02 How BLQ rows enter the quantiles
The option `blq_quantiles` has two values.
- **`omit`** (default): BLQ rows, observed and simulated, are left out of the quantiles of VPC-STAT-01 and -02. The count of the values that remain is `n_obs`; the BLQ rows are counted in `n_blq`.
- **`censored_rank`**: BLQ rows stay as values known to lie below the LLOQ. In a bin with n values of which nc are censored, the ordered values are the nc censored ranks followed by the uncensored ones; with h = 1 + (n − 1)p, lo = ⌊h⌋, hi = ⌈h⌉, g = h − lo: if lo ≤ nc the quantile is **below the LLOQ** (not a number, flag `below_lloq`), otherwise it is (1 − g)·x_(lo−nc) + g·x_(hi−nc) of the uncensored values. The observed statistic and every replicate statistic use the same rule; the band is over the replicates for which the statistic is a number, with `n_rep_defined` and the count `n_rep_below`.

Reasons for `omit` as the default: it is what both papers do for a pcVPC and it is the simplest; `censored_rank` shows what happens in the censored region without inventing values. Imputing a number for a BLQ row (LLOQ/2 or the like) is not offered.
- Status: `omit`: `documented, untested`; `censored_rank`: `assumed` (the papers show a panel of percentiles of the data and a panel of the BLQ fraction, and the omitting option for the corrected case, but give no formula for a quantile in the censored region)
- Sources: Bergstrand and Karlsson 2009 (Methods, Visual Predictive Check; Results, Figures 4 and 5); Bergstrand et al. 2011 (Discussion); Karlsson and Holford 2008, slide 29 (show only uncensored data, replace censored values, or check the censoring separately); H-VX4.

### VPC-BLQ-03 The fraction below the LLOQ
For each (stratum, bin) the observed fraction is **n_blq / n_rows**, over the design rows of the bin (VPC-SIM-02: missing and before-dose rows already out, BLQ rows in). For each replicate the same fraction from the simulated BLQ flags; the band is the quantile across replicates at a, 0.5 and 1 − a (columns `sim_lo`, `sim_med`, `sim_hi`). The fraction is computed whatever the option of VPC-BLQ-02. When there is no LLOQ (VPC-BLQ-01), the table is not produced.
- Status: `documented, untested` for the display (observed fraction against the simulated median and interval), `assumed` for the band level
- Sources: Bergstrand and Karlsson 2009 (Methods, Visual Predictive Check: 500 datasets, a second panel for the BLQ fraction); `vpc` help (`vpc_cens`: probability of being below the limit); H-VX4.

### VPC-BLQ-04 Nothing censored
With no BLQ row in the data and none simulated, the table of VPC-BLQ-03 has fractions of 0 and is produced, provided the run has an LLOQ.
- Status: `assumed`
- Sources: D.

## 6. Stratification (card item 6)

### VPC-STR-01 Simulate once, stratify the statistics
The simulation is run **once** for all subjects; strata apply to the statistics only. A stratum is a set of subjects, so a stratum run alone gives the same numbers as the same stratum inside a larger run (VPC-ORA-05), because the draws depend on the subject's label (VPC-SIM-01).
- Status: `assumed`
- Sources: Karlsson and Holford 2008, slides 33 to 36 (stratify on the quantities that differ between groups); D.

### VPC-STR-02 By what
One variable in v1: a **covariate column** (constant per subject, DAT-COV-01), the **route** column, or the **dose group** (the distinct dose amounts). The sources list stratifying variables; the `vpc` help allows two (OV-10).
- Status: `assumed`
- Sources: Karlsson and Holford 2008, slides 33 to 36; `vpc` help (`stratify`).

### VPC-STR-03 Binning in strata
Each stratum is **binned on its own** for `equal_count` and `nominal_time` (the edges are chosen from the stratum's own rows), because strata can have different designs. User `edges` are **shared** by all strata. With prediction correction, PREDbin is per (stratum, bin).
- Status: `assumed`
- Sources: Karlsson and Holford 2008, slides 33 to 36 (silent on binning); D.

### VPC-STR-04 What is refused in stratification
An empty stratum (a level with no used subject), a missing covariate value, more levels than the limit `MAX_STRATA` (the value is OV-12) and a covariate that varies within a subject (DAT-COV-01) are refused (VPC-REF-01).
- Status: `assumed`
- Sources: D; DAT-COV-01.

## 7. The outputs (card item 7)

### VPC-OUT-01 `vpc_bins`
One row per (stratum, bin): `stratum`, `bin`, `left`, `right`, `closed_right` (true for the last bin), `x` (VPC-BIN-04), `x_mean`, `x_middle`, `n_rows`, `n_obs`, `n_blq`, `n_subjects`, `status` (`ok`, `sparse`, `empty`), `pred_bin` (the median PRED of the bin; present whatever the option, because it costs nothing).
- Status: `assumed`
- Sources: D.

### VPC-OUT-02 `vpc_quantiles`
One row per (stratum, bin, p): `stratum`, `bin`, `p`, `obs`, `sim_lo`, `sim_med`, `sim_hi`, `n_rep_defined`, `n_rep_below` (zero under `omit`), `flag` (`ok`, `sparse`, `empty`, `below_lloq`, `band_undefined`), `inside` (true if sim_lo ≤ obs ≤ sim_hi; absent when any of the three is not a number).
- Status: `assumed`
- Sources: D; Bergstrand et al. 2011.

### VPC-OUT-03 `vpc_blq`
One row per (stratum, bin): `stratum`, `bin`, `n_rows`, `n_blq`, `obs_fraction`, `sim_lo`, `sim_med`, `sim_hi`, `inside`.
- Status: `assumed`
- Sources: Bergstrand and Karlsson 2009.

### VPC-OUT-04 `vpc_summary`
Per (stratum, p): the number of bins computed, the number inside the band, and the numbers outside below and above. It is a count, not a test.
- Status: `assumed`
- Sources: D.

### VPC-OUT-05 `vpc_obs`
One row per observed design row: `subject`, `row`, `stratum`, `bin`, `x`, `y`, `pcy` (absent without correction), `blq`, `excluded` (`outside_bins`, or none).
- Status: `assumed`
- Sources: D.

### VPC-OUT-06 The header
The header echoes the inputs that decide the numbers: the seed, R, the `rng` version string of SIM-DET-03, π, c, `bins` and its parameters, `bin_x`, `min_obs`, `prediction_correction`, `lb`, `lloq` and where it came from, `blq_quantiles`, the stratifier, `parameter_uncertainty: none`, the flags (`unconverged_fit`) and the counts `n_subjects`, `n_rows`, `n_missing`, `n_before_dose`, `n_outside_bins`, `n_blq`.
- Status: `assumed`
- Sources: D; SIM-DET-03.

### VPC-OUT-07 The replicate-wise statistics
An option (`keep_replicate_stats`, off by default) adds `vpc_replicate_stats`: (replicate, stratum, bin, p, value). It is what the calibration check and a port need to compare; it is large (R × bins × π) and bounded by the size limits.
- Status: `assumed`
- Sources: D.

## 8. Determinism (card item 8)

### VPC-DET-01 A pure function
The result is a pure function of (θ̂, Ω̂, ξ̂, the design, the observed values, the seed, R and the options). It uses no clock, no global state and **no random number of its own**; all randomness is the simulator's (SIM-DET-01).
- Status: `documented, untested`
- Sources: SIM-DET-01.

### VPC-DET-02 Threads and assembly
The statistics of replicate r depend on replicate r alone, so replicates may be evaluated in any order on any number of threads and in WebAssembly; the library then assembles the tables **in index order** (replicate, then stratum, bin, p). The band sorts the R values (a total order on finite doubles), so it does not depend on the order of arrival.
- Status: `documented, untested`
- Sources: SIM-DET-01 to -03.

### VPC-DET-03 The sums and the order of operations
The only sums are the bin `x_mean` (and the counts, which are integers and associative). `x_mean` is accumulated **sequentially in plain double in dataset order**, then divided by the count. The oracle must use an explicit loop: R's `sum()` and `mean()` accumulate in long double and differ from the double loop in the last digit in 897 of 2000 random vectors of 7 values (H-VX6). The correction is ratio first (VPC-PC-06): the product-first form differs in 34837 of 10⁵ random cases (H-VX3). The quantiles use the form of SIM-SUM-01, and R's `median` equals the type-7 quantile at 0.5 (no difference in 2000 vectors, H-VX6).
- Status: `documented, untested`
- Sources: SIM-SUM-01; H-VX3, H-VX6.

### VPC-DET-04 The canonical text
The canonical text of SIM-DET-04 is extended, after the simulation sections, by: a `vpc` header section (VPC-OUT-06, key order fixed), then `bins`, `quantiles`, `blq` and `obs` sections, one line per row in table order, numbers spelled as in SIM-DET-04 (Rust `{:?}`), a missing number written as `na`. A VPC run is added to the workload whose digest SIM-DET-02 pins; the digest of the simulation alone does not change, because the VPC adds sections after it.
- Status: `assumed`
- Sources: SIM-DET-02 to -04.

## 9. What can be tested, and how (card item 9)

### VPC-ORA-01 The exact part
Given a simulated table and the observed table, binning (VPC-BIN-*) and the quantiles (VPC-STAT-*) are a deterministic function: base R `findInterval` and `quantile(type = 7)` (no package) reproduce them **bit for bit**. In H-VX2, 27 of 27 statistics are `identical()` to R's `quantile`. The oracle takes the simulated table from the engine and recomputes the bins, the observed quantiles, the replicate quantiles and the band.
- Status: `documented, untested`
- Sources: H-VX2; R help of `quantile` and `findInterval`.

### VPC-ORA-02 Calibration under the true model
If the observed data are one draw of the model that is simulated, the observed quantile falls inside the band at level c with probability about **(R − 1)(1 − α)/(R + 1)** with α = 1 − c (0.948102 at R = 1000, α = 0.05; D, from the rank of one draw among R exchangeable draws, up to the interpolation of the quantiles). The check: K datasets with independent seeds, one (bin, p) per dataset, count those inside, and require the count to lie within the exact binomial bounds `qbinom(ε, K, ρ)` and `qbinom(1 − ε, K, ρ)` (ρ as above) at a false-alarm level ε = 5·10⁻⁷ per tail. At a fixed seed the result is repeatable, so a failure is a defect, not bad luck. A **power** check is the same count with a misspecified simulation, which must fall far below the lower bound (H-VX5).
- Status: `assumed` (D, with the interpolation approximation)
- Sources: D; H-VX5; Karlsson and Holford 2008, slides 37 to 39 (false-positive and false-negative rates).

### VPC-ORA-03 The correction
The correction is checked on a worked case (H-VX3) against the formula, bit for bit with the order of VPC-PC-06, and on its effect: the spread of the corrected values in the bin is smaller than the raw spread when the doses differ.
- Status: `documented, untested`
- Sources: Bergstrand et al. 2011; H-VX3.

### VPC-ORA-04 Refusals
Each refusal of section 10 has a case with the expected code.
- Status: `assumed`
- Sources: D.

### VPC-ORA-05 Invariance
(a) A stratum run alone equals the same stratum inside a larger run (VPC-STR-01). (b) The digest is the same on 1 thread and N threads and in WebAssembly (VPC-DET-02). (c) Adding a subject outside a stratum changes none of its numbers.
- Status: `documented, untested`
- Sources: SIM-DET-01 to -03.

## 10. Refusals (card item 10)

### VPC-REF-01 The refusals
Each refusal is an error value with a code and a sentence in the user's words, never a crash (golden rule 6), and names the value that caused it.

| code | when |
|---|---|
| `missing_seed` | no seed (SIM-RNG-02) |
| `fit_not_converged` | VPC-SIM-04 |
| `too_few_replicates` | R·(1 − c) < 2·(1 − 10⁻¹²) (VPC-STAT-05) |
| `bad_level`, `bad_probability` | c or a p outside (0, 1) |
| `no_bins` | the rule leaves no bin (no usable row, or `edges` outside every row) |
| `too_many_bins` | `nominal_time` on measured times, or more bins than the limit `MAX_BINS` (OV-12) |
| `bad_edges` | VPC-BIN-08 |
| `empty_stratum`, `missing_covariate`, `covariate_varies`, `too_many_strata` | VPC-STR-04 |
| `non_positive_pred` | VPC-PC-05 |
| `pc_with_censoring` | VPC-PC-07 |
| `blq_without_limit`, `several_limits` | VPC-BLQ-01 |
| `size_limit` | rows, replicates or table size above the limits (OS-04, OV-12) |

- Status: `assumed`
- Sources: D; `AGENTS.md` golden rule 6; SIM-REF-*.

## 11. Sources (clean room)

Allowed sources only (`AGENTS.md` rule 1). **Read:** Holford 2005, PAGE 14 (Abstr 738), poster abstract and text (thin on construction); Karlsson and Holford 2008, PAGE 17 (Abstr 1434), tutorial slides (slide numbers cited); Bergstrand, Hooker, Wallin and Karlsson 2011, AAPS J 13(2):143-151 (Methods VPC and pcVPC, Eq. 1 and 2 read as images, Discussion); Bergstrand and Karlsson 2009, AAPS J 11(3):371-380 (Methods VPC, Results Figures 4 and 5); R 4.5.2 help of `quantile` and `findInterval`; help pages (not code) of the `vpc` package (MIT), of `saemix` and of nlmixr2plot. **Not read:** the code of any of these packages; the equation of the variability correction (pvcVPC, Bergstrand 2011 Eq. 3, only described in words, so not offered); documentation of any commercial program. The statements of the `saemix` help on `vpc.method` and `vpc.extreme` were not found in the text read.

Key: H-VXn is the hand check VXn of section 12; D is a derivation in this file.

## 12. Worked examples (targets for the oracle, base R 4.5.2)

**Common.** Scripts in the scratchpad: `vpclib.R` (type-7 quantile in the form of SIM-SUM-01, equal-count edges, bin index, correction, censored quantile), `vx1_binning.R` to `vx6_refusals.R`. Doubles are shown as printed by R.

### VX1 Binning (`vx1_binning.R`, H-VX1)
A fixed grid: 71 rows, times 0.5, 1, 2, 4, 8 (12 rows each) and 12 h (11 rows), six distinct times. With k = 10, d = 6 ≤ k, so there are 6 bins, counts 12 12 12 12 12 11. With k = 4: left edges 0.5, 1, 4, 8, last edge 12, counts 12 24 12 23. With k = 3: cumulative counts C = 12 24 36 48 60 71; for l = 1 the distances |C_j·3 − l·71| are 35 1 37 73 109, so the cut is after j = 2; for l = 2 they are 106 70 34 2 38, so the cut is after j = 4: left edges 0.5, 2, 8, last edge 12, counts 24 24 23.

R's data set `Theoph` (132 rows, 78 distinct times), k = 10: left edges 0, 0.25, 0.52, 1.02, 2.02, 3.53, 5.05, 7.08, 9.22, 12.15, last edge 24.65, counts 12 15 14 10 14 14 14 13 13 13. `x_mean` per bin: 0, 0.32533333333333331, 0.73214285714285732, 1.33500000000000019, 2.55928571428571416, 4.30857142857142872, 6.32857142857142829, 8.57692307692307665, 11.60153846153845869, 23.27230769230768814. `x_middle`: 0.125, 0.385, 0.770, 1.520, 2.775, 4.290, 6.065, 8.150, 10.685, 18.400.

Boundaries: a value on an edge goes to the bin on its right and the maximum goes to bin 10; with the default of `findInterval` (`rightmost.closed = FALSE`) the maximum would land in bin 11, which is why VPC-BIN-03 names the argument. User edges 0, 0.5, 1, 1.5, 3, 6, 12 on Theoph: 0 rows below the first edge, 20 rows above the last, counts 24 14 10 12 24 28.

### VX2 The exact part (`vx2_band.R`, H-VX2)
Toy `pk1.iv_bolus`, dose 100, cl = 5, v = 50, Ω = diag(0.09, 0.04), proportional error b = 0.1, 6 subjects at times 1, 2, 4 (observed: seed 11), replicates seed 22, R = 40, band probabilities 0.025 and 0.975, bins `nominal_time`. All 9 observed statistics lie inside the band. Examples: t = 1, p = 0.5: observed 1.803557, band 1.5861768 to 2.113200, simulated median 1.839527; t = 4, p = 0.95: observed 1.735615, band 1.4445083 to 1.967059. Band index: h_lo = 1.975000000000001 (lo = 1) and h_hi = 39.024999999999999 (lo = 39); the lower limit at t = 1, p = 0.5 is 1.5861768244232446. The three smallest replicate medians at t = 1 are 1.554195126, 1.586996868, 1.594470927 and the three largest 2.072543470, 2.111497333, 2.179609348. The six observed values at t = 1, sorted: 1.217889, 1.616782, 1.781081, 1.826032, 2.009830, 2.250520. At all 27 checks the result is `identical()` to R's `quantile(type = 7)`.

### VX3 Prediction correction (`vx3_pc.R`, H-VX3)
Five rows in one bin with doses 50, 100, 100, 200, 200 at times 3.8, 4.0, 4.0, 4.2, 4.0: PRED = 0.68386141, 1.34064009, 1.34064009, 2.62818728, 2.68128018; PREDbin (median) = 1.3406400920712787; corrected values 1.35125137, 1.22431247, 1.95675033, 1.37494657, 0.62479803. The ratio sd/mean falls from 0.496981 (raw) to 0.363286 (corrected). With lb = 0 the bits equal y·(PREDbin/PRED). Lower-bound case, lb = −1: PRED = −0.2, 0.1, 0.4, 0.5, 0.7, PREDbin = 0.4, ratios (PREDbin − lb)/(PRED − lb) = 1.75, 1.27272727, 1.0, 0.93333333, 0.82352941, corrected values 0.1375, 0.52727273, 0.30000000, 0.77333333, 0.31764706; a value at the floor stays at −1; PRED = lb gives an infinite ratio and is refused. Ratio first against product first: 34837 of 10⁵ random cases differ.

### VX4 BLQ (`vx4_blq.R`, H-VX4)
8 subjects, times 1, 2, 4, 8, 12, LLOQ = 0.7, R = 100, observed seed 101, replicates seed 202. Observed BLQ counts per time: 0 0 0 2 7 (of 8), fractions 0 0 0 0.25 0.875. Simulated fraction band (lo, median, hi): at t = 8, 0, 0.25, 0.5; at t = 12, 0.375, 0.75, 0.875; earlier times 0. The observed fraction is inside the band everywhere. Under `omit` at t = 12: one value remains (n_obs = 1), observed 0.81178, 98 replicates defined. Under `censored_rank`: at t = 8, p = 0.05, the observed quantile is below the LLOQ, the lower end of the band is below the LLOQ and 85 replicates are below; at t = 12, p = 0.5, the observed quantile is below the LLOQ and 96 of 100 replicates are below. Hand check: n = 8, 7 censored, h = 1 + 7·0.5 = 4.5, lo = 4 ≤ 7, so not a number.

### VX5 Calibration (`vx5_calibration.R`, H-VX5)
One bin (t = 4), `pk1.iv_bolus` as VX2, m = 8 subjects, R = 1000, K = 400 independent datasets (seed 7001), one check per dataset and probability. ρ = 999·0.95/1001 = 0.948102. Binomial bounds at 5·10⁻⁷ per tail: 354 and 397 (expected 379.241). Counts inside under the true model at p = 0.05, 0.5, 0.95: 376, 375, 381, all inside the bounds (46 s). Misspecified (observed CL = 7.5, simulated 5; seed 7002): 188, 137, 218 of 400, far below 354. The three probabilities use the same datasets, so the three counts are not independent of each other.

### VX6 Refusal arithmetic and sums (`vx6_refusals.R`, H-VX6)
1 − 0.9 = 0.099999999999999978. 2/(1 − c) and its ceiling: c = 0.95, 39.999999999999964, 40; c = 0.90, 20.000000000000004, 21 (wrong, the answer is 20); c = 0.99, 199.99999999999983, 200; c = 0.80, 10.000000000000002, 11 (wrong, the answer is 10). With the test R·(1 − c) ≥ 2·(1 − 10⁻¹²) the smallest R is 40, 20, 200 and 10. `mean()` differs from the double loop in 897 of 2000 vectors of 7 values rounded to two decimals; the first is 13.45, 18.14, 9.1, 8.96, 4.09, 10.88, 6.2, with `mean()` 10.117142857142857 and loop 10.117142857142856. `median` equals the type-7 quantile at 0.5 in 2000 of 2000 vectors.

## 13. Open items for Abdou

Each has a proposal.
- **OV-01 Parameter uncertainty (VPC-SIM-05, OS-09).** Proposal: not in v1; a later card draws from the covariance with the purpose p = 3.
- **OV-02 Default binning and k (VPC-BIN-02).** Proposal: `equal_count`, k = 10. Alternative: a clustering method with an automatic k as the `vpc` package, which needs a written algorithm.
- **OV-03 `min_obs` (VPC-BIN-05).** Proposal: 3, arbitrary. Alternatives: 5, or no flag.
- **OV-04 Defaults R, π, c (VPC-SIM-06, VPC-STAT-04).** Proposal: 1000, (0.05, 0.5, 0.95), 0.95. Alternative: 0.025/0.975 for large data.
- **OV-05 BLQ default (VPC-BLQ-02).** Proposal: `omit`. Alternative: `censored_rank` as the default, shown with the BLQ fraction.
- **OV-06 One value per subject (VPC-STAT-06).** Proposal: not offered; add `per_subject` later if a source states it.
- **OV-07 A limit per row (VPC-BLQ-01).** Proposal: one LLOQ per run, as OS-07.
- **OV-08 Rows with PRED − lb ≤ 0 (VPC-PC-05).** Proposal: refuse. Alternative: drop them and count.
- **OV-09 Variability correction and the log form (VPC-PC-03).** Proposal: `later` for both; the variability correction needs the paper's equation, not read.
- **OV-10 Two stratifiers, continuous covariates (VPC-STR-02).** Proposal: one categorical variable now; two later; a continuous covariate only after the user bins it.
- **OV-11 The calibration false-alarm level (VPC-ORA-02).** Proposal: 5·10⁻⁷ per tail.
- **OV-12 Size limits (VPC-REF-01).** Proposal: symbols `MAX_BINS`, `MAX_STRATA` and the table limits of OS-04; values set from what the oracle measures (as OS-04, OS-05).
- **OV-13 Same dataset for the fit and the VPC.** Proposal: the VPC takes the dataset digest of the fit and refuses a different one (`dataset_changed`) unless an option allows it.
- **OV-14 Other x and a nominal-time column (VPC-BIN-07, -09).** Proposal: time after dose only in v1; add a nominal-time column to `specs/data.md` in its own card.
- **OV-15 Interface (T-156).** Proposal: the interface draws the tables of section 7 and shows the header; it offers `prediction_correction` and `blq_quantiles` as visible switches.
- **OV-16 Bin x (VPC-BIN-04).** Proposal: `mean`. Alternative: `middle`.
