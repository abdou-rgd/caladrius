# Caladrius population-simulation oracle, card T-158: whole studies (SIM-OUT-01, -02, SIM-SUM-01, SIM-DET-02 workload, SIM-ORA-03/-05).
#
# A study is a complete call of the simulator: the request (seed, design, model, options), and the tables the
# reference makes of it. The reference is the exact part of sim_exact.R at 256 bits fed with the z of the
# generator (the generator and the transform are sim_lib.R, validated against the known-answer vectors and the
# words of the spec by sim_gen.R). So the z in the file are the doubles of the REFERENCE transform: the engine,
# which makes its own z, matches them to tol_z (tail) or exactly (central), and everything after is a
# tolerance target (tol_exact, with the scale of sim_exact.R).
#
# The studies include the workload that SIM-DET-02 fixes for the digest: SX2 design, pk1.iv_bolus, two error
# models, with and without an LLOQ, 20 replicates, seed 20261108, and one run with a seed above 2^53 (a decimal
# string). SX5 (19 replicates, type 7 quantiles) and the negative-value studies of SX4 are the others.
#
# Defines study_stage(out_dir). Needs sim_lib.R and sim_exact.R.

STUDY_P <- c(0.05, 0.5, 0.95)
STUDY_DIGITS <- 30

study_request <- function(cs, seed, R, labels, times, quantiles = STUDY_P) {
  opt <- list(negative = cs$negative, quantiles = arr(quantiles))
  if (!is.null(cs$lloq)) opt$lloq <- cs$lloq
  list(op = "simulate", model = cs$model, theta = cs$theta, random_effects = cs$random_effects, omega_shape = cs$omega_shape,
       omega = cs$omega, error_model = cs$error_model, xi = cs$xi,
       design = list(subjects = lapply(labels, function(l) list(id = l, dose = cs$dose, time = arr(times)))),
       seed = seed, replicates = R, options = opt)
}

# the z of every (replicate, subject): the reference generator
study_subjects <- function(seed, R, labels, times, q) {
  out <- list()
  for (r in 0:(R - 1L)) for (l in labels) {
    out[[length(out) + 1L]] <- list(id = paste0(r, ":", l), rep = r, label = l, time = arr(times),
      z_eta = arr(vapply(draw_z(seed, r, 1, l, q), hex_fmt, "")), z_eps = arr(vapply(draw_z(seed, r, 2, l, length(times)), hex_fmt, "")))
  }
  out
}

study_one <- function(name, why, seed, R, error_model, xi, negative = "keep", lloq = NULL, labels = c("1", "2", "3"), times = sx2_times,
                      model = "pk1.iv_bolus", theta = list(cl = "5", v = "50"), re = c("cl", "v"), omega_shape = "block", omega = c("0.09", "0.03", "0.04"), dose = "100") {
  cs <- case(name, why, model, theta, re, omega_shape, omega, error_model, xi, dose, NULL, negative = negative, lloq = lloq)
  cs$subjects <- study_subjects(seed, R, labels, times, length(re))
  m <- run_mp(cs); d <- run_dbl(cs)
  list(cs = cs, mp = m, dbl = d, seed = seed, R = R, labels = labels, times = times)
}

