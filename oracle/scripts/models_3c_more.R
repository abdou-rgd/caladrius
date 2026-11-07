# Caladrius three-compartment oracle (T-142), part 3: refusals (OM-26), multiple dosing and steady
# state (OM-28), and the case list for the engine tests. Sourced by models_3c_derivatives.R.

# ================================================================ refusals (OM-26)
# Expected "not available" rows (an empty value) with the reason, the rule and the words the readable
# message must contain (lower case; each inner list is a set of alternatives of which one must appear).
# NaN and infinity are written as the strings "NaN", "Infinity", "-Infinity" because JSON has none.
cl3 <- list(cl = 3, vc = 8, q2 = 9.5, vp2 = 9.5, q3 = 6.3, vp3 = 31.5)
mi3 <- list(k10 = 0.375, k12 = 1.1875, k21 = 1, k13 = 0.7875, k31 = 0.2, vc = 8)
ma3 <- list(a = 0.759322033898, b = 0.133333333333, c = 0.107344632768, alpha = 3, beta = 0.5, gamma = 0.05)
errors <- list()
err <- function(id, group, spec, model, dose, params, contains, reason, times = 1) {
  errors[[length(errors) + 1L]] <<- list(
    id = id, group = group, spec = spec, model = model, dose = dose, parameters = params,
    times = arr(times), message_contains = lapply(contains, arr), reason = reason)
}
with_p <- function(p, ...) { mod <- do.call(c, list(...)); for (n in names(mod)) p[[n]] <- mod[[n]]; p }
without <- function(p, ...) p[setdiff(names(p), c(...))]
SETS3 <- list(clearance = cl3, micro = mi3)
ID_EXTRA <- list(pk3.iv_bolus = list(), pk3.iv_infusion = list(dur = 2), pk3.oral_1 = list(ka = 1.5),
                 pk3.oral_1_lag = list(ka = 1.5, tlag = 0.5), pk3.oral_0 = list(dur = 2),
                 pk3.oral_0_lag = list(dur = 2, tlag = 0.5))
M3 <- "MOD-3C-06"
# domain of the two native sets: positive and finite
for (sn in names(SETS3)) {
  base <- SETS3[[sn]]
  for (nm in names(base)) {
    peripheral <- nm %in% c("q2", "vp2", "q3", "vp3", "k12", "k13")
    bads <- list(list("negative", -1), list("nan", "NaN"), list("infinite", "Infinity"))
    if (!peripheral) bads <- c(list(list("zero", 0)), bads)
    for (bad in bads)
      err(paste0(sn, "_", nm, "_", bad[[1]]), "domain", paste(M3, "MOD-GEN-04"), "pk3.iv_bolus", 100,
          with_p(base, setNames(list(bad[[2]]), nm)), list(nm), paste0("`", nm, "` must be finite and > 0"))
  }
}
# one peripheral compartment missing (item 1): the message names the parameter and says to use pk2
for (m in c("pk3.iv_bolus", "pk3.iv_infusion", "pk3.oral_1")) {
  for (nm in c("q2", "vp2", "q3", "vp3"))
    err(paste0("one_compartment_", nm, "_zero_", short(m)), "one_compartment", paste(M3, "item 1"), m, 100,
        c(with_p(cl3, setNames(list(0), nm)), ID_EXTRA[[m]]), list(nm, "pk2"),
        paste0("`", nm, "` = 0 leaves two compartments; the message names the parameter and says to use pk2"))
  for (nm in c("k12", "k13"))
    err(paste0("one_compartment_", nm, "_zero_", short(m)), "one_compartment", paste(M3, "item 1"), m, 100,
        c(with_p(mi3, setNames(list(0), nm)), ID_EXTRA[[m]]), list(nm, "pk2"),
        paste0("`", nm, "` = 0 leaves two compartments; the message says to use pk2"))
}
err("one_compartment_both_missing_clearance", "one_compartment", paste(M3, "item 1"), "pk3.iv_bolus", 100,
    with_p(cl3, list(q2 = 0, q3 = 0)), list("pk1"), "both peripherals missing: use pk1")
err("one_compartment_both_missing_micro", "one_compartment", paste(M3, "item 1"), "pk3.iv_bolus", 100,
    with_p(mi3, list(k12 = 0, k13 = 0)), list("pk1"), "both peripherals missing: use pk1")
err("one_compartment_vp2_and_q3_zero", "one_compartment", paste(M3, "item 1"), "pk3.iv_bolus", 100,
    with_p(cl3, list(vp2 = 0, q3 = 0)), list("pk1"), "one peripheral missing on each side: use pk1")
