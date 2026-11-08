# Caladrius three-compartment model oracle (task T-142): ground truth for specs/models.md section 14
# (MOD-3C-01 to 27) before any engine code exists.
#
#   Rscript oracle/scripts/models_3c_closed_form.R        (from the repository root; deterministic, no random number)
#
# Writes oracle/expected/models/pk3/model_pk3_*.{csv,options.json} and
# crates/caladrius-models/tests/oracle_models_3c.cases. Parts:
#   models_3c_common.R       roots, weights, explicit forms, scalars (256 bits)
#   models_3c_values.R       value cases, conditions, expm / deSolve / eigenvalue cross-checks
#   models_3c_derivatives.R  derivatives (OM-21), refusals (OM-26), multiple dosing (OM-28), the .cases file
#
# Conventions of oracle/scripts/models_2c_closed_form.R are kept: long CSV subject,parameter,value;
# subject t=<time> or scalar; options JSON with the parameters, times, method and tool versions.

args <- commandArgs(trailingOnly = FALSE)
here <- sub("^--file=", "", args[grep("^--file=", args)])
script_dir <- if (length(here)) dirname(normalizePath(here)) else file.path(getwd(), "oracle", "scripts")
root <- normalizePath(file.path(script_dir, "..", ".."))
exp_dir <- file.path(root, "oracle", "expected", "models", "pk3")
dir.create(exp_dir, recursive = TRUE, showWarnings = FALSE)
T_START <- Sys.time()
options(warn = 1)
source(file.path(script_dir, "models_3c_common.R"))
source(file.path(script_dir, "models_3c_values.R"))

# ---------------------------------------------------------------- parameter points
clr <- function(cl, vc, q2, vp2, q3, vp3) list(cl = cl, vc = vc, q2 = q2, vp2 = vp2, q3 = q3, vp3 = vp3)
mic <- function(k10, k12, k21, k13, k31, vc) list(k10 = k10, k12 = k12, k21 = k21, k13 = k13, k31 = k31, vc = vc)
points <- list(
  base = list(set = "clearance", p = clr(3, 8, 9.5, 9.5, 6.3, 31.5), dose = 100),
  fast_dist = list(set = "clearance", p = clr(2, 5, 60, 10, 15, 40), dose = 250),
  small2_3 = list(set = "micro", p = mic(0.4, 1e-3, 1, 0.5, 0.2, 10), dose = 100),
  small2_6 = list(set = "micro", p = mic(0.4, 1e-6, 1, 0.5, 0.2, 10), dose = 100),
  small2_9 = list(set = "micro", p = mic(0.4, 1e-9, 1, 0.5, 0.2, 10), dose = 100),
  small2_12 = list(set = "micro", p = mic(0.4, 1e-12, 1, 0.5, 0.2, 10), dose = 100),
  small3_3 = list(set = "micro", p = mic(0.4, 0.6, 1, 1e-3, 0.2, 10), dose = 100),
  small3_6 = list(set = "micro", p = mic(0.4, 0.6, 1, 1e-6, 0.2, 10), dose = 100),
  small3_9 = list(set = "micro", p = mic(0.4, 0.6, 1, 1e-9, 0.2, 10), dose = 100),
  small3_12 = list(set = "micro", p = mic(0.4, 0.6, 1, 1e-12, 0.2, 10), dose = 100),
  tie_2 = list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 - 1e-2, 8), dose = 100),
  tie_4 = list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 - 1e-4, 8), dose = 100),
  tie_6 = list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 - 1e-6, 8), dose = 100),
  tie_9 = list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 - 1e-9, 8), dose = 100),
  tie_12 = list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 - 1e-12, 8), dose = 100),
  tie_exact = list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1, 8), dose = 100),
  # adjacent poles (MOD-3C-22, T-165): k21 and k31 are neighbouring doubles, no double strictly between them. The F1 case
  # (k21 = 1, k31 the adjacent double below), the pair above, pairs at 0.3 and at a power of two (where the spacing differs
  # on the two sides), and the swapped labelling
  adj_below = list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 - 2^-53, 8), dose = 100),
  adj_up = list(set = "micro", p = mic(0.375, 1.1875, 1, 0.7875, 1 + 2^-52, 8), dose = 100),
  adj_0p3 = list(set = "micro", p = mic(0.375, 1.1875, 0.3, 0.7875, 0.3 + 2^-54, 8), dose = 100),
  adj_0p3_swapped = list(set = "micro", p = mic(0.375, 1.1875, 0.3 + 2^-54, 0.7875, 0.3, 8), dose = 100),
  adj_pow2_up = list(set = "micro", p = mic(0.375, 1.1875, 0.5, 0.7875, 0.5 + 2^-53, 8), dose = 100),
  adj_pow2_down = list(set = "micro", p = mic(0.375, 1.1875, 0.5, 0.7875, 0.5 - 2^-54, 8), dose = 100),
  tie_clr = list(set = "clearance", p = clr(3, 8, 9.5, 9.5, 6.3, 6.3), dose = 100),
  swapped = list(set = "micro", p = mic(0.375, 0.7875, 0.2, 1.1875, 1, 8), dose = 100),
  ratio_1e3 = list(set = "macro", p = list(a = 7, b = 2, c = 1, alpha = 10, beta = 0.5, gamma = 0.01), dose = 100),
  ratio_1e6 = list(set = "macro", p = list(a = 7, b = 2, c = 1, alpha = 1000, beta = 1, gamma = 0.001), dose = 100),
  close_exp = list(set = "micro", p = mic(0.2, 1e-4, 0.2, 1e-4, 0.2 * (1 - 1e-3), 10), dose = 100),
  slow = list(set = "micro", p = mic(1e-4, 2e-4, 1e-3, 1e-4, 5e-4, 10), dose = 100),
  fast = list(set = "micro", p = mic(100, 300, 50, 200, 20, 10), dose = 100),
  base_macro = list(set = "macro", p = list(a = 9.491525423728813, b = 1.6666666666666667, c = 1.3418079096045198,
                                              alpha = 3, beta = 0.5, gamma = 0.05), dose = 100)
)
to_set <- function(pt, set) {
  if (pt$set == set) return(pt$p)
  q <- derive_q3(pt$set, pt$p, pt$dose)
  r <- switch(set,
    clearance = list(cl = q$cl, vc = q$V, q2 = q$q2, vp2 = q$vp2, q3 = q$q3, vp3 = q$vp3),
    micro = list(k10 = q$k10, k12 = q$k12, k21 = q$k21, k13 = q$k13, k31 = q$k31, vc = q$V),
    macro = list(a = q$A[1], b = q$A[2], c = q$A[3], alpha = q$l[1], beta = q$l[2], gamma = q$l[3]))
  lapply(r, function(x) signif(asNumeric(x), 12))
}

