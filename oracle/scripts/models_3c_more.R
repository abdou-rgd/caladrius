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
# Couplings so small that a root rounds onto a pole (item 5 of MOD-3C-06, MOD-3C-08; OM-22). MOD-3C-08
# states the criterion as a rounding: the engine refuses when a root "rounds onto" a pole in doubles
# (outside the exact tie of MOD-3C-22). It is decided here at 256 bits on the exact roots: the root is
# rounded to the nearest double and compared with the two poles. A row is a refusal when that happens to
# any of alpha, beta, gamma (round to nearest, so the boundary is exactly half the spacing of the doubles at
# the pole: above it at d, below it at d, which is half as wide when d is a power of two), a conditioning
# case (accepted) when it does not.
N_COUPLING_REFUSALS <- 0L
N_COUPLING_ACCEPTED <- 0L
N_BAND_OMITTED <- 0L
roots_onto_pole <- function(p) {
  q <- derive_q3("micro", p, 100, list())
  vapply(1:3, function(i) { x <- asNumeric(q$l[i]); x == p$k21 || x == p$k31 }, TRUE)
}
coupling_pair <- function(p, tiny) if (tiny == "k12") c("k12", "k21") else c("k13", "k31")
coupling_families <- list(
  alpha_onto_k21 = list(tiny = "k12", root = "alpha", f = function(d, c) list(k10 = d / 10, k12 = c, k21 = d, k13 = 0.05 * d, k31 = 0.02 * d, vc = 1)),
  gamma_onto_k31 = list(tiny = "k13", root = "gamma", f = function(d, c) list(k10 = 5 * d, k12 = 2.5 * d, k21 = 5 * d, k13 = c, k31 = d, vc = 1)),
  alpha_onto_k31 = list(tiny = "k13", root = "alpha", f = function(d, c) list(k10 = d / 10, k12 = 0.05 * d, k21 = 0.02 * d, k13 = c, k31 = d, vc = 1)),
  gamma_onto_k21 = list(tiny = "k12", root = "gamma", f = function(d, c) list(k10 = 5 * d, k12 = c, k21 = d, k13 = 2.5 * d, k31 = 50 * d, vc = 1)))
