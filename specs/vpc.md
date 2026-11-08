# Visual predictive check: simulate the design, bin, compare quantiles (VPC-*, card T-160)

Brick 4 of iteration 3 (T-139). `specs/simulation.md` simulates a population model and `specs/population.md` fits one; this file specifies the check that sets them side by side: **simulate R virtual studies of the observed design from the estimates of a fit, bin the independent variable, and in each bin compare the quantiles of the observed values with the band that the replicates give for the same quantiles.** Only numbers are specified (tables, section 7); no drawing. Written by the `reader` agent on 2026-11-08, in its own words, from the open sources of section 11 and from derivations (D). **Nothing in this file is implemented or exercised by an oracle.** Every rule is `assumed` (the sources are silent or disagree, and the reason is given) or `documented, untested` (a source states it and the arithmetic was checked in base R). The numbers of the worked examples (section 12) come from base R 4.5.2 scripts, seeded, kept outside the repository in the session scratchpad and named next to each example (`vx1_binning.R` to `vx7_edges.R`, helpers in `vpclib.R`). They are hand checks for the future oracle, not results of the engine.

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

### VPC-SIM-02 Design rows: simulate every row, then drop
**The simulator is given every observation row of the dataset** (SIM-MOD-04 item 3: the j-th observation row of a subject uses the word j − 1 of its ε stream, missing, BLQ and before-dose rows included), and rows are **dropped after the simulation**, so the ε positions never shift and row j of replicate r stays the counterpart of observed row j. Rows with a missing value (`DAT-CEN-*`) and rows before the dose (`before_dose`, POP-DAT-01) are then **dropped from the VPC** and counted (`n_missing`, `n_before_dose` in the header, VPC-OUT-06). **BLQ rows are kept**: they are observations, they count in the BLQ fraction (VPC-BLQ-03) and, depending on the option of VPC-BLQ-02, in the quantiles. Reason: the fit leaves BLQ rows out (POP-DAT-02) because it has no censored likelihood; a check of the data must not forget them, and the simulator already knows how to produce them (SIM-ORD-05). The rows dropped by the prediction correction (VPC-PC-05) are dropped the same way, after the simulation and before the binning.
- Status: `assumed` (the sources do not say which rows form the design; the reason is the one above)
- Sources: D; SIM-MOD-04 item 3; POP-DAT-01, -02; DAT-CEN-02; Bergstrand and Karlsson 2009 (the BLQ data are part of the check).

### VPC-SIM-03 Subjects
The subjects are **the subjects of the dataset, minus the user's exclusions** (the option `exclude_subjects` of POP-DAT-01, with its refusal for a label that is not in the dataset; a subject that carries a refusal of the dataset makes the run refuse, as POP-DAT-01 says). The fit counts a subject with n_i = 0 (all rows missing or BLQ) out of m; the VPC does not: such a subject **stays in** and contributes its BLQ rows to the BLQ fraction. A subject left with no observation row (all missing or before the dose) contributes nothing and is counted in the header (`n_subjects_no_rows`). The dataset is the one the fit used (OV-13).
- Status: `assumed` (D: the check is of the fitted model on the data it was fitted to; the BLQ rows are the reason for keeping the subjects the fit counts out)
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

Equal x values are never split between bins. **The valid range of k is the integers k ≥ 1** (k < 1 or not an integer is `invalid_option`, VPC-REF-01). k = 1 gives one bin from the smallest to the largest x (no cut); k larger than d gives d bins; **a single distinct x gives one bin with left = right = that x, closed on both sides**, and every row falls in it (H-VX7).
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
For each (bin, p), the band is the type-7 quantile **across the R replicate-wise statistics** at a = (1 − c)/2 and 1 − a, and at 0.5 (the simulated median): columns `sim_lo`, `sim_med`, `sim_hi`. A replicate for which the statistic is not defined because the bin has no value in that replicate (an empty bin) is left out of the quantile and not counted in `n_rep_defined`; if fewer than 2 replicates are defined the band is not computed (flag `band_undefined`). **Exception under `censored_rank`**: a replicate whose statistic falls on a censored rank is not left out; it takes the lowest ranks of the band (VPC-BLQ-02), so the band can end below the LLOQ.
- Status: `documented, untested`
- Sources: Bergstrand et al. 2011 (Methods, VPC: the 95% confidence interval from the 2.5 and 97.5 percentiles of the replicate-wise statistics); Karlsson and Holford 2008, slide 12; H-VX2.

### VPC-STAT-04 Defaults: π and the level
π = (0.05, 0.5, 0.95); c = 0.95. Reason: the median with the 5% and 95% percentiles is the interval of the sources (the 2.5/97.5 pair is the alternative for large data sets), and the 95% band is the one of both papers. The `vpc` help uses (0.05, 0.95) both for the interval and for the band, and always draws the median.
- Status: `documented, untested` for the values, `assumed` for the choice of 0.05/0.95 over 0.025/0.975
- Sources: Bergstrand et al. 2011 (Methods, VPC); Karlsson and Holford 2008, slide 3; `vpc` help (`pi`, `ci`); `saemix` help (`vpc.interval` = 0.95).

