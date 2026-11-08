# Caladrius population-simulation oracle, card T-158 item 7 (SIM-ORA, SIM-REF-01..06, SX6): REFUSALS as expected-error rows.
#
# Every row is a request that the simulator must refuse before any output is produced (a run never returns a
# partial table), with the code and the words the message must contain; most rows carry a `control`, the same
# request with the one change that removes the cause, which must NOT be refused. Rows that the spec fixes by
# a sentence say so in `words_basis` ("spec_sentence"); rows where the spec fixes the option to be named but
# not the sentence have the smallest words that name it ("rule_names_option"); the rest are "inferred" and a
# question for Abdou in the report. Rows that cannot be written as a request are `constructible: false`.
#
# The request is the `simulate` request of oracle_sim.rs (assumed vocabulary). A JSON number that R would
# round (a seed near 2^53, a variance of 1 + 1e-17) is written as raw text.
#
# Run: Rscript oracle/scripts/sim_oracle.R   (writes oracle/expected/sim/errors.json). Needs sim_lib.R.

RAW <- function(txt) paste0("@@RAW:", txt, "@@")

sub_ <- function(id, time, dose = 100) list(id = id, dose = dose, time = arr(time))
SX2_T <- c(0.5, 1, 2, 4, 8, 12)
base_request <- function() list(
  op = "simulate", model = "pk1.iv_bolus", theta = list(cl = 5, v = 50), random_effects = arr(c("cl", "v")), omega_shape = "block",
  omega = arr(c(0.09, 0.03, 0.04)), error_model = "proportional", xi = list(b = 0.1),
  design = list(subjects = list(sub_("1", SX2_T), sub_("2", SX2_T), sub_("3", SX2_T))), seed = 20261108, replicates = 2, options = list(negative = "keep"))
mod <- function(r, ...) { ch <- list(...); for (n in names(ch)) r[n] <- list(ch[[n]]); r[!vapply(r, is.null, TRUE)] }
mod_opt <- function(r, ...) { ch <- list(...); for (n in names(ch)) r$options[n] <- list(ch[[n]]); r }
without <- function(r, name) { r[[name]] <- NULL; r }