add_coupling_row <- function(id, p, tiny, root, note) {
  onto <- roots_onto_pole(p)
  if (any(onto)) {
    N_COUPLING_REFUSALS <<- N_COUPLING_REFUSALS + 1L
    err(id, "numeric", paste(M3, "item 5", "MOD-3C-08"), "pk3.iv_bolus", 100, p,
        list(c("degenerate", "exponent", "offset"), coupling_pair(p, tiny)),
        sprintf("%s: %s rounds onto a pole (DegenerateExponents)", note, paste(c("alpha", "beta", "gamma")[onto], collapse = " and ")))
    TRUE
  } else {
    N_COUPLING_ACCEPTED <<- N_COUPLING_ACCEPTED + 1L
    FALSE
  }
}
for (fam in names(coupling_families)) for (e in 17:30) {
  F <- coupling_families[[fam]]
  d <- switch(fam, alpha_onto_k21 = 10, gamma_onto_k31 = 0.2, alpha_onto_k31 = 10, gamma_onto_k21 = 0.2)
  pr <- 10^-e
  p <- F$f(d, if (F$tiny == "k12") pr / (if (fam == "alpha_onto_k21") 10 else 0.2) else pr / (if (fam == "alpha_onto_k31") 10 else 0.2))
  add_coupling_row(sprintf("numeric_coupling_%s_1e%d", fam, e), p, F$tiny, F$root, sprintf("coupling product 1e-%d", e))
}
# the boundary itself (MOD-3C-08 "rounds onto"): the coupling is tuned at 256 bits so that the offset of the
# root from the pole is 0.9 (refused) or 1.1 (accepted) times half the spacing of the doubles on that side of
# the pole, for poles at the bottom, the middle and the top of a binade and for the four families.
spacing_side <- function(d, above) {
  e <- floor(log2(d)); u <- 2^(e - 52)
  if (above || d != 2^e) u else u / 2
}
tune_coupling <- function(F, fam, d, target) {
  cpl <- 1e-17
  for (it in 1:8) {
    p <- F$f(d, cpl)
    m <- rel_offsets("micro", p)[[F$root]] * d
    cpl <- cpl * target / m
  }
  p <- F$f(d, cpl)
  m <- rel_offsets("micro", p)[[F$root]] * d
  check(abs(m / target - 1) < 1e-3, "edge tuning of ", fam, " d = ", d, ": offset ", m, " target ", target)
  p
}
N_EDGE_REFUSALS <- 0L; N_EDGE_ACCEPTED <- 0L
EDGE_ACCEPTED <- list()
POLES <- c(bottom = 1, mid = 1.5, top = 2 - 2^-52)
for (fam in names(coupling_families)) for (pl in names(POLES)) {
  d0 <- POLES[[pl]]
  F <- coupling_families[[fam]]
  for (fr in c(0.9, 1.1)) {
    target <- fr * spacing_side(d0, F$root == "alpha") / 2
    p <- tune_coupling(F, fam, d0, target)
    id <- sprintf("numeric_coupling_edge_%s_%s_%s", fam, pl, if (fr < 1) "inside" else "outside")
    refused <- add_coupling_row(id, p, F$tiny, F$root, sprintf("offset %.2f times half the spacing at the pole %.17g", fr, d0))
    check(refused == (fr < 1), id, ": the rounding of the 256-bit root disagrees with the tuning")
    if (refused) N_EDGE_REFUSALS <- N_EDGE_REFUSALS + 1L else {
      N_EDGE_ACCEPTED <- N_EDGE_ACCEPTED + 1L
      EDGE_ACCEPTED[[sub("^numeric_", "", id)]] <- p
    }
  }
}
# the inputs just outside the boundary are accepted: conditioning cases (exponents, weights and curve at 256 bits)
for (nm in names(EDGE_ACCEPTED))
  add_cond(nm, "micro", EDGE_ACCEPTED[[nm]], 100,
           "offset of the root from the pole 1.1 times half the spacing of the doubles at the pole: the root rounds to the next double, so the model is accepted (MOD-3C-08)")
# beta onto a pole (round 3 of T-142): the root that sits next to a pole is beta, not alpha or gamma, when the
# remaining part of F has the other sign; MOD-3C-07 puts beta strictly inside (d_min, d_max) and MOD-3C-08
# refuses a root that rounds onto a pole, so the probes below are refusals when beta (or the root next to
# the pole) rounds onto it. Each probe is classified at 256 bits like the rows above.
beta_probes <- list(
  beta_onto_k21_1e17 = list(p = list(k10 = 0.375, k12 = 1e-17, k21 = 1, k13 = 0.7875, k31 = 0.2, vc = 1), tiny = "k12"),
  beta_onto_k21_1e18 = list(p = list(k10 = 0.375, k12 = 1e-18, k21 = 1, k13 = 0.7875, k31 = 0.2, vc = 1), tiny = "k12"),
  beta_onto_k31_1e17 = list(p = list(k10 = 0.375, k12 = 0.7875, k21 = 0.2, k13 = 1e-17, k31 = 1, vc = 1), tiny = "k13"),
  beta_onto_k31_1e18 = list(p = list(k10 = 0.375, k12 = 0.7875, k21 = 0.2, k13 = 1e-18, k31 = 1, vc = 1), tiny = "k13"),
  beta_onto_k21_1e300 = list(p = list(k10 = 0.375, k12 = 1e-300, k21 = 0.5, k13 = 0.7875, k31 = 1, vc = 1), tiny = "k12"),
  beta_onto_k31_1e300 = list(p = list(k10 = 0.375, k12 = 0.7875, k21 = 1, k13 = 1e-300, k31 = 0.5, vc = 1), tiny = "k13"),
  # round 4 (F2): k21 = 0.2 is the double nearest 0.2 (the only value the engine ever sees); beta lies 0.659 half-spacings
  # of that double above it, so it rounds onto it: decided by the spec (MOD-3C-08), checked at 256 bits by the loop
  beta_onto_k21_p02_1e18 = list(p = list(k10 = 0.375, k12 = 1e-18, k21 = 0.2, k13 = 0.7875, k31 = 1, vc = 1), tiny = "k12"))
