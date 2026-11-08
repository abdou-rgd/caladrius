# Caladrius population-simulation oracle, card T-158: the quantile of type 7 (SIM-SUM-01, SX5), the invariance requests
# (SIM-ORA-05), and the round trip with the fit (SIM-ORA-06). Needs sim_lib.R, sim_exact.R and sim_study.R.
#
# quantile/cases.json   : arrays of doubles (hex) and probabilities, the expected type 7 quantile as a hex double, compared
#                         bit for bit (no sum, no libm: a sort and one expression, SIM-SUM-01).
# invariance/cases.json : pairs of requests whose answers must agree (or differ) in the stated fields; the engine is compared
#                         with itself, so no number of R is needed, except the eta of the reversed run (the guard of item 6).
# roundtrip/*.json      : the call (seed, design, K replicates) and the aggregate rule of the two sealed designs of POP-ORA-04.

# ------------------------------------------------------------------------------------------------ quantile
quantile_stage <- function(out_dir) {
  q_case <- function(name, why, x, p) {
    e <- quantile7(x, p)
    stopifnot(identical(e, unname(quantile(x, p, type = 7))))        # R's own quantile, to the bit
    list(name = name, why = why, x = arr(vapply(x, hex_fmt, "")), p = arr(vapply(p, hex_fmt, "")), expected = arr(vapply(e, hex_fmt, "")))
  }
  s5 <- study_one("sx5", "", "20261108", 19, "additive", list(a = "0.2"), labels = "1")
  y19 <- vapply(seq_len(19), function(i) as.numeric(s5$mp$subjects[[i]]$y[4]), 0)
  p3 <- c(0.05, 0.5, 0.95)
  q19 <- quantile7(y19, p3)
  close_to(q19, c(0.85540657059604297, 1.378773104187816, 1.8214186412665074), 1e-14, "SX5 quantiles")
  # the form the spec rejects: x_lo + g (x_hi - x_lo) differs in the last digit at p = 0.05 (SX5)
  xs <- sort(y19); h <- 1 + 18 * 0.05; g <- h - floor(h)
  alt <- xs[1] + g * (xs[2] - xs[1])
  cases <- list(
    q_case("sx5_19_values", "SX5: the 19 values y(t = 4) of subject 1, replicates 0..18 (the doubles of the reference), p = 0.05, 0.5, 0.95; the values are in replicate order, the engine sorts",
           y19, p3),
    q_case("sx5_19_values_sorted", "the same values sorted ascending", sort(y19), p3),
    q_case("sx5_19_values_reversed", "the same values sorted descending", sort(y19, decreasing = TRUE), p3),
    q_case("four_values", "SX5: 1, 2, 3, 10 at p = 0.25, 0.5, 0.9 give 1.75, 2.5, 7.9", c(1, 2, 3, 10), c(0.25, 0.5, 0.9)),
    q_case("r_equals_one", "R = 1: every quantile equals the value", 3.25, c(0, 0.05, 0.5, 0.95, 1)),
    q_case("ends", "p = 0 is the minimum and p = 1 the maximum", c(5, -2, 9, 0.5), c(0, 1)),
    q_case("two_values", "R = 2", c(1, 2), c(0.1, 0.5, 0.9)),
    q_case("ties", "equal values", c(2, 2, 2, 5, 5), c(0.1, 0.4, 0.5, 0.75, 0.9)),
    q_case("negative_values", "negative and positive values", c(-3.5, -0.25, 0, 1.5, 4, -1e-3), c(0.05, 0.5, 0.95)),
    q_case("grid_of_probabilities", "21 probabilities, 7 values", c(0.3, 1.7, 2.2, 2.9, 3.1, 8, 9.5), seq(0, 1, by = 0.05)))
  # a case that separates the two forms: the first (adjacent sorted pair, probability) where x_lo + g (x_hi - x_lo) is not (1 - g) x_lo + g x_hi
  sep <- NULL
  for (i in 1:18) for (pp in c(0.05, 0.1, 0.15, 0.3, 0.35, 0.65, 0.7, 0.85)) if (is.null(sep)) {
    xx <- xs[c(i, i + 1)]; gg <- (1 + pp) - 1
    if (((1 - gg) * xx[1] + gg * xx[2]) != xx[1] + gg * (xx[2] - xx[1])) sep <- list(x = xx, p = pp)
  }
  stopifnot(!is.null(sep))
  cases[[length(cases) + 1L]] <- q_case("forms_differ", "two values and a probability at which x_lo + g (x_hi - x_lo) differs from (1 - g) x_lo + g x_hi in the last digit; the expected value is (1 - g) x_lo + g x_hi", sep$x, sep$p)
  alt_differs <- hex_parse(cases[[1]]$expected[[1]]) != alt;stopifnot(abs(alt - q19[1]) < 1e-15)
  write_json_lf(list(rule = "SIM-SUM-01, SX5", RNGkind = paste(RNGkind(), collapse = "/"),
                     note = "x and expected are hexadecimal doubles compared as parsed values; the quantile is compared bit for bit; type 7 in the form (1 - g) x_lo + g x_hi",
                     alt_form_at_p_0_05 = hex_fmt(alt), alt_form_differs_on_these_doubles = alt_differs, cases = cases), file.path(out_dir, "quantile", "cases.json"))
  cat(sprintf("sim_extras: quantile %d cases; the rejected form differs at p = 0.05 by %.3g\n", length(cases), abs(alt - q19[1])))
  invisible(cases)
}