# input parameters (MOD-GEN-04)
for (m in c("pk3.oral_1", "pk3.oral_1_lag")) {
  ex <- ID_EXTRA[[m]]
  err(paste0("ka_zero_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(ka = 0))), list("ka"), "`ka` must be > 0")
  err(paste0("ka_negative_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(ka = -1))), list("ka"), "`ka` must be > 0")
  err(paste0("ka_nan_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(ka = "NaN"))), list("ka"), "`ka` must be finite")
  err(paste0("ka_infinite_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(ka = "Infinity"))), list("ka"), "`ka` must be finite")
}
for (m in c("pk3.oral_1_lag", "pk3.oral_0_lag")) {
  ex <- ID_EXTRA[[m]]
  err(paste0("tlag_negative_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(tlag = -0.5))), list("tlag"), "`tlag` must be >= 0")
  err(paste0("tlag_nan_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(tlag = "NaN"))), list("tlag"), "`tlag` must be finite")
}
for (m in c("pk3.iv_infusion", "pk3.oral_0", "pk3.oral_0_lag")) {
  ex <- ID_EXTRA[[m]]
  err(paste0("dur_zero_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(dur = 0))), list("dur"), "`dur` must be > 0")
  err(paste0("dur_negative_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(dur = -1))), list("dur"), "`dur` must be > 0")
  err(paste0("dur_infinite_", short(m)), "domain", "MOD-GEN-04", m, 100, c(cl3, with_p(ex, list(dur = "Infinity"))), list("dur"), "`dur` must be finite")
}
err("dose_negative", "domain", "MOD-GEN-04", "pk3.iv_bolus", -1, cl3, list("dose"), "the dose must be >= 0")
err("dose_nan", "domain", "MOD-GEN-04", "pk3.iv_bolus", "NaN", cl3, list("dose"), "the dose must be finite")
err("dose_infinite", "domain", "MOD-GEN-04", "pk3.iv_bolus", "Infinity", cl3, list("dose"), "the dose must be finite")
# macro set (item 2)
MR <- paste(M3, "item 2")
err("macro_alpha_equals_beta", "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, alpha = 0.5), list("alpha", "beta"), "alpha must be larger than beta")
err("macro_alpha_below_beta", "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, list(alpha = 0.4)), list("alpha", "beta"), "the exponents must be named in decreasing order; no sorting")
err("macro_beta_equals_gamma", "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, beta = 0.05), list("beta", "gamma"), "beta must be larger than gamma")
err("macro_beta_below_gamma", "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, list(beta = 0.04)), list("beta", "gamma"), "beta must be larger than gamma")
err("macro_swapped_beta_gamma", "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, list(beta = 0.05, gamma = 0.5)), list("beta", "gamma"), "the exponents must be named in decreasing order")
err("macro_increasing", "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, list(alpha = 0.05, beta = 0.5, gamma = 3)), list("alpha", "beta", "gamma"), "exponents in increasing order")
for (nm in c("a", "b", "c")) {
  err(paste0("macro_", nm, "_zero"), "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, setNames(list(0), nm)), list(nm), paste0("`", nm, "` must be > 0"))
  err(paste0("macro_", nm, "_negative"), "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, setNames(list(-1), nm)), list(nm), paste0("`", nm, "` must be > 0"))
  err(paste0("macro_", nm, "_nan"), "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, setNames(list("NaN"), nm)), list(nm), paste0("`", nm, "` must be finite"))
}
for (nm in c("alpha", "beta", "gamma")) {
  err(paste0("macro_", nm, "_nan"), "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, setNames(list("NaN"), nm)), list(nm), paste0("`", nm, "` must be finite"))
}
err("macro_gamma_zero", "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, gamma = 0), list("gamma"), "`gamma` must be > 0")
err("macro_gamma_negative", "macro", MR, "pk3.iv_bolus", 100, with_p(ma3, gamma = -0.05), list("gamma"), "`gamma` must be > 0")
err("macro_dose_zero", "macro", MR, "pk3.iv_bolus", 0, ma3, list("dose"), "vc = D / (a + b + c) needs a dose > 0")
err("macro_dose_zero_oral", "macro", MR, "pk3.oral_1", 0, c(ma3, list(ka = 2)), list("dose"), "vc = D / (a + b + c) needs a dose > 0")
# macro sets for which the reverse conversion gives no valid three-compartment model (item 3)
bad_macro <- list(list(a = 0.98, b = 0.01, c = 0.01), list(a = 0.01, b = 0.98, c = 0.01), list(a = 0.01, b = 0.01, c = 0.98),
                  list(a = 0.3, b = 0.3, c = 0.4), list(a = 0.05, b = 0.9, c = 0.05), list(a = 0.9, b = 0.05, c = 0.05),
                  list(a = 0.5, b = 0.01, c = 0.49), list(a = 0.2, b = 0.7, c = 0.1))
n_bad <- 0L
for (i in seq_along(bad_macro)) {
  pm <- c(bad_macro[[i]], list(alpha = 3, beta = 0.5, gamma = 0.05))
  ok <- tryCatch({ derive_q3("macro", pm, 100); TRUE }, error = function(e) FALSE)
  if (!ok) {
    n_bad <- n_bad + 1L
    err(paste0("numeric_macro_not_a_model_", i), "numeric", paste(M3, "item 3"), "pk3.iv_bolus", 100, pm,
        list(c("degenerate", "k12", "k13", "k21", "k31", "k10", "non-positive", "not real")),
        "the macro set gives a non-real or non-positive micro-constant: DegenerateExponents naming the quantity")
  }
}
cat(sprintf("macro sets that are not a model: %d of %d candidates\n", n_bad, length(bad_macro)))
# sets and names (item 4)
S4 <- paste(M3, "item 4")
err("sets_clearance_plus_k10", "sets", S4, "pk3.iv_bolus", 100, c(cl3, list(k10 = 0.375)), list(c("k10", "cl")), "two sets mixed names the offender")
err("sets_clearance_plus_micro", "sets", S4, "pk3.iv_bolus", 100, c(cl3, mi3[c("k10", "k12", "k21", "k13", "k31")]), list(c("k10", "k12", "k21", "k13", "k31", "cl")), "two complete sets")
err("sets_clearance_plus_alpha", "sets", S4, "pk3.iv_bolus", 100, c(cl3, list(alpha = 3)), list(c("alpha", "cl")), "a macro parameter next to the clearance set")
err("sets_micro_plus_a", "sets", S4, "pk3.iv_bolus", 100, c(mi3, list(a = 5)), list(c("a", "k10", "k12", "k21")), "a macro parameter next to the micro set")
err("sets_macro_plus_cl", "sets", S4, "pk3.iv_bolus", 100, c(ma3, list(cl = 3)), list(c("cl", "alpha")), "a clearance parameter next to the macro set")
for (nm in names(cl3)) err(paste0("sets_missing_", nm), "sets", S4, "pk3.iv_bolus", 100, without(cl3, nm), list(nm), "an incomplete clearance set names the missing parameter")
for (nm in c("k10", "k12", "k21", "k13", "k31")) err(paste0("sets_missing_", nm), "sets", S4, "pk3.iv_bolus", 100, without(mi3, nm), list(nm), "an incomplete micro set names the missing parameter")
for (nm in c("a", "b", "c", "alpha", "beta", "gamma")) err(paste0("sets_missing_", nm), "sets", S4, "pk3.iv_bolus", 100, without(ma3, nm), list(nm), "an incomplete macro set names the missing parameter")
err("sets_none", "sets", S4, "pk3.iv_bolus", 100, list(), list(c("cl", "vc", "k10", "alpha")), "no parameter at all")
err("sets_two_compartment_names", "sets", S4, "pk3.iv_bolus", 100, list(cl = 3, vc = 8, q = 9.5, vp = 9.5), list(c("q2", "vp2", "q3", "vp3", "q", "vp")), "the two-compartment names are not three-compartment parameters")
err("sets_one_compartment_names", "sets", S4, "pk3.iv_bolus", 100, list(v = 10, cl = 2), list(c("v", "q2", "vc")), "the one-compartment names")
err("sets_pk2_macro_names", "sets", S4, "pk3.iv_bolus", 100, list(a = 5, b = 4, alpha = 1, beta = 0.1), list(c("c", "gamma")), "a two-compartment macro set lacks c and gamma")
err("sets_unknown_name", "sets", S4, "pk3.iv_bolus", 100, c(cl3, list(volume = 3)), list("volume"), "an unknown name is refused by name")
err("sets_ka_on_bolus", "sets", S4, "pk3.iv_bolus", 100, c(cl3, list(ka = 1)), list("ka"), "`ka` does not belong to this model")
err("sets_dur_on_oral_1", "sets", S4, "pk3.oral_1", 100, c(cl3, list(ka = 1, dur = 2)), list("dur"), "`dur` does not belong to this model")
err("sets_tlag_on_oral_1", "sets", S4, "pk3.oral_1", 100, c(cl3, list(ka = 1, tlag = 0.5)), list("tlag"), "`tlag` does not belong to the model without lag")
err("sets_ka_on_oral_0", "sets", S4, "pk3.oral_0", 100, c(cl3, list(dur = 2, ka = 1)), list("ka"), "`ka` does not belong to this model")
err("sets_tlag_on_infusion", "sets", S4, "pk3.iv_infusion", 100, c(cl3, list(dur = 2, tlag = 0.5)), list("tlag"), "an intravenous model has no lag")
err("sets_missing_ka", "sets", S4, "pk3.oral_1", 100, cl3, list("ka"), "`ka` is required")
err("sets_missing_dur", "sets", S4, "pk3.iv_infusion", 100, cl3, list("dur"), "`dur` is required")
err("sets_missing_tlag", "sets", S4, "pk3.oral_1_lag", 100, c(cl3, list(ka = 1)), list("tlag"), "`tlag` is required")
# numerics (item 3 and 5, MOD-3C-26)
N3 <- paste(M3, "item 3")
err("numeric_underflow_k12_k21", "numeric", N3, "pk3.iv_bolus", 100, list(k10 = 1, k12 = 1e-200, k21 = 1e-200, k13 = 0.5, k31 = 0.2, vc = 1), list(c("degenerate", "underflow", "k12", "k21")), "k12 * k21 underflows to 0: DegenerateExponents")
err("numeric_underflow_k13_k31", "numeric", N3, "pk3.iv_bolus", 100, list(k10 = 1, k12 = 0.5, k21 = 0.2, k13 = 1e-200, k31 = 1e-200, vc = 1), list(c("degenerate", "underflow", "k13", "k31")), "k13 * k31 underflows to 0: DegenerateExponents")
err("numeric_underflow_a0", "numeric", N3, "pk3.iv_bolus", 100, list(k10 = 1e-110, k12 = 0.5, k21 = 1e-110, k13 = 0.5, k31 = 1e-110, vc = 1), list(c("degenerate", "underflow", "a0", "k10")), "a0 = k10 k21 k31 underflows to 0: DegenerateExponents")
err("numeric_overflow_c0", "numeric", "MOD-3C-26", "pk3.iv_bolus", 1e300, with_p(cl3, list(vc = 1e-300, cl = 3e-300, q2 = 9.5e-300, q3 = 6.3e-300)), list(c("overflow", "finite")), "D / vc overflows: an Overflow error, never an infinity")
err("numeric_overflow_derived_rate", "numeric", N3, "pk3.iv_bolus", 100, with_p(cl3, list(q2 = 1e300, vp2 = 1e-300)), list(c("q2", "vp2", "k21", "overflow", "finite")), "q2 / vp2 is not finite")
err("numeric_overflow_macro_sum", "numeric", "MOD-3C-26", "pk3.iv_bolus", 100, with_p(ma3, list(a = 1e308, b = 1e308, c = 1e308)), list(c("a", "b", "c", "vc", "overflow", "finite")), "a + b + c overflows")
# times (MOD-GEN-04)
err("times_nan", "times", "MOD-GEN-04", "pk3.iv_bolus", 100, cl3, list(c("time", "nan", "finite")), "a NaN time is refused", times = list(1, "NaN"))
err("times_infinite", "times", "MOD-GEN-04", "pk3.iv_bolus", 100, cl3, list(c("time", "infinite", "finite")), "an infinite time is refused", times = list("Infinity"))
err("times_negative_infinite", "times", "MOD-GEN-04", "pk3.iv_bolus", 100, cl3, list(c("time", "infinite", "finite")), "an infinite time is refused", times = list("-Infinity"))

write_error_suite <- function(base, errs, groups, note) {
  ids <- vapply(errs, function(e) e$id, "")
  check(!any(duplicated(ids)), "duplicate error ids")
  errs <- lapply(errs, function(e) { if (length(e$parameters) == 0L) e$parameters <- structure(list(), names = character(0)); e })
  write_lines_lf(c("subject,parameter,value", sprintf("%s,result,", ids)), file.path(exp_dir, paste0(base, ".csv")))
  write_lines_lf(jenc(list(schema = 1L, kind = "model_errors", case = base,
                           generated_by = "oracle/scripts/models_3c_closed_form.R", note = note,
                           groups = arr(groups), cases = errs, versions = versions, n_values = length(errs))),
                 file.path(exp_dir, paste0(base, ".options.json")))
  length(errs)
}
n_err <- write_error_suite("model_pk3_errors", errors, c("domain", "one_compartment", "macro", "sets", "numeric", "times"),
  "inputs the three-compartment engine must refuse (MOD-3C-06, MOD-3C-26); the expected table has one not-available row (empty value) per case, the reason is in `reason`, the rule in `spec`, and the readable message must contain, for each inner list of `message_contains`, at least one of its words (lower case)")
cat(sprintf("error cases: %d\n", n_err))

source(file.path(script_dir, "models_3c_md.R"))