for (nm in names(beta_probes)) {
  b <- beta_probes[[nm]]
  onto <- roots_onto_pole(b$p)
  check(onto[2], nm, ": the probe is meant to put beta onto a pole, but beta does not round onto it")
  add_coupling_row(paste0("numeric_", nm), b$p, b$tiny, "beta", "probe")
  N_BETA_PROBES <- (if (exists("N_BETA_PROBES")) N_BETA_PROBES else 0L) + 1L
}
BAND_LO <- 2^-55
BAND_HI <- 2^-50
# the same through the macro set (item 3): a weight so small that the micro constants returned put a root on a pole.
# MOD-3C-08 (T-165) decides every macro input by its zone, computed at 256 bits on the exact conversion of the doubles
# (macro_zone in models_3c_values.R): (i) refused, (ii) accepted with values, (0) and (iii) free (the engine may refuse
# or accept; when it accepts the obligations are checked: the free cases below). The rows of the first two kinds keep
# their outcomes; those of the band that used to be omitted, and the ones with rho between 2^-50 and 2^-48/tau that
# used to be accepted, are free cases now.
macro_words <- list(c("degenerate", "macro", "valid"), c("a", "b", "c", "offset", "exponent"))
N_MACRO_FREE <- 0L; N_MACRO_I <- 0L; N_MACRO_II <- 0L
FREE_NAMES <- character(0)
free_row <- function(name, p, note) {
  z <- macro_zone(p)
  check(z$zone %in% c("0", "iii"), name, ": expected a free zone, got ", z$zone)
  N_MACRO_FREE <<- N_MACRO_FREE + 1L
  FREE_NAMES <<- c(FREE_NAMES, name)
  n_rows <<- n_rows + write_free_case(name, p, 100, sprintf("%s (zone (%s): log2 rho %.2f, log2 tau %.2f, log2 rho*tau %.2f, limited by %s)",
                                                           note, z$zone, z$log2rho, z$log2tau, z$log2rho_tau, z$limit))
}
refusal_row <- function(id, p, note, words = macro_words, spec = paste(M3, "item 3", "MOD-3C-08")) {
  z <- macro_zone(p)
  check(z$zone == "i", id, ": expected zone (i), got ", z$zone)
  N_MACRO_I <<- N_MACRO_I + 1L
  err(id, "macro", spec, "pk3.iv_bolus", 100, p, words,
      sprintf("%s (zone (i): log2 rho %.2f < -55, limited by %s; DegenerateExponents)", note, z$log2rho, z$limit))
}
for (nm in c("a", "c")) for (e in 17:30) {
  w <- list(a = 0.7, b = 0.3, c = 0.1); w[[nm]] <- 10^-e
  p <- c(w, list(alpha = 3, beta = 0.5, gamma = 0.05))
  z <- macro_zone(p)
  id <- sprintf("macro_weight_%s_1e%d", nm, e)
  if (z$zone == "i") {
    N_COUPLING_REFUSALS <- N_COUPLING_REFUSALS + 1L
    err(id, "macro", paste(M3, "item 3", "MOD-3C-08"), "pk3.iv_bolus", 100, p,
        macro_words, sprintf("weight `%s` = 1e-%d: the micro constants of the macro set put a root on a pole (relative offset %.2g < 2^-55; zone (i), DegenerateExponents)", nm, e, z$rho))
  } else if (z$zone == "iii" || z$zone == "0") {
    free_row(id, p, sprintf("weight `%s` = 1e-%d: in the band between the floor 2^-55 of zone (i) and the accepted zone (ii)", nm, e))
  } else N_COUPLING_ACCEPTED <- N_COUPLING_ACCEPTED + 1L
}
# ---- the edges of the zones, by family: the weight that limits the root is tuned at 256 bits so that the metric hits the edge
#   a-family: a tiny, rho limited by alpha;  c-family: c tiny, by gamma;  b-family: b tiny, by beta (beta next to a pole)
#   rho: 0.9 * 2^-55 (zone (i), refused) | 1.1 * 2^-55 (the bottom of the band, free) | 2^-51.5 (mid-band, free)
#   rho*tau: 0.9 * 2^-48 (the top of the band, free) | 1.1 * 2^-48 (zone (ii), accepted with values)
MACRO_FAMILIES <- list(
  a = list(x0 = 1e-16, make = function(x) list(a = x, b = 0.3, c = 0.1, alpha = 3, beta = 0.5, gamma = 0.05)),
  c = list(x0 = 1e-16, make = function(x) list(a = 0.7, b = 0.3, c = x, alpha = 3, beta = 0.5, gamma = 0.05)),
  b = list(x0 = 1e-16, make = function(x) list(a = 0.7, b = x, c = 0.1, alpha = 3, beta = 0.5, gamma = 0.05)))