### VPC-STAT-05 R must allow the band level
The band at level c is computed for any R ≥ 2 by the type-7 formula; when R·a < 1 the band's end is the smallest or largest replicate and says little about the tail a. **The floor R·(1 − c) ≥ 2 (R·a ≥ 1: at least one replicate's worth of probability in each tail) is this file's own choice**, not a source's. The test is **R·(1 − c) ≥ 2·(1 − 10⁻¹²)**, not `ceil(2/(1 − c))`, because in double 2/(1 − 0.9) is 20.000000000000004 and the ceiling gives 21 where 20 is right (H-VX6). With c = 0.95 the smallest R is 40. A call below it is refused (`too_few_replicates`, VPC-REF-01).
- Status: `assumed` (D: the floor is this file's own)
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

where PRED_j is the typical prediction of row j (all η = 0, POP-DIA-01) and PREDbin a typical prediction that stands for the bin (Bergstrand et al. 2011, Eq. 1). The correction is applied to the observed values and to every replicate, with the same PREDbin. The default is `none`.

**Which PREDbin: the sources differ, and this file's choice is marked.** The abstract of Bergstrand et al. 2011 (AAPS J 13(2):143-151, p. 143) says the correction uses "the typical population prediction for the median independent variable in the bin", that is PRED evaluated at the median x of the bin. The Methods, read through a fetch summary in this card and **not re-read verbatim** (the full text was behind a verification page when this was corrected, so no Methods sentence is quoted), describe PREDbin as the median of the typical predictions of the bin; Karlsson and Holford 2008 (slide 36) write an average, PREDav,bin. The three definitions agree when the bin holds one design (one x, one dose, one covariate set) and differ otherwise. **This file takes the median (type 7) of the PRED_j of the rows of the bin** (VPC-PC-04), because it is a function of the rows already in the table and needs no model evaluation at a new x; the alternative `pred_at_median_x` (the model evaluated at the median x of the bin, for the dose and covariates of which subject is not given by any source) and the mean are not offered, and the difference is an open item (OV-17).
- Status: `assumed` (the formula is Eq. 1 as read from the paper's image; the definition of PREDbin is the choice just described)
- Sources: Bergstrand et al. 2011 (abstract, p. 143; Methods, pcVPC, Eq. 1, read as an image); Karlsson and Holford 2008, slide 36 (the form OBS·PREDav,bin/PRED, which is lb = 0); H-VX3.

### VPC-PC-02 A named option that changes the data
The correction changes every value; it is **never applied silently**. The header of every table says `prediction_correction: none | pc`, and the table `vpc_obs` carries both `y` and `pcy`.
- Status: `assumed` (the card's requirement; the sources describe the correction as a choice)
- Sources: D; Bergstrand et al. 2011.

### VPC-PC-03 The lower bound
lb is the lower bound of the dependent variable, taken from the equation as the paper writes it: default 0 (a concentration), an option `lb` (finite) for another (for example a response with a floor of −1). A value at the floor stays at the floor, because Y − lb = 0. The paper's log form (Eq. 2: ln pcY = ln Y + (ln PREDbin − ln PRED)) is for a log-transformed DV and is **not offered** (`later`, OV-09).
- Status: `documented, untested` for the formula, `assumed` for the default lb = 0
- Sources: Bergstrand et al. 2011 (Eq. 1 and Eq. 2); `vpc` help (`pred_corr_lower_bnd`, default 0); H-VX3.

### VPC-PC-04 Which rows give PREDbin
PREDbin is the median (type 7) of PRED_j over **the design rows of the bin and stratum that are still in the VPC, BLQ rows included** (the typical prediction exists for every row; leaving BLQ rows out would make PREDbin depend on the data's censoring). Rows dropped by VPC-PC-05 are not in the bin. The PRED_j come from the same estimates as the replicates.
- Status: `assumed` (no source says; the reason is the one above)
- Sources: D; Bergstrand et al. 2011.

### VPC-PC-05 A non-positive denominator: the row is dropped and counted
A row with PRED_j − lb ≤ 0 cannot be corrected (the ratio is infinite or negative). **This is routine, not an error**: an oral model has PRED(0) = 0, and an observation at the dose time is a valid row (DAT-ORD-01) that is not `before_dose`; with lb = 0 its denominator is exactly 0 (H-VX7). Such a row is **dropped from the whole VPC**, observed and simulated, after the simulation and before the binning (VPC-SIM-02), and counted: `n_pred_dropped` in the header (VPC-OUT-06), `excluded = non_positive_pred` in `vpc_obs` (VPC-OUT-05); it is 0 without the correction. The call is **refused** (`non_positive_pred`, naming the first row and its value) only when **no row is left**. A user who prefers a refusal at the first such row is offered in OV-08.
- Status: `assumed`
- Sources: D; DAT-ORD-01; H-VX3 (PRED = lb gives an infinite ratio), H-VX7 (the oral case).

### VPC-PC-06 Order of evaluation
The correction is evaluated ratio first: t = (PREDbin − lb)/(PRED_j − lb); pcY = lb + ((Y − lb)·t) (VPC-DET-03).
- Status: `assumed` (the order is fixed for bit equality)
- Sources: H-VX3.

### VPC-PC-07 Correction and BLQ
pcVPC with `blq_quantiles = censored_rank` is refused (`pc_with_censoring`): the correction moves the values and the LLOQ would have to move with them, and the source says the rank order is lost. With `omit` the BLQ rows are left out of the quantiles before the correction, as the paper advises.
- Status: `documented, untested` for the use of `omit`, `assumed` for the refusal
- Sources: Bergstrand et al. 2011 (Discussion: censored observations are omitted from the observed and the simulated data in a pcVPC; the rank order is lost after correction).

## 5. BLQ and censoring (card item 5)

### VPC-BLQ-01 The one LLOQ, and which observed rows are BLQ
The run has **one LLOQ** (SIM-ORD-05, OS-07). It comes from the option `lloq` or, if absent, from the `limit` of the BLQ rows of the dataset (DAT-CEN-02). Several different limits in the data are refused (`several_limits`) unless the option `lloq` is given; BLQ rows with no limit and no option are refused (`blq_without_limit`). The simulated value is BLQ if and only if y < LLOQ, strictly (SIM-ORD-05).
**The observed flag is defined by the same test.** An observed row is BLQ when it is flagged censored in the dataset (DAT-CEN-02) **or** its value is a number below the LLOQ in force; with the LLOQ of the data (no option, or an option equal to it) the two coincide for a coherent dataset. When the option differs from the data's limit, the observed rows are **re-flagged by y < LLOQ(option)**, as the simulated ones are, so the two fractions of VPC-BLQ-03 measure the same thing; a censored row (no number, only the limit) stays BLQ if its limit is at most the option and is refused (`blq_limit_above_lloq`) if its limit is above it, because its value is then not known to be below the option. The header says where the LLOQ came from and how many observed rows the option re-flagged (`n_reflagged`). Whether re-flagging is what a user wants is OV-18.
- Status: `assumed`
- Sources: SIM-ORD-05; DAT-CEN-02; Bergstrand and Karlsson 2009 (a single quantification limit in the example); D.

### VPC-BLQ-02 How BLQ rows enter the quantiles
The option `blq_quantiles` has two values.
- **`omit`** (default): BLQ rows, observed and simulated, are left out of the quantiles of VPC-STAT-01 and -02. The count of the values that remain is `n_obs`; the BLQ rows are counted in `n_blq`.
- **`censored_rank`**: BLQ rows stay as values known to lie below the LLOQ. In a bin with n values of which nc are censored, the ordered values are the nc censored ranks followed by the uncensored ones; with h = 1 + (n − 1)p, lo = ⌊h⌋, hi = ⌈h⌉, g = h − lo: if lo ≤ nc the quantile is **below the LLOQ** (not a number, `obs_below` or, for a replicate, a replicate below), otherwise it is (1 − g)·x_(lo−nc) + g·x_(hi−nc) of the uncensored values. The observed statistic and every replicate statistic use this rule. **The band uses the same function across replicates**: the replicates whose statistic is below the LLOQ are the n_below lowest ranks of the band, the replicates with a number are the others, and the band end at a, 0.5 or 1 − a is computed with nc = n_below, so a band end is **below the LLOQ** (`lo_below`, `med_below`, `hi_below`) when its rank lo ≤ n_below. Only a replicate with no value at all in the bin (an empty bin) is left out. `n_rep_defined` is the number of replicates that are not empty, `n_rep_below` the number below the LLOQ.

Reasons for `omit` as the default: it is what both papers do for a pcVPC and it is the simplest; `censored_rank` shows what happens in the censored region without inventing values. Imputing a number for a BLQ row (LLOQ/2 or the like) is not offered.
- Status: `omit`: `documented, untested`; `censored_rank`: `assumed` (the papers show a panel of percentiles of the data and a panel of the BLQ fraction, and the omitting option for the corrected case, but give no formula for a quantile in the censored region)
- Sources: Bergstrand and Karlsson 2009 (Methods, Visual Predictive Check; Results, Figures 4 and 5); Bergstrand et al. 2011 (Discussion); Karlsson and Holford 2008, slide 29 (show only uncensored data, replace censored values, or check the censoring separately); H-VX4.

### VPC-BLQ-03 The fraction below the LLOQ
For each (stratum, bin) the observed fraction is **n_blq / n_rows**, over the rows of the bin that are in the VPC (VPC-SIM-02: missing, before-dose and prediction-dropped rows already out, BLQ rows in), with the observed flag of VPC-BLQ-01. For each replicate the same fraction from the simulated BLQ flags; the band is the quantile across replicates at a, 0.5 and 1 − a (columns `sim_lo`, `sim_med`, `sim_hi`). The fraction is computed whatever the option of VPC-BLQ-02. When there is no LLOQ (VPC-BLQ-01), the table is not produced.
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
An empty stratum (a level with no used subject), a missing covariate value, an unknown stratifier column, and more levels than the limit `MAX_STRATA` (the value is OV-12) are refused (VPC-REF-01). A covariate that varies within a subject is **already refused by the dataset** (`covariate_varies`, DAT-ERR-05, DAT-COV-01) before the VPC runs; the VPC has no second check of it.
- Status: `assumed`
- Sources: D; DAT-ERR-05; DAT-COV-01.

## 7. The outputs (card item 7)

### VPC-OUT-01 `vpc_bins`
One row per (stratum, bin): `stratum`, `bin`, `left`, `right`, `closed_right` (true for the last bin), `x` (VPC-BIN-04), `x_mean`, `x_middle`, `n_rows`, `n_obs`, `n_blq`, `n_subjects`, `status` (`ok`, `sparse`, `empty`), `pred_bin` (the median PRED of the bin; present whatever the option, because it costs nothing). Without a stratifier the stratum is the text `all`.
- Status: `assumed`
- Sources: D.

### VPC-OUT-02 `vpc_quantiles`
One row per (stratum, bin, p): `stratum`, `bin`, `p`, `obs`, `sim_lo`, `sim_med`, `sim_hi`, `n_rep_defined`, `n_rep_below` (zero under `omit`), `obs_below`, `lo_below`, `med_below`, `hi_below` (booleans, true when the observed statistic, the lower band end, the median or the upper band end is below the LLOQ under `censored_rank`, and then the matching number is not a number; always false under `omit`), `flag` (`ok`, `sparse`, `empty`, `band_undefined`; the first that applies in the order empty, band_undefined, sparse), `inside` (true if sim_lo ≤ obs ≤ sim_hi with all three numbers; absent otherwise). The four booleans are needed because the observed statistic and each band end can be below the LLOQ independently (H-VX4: observed below, the upper end a number).
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
One row per observation row of the dataset (subjects of VPC-SIM-03), in dataset order: `subject`, `row` (1-based, in the dataset order of the subject's observation rows), `stratum`, `bin` (absent when excluded), `x`, `y`, `pcy` (absent without correction or when excluded), `blq`, `excluded` (`none`, `missing`, `before_dose`, `non_positive_pred`, `outside_bins`).
- Status: `assumed`
- Sources: D.

### VPC-OUT-06 The header
The header echoes the inputs that decide the numbers, in the order of VPC-DET-04: the seed, R, the `rng` version string of SIM-DET-03, π, c, `bins` and its parameters, `bin_x`, `min_obs`, `prediction_correction`, `lb`, `lloq` and where it came from, `blq_quantiles`, the stratifier, `parameter_uncertainty: none`, the flags (`unconverged_fit`) and the counts `n_subjects`, `n_subjects_no_rows`, `n_rows`, `n_missing`, `n_before_dose`, `n_pred_dropped`, `n_outside_bins`, `n_blq`, `n_reflagged`.
- Status: `assumed`
- Sources: D; SIM-DET-03.

### VPC-OUT-07 The replicate-wise statistics
An option (`keep_replicate_stats`, off by default) adds `vpc_replicate_stats`: (replicate, stratum, bin, p, value, below). It is what the calibration check and a port need to compare; it is large (R × bins × π) and bounded by the size limits. It is not part of the canonical text.
- Status: `assumed`
- Sources: D.

## 8. Determinism (card item 8)

### VPC-DET-01 A pure function
The result is a pure function of (θ̂, Ω̂, ξ̂, the design, the observed values, the seed, R and the options). It uses no clock, no global state and **no random number of its own**; all randomness is the simulator's (SIM-DET-01).
- Status: `assumed` (its only source, SIM-DET-01, is `assumed`)
- Sources: SIM-DET-01.

### VPC-DET-02 Threads and assembly
The statistics of replicate r depend on replicate r alone, so replicates may be evaluated in any order on any number of threads and in WebAssembly; the library then assembles the tables **in index order** (replicate, then stratum, bin, p). The band sorts the R values (a total order on finite doubles), so it does not depend on the order of arrival.
- Status: `assumed` (SIM-DET-01 to -03 are `assumed`)
- Sources: SIM-DET-01 to -03.

### VPC-DET-03 The sums and the order of operations
The only sums are the bin `x_mean` (and the counts, which are integers and associative). `x_mean` is accumulated **sequentially in plain double in dataset order**, then divided by the count; this is a decision of Caladrius. The measurement supports its reason: R's `sum()` and `mean()` differ from the double loop in the last digit in 897 of 2000 random vectors of 7 values on the machine used (H-VX6), so the oracle must use an explicit loop. The correction is ratio first (VPC-PC-06): the product-first form differs in 34837 of 10⁵ random cases (H-VX3). The quantiles use the form of SIM-SUM-01, and R's `median` equals the type-7 quantile at 0.5 (no difference in 2000 vectors, H-VX6).
- Status: `assumed`
- Sources: SIM-SUM-01; H-VX3, H-VX6.

### VPC-DET-04 The canonical text
The canonical text of a VPC run is the canonical text of SIM-DET-04 of the embedded simulation, **all six of its parts, the `summary` section included**, followed by the parts below. The embedded simulation is run with the VPC's R, seed and `lloq`, and its `quantiles` option is π (its `negative` option is `keep`, the default of SIM-DET-04 part 2), so the `summary` section is the one of SIM-SUM-01 at those probabilities. Spelling as in SIM-DET-04 (one line feed after every line, no carriage return, doubles as Rust `{:?}`, integers in decimal); fields are separated by `|` as in SIM-DET-04 part 4, lists are joined by `,` with no space.
7. The line `vpc`, then one line `key=value` per header key **in this exact order**: `seed`, `replicates`, `rng`, `pi`, `level`, `bins`, `k`, `edges`, `bin_x`, `min_obs`, `prediction_correction`, `lb`, `lloq`, `lloq_source`, `blq_quantiles`, `stratifier`, `parameter_uncertainty`, `flags`, `n_subjects`, `n_subjects_no_rows`, `n_rows`, `n_missing`, `n_before_dose`, `n_pred_dropped`, `n_outside_bins`, `n_blq`, `n_reflagged`. A value that does not apply is `none` (`k` for `nominal_time`, `edges` unless `edges`, `lloq`, `stratifier`); `flags` is the flags sorted in ascending byte order, joined by `,`, or `none`; `lloq_source` is `option`, `data` or `none`.
8. The line `bins`, then one line per row of `vpc_bins`: `stratum|bin|left|right|closed_right|x|x_mean|x_middle|n_rows|n_obs|n_blq|n_subjects|status|pred_bin`.
9. The line `quantiles`, then one line per row of `vpc_quantiles`: `stratum|bin|p|obs|sim_lo|sim_med|sim_hi|n_rep_defined|n_rep_below|obs_below|lo_below|med_below|hi_below|flag|inside`.
10. The line `blq`, then one line per row of `vpc_blq` (no line when there is no LLOQ): `stratum|bin|n_rows|n_blq|obs_fraction|sim_lo|sim_med|sim_hi|inside`.
11. The line `obs`, then one line per row of `vpc_obs`: `subject|row|stratum|bin|x|y|pcy|blq|excluded`.

Booleans are `true` and `false`; `status`, `flag`, `excluded` and `prediction_correction` are the lower-case words of this file; **a number or a boolean that is absent or not a number is written `na`** (SIM-DET-04 says every number is finite; `na` is the only addition). Rows are in the order: stratum levels ascending in byte order of their text, then bin ascending, then p ascending; `obs` rows in dataset order. A VPC run is added to the workload whose digest SIM-DET-02 pins; the digest of the simulation alone does not change, because parts 7 to 11 come after part 6.
- Status: `assumed`
- Sources: SIM-DET-02 to -04.

## 9. What can be tested, and how (card item 9)

### VPC-ORA-01 The exact part
Given a simulated table and the observed table, binning (VPC-BIN-*) and the quantiles (VPC-STAT-*) are a deterministic function: base R `findInterval` and `quantile(type = 7)` (no package) reproduce them **bit for bit**. In H-VX2, 27 of 27 checks (9 replicate-wise vectors, 9 lower and 9 upper band limits) are `identical()` to R's `quantile`. The oracle takes the simulated table from the engine and recomputes the bins, the observed quantiles, the replicate quantiles and the band.
- Status: `documented, untested`
- Sources: H-VX2; R help of `quantile` and `findInterval`.

### VPC-ORA-02 Calibration under the true model
If the observed data are one draw of the model that is simulated, the observed quantile falls inside the band at level c with probability about **(R − 1)(1 − α)/(R + 1)** with α = 1 − c (0.948102 at R = 1000, α = 0.05; D, from the rank of one draw among R exchangeable draws, up to the interpolation of the quantiles). **The design**: `pk1.iv_bolus`, dose 100, cl = 5, v = 50, Ω = diag(0.09, 0.04), proportional error b = 0.1, m = 8 subjects, one bin at t = 4, R = 1000, c = 0.95, K = 400 datasets (H-VX5). The check: K datasets, one (bin, p) per dataset, count those inside, and require the count to lie within the exact binomial bounds `qbinom(ε, K, ρ)` and `qbinom(1 − ε, K, ρ)` (ρ as above; 354 and 397) at a false-alarm level ε = 5·10⁻⁷ per tail. **The observed dataset of each check and its R replicates use different seeds** (2K distinct seeds in the engine oracle; in the R script one seeded stream gives the observed data and then the replicates as successive draws, never the same numbers), so the K checks are independent. At a fixed seed the result is repeatable, so a failure is a defect, not bad luck. The **power** check is the same count with the observed data generated at cl = 7.5 and the replicates at cl = 5; **the count must be below the lower bound 354** at each of p = 0.05, 0.5, 0.95 (H-VX5: 188, 137, 218).
- Status: `assumed` (D, with the interpolation approximation)
- Sources: D; H-VX5; Karlsson and Holford 2008, slides 37 to 39 (false-positive and false-negative rates).

### VPC-ORA-03 The correction
The correction is checked on a worked case (H-VX3) against the formula, bit for bit with the order of VPC-PC-06. In that case the spread of the corrected values is smaller than the raw spread (sd/mean 0.496981 to 0.363286); that is a property of the case, not a general one, and is not an assertion of the oracle.
- Status: `documented, untested`
- Sources: Bergstrand et al. 2011; H-VX3.

### VPC-ORA-04 Refusals
Each code of VPC-REF-01 has one case with the expected code, **in the order of the table** (so a case with two causes names the code of the earlier row). The list: (1) no seed; (2) one case per option error: R = 0, level 1.2, π with a duplicate, π empty, `bins` unknown, k = 0, `bin_x` unknown, `min_obs` = −1, `lb` = NaN, a stratifier column that is not in the dataset; (3) `censored_rank` with the correction; (4) R = 39 at c = 0.95 (refused) and R = 40 (accepted), R = 19 and 20 at c = 0.9; (5) a fit that did not converge, with and without `allow_unconverged_fit`; (6) BLQ rows with no limit and no option, two different limits, a limit above the option; (7) a stratum with no subject, a missing covariate value, more strata than `MAX_STRATA`; (8) a size above the limits; (9) duplicate edges; (10) the correction when no row is left; (11) `nominal_time` on measured times and `edges` outside every row; (12) **the accepted case**: the oral design of H-VX7 with a sample at the dose time and `prediction_correction = pc`, expected to run with `n_pred_dropped` = 6 and no refusal; (13) two causes at once (R = 39 and duplicate edges), which must report `too_few_replicates`.
- Status: `assumed`
- Sources: D; H-VX6, H-VX7.

### VPC-ORA-05 Invariance
(a) A stratum run alone equals the same stratum inside a larger run (VPC-STR-01). (b) The digest is the same on 1 thread and N threads and in WebAssembly (VPC-DET-02). (c) Adding a subject outside a stratum changes none of its numbers.
- Status: `assumed` (SIM-DET-01 to -03 are `assumed`)
- Sources: SIM-DET-01 to -03.

## 10. Refusals (card item 10)

### VPC-REF-01 The refusals and their order
Each refusal is an error value with a code and a sentence in the user's words, never a crash (golden rule 6), and names the value that caused it. **When a call has several causes, the one reported is the first in the column `order`, and within an order the first row of the table.**

| order | code | when |
|---|---|---|
| 1 | `missing_seed` | no seed (SIM-RNG-02) |
| 2 | `invalid_option` | names the option: R not an integer ≥ 1; level outside (0, 1); π empty, with a duplicate or a value outside (0, 1); `bins` not one of the three; k not an integer ≥ 1 (VPC-BIN-06); `bin_x` not `mean` or `middle`; `min_obs` not an integer ≥ 0; `lb` not finite; `prediction_correction` or `blq_quantiles` unknown; `lloq` not finite and positive; a stratifier column that is not in the dataset; an `exclude_subjects` label that is not in the dataset (POP-DAT-01). Checked in this order |
| 2 | `pc_with_censoring` | VPC-PC-07 |
| 2 | `too_few_replicates` | R·(1 − c) < 2·(1 − 10⁻¹²) (VPC-STAT-05) |
| 3 | `fit_not_converged` | VPC-SIM-04 |
| 4 | `blq_without_limit`, `several_limits`, `blq_limit_above_lloq` | VPC-BLQ-01 |
| 4 | `missing_covariate`, `empty_stratum`, `too_many_strata` | VPC-STR-04 (`MAX_STRATA`, OV-12) |
| 5 | `size_limit` | rows, replicates or table size above the limits (OS-04, OV-12) |
| 6 | `bad_edges` | VPC-BIN-08 |
| 6 | `non_positive_pred` | VPC-PC-05, only when no row is left (rows are dropped before the binning, so this precedes the bin refusals below) |
| 7 | `no_bins` | the rule leaves no bin (no usable row, or `edges` outside every row) |
| 7 | `too_many_bins` | `nominal_time` on measured times, or more bins than `MAX_BINS` (OV-12) |

`covariate_varies` is the dataset's refusal (DAT-ERR-05), raised at import and not repeated here. `dataset_changed` exists only if OV-13 is adopted, at order 3.
- Status: `assumed`
- Sources: D; `AGENTS.md` golden rule 6; SIM-REF-*; SIM-MOD-05 (`InvalidOption`); DAT-ERR-05.

## 11. Sources (clean room)

Allowed sources only (`AGENTS.md` rule 1). **Read:** Holford 2005, PAGE 14 (Abstr 738), poster abstract and text (thin on construction); Karlsson and Holford 2008, PAGE 17 (Abstr 1434), tutorial slides (slide numbers cited); Bergstrand, Hooker, Wallin and Karlsson 2011, AAPS J 13(2):143-151 (abstract; Methods VPC and pcVPC, Eq. 1 and 2 read as images; Discussion; the Methods text on PREDbin was read through a fetch summary and could not be re-read verbatim when corrected); Bergstrand and Karlsson 2009, AAPS J 11(2):371-380 (Methods VPC, Results Figures 4 and 5); R 4.5.2 help of `quantile` and `findInterval`; help pages (not code) of the `vpc` package (MIT), of `saemix` and of nlmixr2plot. **Not read:** the code of any of these packages; the equation of the variability correction (pvcVPC, Bergstrand 2011 Eq. 3, only described in words, so not offered); documentation of any commercial program. The statements of the `saemix` help on `vpc.method` and `vpc.extreme` were not found in the text read.

Keys: the sources are S-68 to S-74 of `specs/sources.md` (section 16); H-VXn is the hand check VXn of section 12; D is a derivation in this file.

## 12. Worked examples (targets for the oracle, base R 4.5.2)

**Common.** Scripts in the scratchpad: `vpclib.R` (type-7 quantile in the form of SIM-SUM-01, equal-count edges, bin index, correction, censored quantile), `vx1_binning.R` to `vx7_edges.R`. Doubles are shown as printed by R.

### VX1 Binning (`vx1_binning.R`, H-VX1)
A fixed grid: 71 rows, times 0.5, 1, 2, 4, 8 (12 rows each) and 12 h (11 rows), six distinct times. With k = 10, d = 6 ≤ k, so there are 6 bins, counts 12 12 12 12 12 11. With k = 4: left edges 0.5, 1, 4, 8, last edge 12, counts 12 24 12 23. With k = 3: cumulative counts C = 12 24 36 48 60 71; for l = 1 the distances |C_j·3 − l·71| are 35 1 37 73 109, so the cut is after j = 2; for l = 2 they are 106 70 34 2 38, so the cut is after j = 4: left edges 0.5, 2, 8, last edge 12, counts 24 24 23.

R's data set `Theoph` (132 rows, 78 distinct times), k = 10: left edges 0, 0.25, 0.52, 1.02, 2.02, 3.53, 5.05, 7.08, 9.22, 12.15, last edge 24.65, counts 12 15 14 10 14 14 14 13 13 13. `x_mean` per bin: 0, 0.32533333333333331, 0.73214285714285732, 1.33500000000000019, 2.55928571428571416, 4.30857142857142872, 6.32857142857142829, 8.57692307692307665, 11.60153846153845869, 23.27230769230768814. `x_middle`: 0.125, 0.385, 0.770, 1.520, 2.775, 4.290, 6.065, 8.150, 10.685, 18.400.

Boundaries: a value on an edge goes to the bin on its right and the maximum goes to bin 10; with the default of `findInterval` (`rightmost.closed = FALSE`) the maximum would land in bin 11, which is why VPC-BIN-03 names the argument. User edges 0, 0.5, 1, 1.5, 3, 6, 12 on Theoph: 0 rows below the first edge, 20 rows above the last, counts 24 14 10 12 24 28.

### VX2 The exact part (`vx2_band.R`, H-VX2)
Toy `pk1.iv_bolus`, dose 100, cl = 5, v = 50, Ω = diag(0.09, 0.04), proportional error b = 0.1, 6 subjects at times 1, 2, 4 (observed: seed 11), replicates seed 22, R = 40, band probabilities 0.025 and 0.975, bins `nominal_time`. All 9 observed statistics lie inside the band. Examples: t = 1, p = 0.5: observed 1.803557, band 1.5861768 to 2.113200, simulated median 1.839527; t = 4, p = 0.95: observed 1.735615, band 1.4445083 to 1.967059. Band index: h_lo = 1.975000000000001 (lo = 1) and h_hi = 39.024999999999999 (lo = 39); the lower limit at t = 1, p = 0.5 is 1.5861768244232446. The three smallest replicate medians at t = 1 are 1.554195126, 1.586996868, 1.594470927 and the three largest 2.072543470, 2.111497333, 2.179609348. The six observed values at t = 1, sorted: 1.217889, 1.616782, 1.781081, 1.826032, 2.009830, 2.250520. At all 27 checks the result is `identical()` to R's `quantile(type = 7)`.

### VX3 Prediction correction (`vx3_pc.R`, H-VX3)
Five rows in one bin with doses 50, 100, 100, 200, 200 at times 3.8, 4.0, 4.0, 4.2, 4.0: PRED = 0.68386141, 1.34064009, 1.34064009, 2.62818728, 2.68128018; PREDbin (median) = 1.3406400920712787; corrected values 1.35125137, 1.22431247, 1.95675033, 1.37494657, 0.62479803. The ratio sd/mean falls from 0.496981 (raw) to 0.363286 (corrected). With lb = 0 the bits equal y·(PREDbin/PRED). Lower-bound case, lb = −1: PRED = −0.2, 0.1, 0.4, 0.5, 0.7, PREDbin = 0.4, ratios (PREDbin − lb)/(PRED − lb) = 1.75, 1.27272727, 1.0, 0.93333333, 0.82352941, corrected values 0.1375, 0.52727273, 0.30000000, 0.77333333, 0.31764706; a value at the floor stays at −1; PRED = lb gives an infinite ratio and is refused. Ratio first against product first: 34837 of 10⁵ random cases differ.

### VX4 BLQ (`vx4_blq.R`, H-VX4)
8 subjects, times 1, 2, 4, 8, 12, LLOQ = 0.7, R = 100, observed seed 101, replicates seed 202. Observed BLQ counts per time: 0 0 0 2 7 (of 8), fractions 0 0 0 0.25 0.875. Simulated fraction band (lo, median, hi): at t = 8, 0, 0.25, 0.5; at t = 12, 0.375, 0.75, 0.875; earlier times 0. The observed fraction is inside the band everywhere. Under `omit` at t = 12: one value remains (n_obs = 1), observed 0.81178, 98 replicates defined. Under `censored_rank`: at t = 8, p = 0.05, the observed quantile is below the LLOQ, the lower end of the band is below the LLOQ (the 85 replicates below are the 85 lowest ranks of the band, and the rank of the lower end, 1 + 99·0.025 = 3.475, floors to 3, which is at most 85; `obs_below` and `lo_below` are true) and 85 replicates are below; at t = 12, p = 0.5, the observed quantile is below the LLOQ and 96 of 100 replicates are below. Hand check: n = 8, 7 censored, h = 1 + 7·0.5 = 4.5, lo = 4 ≤ 7, so not a number.

### VX5 Calibration (`vx5_calibration.R`, H-VX5)
One bin (t = 4), `pk1.iv_bolus` as VX2, m = 8 subjects, R = 1000, K = 400 independent datasets (seed 7001), one check per dataset and probability. ρ = 999·0.95/1001 = 0.948102. Binomial bounds at 5·10⁻⁷ per tail: 354 and 397 (expected 379.241). Counts inside under the true model at p = 0.05, 0.5, 0.95: 376, 375, 381, all inside the bounds (46 s). Misspecified (observed CL = 7.5, simulated 5; seed 7002): 188, 137, 218 of 400, far below 354. The three probabilities use the same datasets, so the three counts are not independent of each other.

### VX6 Refusal arithmetic and sums (`vx6_refusals.R`, H-VX6)
1 − 0.9 = 0.099999999999999978. 2/(1 − c) and its ceiling: c = 0.95, 39.999999999999964, 40; c = 0.90, 20.000000000000004, 21 (wrong, the answer is 20); c = 0.99, 199.99999999999983, 200; c = 0.80, 10.000000000000002, 11 (wrong, the answer is 10). With the test R·(1 − c) ≥ 2·(1 − 10⁻¹²) the smallest R is 40, 20, 200 and 10. `mean()` differs from the double loop in 897 of 2000 vectors of 7 values rounded to two decimals; the first is 13.45, 18.14, 9.1, 8.96, 4.09, 10.88, 6.2, with `mean()` 10.117142857142857 and loop 10.117142857142856. `median` equals the type-7 quantile at 0.5 in 2000 of 2000 vectors.

### VX7 The edge cases of binning, and the oral time-0 row (`vx7_edges.R`, H-VX7)
On the fixed grid of VX1 (71 rows): k = 1 gives one bin, left edge 0.5, last edge 12, count 71. Five rows all at x = 3 with k = 4 give one bin with left = last = 3, every row in bin 1. k = 50 on the 6 distinct times gives 6 bins. Oral one-compartment model, dose 100, ka = 1.5, ke = 0.2, V = 20, six subjects sampled at 0, 1, 2, 4: PRED = 0, 3.4361573, 3.5799979, 2.5779820; PRED(0) − lb = 0 exactly with lb = 0, so 6 of the 24 rows are dropped by VPC-PC-05 (`n_pred_dropped` = 6) and 18 remain; the bin at x = 0 disappears before the binning, leaving 3 bins.

## 13. Open items for Abdou

Each has a proposal.
- **OV-01 Parameter uncertainty (VPC-SIM-05, OS-09).** Proposal: not in v1; a later card draws from the covariance with the purpose p = 3.
- **OV-02 Default binning and k (VPC-BIN-02).** Proposal: `equal_count`, k = 10. Alternative: a clustering method with an automatic k as the `vpc` package, which needs a written algorithm.
- **OV-03 `min_obs` (VPC-BIN-05).** Proposal: 3, arbitrary. Alternatives: 5, or no flag.
- **OV-04 Defaults R, π, c (VPC-SIM-06, VPC-STAT-04).** Proposal: 1000, (0.05, 0.5, 0.95), 0.95. Alternative: 0.025/0.975 for large data.
- **OV-05 BLQ default (VPC-BLQ-02).** Proposal: `omit`. Alternative: `censored_rank` as the default, shown with the BLQ fraction.
- **OV-06 One value per subject (VPC-STAT-06).** Proposal: not offered; add `per_subject` later if a source states it.
- **OV-07 A limit per row (VPC-BLQ-01).** Proposal: one LLOQ per run, as OS-07.
- **OV-08 Rows with PRED − lb ≤ 0 (VPC-PC-05).** Proposal: drop the row and count it (`n_pred_dropped`), refuse only when no row is left, because an oral model has PRED(0) = 0 at a dose-time sample. Alternative: refuse at the first such row.
- **OV-09 Variability correction and the log form (VPC-PC-03).** Proposal: `later` for both; the variability correction needs the paper's equation, not read.
- **OV-10 Two stratifiers, continuous covariates (VPC-STR-02).** Proposal: one categorical variable now; two later; a continuous covariate only after the user bins it.
- **OV-11 The calibration false-alarm level (VPC-ORA-02).** Proposal: 5·10⁻⁷ per tail.
- **OV-12 Size limits (VPC-REF-01).** Proposal: symbols `MAX_BINS`, `MAX_STRATA` and the table limits of OS-04; values set from what the oracle measures (as OS-04, OS-05).
- **OV-13 Same dataset for the fit and the VPC.** Proposal: the VPC takes the dataset digest of the fit and refuses a different one (`dataset_changed`) unless an option allows it.
- **OV-14 Other x and a nominal-time column (VPC-BIN-07, -09).** Proposal: time after dose only in v1; add a nominal-time column to `specs/data.md` in its own card.
- **OV-15 Interface (T-156).** Proposal: the interface draws the tables of section 7 and shows the header; it offers `prediction_correction` and `blq_quantiles` as visible switches.
- **OV-16 Bin x (VPC-BIN-04).** Proposal: `mean`. Alternative: `middle`.
- **OV-17 Definition of PREDbin (VPC-PC-01).** Proposal: the median (type 7) of the PRED_j of the rows of the bin. Alternatives: PRED evaluated at the median x of the bin, as the abstract of Bergstrand et al. 2011 reads, which needs a rule for the dose and the covariates of that virtual row; the mean, as the slides of Karlsson and Holford 2008 write. Abdou to decide, after the Methods of the paper are re-read by someone who can reach the full text.
- **OV-18 Observed BLQ flag when the option `lloq` differs from the data (VPC-BLQ-01).** Proposal: re-flag the observed rows by y < LLOQ(option), as the simulated ones are, and count them (`n_reflagged`). Alternative: use the dataset's `cens` flag only and refuse an option that differs from the data's limit.