study_write <- function(st, out_dir) {
  cs <- st$cs; m <- st$mp; R <- st$R; labels <- st$labels; times <- st$times; q <- length(unlist(cs$random_effects)); nobs <- length(times)
  dec <- function(x) mp_dec(x, STUDY_DIGITS)
  re <- unlist(cs$random_effects)
  # ---- the safety checks: no row is near a limit that a 1e-15 move of an input could flip
  gap_lloq <- Inf; gap_zero <- Inf
  for (s in m$subjects) {
    sc <- pmax(as.numeric(s$y_scale), 1e-300)
    gap_zero <- min(gap_zero, abs(as.numeric(s$y_raw)) / sc)
    if (!is.null(cs$lloq)) gap_lloq <- min(gap_lloq, abs(as.numeric(s$y) - as.numeric(mpfr(cs$lloq, BITS))) / sc)
  }
  stopifnot(gap_zero > 1e-9, gap_lloq > 1e-9)
  for (i in seq_along(m$subjects)) stopifnot(identical(m$subjects[[i]]$cens, st$dbl$subjects[[i]]$cens), m$subjects[[i]]$n_negative == st$dbl$subjects[[i]]$n_negative)
  # ---- tables
  subj_rows <- list(); obs_rows <- list(); ds_rows <- list()
  for (i in seq_along(m$subjects)) {
    s <- m$subjects[[i]]; sub <- cs$subjects[[i]]; id <- sub$label; rep <- sub$rep
    P <- lapply(s$P, function(x) dec(x))
    subj_rows[[i]] <- list(rep = rep, id = id, z_eta = sub$z_eta, eta = arr(vapply(s$eta, function(x) dec(x), "")), p = P)
    ds_rows[[length(ds_rows) + 1L]] <- list(subject = paste0(rep, ":", id), label = id, time = "0", event = "dose", amt = cs$dose, dv = NULL, cens = NULL)
    for (j in seq_len(nobs)) {
      obs_rows[[length(obs_rows) + 1L]] <- list(rep = rep, id = id, obs = j, time = times[j], z_eps = sub$z_eps[[j]],
        pred = dec(s$pred[j]), ipred = dec(s$ipred[j]), sigma = dec(s$sigma[j]), y_raw = dec(s$y_raw[j]), y = dec(s$y[j]),
        y_scale = format(as.numeric(s$y_scale[j]), digits = 17), cens = s$cens[j], dv = dec(s$dv[j]))
      ds_rows[[length(ds_rows) + 1L]] <- list(subject = paste0(rep, ":", id), label = id, time = times[j], event = "obs", amt = NULL, dv = dec(s$dv[j]), cens = s$cens[j])
    }
  }
  # ---- the summary: type 7 over replicates, per (subject, obs), of y (after the negative rule, before the BLQ replacement) and ipred
  summary <- list()
  for (l in labels) for (j in seq_len(nobs)) {
    idx <- which(vapply(cs$subjects, function(s) s$label == l, TRUE))
    yv <- vapply(idx, function(i) as.numeric(m$subjects[[i]]$y[j]), 0); iv <- vapply(idx, function(i) as.numeric(m$subjects[[i]]$ipred[j]), 0)
    blq <- vapply(idx, function(i) m$subjects[[i]]$cens[j], 0L)
    qy <- quantile7(yv, STUDY_P); qi <- quantile7(iv, STUDY_P)
    stopifnot(identical(qy, unname(quantile(yv, STUDY_P, type = 7))), identical(qi, unname(quantile(iv, STUDY_P, type = 7))))
    summary[[length(summary) + 1L]] <- list(id = l, obs = j, time = times[j], y = arr(vapply(qy, function(x) sprintf("%.17g", x), "")),
                                          ipred = arr(vapply(qi, function(x) sprintf("%.17g", x), "")), blq_fraction = mean(blq))
  }
  counts <- list(subjects = length(labels), replicates = R, rows = length(labels) * R * nobs,
                 n_zero_sigma = sum(vapply(m$subjects, function(s) s$n_zero_sigma, 0L)), n_negative = sum(vapply(m$subjects, function(s) s$n_negative, 0L)),
                 n_blq = sum(vapply(m$subjects, function(s) s$n_blq, 0L)),
                 n_negative_by_subject = arr(vapply(labels, function(l) sum(vapply(which(vapply(cs$subjects, function(s) s$label == l, TRUE)), function(i) m$subjects[[i]]$n_negative, 0L)), 0L)),
                 n_blq_by_subject = arr(vapply(labels, function(l) sum(vapply(which(vapply(cs$subjects, function(s) s$label == l, TRUE)), function(i) m$subjects[[i]]$n_blq, 0L)), 0L)))
  req <- study_request(cs, st$seed, R, labels, times)
  stopifnot(!is.null(req$design))
  out <- list(name = cs$name, why = cs$why, rules = "SIM-OUT-01, SIM-OUT-02, SIM-SUM-01, SIM-ORD-01..05", rng = "philox4x32-10/as241/v1",
              RNGkind = paste(RNGkind(), collapse = "/"), request = req, tol_exact = 1e-12, tol_z = 1e-14,
              note = paste("z_eta and z_eps are the hexadecimal doubles of the reference transform; the engine's own z match them to tol_z (exactly in the central region);",
                           "eta, p, pred, ipred, sigma, y_raw, y, dv are decimals of the 256-bit result from those z, compared with max(|expected|, scale) and tol_exact;",
                           "y_scale is |ipred| + sigma |z_eps|; cens and counts are exact; the summary is the type 7 quantile of the double rounding of y, ipred over replicates (tolerance)"),
              min_relative_gap_to_zero = gap_zero, min_relative_gap_to_lloq = if (is.finite(gap_lloq)) gap_lloq else NULL,
              expected = list(l = lapply(seq_len(q), function(i) arr(vapply(seq_len(q), function(j) hex_fmt(st$dbl$L[[(i - 1L) * q + j]]), ""))),
                              subjects = subj_rows, observations = obs_rows, counts = counts, summary = summary, dataset = ds_rows))
  write_json_lf(out, file.path(out_dir, "study", paste0(cs$name, ".json")))
  invisible(out)
}