tune_macro <- function(F, kind, target, label) {
  x <- F$x0
  for (it in 1:10) {
    z <- macro_zone(F$make(x))
    x <- x * target / (if (kind == "rho") z$rho else z$rho_tau)
  }
  x <- signif(x, 5)
  p <- F$make(x)
  z <- macro_zone(p)
  m <- if (kind == "rho") z$rho else z$rho_tau
  check(abs(m / target - 1) < 2e-3, label, ": tuned ", kind, " ", m, " target ", target)
  p
}
EDGE_II <- character(0)
for (fam in names(MACRO_FAMILIES)) {
  F <- MACRO_FAMILIES[[fam]]
  refusal_row(sprintf("macro_edge_%s_rho_0p9", fam), tune_macro(F, "rho", 0.9 * 2^-55, paste("edge", fam, "rho 0.9")),
              sprintf("family %s, rho = 0.9 * 2^-55, just below the floor of zone (i)", fam))
  free_row(sprintf("macro_edge_%s_rho_1p1", fam), tune_macro(F, "rho", 1.1 * 2^-55, paste("edge", fam, "rho 1.1")),
           sprintf("family %s, rho = 1.1 * 2^-55, just above the floor of zone (i): the bottom of the band", fam))
  free_row(sprintf("macro_edge_%s_mid", fam), tune_macro(F, "rho", 2^-51.5, paste("edge", fam, "mid")),
           sprintf("family %s, rho = 2^-51.5, mid-band", fam))
  free_row(sprintf("macro_edge_%s_rhotau_0p9", fam), tune_macro(F, "rho_tau", 0.9 * 2^-48, paste("edge", fam, "rhotau 0.9")),
           sprintf("family %s, rho * tau = 0.9 * 2^-48, the top of the band", fam))
  pii <- tune_macro(F, "rho_tau", 1.1 * 2^-48, paste("edge", fam, "rhotau 1.1"))
  check(macro_zone(pii)$zone == "ii", fam, ": rho*tau 1.1 * 2^-48 is not zone (ii)")
  N_MACRO_II <- N_MACRO_II + 1L
  nm <- sprintf("macro_edge_%s_rhotau_1p1", fam)
  n_rows <- n_rows + write_case(nm, "pk3.iv_bolus", "macro", pii, 100, list(),
    sprintf("family %s, rho * tau = 1.1 * 2^-48: the first accepted values above the band (zone (ii)); the scalars derived through the micro constants are written with their conditions (scalar_ill where above 1e3)", fam))
}
# ---- tau: the small-tau edge, zone (0) below 2^-20 and (ii) above, on a near-tie micro set converted to the macro doubles
tau_edge_set <- function(target) {
  delta <- 2 * target
  for (it in 1:12) {
    pm <- to_set(list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 - delta, 8), dose = 100), "macro")
    delta <- delta * target / macro_zone(pm)$tau
  }
  pm <- to_set(list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 - delta, 8), dose = 100), "macro")
  check(abs(macro_zone(pm)$tau / target - 1) < 2e-2, "tau edge ", target, ": got ", macro_zone(pm)$tau)
  lapply(pm, function(v) signif(as.numeric(v), 15))
}
free_row("macro_edge_tau_0p9", tau_edge_set(0.9 * 2^-20), "near-tie macro set, tau = 0.9 * 2^-20: zone (0), below the edge")
pt1 <- tau_edge_set(1.1 * 2^-20)
check(macro_zone(pt1)$zone == "ii", "tau 1.1 * 2^-20 is zone (ii)")
n_rows <- n_rows + write_case("macro_edge_tau_1p1", "pk3.iv_bolus", "macro", pt1, 100, list(),
  "near-tie macro set, tau = 1.1 * 2^-20: just above zone (0), accepted with values (zone (ii)); w_beta is of order 1e-13 and the scalars through the micro constants are ill-conditioned")