errors_cases <- function(collision = NULL) {
  cases <- list()
  add <- function(id, rule, description, req, code, words, basis, control = NULL, must_not_contain = NULL, note = NULL, expect = "refused", expect_counts = NULL, constructible = TRUE) {
    c <- list(case = id, rule = rule, description = description, expect = expect, constructible = constructible)
    if (!is.null(req)) c$request <- req
    if (expect == "refused") c$error <- list(code = code, words = arr(words), words_basis = basis)
    if (!is.null(must_not_contain)) c$error$must_not_contain <- arr(must_not_contain)
    if (!is.null(control)) c$control <- control
    if (!is.null(expect_counts)) c$expect_counts <- expect_counts
    if (!is.null(note)) c$note <- note
    cases[[length(cases) + 1L]] <<- c
  }
  ctl <- function(change, request) list(change = change, request = request)
  B <- base_request()
  rn <- "rule_names_option"

  # ===== SIM-REF-01 the seed
  seed_sentence <- c("a simulation needs a seed", "give an integer from 0 to", "18446744073709551615", "9007199254740991", "decimal string")
  add("missing_seed", "SIM-REF-01; SX6", "no seed: the sentence of SIM-REF-01 (the range and the decimal-string advice)", without(B, "seed"), "missing_seed", seed_sentence, "spec_sentence",
      control = ctl("seed = 20261108", B))
  for (sd in list(list("seed_negative", -1, "seed = -1"), list("seed_fractional", 1.5, "seed = 1.5"))) {
    add(sd[[1]], "SIM-REF-01", sprintf("%s: InvalidOption naming seed", sd[[3]]), mod(B, seed = sd[[2]]), "invalid_option", "seed", rn, control = ctl("seed = 1", mod(B, seed = 1)))
  }
  add("seed_not_a_number", "SIM-REF-01", "seed = \"twenty\": not a number, not digits", mod(B, seed = "twenty"), "invalid_option", "seed", rn, control = ctl("seed = \"20261108\" (a decimal string)", mod(B, seed = "20261108")))
  add("seed_boolean", "SIM-REF-01", "seed = true: not a number", mod(B, seed = TRUE), "invalid_option", "seed", rn, control = ctl("seed = 1", mod(B, seed = 1)))
  add("seed_number_2_pow_53", "SIM-REF-01", "a JSON number 9007199254740992 = 2^53 is above 2^53 - 1: must be written as a decimal string",
      mod(B, seed = RAW("9007199254740992")), "invalid_option", "seed", rn, control = ctl("the same value written as a decimal string", mod(B, seed = "9007199254740992")))
  add("seed_string_2_pow_64", "SIM-REF-01", "the decimal string 18446744073709551616 = 2^64 is not below 2^64", mod(B, seed = "18446744073709551616"), "invalid_option", "seed", rn,
      control = ctl("seed = \"18446744073709551615\" (the largest seed)", mod(B, seed = "18446744073709551615")))
  add("seed_string_negative", "SIM-REF-01", "the decimal string \"-5\"", mod(B, seed = "-5"), "invalid_option", "seed", rn, control = ctl("seed = \"5\"", mod(B, seed = "5")))
  for (s in list(list("seed_string_small", "20261108", "a small seed written as a decimal string is accepted (and gives the same stream as the number 20261108: tested in the study cases)"),
                 list("seed_string_above_2_53", "9007199254740993", "a seed above 2^53 as a decimal string is accepted"),
                 list("seed_string_max_u64", "18446744073709551615", "the largest seed 2^64 - 1 as a decimal string is accepted"),
                 list("seed_zero", 0, "seed 0 is accepted"))) {
    add(s[[1]], "SIM-REF-01", s[[3]], mod(B, seed = s[[2]]), NULL, NULL, NULL, expect = "accepted")
  }
  add("seed_number_max_safe", "SIM-REF-01", "the JSON number 9007199254740991 = 2^53 - 1 is accepted", mod(B, seed = RAW("9007199254740991")), NULL, NULL, NULL, expect = "accepted")

  # ===== SIM-REF-02 Omega
  L21 <- 0.3 / sqrt(0.09); pivot <- 0.04 - L21 * L21     # the pivot of SX6, in doubles
  stopifnot(abs(pivot + 0.96) < 1e-15)
  add("omega_not_positive_definite", "SIM-REF-02; SX6", "SX6: Omega lower triangle (0.09, 0.3, 0.04), correlation 5: L21 = 1 and the pivot of the second effect is 0.04 - 1 = -0.96",
      mod(B, omega = arr(c(0.09, 0.3, 0.04))), "omega_not_positive_definite", c("omega is not positive definite", "Cholesky pivot of random effect v", "-0.96"), "spec_sentence",
      control = ctl("omega = (0.09, 0.03, 0.04)", B))
  add("omega_zero_variance", "SIM-REF-02", "a variance of 0 is a pivot of 0 and is refused, unlike the fit (an effect without variance is no random effect)",
      mod(B, omega_shape = "diagonal", omega = arr(c(0.09, 0))), "omega_not_positive_definite", c("omega is not positive definite", "random effect v"), "spec_sentence",
      control = ctl("omega = (0.09, 0.04)", mod(B, omega_shape = "diagonal", omega = arr(c(0.09, 0.04)))))
  add("omega_pivot_zero_in_double", "SIM-REF-02", "positive definite in exact arithmetic but not in doubles: Omega22 = 1.00000000000000001 is the double 1, so the pivot 1 - 1*1 is 0",
      mod(B, omega = arr(c(1, 1, RAW("1.00000000000000001")))), "omega_not_positive_definite", c("omega is not positive definite", "random effect v"), "spec_sentence",
      control = ctl("Omega22 = 1.0000000000000002 (the next double above 1): the pivot is 2.2e-16 > 0", mod(B, omega = arr(c(1, 1, RAW("1.0000000000000002"))))),
      note = "SIM-REF-02: a matrix positive definite in exact arithmetic whose double Cholesky pivot is <= 0 is refused by the same rule")
  add("omega_negative_variance", "SIM-REF-02", "a negative variance: InvalidOption naming omega (the pivot test would catch it, the first message is clearer)",
      mod(B, omega_shape = "diagonal", omega = arr(c(0.09, -0.04))), "invalid_option", "omega", rn, control = ctl("omega = (0.09, 0.04)", mod(B, omega_shape = "diagonal", omega = arr(c(0.09, 0.04)))))
  add("omega_block_too_few", "SIM-REF-02", "a block Omega for q = 2 needs 3 numbers; 2 are given: the message names both counts",
      mod(B, omega = arr(c(0.09, 0.03))), "invalid_option", c("omega", "2", "3"), "inferred", control = ctl("omega = (0.09, 0.03, 0.04)", B))
  add("omega_diagonal_too_many", "SIM-REF-02", "a diagonal Omega for q = 2 needs 2 numbers; 3 are given: the message names both counts",
      mod(B, omega_shape = "diagonal", omega = arr(c(0.09, 0.04, 0.01))), "invalid_option", c("omega", "3", "2"), "inferred",
      control = ctl("omega = (0.09, 0.04)", mod(B, omega_shape = "diagonal", omega = arr(c(0.09, 0.04)))))
  add("omega_block_q1_three_numbers", "SIM-REF-02", "a block Omega given for q = 1 with more than one number",
      mod(B, random_effects = arr("v"), omega = arr(c(0.09, 0.03, 0.04))), "invalid_option", "omega", rn,
      control = ctl("omega = (0.04)", mod(B, random_effects = arr("v"), omega = arr(0.04))))
  add("omega_non_finite", "SIM-REF-02", "a non-finite entry of Omega (NaN, infinity): InvalidOption naming omega", NULL, "invalid_option", "omega", rn, expect = "refused", constructible = FALSE,
      note = "not constructible: JSON has no NaN or infinity; the adapter's own parser is the place for this check")

  # ===== SIM-REF-03 options
  add("negative_unknown_value", "SIM-REF-03; SX6", "negative = \"clip\": unknown value of `negative`: use `keep` or `zero`", mod_opt(B, negative = "clip"), "invalid_option", c("negative", "keep", "zero"), "spec_sentence",
      control = ctl("negative = \"zero\"", mod_opt(B, negative = "zero")))
  add("lloq_zero", "SIM-REF-03; SX6", "lloq = 0: not > 0", mod_opt(B, lloq = 0), "invalid_option", "lloq", rn, control = ctl("lloq = 0.8", mod_opt(B, lloq = 0.8)))
  add("lloq_negative", "SIM-REF-03", "lloq = -1", mod_opt(B, lloq = -1), "invalid_option", "lloq", rn, control = ctl("lloq = 0.8", mod_opt(B, lloq = 0.8)))
  add("lloq_text", "SIM-REF-03", "lloq = \"low\": not a number", mod_opt(B, lloq = "low"), "invalid_option", "lloq", rn, control = ctl("lloq = 0.8", mod_opt(B, lloq = 0.8)))
  add("unknown_option_name", "SIM-REF-03; SIM-MOD-05", "an unknown option name", mod_opt(B, nonsense = 1), "invalid_option", "nonsense", rn, control = ctl("the option removed", B))
  add("error_model_unknown", "SIM-REF-03", "error_model = \"gaussian\"", mod(B, error_model = "gaussian"), "invalid_option", "error_model", rn, control = ctl("error_model = \"proportional\"", B))
  add("error_model_missing", "SIM-REF-03; SIM-MOD-05", "no error model, and no default", without(without(B, "error_model"), "xi"), "invalid_option", "error_model", rn, control = ctl("error_model and xi given", B))
  add("error_parameter_b_missing", "SIM-REF-03", "proportional without b", mod(B, xi = list()), "invalid_option", "b", rn, control = ctl("b = 0.1", B))
  add("error_parameter_combined_one_of_two", "SIM-REF-03", "combined with a only: both are required", mod(B, error_model = "combined", xi = list(a = 0.2)), "invalid_option", "b", rn,
      control = ctl("a and b given", mod(B, error_model = "combined", xi = list(a = 0.2, b = 0.1))))
  add("error_parameter_combined_sd_a_missing", "SIM-REF-03", "combined_sd with b only", mod(B, error_model = "combined_sd", xi = list(b = 0.1)), "invalid_option", "a", rn,
      control = ctl("a and b given", mod(B, error_model = "combined_sd", xi = list(a = 0.2, b = 0.1))))
  add("error_parameter_negative", "SIM-REF-03", "b = -0.1", mod(B, xi = list(b = -0.1)), "invalid_option", "b", rn, control = ctl("b = 0.1", B))
  add("quantile_above_one", "SIM-REF-03", "a quantiles entry outside [0, 1]", mod_opt(B, quantiles = arr(c(0.5, 1.5))), "invalid_option", "quantiles", rn,
      control = ctl("quantiles = (0.5, 1)", mod_opt(B, quantiles = arr(c(0.5, 1)))))
  add("replicates_zero", "SIM-REF-03", "replicates = 0", mod(B, replicates = 0), "invalid_option", "replicates", rn, control = ctl("replicates = 1", mod(B, replicates = 1)))
  add("replicates_fractional", "SIM-REF-03", "replicates = 2.5", mod(B, replicates = 2.5), "invalid_option", "replicates", rn, control = ctl("replicates = 2", B))
  add("random_effects_empty", "SIM-REF-03; SIM-MOD-03", "random_effects empty: a simulation of residual error alone is not offered",
      mod(B, random_effects = list(), omega = list()), "invalid_option", "random_effects", rn, control = ctl("random_effects = (cl), omega = (0.09)", mod(B, random_effects = arr("cl"), omega_shape = "diagonal", omega = arr(0.09))))
  add("random_effects_duplicate", "SIM-REF-03", "a duplicate in random_effects", mod(B, random_effects = arr(c("cl", "cl"))), "invalid_option", c("random_effects", "cl"), rn,
      control = ctl("random_effects = (cl, v)", B))
  add("random_effect_on_missing_parameter", "SIM-REF-03; POP-MOD-02", "a random effect on a parameter the model does not have (ka on pk1.iv_bolus)",
      mod(B, random_effects = arr(c("cl", "ka"))), "invalid_option", "ka", rn, control = ctl("random_effects = (cl, v)", B))
  oral_lag <- mod(B, model = "pk1.oral_1_lag", theta = list(cl = 5, v = 50, ka = 1.5, tlag = 0.5), random_effects = arr(c("cl", "v")))
  add("random_effect_on_tlag", "SIM-REF-03; SIM-MOD-03", "tlag carries no random effect in a simulation (decision of 2026-11-08, OS-03)",
      mod(oral_lag, random_effects = arr(c("cl", "tlag"))), "invalid_option", "tlag", rn, control = ctl("random_effects = (cl, v), tlag stays in theta", oral_lag))
  infusion <- mod(B, model = "pk1.iv_infusion", theta = list(cl = 5, v = 50, dur = 2), random_effects = arr(c("cl", "v")))
  add("random_effect_on_dur", "SIM-REF-03; SIM-MOD-03", "dur carries no random effect in a simulation",
      mod(infusion, random_effects = arr(c("cl", "dur"))), "invalid_option", "dur", rn, control = ctl("random_effects = (cl, v), dur stays in theta", infusion))
  add("theta_zero_with_random_effect", "SIM-REF-03; SIM-MOD-03", "theta <= 0 for a parameter with a random effect (v = 0)", mod(B, theta = list(cl = 5, v = 0)), "invalid_option", "v", rn,
      control = ctl("v = 50", B))
  add("theta_parameter_missing", "SIM-REF-03", "a parameter of the model missing in theta", mod(B, theta = list(cl = 5)), "invalid_option", "v", rn, control = ctl("v = 50", B))
  add("theta_parameter_extra", "SIM-REF-03", "a theta for a parameter the model does not have (ka on pk1.iv_bolus)", mod(B, theta = list(cl = 5, v = 50, ka = 1.5)), "invalid_option", "ka", rn, control = ctl("ka removed", B))
  add("model_pk3_not_available", "SIM-REF-03; SIM-MOD-02", "a pk3.* id is InvalidOption until section 14 of models.md is on the engine's branch",
      mod(B, model = "pk3.iv_bolus", theta = list(cl = 5, vc = 50, q2 = 3, vp2 = 40, q3 = 1, vp3 = 30), random_effects = arr(c("cl", "vc"))), "invalid_option", c("model not available in this version"), "spec_sentence",
      control = ctl("model = pk1.iv_bolus with its own theta and random effects (the request B)", B),
      note = "valid only while specs/models.md section 14 is not on the engine branch (SIM-MOD-02): when it is, this row must be removed with a note in the same commit; the id is assumed (the six pk3 routes follow the pk1 naming)")
  add("model_unknown", "SIM-REF-03", "an unknown model id", mod(B, model = "pk1.nonsense"), "invalid_option", "model", rn, control = ctl("model = pk1.iv_bolus", B))

  # ===== SIM-REF-04 the design
  design_subjects <- function(...) list(subjects = list(...))
  add("duplicate_times", "SIM-REF-04; SIM-MOD-04", "two observations at the same time for one subject (subject 2: 1, 2, 2, 3)",
      mod(B, design = design_subjects(sub_("1", SX2_T), sub_("2", c(1, 2, 2, 3)))), "duplicate_times", "subject 2", "inferred",
      control = ctl("times 1, 2, 3, 4", mod(B, design = design_subjects(sub_("1", SX2_T), sub_("2", c(1, 2, 3, 4))))),
      note = "SIM-REF-04: the dataset's own sentence (DAT-ORD-01, today's text in the importer) with `in subject {S}`; the code is asserted, the words are the subject only")
  add("time_not_increasing", "SIM-REF-04; DAT-ERR-01", "time goes backwards for subject 2 (2 after 4)",
      mod(B, design = design_subjects(sub_("1", SX2_T), sub_("2", c(1, 4, 2, 8)))), "time_not_increasing", c("time goes back", "subject 2"), "spec_sentence",
      control = ctl("times 1, 2, 4, 8", mod(B, design = design_subjects(sub_("1", SX2_T), sub_("2", c(1, 2, 4, 8))))))
  add("subject_without_observation", "SIM-REF-04; DAT-ERR-04", "subject 2 has a dose and no observation row",
      mod(B, design = design_subjects(sub_("1", SX2_T), sub_("2", list()))), "subject_without_observation", c("subject 2", "no observation"), "spec_sentence",
      control = ctl("subject 2 has observation times", mod(B, design = design_subjects(sub_("1", SX2_T), sub_("2", c(1, 2))))))
  add("design_without_subject", "SIM-REF-04", "a design with no subject", mod(B, design = design_subjects()), "invalid_option", "subject", "inferred",
      control = ctl("one subject", mod(B, design = design_subjects(sub_("1", SX2_T)))), note = "the spec says the design is refused; the code and the sentence are not fixed (inferred: invalid_option, naming subject)")
  tbl <- function(rows) list(table = rows)
  tr <- function(subject, time, event, amt = NULL, dv = NULL) { r <- list(subject = subject, time = time, event = event); if (!is.null(amt)) r$amt <- amt; if (!is.null(dv)) r$dv <- dv; r }
  ok_table <- tbl(list(tr("1", 0, "dose", 100), tr("1", 1, "obs"), tr("1", 2, "obs"), tr("2", 0, "dose", 100), tr("2", 1, "obs")))
  add("two_dose_rows", "SIM-REF-04; POP-DAT-01", "a subject with more than one dose row (multiple dosing is later)",
      mod(B, design = tbl(list(tr("1", 0, "dose", 100), tr("1", 12, "dose", 100), tr("1", 1, "obs"), tr("1", 13, "obs")))), NULL, c("subject 1", "dose"), "inferred",
      control = ctl("one dose row", mod(B, design = tbl(list(tr("1", 0, "dose", 100), tr("1", 1, "obs"), tr("1", 13, "obs"))))),
      note = "the sentence is that of DAT-USE-02 (not in this card's reading); code not asserted")
  add("dose_without_amount", "SIM-REF-04; DAT-ERR-02", "a dose row without an amount", mod(B, design = tbl(list(tr("1", 0, "dose"), tr("1", 1, "obs")))), "dose_without_amount", c("subject 1", "no amount"), "spec_sentence",
      control = ctl("amt = 100", mod(B, design = tbl(list(tr("1", 0, "dose", 100), tr("1", 1, "obs"))))))
  add("amount_on_observation", "SIM-REF-04; DAT-ERR-03", "an observation row with an amount",
      mod(B, design = tbl(list(tr("1", 0, "dose", 100), tr("1", 1, "obs", amt = 5)))), "amount_on_observation", c("subject 1", "is an observation but has an amount"), "spec_sentence",
      control = ctl("amount removed", mod(B, design = tbl(list(tr("1", 0, "dose", 100), tr("1", 1, "obs"))))))
  add("table_design_accepted", "SIM-MOD-04", "a table design, dose then observations, two subjects", mod(B, design = ok_table), NULL, NULL, NULL, expect = "accepted")
  add("table_dv_ignored", "SIM-MOD-04 item 1", "the value cell of an observation row is ignored and counted in n_dv_ignored (2 of the 3 observation rows have a value)",
      mod(B, design = tbl(list(tr("1", 0, "dose", 100), tr("1", 1, "obs", dv = 3.5), tr("1", 2, "obs", dv = 1.25), tr("1", 4, "obs")))), NULL, NULL, NULL, expect = "accepted", expect_counts = list(n_dv_ignored = 2))
  add("design_without_table", "SIM-MOD-04 item 5", "a list of times, a dose and m: the subjects are labelled \"1\", \"2\", ... \"m\" (the same stream as the explicit design: tested in the study cases)",
      mod(B, design = list(times = arr(SX2_T), dose = 100, m = 3)), NULL, NULL, NULL, expect = "accepted")
  add("dose_and_observation_same_time", "SIM-MOD-04 item 3", "a dose and an observation at the same time are allowed", mod(B, design = design_subjects(sub_("1", c(0, 1, 2)))), NULL, NULL, NULL, expect = "accepted")
  add("predose_observation", "SIM-MOD-04 item 4", "an observation before the dose is simulated (IPRED = 0)", mod(B, design = design_subjects(sub_("1", c(-1, 1, 2)))), NULL, NULL, NULL, expect = "accepted")
  add("tlag_fixed_accepted", "SIM-MOD-03", "tlag in theta with no random effect on it is accepted", oral_lag, NULL, NULL, NULL, expect = "accepted")
  add("random_effects_reversed_accepted", "SIM-ORA-05 item 6", "random_effects (v, cl) with Omega reordered is accepted", mod(B, random_effects = arr(c("v", "cl")), omega = arr(c(0.04, 0.03, 0.09))), NULL, NULL, NULL, expect = "accepted")
  add("design_non_finite", "SIM-REF-04", "a non-finite time or dose (MOD-GEN-04)", NULL, NULL, c("time"), "inferred", expect = "refused", constructible = FALSE,
      note = "not constructible: JSON has no NaN or infinity")

  # ===== SIM-REF-05 what the model refuses at a draw
  huge <- mod(B, design = design_subjects(sub_("1", SX2_T)), replicates = 1, random_effects = arr("cl"), omega_shape = "diagonal", omega = arr(0.09))
  # a label whose first eta normal is positive and large enough: Omega = 1e8 on cl, eta = 1e4 z > 709.78 for z > 0.071
  zlab <- NULL
  for (lab in as.character(1:50)) { z <- draw_z("20261108", 0, 1, lab, 1); if (z > 0.5) { zlab <- lab; break } }
  stopifnot(!is.null(zlab))
  big_omega <- mod(huge, design = design_subjects(sub_(zlab, SX2_T)), omega = arr(1e8))
  add("non_finite_value", "SIM-REF-05; SX6", sprintf("the individual parameter overflows: Omega = 1e8 on cl, subject %s has z_eta > 0.5 so eta > 5000 and exp(eta) is infinite", zlab),
      big_omega, "non_finite_value", c(sprintf("in subject %s, replicate 0, with cl", zlab)), "spec_sentence",
      control = ctl("omega = 0.09", mod(big_omega, omega = arr(0.09))),
      note = "SX6 suggests Omega = 1e4 (omega = 100): that overflows only for z > 7.09 (never in practice, |z| <= 8.21 with probability about 1e-12), so the row uses 1e8; one subject and one replicate so that the cell that fails is unique")

  # ===== SIM-REF-06 size limits
  add("size_limit_replicates", "SIM-REF-06; SX6", "R = 16777217 replicates: one more than the replicate field holds",
      mod(B, replicates = 16777217), "size_limit", c("replicates 16777217 exceed 16777216"), "spec_sentence",
      control = ctl("replicates = 1", mod(B, replicates = 1)),
      note = "the repaired request at the limit itself (R = 16777216) is a run of 16.7 million replicates: the control is the small run, and the limit value is asserted by the sentence")
  # label_key_collision
  if (!is.null(collision)) {
    A <- collision[1]; Bl <- collision[2]
    stopifnot(A != Bl, identical(label_key(A), label_key(Bl)), label_hash_hex(A) == label_hash_hex(Bl))
    add("label_key_collision", "SIM-REF-06; SIM-RNG-03", sprintf("two different labels with the same 64-bit stream key: %s and %s (first 8 bytes of SHA-256 = %s)", A, Bl, label_hash_hex(A)),
        mod(B, design = design_subjects(sub_(A, SX2_T), sub_(Bl, SX2_T))), "label_key_collision", c(A, Bl, "same 64-bit stream key"), "spec_sentence",
        control = ctl(sprintf("the second label renamed to \"2\" (a different 64-bit key)"), mod(B, design = design_subjects(sub_(A, SX2_T), sub_("2", SX2_T)))),
        note = "found by a parallel collision search on the first 8 bytes of SHA-256 (van Oorschot and Wiener, about 1e10 hash evaluations); both labels are 16 lowercase hexadecimal digits; checked in R with digest and in the testkit with its own SHA-256")
  } else {
    add("label_key_collision", "SIM-REF-06; SIM-RNG-03", "two different labels with the same 64-bit stream key", NULL, "label_key_collision", c("same 64-bit stream key"), "spec_sentence", expect = "refused", constructible = FALSE,
        note = "cannot be tested: no pair of labels with equal first 8 bytes of SHA-256 was found (2^32 evaluations by the birthday bound)")
  }
  cases
}