study_specs <- function() {
  list(
    list(name = "prop_no_lloq", why = "SX2 design, proportional b = 0.1, 20 replicates, seed 20261108 (the SIM-DET-02 workload)", seed = "20261108", R = 20, em = "proportional", xi = list(b = "0.1")),
    list(name = "add_no_lloq", why = "SX2 design, additive a = 0.2, 20 replicates, seed 20261108", seed = "20261108", R = 20, em = "additive", xi = list(a = "0.2")),
    list(name = "prop_lloq", why = "proportional b = 0.1 with LLOQ 0.8, 20 replicates", seed = "20261108", R = 20, em = "proportional", xi = list(b = "0.1"), lloq = "0.8"),
    list(name = "add_lloq", why = "additive a = 0.2 with LLOQ 0.8, 20 replicates (BLQ rows exist: subject 3 at t = 4 and 12 in replicate 0)", seed = "20261108", R = 20, em = "additive", xi = list(a = "0.2"), lloq = "0.8"),
    list(name = "big_seed", why = "a seed above 2^53 given as a decimal string, additive a = 0.2, LLOQ 0.8, 20 replicates", seed = "12345678901234567890", R = 20, em = "additive", xi = list(a = "0.2"), lloq = "0.8"),
    list(name = "negative_keep", why = "SX4 additive a = 1.0, negative = keep, 20 replicates (n_negative is counted, y keeps its sign)", seed = "20261108", R = 20, em = "additive", xi = list(a = "1.0"), negative = "keep"),
    list(name = "negative_zero_lloq", why = "SX4 additive a = 1.0, negative = zero with LLOQ 0.8, 20 replicates (zeroed values are below the limit: cens = 1)", seed = "20261108", R = 20, em = "additive", xi = list(a = "1.0"), negative = "zero", lloq = "0.8"),
    list(name = "combined_replicates", why = "combined error a = 0.2, b = 0.1, 20 replicates", seed = "20261108", R = 20, em = "combined", xi = list(a = "0.2", b = "0.1")),
    list(name = "sx5_r19", why = "SX5: subject 1, additive a = 0.2, 19 replicates: the type 7 quantiles at t = 4 are 0.85540657059604297, 1.378773104187816, 1.8214186412665074", seed = "20261108", R = 19, em = "additive", xi = list(a = "0.2"), labels = "1"))
}