# scale of the time-like extras of a point
ext <- function(model, pname, kind = NULL) {
  pt <- points[[pname]]
  q <- derive_q3(pt$set, pt$p, pt$dose)
  l <- asNumeric(q$l)
  ex <- list()
  if (is_first_order(model)) ex$ka <- signif(sqrt(l[1] * l[2]) * 1.1, 3)
  if (is_zero_order(model)) ex$dur <- signif(1 / l[2], 2)
  if (model %in% c("pk3.oral_0_lag", "pk3.oral_1_lag")) ex$tlag <- signif(0.25 / l[2], 2)
  ex
}
short <- function(model) sub("^pk3\\.", "", model)
IDS <- c("pk3.iv_bolus", "pk3.iv_infusion", "pk3.oral_1", "pk3.oral_1_lag", "pk3.oral_0", "pk3.oral_0_lag")
n_rows <- 0L
add_case <- function(model, name, pname, set = NULL, extra = NULL, dose = NULL, note = "", textbook_ok = NULL) {
  pt <- points[[pname]]
  set <- set %||% pt$set
  extra <- extra %||% ext(model, pname)
  if (model == "pk3.iv_infusion") extra$tlag <- NULL
  n_rows <<- n_rows + write_case(paste0(short(model), "_", name), model, set, to_set(pt, set),
                                  dose %||% pt$dose, extra, note, textbook_ok)
}