# ------------------------------------------------------------------------------------------------ invariance
inv_times <- c(0.5, 1, 2, 4, 8, 12)
inv_request <- function(labels = as.character(1:6), R = 10, threads = NULL, error_model = "combined", xi = list(a = 0.2, b = 0.1), lloq = 0.8, negative = "keep",
                        model = "pk1.iv_bolus", theta = list(cl = 5, v = 50), re = c("cl", "v"), omega_shape = "block", omega = c(0.09, 0.03, 0.04), seed = "20261108") {
  opt <- list(negative = negative, quantiles = arr(c(0.05, 0.5, 0.95)))
  if (!is.null(lloq)) opt$lloq <- lloq
  if (!is.null(threads)) opt$threads <- threads
  list(op = "simulate", model = model, theta = theta, random_effects = arr(re), omega_shape = omega_shape, omega = arr(omega), error_model = error_model, xi = xi,
       design = list(subjects = lapply(labels, function(l) list(id = l, dose = 100, time = arr(inv_times)))), seed = seed, replicates = R, options = opt)
}

invariance_stage <- function(out_dir) {
  base <- inv_request(threads = 1)
  cmp <- function(name, rule, why, a, b, relation = "equal", fields = NULL, tables = c("subjects", "observations"), ids = NULL, max_rep = NULL, eta_len = NULL, note = NULL) {
    c <- list(name = name, rule = rule, why = why, kind = if (is.null(fields) && relation == "equal") "answers_identical" else "compare", relation = relation, a = a, b = b)
    if (c$kind == "compare") { c$tables <- arr(tables); c$fields <- arr(fields) }
    if (!is.null(ids)) c$ids <- arr(ids)
    if (!is.null(max_rep)) c$max_rep <- max_rep
    if (!is.null(eta_len)) c$eta_len <- eta_len
    if (!is.null(note)) c$note <- note
    c
  }
  cells <- 6L * 10L
  stopifnot(cells %% 7L != 0L, 64L > cells)
  all_obs <- c("pred", "ipred", "sigma", "eps", "y_raw", "y", "cens", "dv")
  checks <- list(
    cmp("threads_2", "SIM-ORA-05 (1); SIM-DET-01", "1 thread against 2 threads: the whole answer is identical bit for bit", base, inv_request(threads = 2)),
    cmp("threads_7", "SIM-ORA-05 (1); SIM-DET-01", "1 thread against 7 threads: 60 cells (6 subjects x 10 replicates), 7 does not divide 60", base, inv_request(threads = 7)),
    cmp("threads_64", "SIM-ORA-05 (1); SIM-DET-01", "1 thread against 64 threads: more threads than the 60 cells", base, inv_request(threads = 64)),
    cmp("threads_default", "SIM-ORA-05 (1); SIM-DET-01", "1 thread against the default (the option omitted)", base, inv_request()),
    cmp("subject_alone", "SIM-ORA-05 (2); SIM-RNG-03", "subject \"4\" alone equals subject \"4\" inside the design of six (every field of both tables)", base, inv_request(labels = "4", threads = 1),
        fields = c("eta", "p", all_obs), ids = "4"),
    cmp("subject_order_reversed", "SIM-ORA-05 (2); SIM-RNG-03", "the subjects in reverse order give the same rows for every subject", base, inv_request(labels = as.character(6:1), threads = 1),
        fields = c("eta", "p", all_obs)),
    cmp("subject_removed", "SIM-ORA-05 (2); SIM-RNG-03", "removing subjects \"1\" and \"2\" changes nothing for the others", base, inv_request(labels = as.character(3:6), threads = 1),
        fields = c("eta", "p", all_obs), ids = as.character(3:6)),
    cmp("replicate_prefix", "SIM-ORA-05 (3); SIM-RNG-03", "a run of 4 replicates equals replicates 0..3 of a run of 10", base, inv_request(R = 4, threads = 1),
        fields = c("eta", "p", all_obs), max_rep = 3),
    cmp("replicate_prefix_one", "SIM-ORA-05 (3)", "a run of 1 replicate equals replicate 0 of a run of 10", base, inv_request(R = 1, threads = 1),
        fields = c("eta", "p", all_obs), max_rep = 0),
    cmp("error_model_change", "SIM-ORA-05 (4); SIM-RNG-06", "another error model leaves eta, eps and ipred unchanged", base, inv_request(threads = 1, error_model = "additive", xi = list(a = 0.2)),
        fields = c("eta", "p", "eps", "pred", "ipred")),
    cmp("lloq_change", "SIM-ORA-05 (4)", "no LLOQ leaves eta, eps and ipred unchanged", base, inv_request(threads = 1, lloq = NULL), fields = c("eta", "p", "eps", "pred", "ipred")),
    cmp("negative_change", "SIM-ORA-05 (4)", "negative = zero leaves eta, eps and ipred unchanged", base, inv_request(threads = 1, negative = "zero"), fields = c("eta", "p", "eps", "pred", "ipred")))
  # (5) appending a random effect at the end
  oral <- function(re, omega, shape = "block") inv_request(labels = as.character(1:4), R = 6, threads = 1, model = "pk1.oral_1", theta = list(cl = 5, v = 50, ka = 1.5), re = re, omega_shape = shape, omega = omega)
  checks[[length(checks) + 1L]] <- cmp("append_random_effect", "SIM-ORA-05 (5); SIM-MOD-03", "appending ka at the end of random_effects (Omega extended by a last row) leaves the first two eta and all eps unchanged",
    oral(c("cl", "v"), c(0.09, 0.03, 0.04)), oral(c("cl", "v", "ka"), c(0.09, 0.03, 0.04, 0.01, 0.015, 0.0625)), fields = c("eta", "eps"), tables = c("subjects", "observations"), eta_len = 2,
    note = "eta is compared on its first 2 entries; the individual parameters and the predictions differ (ka now varies), and are not compared")
  # (6) reversal changes eta; the reference values of the reversed run
  rev_b <- inv_request(labels = as.character(1:6), R = 10, threads = 1, re = c("v", "cl"), omega = c(0.04, 0.03, 0.09))
  checks[[length(checks) + 1L]] <- cmp("reverse_random_effects", "SIM-ORA-05 (6); SIM-MOD-03", "reversing random_effects (with Omega reordered) changes the eta: eta_cl of the reversed run is not that of the base (a guard against a silent reordering)",
    base, rev_b, relation = "differ", fields = "p", tables = "subjects")
  L_ab <- chol_spec(omega_matrix(c(0.04, 0.03, 0.09), 2))$L
  exp_rev <- list()
  for (r in 0:1) for (l in as.character(1:6)) {
    z <- draw_z("20261108", r, 1, l, 2); eta <- eta_spec(L_ab, z)
    sc <- c(abs(L_ab[1, 1] * z[1]), abs(L_ab[2, 1] * z[1]) + abs(L_ab[2, 2] * z[2]))
    exp_rev[[length(exp_rev) + 1L]] <- list(rep = r, id = l, eta = arr(vapply(eta, hex_fmt, "")), scale = arr(vapply(sc, hex_fmt, "")),
                                          z_eta = arr(vapply(z, hex_fmt, "")))
  }
  zb <- draw_z("20261108", 0, 1, "1", 2); L_ba <- chol_spec(omega_matrix(c(0.09, 0.03, 0.04), 2))$L
  stopifnot(abs(eta_spec(L_ba, zb)[1] - eta_spec(L_ab, zb)[1]) > 1e-3)   # the guard's premise: eta_cl differs for label 1
  write_json_lf(list(rule = "SIM-ORA-05", RNGkind = paste(RNGkind(), collapse = "/"), cells = cells,
                     note = paste("kind answers_identical: the two answers are equal as JSON values; kind compare: rows matched by (rep, id, obs) of the tables listed (subjects: rep, id), restricted to ids and rep <= max_rep when given;",
                                  "relation equal: every listed field of every matched row equal (numbers bit for bit); relation differ: at least one matched row has a different value; eta_len: compare the first eta_len entries of eta.",
                                  "reversed_run_reference: the eta of the run 'reverse_random_effects' (b) in the order (v, cl), tolerance form |a - e| <= tol_exact * max(|e|, scale)"),
                     tol_exact = 1e-12, checks = checks, reversed_run_reference = exp_rev),
                file.path(out_dir, "invariance", "cases.json"))
  cat(sprintf("sim_extras: invariance %d checks\n", length(checks)))
  invisible(checks)
}