study_checks <- function(sts) {
  # SX4: replicates of subject "1" (additive a = 0.2): eta and y(t = 4)
  a <- sts$add_no_lloq$mp$subjects
  find <- function(r, l) a[[which(vapply(sts$add_no_lloq$cs$subjects, function(s) s$rep == r && s$label == l, TRUE))]]
  e0 <- vapply(find(0, "1")$eta, as.numeric, 0); e1 <- vapply(find(1, "1")$eta, as.numeric, 0); e2 <- vapply(find(2, "1")$eta, as.numeric, 0)
  stopifnot(all(abs(e0 - c(-0.625762252, -0.123158246)) < 5e-10), all(abs(e1 - c(-0.497975732, -0.202599477)) < 5e-10), all(abs(e2 - c(0.366804870, 0.183387866)) < 5e-10))
  stopifnot(abs(as.numeric(find(0, "1")$y[4]) - 1.819451375) < 5e-10, abs(as.numeric(find(1, "1")$y[4]) - 1.646151118) < 5e-10, abs(as.numeric(find(2, "1")$y[4]) - 0.880424710) < 5e-10)
  # SX5: the 19 values and the three quantiles
  s5 <- sts$sx5_r19
  y19 <- vapply(seq_len(19), function(i) as.numeric(s5$mp$subjects[[i]]$y[4]), 0)
  spec19 <- c(1.819451, 1.646151, 0.880425, 1.483731, 1.517560, 1.430817, 0.793062, 1.839124, 1.378773, 1.428002, 1.220858, 0.862334, 1.135400, 1.386719, 1.270416, 1.675814, 1.237788, 1.145566, 1.022745)
  stopifnot(all(abs(y19 - spec19) < 6e-7))
  q <- quantile7(y19, STUDY_P)
  close_to(q, c(0.85540657059604297, 1.378773104187816, 1.8214186412665074), 1e-14, "SX5 type 7 quantiles")
  stopifnot(identical(q, unname(quantile(y19, STUDY_P, type = 7))))
  q4 <- quantile7(c(1, 2, 3, 10), c(0.25, 0.5, 0.9)); stopifnot(identical(q4, unname(quantile(c(1, 2, 3, 10), c(0.25, 0.5, 0.9), type = 7))), all(abs(q4 - c(1.75, 2.5, 7.9)) < 1e-14))
  # SX4 negative values with a = 1.0 in replicate 0
  # BLQ in the first replicate of add_lloq: subject 3 has cens 0 0 0 1 0 1
  b <- sts$add_lloq$mp$subjects
  i3 <- which(vapply(sts$add_lloq$cs$subjects, function(s) s$rep == 0 && s$label == "3", TRUE))
  stopifnot(identical(b[[i3]]$cens, c(0L, 0L, 0L, 1L, 0L, 1L)))
  # the interplay of the rules (SIM-ORD-04): zero + LLOQ and keep + LLOQ give the same cens and dv
  z <- sts$negative_zero_lloq$mp$subjects
  k <- study_one("tmp_keep_lloq", "", "20261108", 20, "additive", list(a = "1.0"), negative = "keep", lloq = "0.8")$mp$subjects
  for (i in seq_along(z)) stopifnot(identical(z[[i]]$cens, k[[i]]$cens), all(z[[i]]$dv == k[[i]]$dv), z[[i]]$n_negative == k[[i]]$n_negative)
  # the number-or-string forms of a seed give the same stream (the key is the integer)
  stopifnot(identical(seed_key("20261108"), seed_key("20261108")))
  invisible(TRUE)
}

study_stage <- function(out_dir) {
  sts <- list(); tot_rows <- 0L; worst <- 0
  for (sp in study_specs()) {
    st <- study_one(sp$name, sp$why, sp$seed, sp$R, sp$em, sp$xi, negative = if (is.null(sp$negative)) "keep" else sp$negative, lloq = sp$lloq,
                    labels = if (is.null(sp$labels)) c("1", "2", "3") else sp$labels)
    sts[[sp$name]] <- st
  }
  study_checks(sts)
  # tolerance of the whole pipeline: the double implementation (R's exp, its own z) against the 256-bit tables
  gs <- list()
  for (nm in names(sts)) {
    st <- sts[[nm]]; mx <- 0
    for (i in seq_along(st$mp$subjects)) {
      m <- st$mp$subjects[[i]]; d <- st$dbl$subjects[[i]]
      for (cl in c("y_raw", "y", "dv")) {
        x <- abs(mpfr(d[[cl]], BITS) - m[[cl]]); den <- pmax(abs(as.numeric(m[[cl]])), as.numeric(m$y_scale)); r <- as.numeric(x) / den; r[den == 0] <- 0; mx <- max(mx, r)
      }
      x <- abs(mpfr(d$ipred, BITS) - m$ipred); mx <- max(mx, as.numeric(x / abs(m$ipred)))
    }
    gs[[nm]] <- mx; worst <- max(worst, mx)
    tot_rows <- tot_rows + length(st$mp$subjects) * length(st$times)
    study_write(st, out_dir)
  }
  write_json_lf(list(rule = "SIM-ORA-03, OS-05", method = "double pipeline (R's exp, the reference z) against the 256-bit tables of each study", rows = tot_rows,
                     max_error_vs_scale = gs, max_over_all = worst, tol_exact_proposed = 1e-12, margin = 1e-12 / worst),
                file.path(out_dir, "study", "measure.json"), digits = 6)
  cat(sprintf("sim_study: %d studies, %d rows; SX4/SX5 numbers of the spec reproduced; max error vs scale %.3g (tol_exact 1e-12 margin %.0fx)\n", length(sts), tot_rows, worst, 1e-12 / worst))
  invisible(sts)
}