# ---- Q1 to Q5 and the families of OM-19
base_p <- points$base
for (id in IDS) {
  # base case: the worked examples Q1 (bolus), Q2 (ka = 1.5), Q3 (dur = 2)
  ex <- list()
  if (is_first_order(id)) ex$ka <- 1.5
  if (is_zero_order(id)) ex$dur <- 2
  if (id %in% c("pk3.oral_0_lag", "pk3.oral_1_lag")) ex$tlag <- 0.5
  add_case(id, "base", "base", extra = ex, note = "worked example of specs/models.md section 14.6 (Q1 bolus, Q2 ka = 1.5, Q3 dur = 2)")
  for (pn in c("fast_dist", "small2_6", "small2_12", "small3_9", "tie_4", "tie_9", "tie_exact", "tie_clr",
               "swapped", "ratio_1e3", "ratio_1e6", "close_exp", "slow", "fast", "base_macro"))
    add_case(id, pn, pn, note = paste("point", pn))
  add_case(id, "base_micro", "base", set = "micro", extra = ex, note = "base case in the micro set")
  add_case(id, "base_macro_set", "base", set = "macro", extra = ex, note = "base case in the macro set")
  add_case(id, "zero_dose", "base", dose = 0, extra = ex, note = "dose 0: every value is 0")
}
# the bolus carries every family (no extras to vary)
for (pn in c("small2_3", "small2_9", "small2_12", "small3_3", "small3_6", "small3_12", "tie_2", "tie_6", "tie_12"))
  add_case("pk3.iv_bolus", pn, pn, note = paste("point", pn))
# the other ids carry the near ties and small exchanges at the extreme steps too
for (id in IDS[-1]) for (pn in c("small3_12", "tie_6", "tie_12"))
  add_case(id, pn, pn, note = paste("point", pn))

# zero-order variants
for (id in IDS[is_zero_order(IDS)]) {
  tl <- if (id == "pk3.oral_0_lag") list(tlag = 0.5) else list()
  add_case(id, "dur_short", "base", extra = c(list(dur = 0.01), tl), note = "very short input")
  add_case(id, "dur_long", "base", extra = c(list(dur = 50), tl), note = "input much longer than the mean lives")
}
# first-order: ka families (Q2, Q5, OM-19)
for (id in IDS[is_first_order(IDS)]) {
  tl <- if (id == "pk3.oral_1_lag") list(tlag = 0.5) else list()
  eq <- c(alpha = 3, beta = 0.5, gamma = 0.05)
  for (nm in names(eq))
    add_case(id, paste0("ka_eq_", nm), "base_macro", extra = c(list(ka = eq[[nm]]), tl),
             note = paste("ka =", nm, "exactly (macro set): the confluent limit of MOD-3C-12"))
  for (nm in names(eq)) for (d in c(1e-3, 1e-6, 1e-9)) {
    if (id == "pk3.oral_1_lag" && !(nm == "beta" || d == 1e-6)) next
    add_case(id, sprintf("ka_%s_p%s", nm, format(d, scientific = TRUE)), "base",
             extra = c(list(ka = eq[[nm]] * (1 + d)), tl), note = paste("ka =", nm, "* (1 +", d, ")"))
  }
  for (nm in names(eq)) {
    d <- c(alpha = 1e-9, beta = 1e-6, gamma = 1e-3)[[nm]]
    add_case(id, sprintf("ka_%s_m%s", nm, format(d, scientific = TRUE)), "base",
             extra = c(list(ka = eq[[nm]] * (1 - d)), tl), note = paste("ka =", nm, "* (1 -", d, ")"))
  }
  add_case(id, "ka_between_ab", "base", extra = c(list(ka = 1.5), tl), note = "ka between alpha and beta (Q2)")
  add_case(id, "ka_between_bg", "base", extra = c(list(ka = 0.2), tl), note = "ka between beta and gamma")
  add_case(id, "ka_below_gamma", "base", extra = c(list(ka = 0.01), tl), note = "ka below gamma (flip-flop)")
  add_case(id, "ka_500_alpha", "base", extra = c(list(ka = 1500), tl), note = "ka / alpha = 500")
  add_case(id, "ka_near_tie", "tie_6", extra = c(list(ka = 0.9999995), tl), note = "ka at the near-tie exponent, relative 5e-7")
}

# ---------------------------------------------------------------- conditioning (OM-22)
n_cond <- 0L
add_cond <- function(name, set, p, dose, note) {
  n_cond <<- n_cond + 1L
  write_case(paste0("cond_", name), "pk3.iv_bolus", set, p, dose, list(), note, conditioning = TRUE)
}
for (dl in c(1e-2, 1e-4, 1e-6, 1e-9, 1e-12))
  add_cond(sprintf("tie_%s", format(dl, scientific = TRUE)), "micro", mic(0.375, 1.1875, 1, 0.7875, 1 - dl, 8), 100,
           paste("H22 family, k31 = k21 (1 -", dl, "): w_beta is O(delta^2) and ill-conditioned by nature (MOD-3C-08)"))