# ---- the macro path: a sign that the plain evaluation of MOD-3C-04 in doubles cannot decide (N1, MOD-3C-06 item 3)
# The plain double evaluation (n1 and n0 from the weights, the stable quadratic, k12 and k13 from P(x) = (x - alpha)(x - beta)(x - gamma))
# is the one the spec uses to define "failed". The inputs below are in zone (i) at 256 bits AND fail that evaluation with a
# non-positive k12 or k13 (or gap of the poles, or discriminant): the engine must refuse them with DegenerateExponents and a message
# that names the pole and the coupling (not DerivedOutOfRange, not an overflow wording). They are a separate group, macro_n1.
plain_double_macro <- function(p) {
  w <- c(p$a, p$b, p$c) / (p$a + p$b + p$c)
  l <- c(p$alpha, p$beta, p$gamma); S <- l[1] + l[2] + l[3]
  n1 <- w[1] * (S - l[1]) + w[2] * (S - l[2]) + w[3] * (S - l[3])
  n0 <- w[1] * l[2] * l[3] + w[2] * l[1] * l[3] + w[3] * l[1] * l[2]
  disc <- n1 * n1 - 4 * n0
  if (!(disc >= 0)) return("discriminant")
  big <- (n1 + sqrt(disc)) / 2; small <- n0 / big
  P <- function(x) (x - l[1]) * (x - l[2]) * (x - l[3])
  if (!(big > small)) return("gap")
  k12 <- -P(big) / ((big - small) * big); k13 <- -P(small) / ((small - big) * small)
  if (!(k12 > 0)) "k12" else if (!(k13 > 0)) "k13" else "ok"
}
N1_KINDS <- c(k12 = 0L, k13 = 0L, gap = 0L, discriminant = 0L)
N1_CAND <- list(
  a = function(x, v) list(a = x, b = 0.25 * v, c = 0.15 * v, alpha = 4, beta = 0.6, gamma = 0.04),
  c = function(x, v) list(a = 0.6 * v, b = 0.2 * v, c = x, alpha = 4, beta = 0.6, gamma = 0.04),
  b = function(x, v) list(a = 0.55 * v, b = x, c = 0.2 * v, alpha = 4, beta = 0.6, gamma = 0.04))