errors_stage <- function(out_dir, collision = NULL) {
  cases <- errors_cases(collision)
  ids <- vapply(cases, function(c) c$case, "")
  stopifnot(!anyDuplicated(ids))
  txt <- toJSON(list(rule = "SIM-REF-01..06, SX6", note = paste("requests are `simulate` requests of oracle_sim.rs; a control is the same request with the one change that removes the cause and must not be refused;",
                                                               "words_basis: spec_sentence (the sentence is written in specs/simulation.md), rule_names_option (the rule says the error names the option), inferred (the spec fixes the code or the subject, not the sentence)"),
                     cases = cases), auto_unbox = TRUE, pretty = TRUE, digits = NA, null = "null", na = "null")
  txt <- gsub('"@@RAW:([^@]*)@@"', "\\1", txt)
  stopifnot(!grepl("@@RAW", txt))
  write_lines_lf(txt, file.path(out_dir, "errors.json"))
  ref <- vapply(cases, function(c) c$expect == "refused" && c$constructible, TRUE)
  ctrl <- vapply(cases, function(c) !is.null(c$control), TRUE)
  cat(sprintf("sim_errors: %d rows (%d refused, %d with a control, %d accepted, %d sentence-only)\n", length(cases), sum(ref), sum(ctrl),
              sum(vapply(cases, function(c) c$expect == "accepted", TRUE)), sum(!vapply(cases, function(c) c$constructible, TRUE))))
  invisible(cases)
}