add_cond("tie_exact", "micro", mic(0.375, 1.1875, 1, 0.7875, 1, 8), 100, "exact tie k21 = k31: beta = k21, w_beta = 0 exactly (structural)")
# adjacent poles (MOD-3C-22, T-165): VALUE cases with a time grid (conc, auc, aumc at 1e-12 relative) and the scalars; w_beta is
# in the group scalar_abs (1e-12 absolute, whatever its condition number), the other scalars keep their groups. The first two
# were scalar-only conditioning cases until round 4; the same inputs, the same scalars.
add_adj <- function(name, pn, note) {
  n_cond <<- n_cond + 1L
  pt <- points[[pn]]
  check(adjacent_poles(derive_q3("micro", pt$p, 100)), name, ": the poles k21 and k31 are not adjacent doubles")
  write_case(paste0("cond_", name), "pk3.iv_bolus", "micro", pt$p, pt$dose, list(), note)
}
add_adj("tie_ulp", "adj_below", "k31 one ulp below k21 (the F1 case: k21 = 1, k31 = 1 - 2^-53, adjacent doubles, a power of two on the lower side)")
add_adj("tie_ulp_up", "adj_up", "k31 one ulp above k21 (adjacent doubles, k21 = 1)")
add_adj("tie_ulp_0p3", "adj_0p3", "k21 = 0.3 and k31 the next double above it (adjacent doubles, not a power of two)")
add_adj("tie_ulp_0p3_swapped", "adj_0p3_swapped", "k31 = 0.3 and k21 the next double above it (the labelling of the previous case swapped)")
add_adj("tie_ulp_pow2_up", "adj_pow2_up", "k21 = 0.5 (a power of two) and k31 the next double above it: the wider spacing 2^-53")
add_adj("tie_ulp_pow2_down", "adj_pow2_down", "k21 = 0.5 (a power of two) and k31 the next double below it: the narrower spacing 2^-54")
# couplings small but not too small: the roots still differ from the poles in doubles, so the model must be
# ACCEPTED and its exponents and weights returned (MOD-3C-08); the refusals start further down (models_3c_more.R)
for (pr in c(1e-12, 1e-13))
  add_cond(sprintf("coupling_alpha_1e%d", round(log10(pr))), "micro", mic(1, pr / 10, 10, 0.5, 0.2, 1), 100,
           sprintf("k12 * k21 = %g: alpha is above k21 by a relative %.2g, still representable: accepted", pr, rel_offsets("micro", mic(1, pr / 10, 10, 0.5, 0.2, 1))[["alpha"]]))
for (pr in c(1e-12, 1e-15))
  add_cond(sprintf("coupling_gamma_1e%d", round(log10(pr))), "micro", mic(1, 0.5, 1, pr / 0.2, 0.2, 1), 100,
           sprintf("k13 * k31 = %g: gamma is below k31 by a relative %.2g, still representable: accepted", pr, rel_offsets("micro", mic(1, 0.5, 1, pr / 0.2, 0.2, 1))[["gamma"]]))