# ------------------------------------------------------------------------------------------------ round trip
roundtrip_stage <- function(out_dir, root) {
  for (nm in c("accurate", "limits")) {
    path <- file.path(root, "oracle", "expected", "pop", "sealed", paste0(nm, ".json"))
    s <- jsonlite::fromJSON(path, simplifyVector = FALSE)
    d <- s$design
    stopifnot(length(d$times) * d$subjects == d$observations_per_replicate, d$replicates == 200)
    ft <- s$model
    req <- list(op = "simulate", model = ft$name, theta = ft$theta, random_effects = ft$random_effects, omega_shape = "diagonal",
                omega = arr(vapply(seq_along(ft$random_effects), function(i) ft$omega[[i]][[i]], "")), error_model = ft$error_model, xi = ft$xi,
                design = list(times = d$times, dose = ft$dose, m = d$subjects), seed = "20261108", replicates = d$replicates, options = list(negative = "keep"))
    # the sealed omega is diagonal: check the off-diagonal entries are zero strings
    for (i in seq_along(ft$random_effects)) for (j in seq_along(ft$random_effects)) if (i != j) stopifnot(as.numeric(ft$omega[[i]][[j]]) == 0)
    write_json_lf(list(
      name = nm, rule = "SIM-ORA-06; POP-ORA-04", pending = "T-154 (the population fit); the test is #[ignore]d until the fit exists",
      description = paste("the", nm, "design of POP-ORA-04 simulated by the simulator of specs/simulation.md instead of R: the call below, K = 200 replicates (replicates 0..199 of one call) of one seed"),
      sealed_design_file = paste0("oracle/expected/pop/sealed/", nm, ".json"), sealed_design_sha256 = sha256_file(path),
      simulate = req,
      fit = list(model = ft$name, random_effects = ft$random_effects, error_model = ft$error_model, truth = s$truth,
                 start_factors = list(theta = 1.25, omega = 1.5, xi = 1.25, source = "the same factors as oracle_pop.rs sealed_start")),
      aggregate_checks = s$aggregate_checks,
      note = paste("the dataset of replicate k is the rows of the table `dataset` whose subject column starts with \"k:\" (SIM-OUT-02);",
                   "per-replicate deviations are recorded and never asserted; only the aggregates are asserted, and only when aggregate_checks.asserted is true (as the sealed design says);",
                   "the committed dataset file and its hash (SIM-ORA-06 i) are a card of the engine (T-159/T-154): the call is committed here before any engine commit")),
      file.path(out_dir, "roundtrip", paste0(nm, ".json")))
  }
  cat("sim_extras: roundtrip 2 designs\n")
}