N1_COUNT <- 0L
for (fam in names(N1_CAND)) for (e in 19:26) for (v in c(1, 3)) {
  p <- N1_CAND[[fam]](10^-e, v)
  if (macro_zone(p)$zone != "i") next
  res <- plain_double_macro(p)
  if (res == "ok") next
  N1_COUNT <- N1_COUNT + 1L
  N1_KINDS[[res]] <- N1_KINDS[[res]] + 1L
  z <- macro_zone(p)
  err(sprintf("macro_n1_%s_1e%d_v%d", fam, e, v), "macro_n1", paste(M3, "item 3", "N1"), "pk3.iv_bolus", 100, p,
      list(c("k21", "k31"), c("k12", "k13"), c("valid")),
      sprintf("weight `%s` = 1e-%d (zone (i), rho %.2g): the plain double evaluation of MOD-3C-04 fails on %s; refused with DegenerateExponents, the message naming the pole (k21 or k31), the coupling (k12 or k13) and saying that the macro set gives no valid three-compartment model", fam, e, z$rho, res))
}
cat(sprintf("N1 rows (zone (i), plain double evaluation fails): %d (%s)\n", N1_COUNT, paste(names(N1_KINDS), N1_KINDS, sep = " ", collapse = ", ")))
# ---- two poles two spacings apart (MOD-3C-22 asks the engine to refuse a beta that rounds onto a pole): k21 = 1 and k31 = 1 - 2^-52,
# one double strictly between them; k13 = 0.7875 puts beta at 0.80 of a spacing from k31: it rounds to the double in the middle,
# accepted; k13 = 0.3 puts it at 0.40: it rounds onto k31, refused
two_spacing <- function(k13) mic(0.375, 1.1875, 1, k13, 1 - 2^-52, 8)
check(!any(roots_onto_pole(two_spacing(0.7875))), "two-spacing pair: k13 = 0.7875 must not round onto a pole")
check(roots_onto_pole(two_spacing(0.3))[2], "two-spacing pair: k13 = 0.3 must put beta onto a pole")
check(!adjacent_poles(derive_q3("micro", two_spacing(0.7875), 100)), "two-spacing pair: the poles must not be adjacent")
n_cond <- n_cond + 1L
n_rows <- n_rows + write_case("cond_tie_two_spacings", "pk3.iv_bolus", "micro", two_spacing(0.7875), 100, list(),
  "k21 = 1 and k31 = 1 - 2^-52, two spacings apart (one double between them); beta lies 0.80 of a spacing from k31, so it rounds to the double in the middle and the model is accepted (w_beta is ill-conditioned: scalar_ill)")
add_coupling_row("numeric_tie_two_spacings_k13_0p3", two_spacing(0.3), "k13", "beta",
                 "k21 = 1 and k31 = 1 - 2^-52, two spacings apart; beta lies 0.40 of a spacing from k31 and rounds onto it")
cat(sprintf("coupling refusals: %d rows (%d at the rounding boundary, %d beta probes); %d inputs accepted (not refused; %d of them at the boundary)\n", N_COUPLING_REFUSALS, N_EDGE_REFUSALS, N_BETA_PROBES, N_COUPLING_ACCEPTED, length(EDGE_ACCEPTED)))
cat(sprintf("macro zones: %d refusals in zone (i), %d accepted edge rows in zone (ii), %d free rows (zones (0) and (iii)), %d N1 rows\n", N_MACRO_I + sum(grepl("^macro_weight_", vapply(errors, function(e) e$id, ""))), N_MACRO_II, N_MACRO_FREE, N1_COUNT))
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
n_err <- write_error_suite("model_pk3_errors", errors, c("domain", "one_compartment", "macro", "macro_n1", "sets", "numeric", "times"),
  "inputs the three-compartment engine must refuse (MOD-3C-06, MOD-3C-26); the expected table has one not-available row (empty value) per case, the reason is in `reason`, the rule in `spec`, and the readable message must contain, for each inner list of `message_contains`, at least one of its words (lower case)")
cat(sprintf("error cases: %d\n", n_err))

source(file.path(script_dir, "models_3c_md.R"))