add_cond("tie_swapped_9", "micro", mic(0.375, 0.7875, 1 - 1e-9, 1.1875, 1, 8), 100, "k21 just below k31")
for (e in c(3, 6, 9, 12)) {
  add_cond(sprintf("k12_%d", e), "micro", mic(0.4, 10^-e, 1, 0.5, 0.2, 10), 100, paste("k12 = 1e-", e, sep = ""))
  add_cond(sprintf("k13_%d", e), "micro", mic(0.4, 0.6, 1, 10^-e, 0.2, 10), 100, paste("k13 = 1e-", e, sep = ""))
}
add_cond("both_tiny", "micro", mic(0.4, 1e-9, 1, 1e-9, 0.2, 10), 100, "both couplings tiny")
add_cond("ratio_1e6", "macro", points$ratio_1e6$p, 100, "alpha / gamma = 1e6")
add_cond("close_exp", "micro", points$close_exp$p, 100, "three very close exponents")
# ---- well-conditioned sets with one large coupling (T-142 round 3): k12 or k13 many orders above beta,
# kappa about 1 for the exponents, so the engine must keep 1e-12 relative on beta and on gamma next to its pole
# (the first three are the sets measured by the reviewer against a 300-bit reference). Value cases with a
# time grid: the bolus for every set, the two other inputs for the reviewer sets and the deep compartment.
# A case whose name has _pole_ claims that one root sits within a stated relative distance of one pole; the
# generator checks that claim at 256 bits (the claimed root is the one nearest to that pole, and within the stated
# distance), and that the note says it in these words: "<root> within a relative <distance> of its pole <k21|k31>".
spacing_at <- function(x, above) { e <- floor(log2(x)); if (above || x != 2^e) 2^(e - 52) else 2^(e - 53) }
check_pole_claim <- function(name, p, note, root, pole, maxrel, side) {
  q <- derive_q3("micro", p, 100, list())
  pv <- if (pole == "k21") q$k21 else q$k31
  d <- sapply(1:3, function(i) asNumeric(abs(q$l[i] - pv) / pv))
  want <- match(root, c("alpha", "beta", "gamma"))
  check(which.min(d) == want, name, ": the root nearest to the pole ", pole, " is not ", root, " (relative distances ", paste(signif(d, 3), collapse = ", "), ")")
  check(d[want] <= maxrel, name, ": ", root, " is ", signif(d[want], 3), " from the pole ", pole, ", not within ", maxrel)
  check(grepl(sprintf("%s within a relative %g of its pole %s", root, maxrel, pole), note, fixed = TRUE), name, ": the note does not state the claim")
  # the side (T-165): the root lies above or below its pole, measured at 256 bits, and the note says so
  above <- q$l[want] > pv
  check(side %in% c("above", "below"), name, ": the claim states the side, above or below")
  check((side == "above") == above, name, ": ", root, " lies ", if (above) "above" else "below", " the pole ", pole, ", the claim says ", side)
  check(grepl(sprintf("(%s it)", side), note, fixed = TRUE), name, ": the note does not say '(", side, " it)'")
  sp <- spacing_at(asNumeric(pv), above)
  half <- asNumeric(abs(q$l[want] - pv) / (mpfr(sp, PREC) / 2))
  POLE_CLAIMS[[paste0("lc_", name)]] <<- list(pole_root = root, pole = pole, pole_side = side, pole_max_relative_distance = maxrel,
                                              pole_relative_distance = signif(d[want], 6), pole_distance_half_spacings = signif(half, 6))
  cat(sprintf("  pole claim %s: %s is %.2g (relative) %s %s, %.3g half-spacings of the doubles\n", name, root, d[want], side, pole, half))
}
add_lc <- function(name, p, note, ids = "pk3.iv_bolus", pole = NULL) {
  pn <- paste0("lc_", name)
  if (grepl("_pole_", name, fixed = TRUE)) {
    check(!is.null(pole), name, ": a case named _pole_ states the root, the pole, the side and the distance it claims")
    check_pole_claim(name, p, note, pole$root, pole$pole, pole$maxrel, pole$side)
  }
  points[[pn]] <<- list(set = "micro", p = p, dose = 100)
  # the textbook flag is measured by write_case (T-165), not declared here
  for (id in ids) add_case(id, pn, pn, note = paste("large coupling:", note))
}
EXTRA_IDS <- c("pk3.iv_bolus", "pk3.oral_1_lag", "pk3.oral_0", "pk3.iv_infusion")
add_lc("rev1", mic(7.83e-1, 4.52e5, 1.27e10, 1.69e-4, 1.08e-1, 2), "k12 = 4.5e5, k21 = 1.3e10 (reviewer set 1)", EXTRA_IDS)
add_lc("rev2", mic(5.38e-5, 1.04e3, 9.03e2, 2.65e-5, 1.72e-4, 2), "k12 = 1e3, k21 = 9e2 (reviewer set 2)", EXTRA_IDS)
add_lc("rev3", mic(1.16e-6, 3.35, 12.2, 1.51e-11, 6.01e-9, 2), "k13 = 1.5e-11, k31 = 6e-9 (reviewer set 3)", EXTRA_IDS)
for (e in c(3, 5, 8)) {
  add_lc(sprintf("k12_1e%d_a", e), mic(0.4, 10^e, 1, 0.5, 0.2, 5), sprintf("k12 = 1e%d, k21 > k31", e))
  add_lc(sprintf("k12_1e%d_b", e), mic(0.4, 10^e, 0.2, 0.5, 1, 5), sprintf("k12 = 1e%d, k21 < k31", e))
  add_lc(sprintf("k13_1e%d_a", e), mic(0.4, 0.5, 1, 10^e, 0.2, 5), sprintf("k13 = 1e%d, k21 > k31", e))
  add_lc(sprintf("k13_1e%d_b", e), mic(0.4, 0.5, 0.2, 10^e, 1, 5), sprintf("k13 = 1e%d, k21 < k31", e))
}
add_lc("both_1e4_1e6", mic(0.4, 1e4, 1, 1e6, 0.2, 5), "k12 = 1e4 and k13 = 1e6")
add_lc("both_1e7_1e5", mic(0.4, 1e7, 1, 1e5, 0.2, 5), "k12 = 1e7 and k13 = 1e5")
# with k10 = 0.4 the rest of the secular function is negative at the pole, so the root next to the tiny coupling lies
# ABOVE it and is beta (gamma is then 4e-6, far from any pole): these two are beta cases, named so
add_lc("beta_pole_a", mic(0.4, 1e5, 1, 1e-8, 0.2, 5), "k12 = 1e5 large; beta within a relative 1e-12 of its pole k31 (above it); gamma is far from both poles", pole = list(root = "beta", pole = "k31", maxrel = 1e-12, side = "above"))
add_lc("beta_pole_b", mic(0.4, 1e-8, 0.2, 1e5, 1, 5), "k13 = 1e5 large; beta within a relative 1e-12 of its pole k21 (above it); gamma is far from both poles", pole = list(root = "beta", pole = "k21", maxrel = 1e-12, side = "above"))
# with a large k10 the rest of the function is positive at the pole: the root next to the tiny coupling lies BELOW
# it and is gamma (k10 = 3e4, reviewer proposal of round 4)
add_lc("gamma_pole_a", mic(3e4, 1e5, 1, 1e-8, 0.2, 5), "k12 = 1e5 large; gamma within a relative 3e-12 of its pole k31 (below it); beta 0.2308 and alpha 1.3e5", EXTRA_IDS, pole = list(root = "gamma", pole = "k31", maxrel = 3e-12, side = "below"))
add_lc("gamma_pole_b", mic(3e4, 1e-8, 0.2, 1e5, 1, 5), "k13 = 1e5 large; gamma within a relative 3e-12 of its pole k21 (below it); beta 0.2308 and alpha 1.3e5", EXTRA_IDS, pole = list(root = "gamma", pole = "k21", maxrel = 3e-12, side = "below"))
add_lc("deep_a", mic(0.3, 10, 5, 0.01, 1e-3, 10), "realistic deep compartment: k31 = 1e-3, k12 = 10", EXTRA_IDS)
add_lc("deep_b", mic(0.3, 0.01, 1e-3, 10, 5, 10), "realistic deep compartment, peripherals swapped")
# END-LARGE-COUPLING

# ---------------------------------------------------------------- conversions (OM-23)
n_conv <- 0L
conv_points <- c("base", "fast_dist", "small2_3", "small3_3", "tie_2", "tie_4", "tie_6", "close_exp", "ratio_1e3",
                 "slow", "fast", "swapped", "base_macro")
roundtrip_err <- numeric(0)
for (pn in conv_points) {
  pt <- points[[pn]]
  q0 <- derive_q3(pt$set, pt$p, pt$dose)
  # 256-bit round trip micro -> clearance -> macro -> micro (the macro values are the exact ones)
  back <- exp_to_micro(q0$l, q0$w, q0$V)
  err <- max(sapply(c("k10", "k12", "k21", "k13", "k31"), function(n) asNumeric(abs(back[[n]] - q0[[n]]) / q0[[n]])))
  # the macro set does not tell the two peripheral compartments apart (MOD-3C-05): the pairs
  # (k12, k21) and (k13, k31) may come back exchanged, which is the same model
  sw <- c(k10 = "k10", k12 = "k13", k21 = "k31", k13 = "k12", k31 = "k21")
  err_sw <- max(sapply(names(sw), function(n) asNumeric(abs(back[[sw[[n]]]] - q0[[n]]) / q0[[n]])))
  err <- min(err, err_sw)
  roundtrip_err <- c(roundtrip_err, err)
  gap <- min(asNumeric((q0$l[2] - q0$l[3]) / q0$l[2]), asNumeric(abs(q0$k21 - q0$k31) / q0$k21))
  check(err < 1e-20, pn, ": round trip error ", err)
  # a swapped labelling: when k21 < k31 the macro -> micro conversion returns the pair swapped (MOD-3C-05)
  for (set in c("clearance", "micro", "macro")) {
    if (set == "macro" && gap < 1e-3) next
    n_conv <- n_conv + 1L
    p <- to_set(pt, set)
    write_case(paste0("conv_", pn, "_", set), "pk3.iv_bolus", set, p, pt$dose, list(),
               paste("conversion of point", pn, "given in the", set, "set: the other two sets are the expected scalars; k21_ge_k31 is the labelling flag"),
               conditioning = TRUE)
  }
}
for (nm in ls(REG)) if (grepl("model_pk3_.*conv_", nm)) NULL
# the conversion cases carry their kind in the registry
for (nm in ls(REG)) if (grepl("_conv_", nm)) register(nm, "conversion")
cat(sprintf("round trip: %d points, largest 256-bit relative error %.3g\n", length(roundtrip_err), max(roundtrip_err)))

# ---------------------------------------------------------------- Q1 to Q5 at the printed digits
q_ref <- function(file, t, expected, label) {
  tab <- read.csv(file.path(exp_dir, file), stringsAsFactors = FALSE)
  got <- tab$value[tab$subject == paste0("t=", num17(t)) & tab$parameter == "conc"]
  check(length(got) == 1L && abs(got - expected) <= 6e-13, "Q check ", label, ": got ", got, " expected ", expected)
  cat(sprintf("  %-34s %.12f  (printed %.12f)  ok\n", label, got, expected))
}
cat("Q1 to Q5 at the printed digits:\n")
q_ref("model_pk3_iv_bolus_base.csv", 1, 2.759806823685, "Q1 bolus C(1)")
q_ref("model_pk3_iv_bolus_base.csv", 12, 0.740531047822, "Q1 bolus C(12)")
q_ref("model_pk3_oral_1_base.csv", 1, 3.614449561800, "Q2 oral_1 ka = 1.5 C(1)")
q_ref("model_pk3_oral_1_base.csv", 12, 0.767989856328, "Q2 oral_1 ka = 1.5 C(12)")
q_ref("model_pk3_iv_infusion_base.csv", 1, 2.813351373178, "Q3 infusion dur = 2 C(1)")
q_ref("model_pk3_iv_infusion_base.csv", 12, 0.781577082298, "Q3 infusion dur = 2 C(12)")
q_ref("model_pk3_oral_1_ka_eq_alpha.csv", 1, 3.761216400803, "Q5 oral_1 ka = alpha C(1)")
{
  tab <- read.csv(file.path(exp_dir, "model_pk3_iv_bolus_base.csv"), stringsAsFactors = FALSE)
  sc <- setNames(tab$value[tab$subject == "scalar"], tab$parameter[tab$subject == "scalar"])
  chk <- c(alpha = 3, beta = 0.5, gamma = 0.05, w_alpha = 0.759322033898, w_beta = 0.133333333333,
           w_gamma = 0.107344632768, vss = 49, vz = 60, v_extrap = 74.526315789474, mrt_system = 49 / 3)
  for (n in names(chk)) check(abs(sc[[n]] - chk[[n]]) <= 6e-13 * max(1, chk[[n]]), "Q1 scalar ", n)
  check(abs(sc[["auc_inf"]] - 100 / 3) < 1e-12 && abs(sc[["aumc_inf"]] - 544.4444444444) < 1e-9, "Q1 areas")
  cat("  Q1 scalars (roots, weights, volumes, MRT, areas) ok\n")
}

source(file.path(script_dir, "models_3c_derivatives.R"))
cat(sprintf("done: runtime %.1f minutes, %d values in value cases, %d values written as 0 below the double range\n",
            as.numeric(difftime(Sys.time(), T_START, units = "mins")), n_rows, N_UNDERFLOW))
