# Caladrius population-simulation oracle, card T-158 item 2 (SIM-ORA-02): the normal transform against 256 bits,
# and the measurement of tol_z.
#
# The grid has the points the spec requires exactly (0.075, 0.925, exp(-25), 1 - exp(-25), 2^-53, 1 - 2^-53,
# 0.0048183637779074528, 0.013215994403526499, given as hexadecimal doubles), the neighbours of the region
# edges |q| = 0.425 and t = 5, and a spread over the three regions. The reference is the 256-bit inverse of
# sim_lib.R (Newton on erfc, residual checked below 2^-230).
#
# Bit-exact targets: the z of the central region (z_bit_exact). Tolerance targets: every other z, relative
# to the 256-bit value. At the t = 5 edges the case file asserts the value within tol_z and never the branch.
#
# tol_z is measured, not guessed (OS-05): on the grid, on 3000 uniforms made by the generator itself, on 3000
# tail uniforms spread in log scale, and with the logarithm moved by one unit in the last place in both
# directions (an engine whose libm `ln` differs from R's by 1 ulp). Defines transform_stage(out_dir).

# the grid ------------------------------------------------------------------------------------------------
ulp_up <- function(x) x + 2^(floor(log2(abs(x))) - 52)
ulp_dn <- function(x) x - 2^(floor(log2(abs(x))) - 52)

transform_points <- function() {
  pts <- list()
  add <- function(name, u, why) pts[[length(pts) + 1L]] <<- list(name = name, u = u, why = why)
  e25 <- exp(-25)
  stopifnot(identical(e25, hex_parse("0x1.e8a37a45fc32ep-37")), identical(1 - e25, hex_parse("0x1.ffffffffe175dp-1")))
  add("u_0.075", 0.075, "SX1: the edge |q| = 0.425 below; the double 0.075 is central")
  add("u_0.925", 0.925, "SX1: the edge |q| = 0.425 above; the double 0.925 has q = 0.42500000000000004 and is in the tail branch")
  add("u_exp_minus_25", e25, "the edge t = 5 (value asserted, never the branch)")
  add("u_one_minus_exp_minus_25", 1 - e25, "the edge t = 5, upper side (value asserted, never the branch)")
  add("u_2_pow_minus_53", 2^-53, "the smallest uniform the generator can make")
  add("u_one_minus_2_pow_minus_53", 1 - 2^-53, "the largest uniform the generator can make")
  add("u_worst_1", 0.0048183637779074528, "the worst point found by the review of T-157")
  add("u_worst_2", 0.013215994403526499, "the worst point found by the review of T-157")
  add("u_half", 0.5, "q = 0: z = 0 exactly")
  add("u_half_plus_ulp", 0.5 + 2^-53, "the smallest q > 0 (q = 2^-53)")
  add("u_half_minus_ulp", 0.5 - 2^-54, "the smallest q < 0 (q = -2^-54)")
  add("u_0.075_up", ulp_up(0.075), "next double above 0.075")
  add("u_0.075_down", ulp_dn(0.075), "next double below 0.075 (tail branch)")
  add("u_0.925_down", ulp_dn(0.925), "next double below 0.925")
  add("u_0.925_up", ulp_up(0.925), "next double above 0.925")
  add("u_exp_minus_25_up", ulp_up(e25), "just above the edge t = 5")
  add("u_exp_minus_25_down", ulp_dn(e25), "just below the edge t = 5")
  add("u_one_minus_exp_minus_25_up", ulp_up(1 - e25), "just above the upper edge t = 5")
  add("u_one_minus_exp_minus_25_down", ulp_dn(1 - e25), "just below the upper edge t = 5")
  add("u_2_pow_minus_52", 2^-52, "second smallest value of the generator's grid")
  add("u_3_halves_2_pow_minus_52", 1.5 * 2^-52, "(m + 0.5) 2^-52 with m = 1")
  add("u_one_minus_2_pow_minus_52", 1 - 2^-52, "close to 1")
  # a spread: the central region, the first tail (t from 1.62 to 4.98) and the second (t from 5.02 to 6.06), both sides
  for (u in seq(0.08, 0.92, length.out = 43)) add(sprintf("central_%.4f", u), u, "central region")
  for (t in seq(1.62, 4.98, by = 0.06)) {
    p <- exp(-t * t)
    add(sprintf("tail1_lo_t%.2f", t), p, "first tail, lower"); add(sprintf("tail1_hi_t%.2f", t), 1 - p, "first tail, upper")
  }
  for (t in seq(5.02, 6.04, by = 0.06)) {
    p <- exp(-t * t)
    add(sprintf("tail2_lo_t%.2f", t), p, "second tail, lower"); add(sprintf("tail2_hi_t%.2f", t), 1 - p, "second tail, upper")
  }
  pts
}

rel_err <- function(z, zref) {            # relative error of doubles z against 256-bit zref (0 where both are 0)
  d <- abs((mpfr(z, BITS) - zref))
  out <- numeric(length(z)); nz <- zref != 0
  out[nz] <- as.numeric(d[nz] / abs(zref[nz])); out[!nz] <- as.numeric(d[!nz])
  out
}

transform_stage <- function(out_dir) {
  pts <- transform_points()
  u <- vapply(pts, function(p) p$u, 0)
  stopifnot(all(u > 0 & u < 1), !anyDuplicated(vapply(pts, function(p) p$name, "")))
  zref <- qnorm_mp(u)
  z <- as241(u)
  # R's own qnorm is AS 241 too: identical, a check of the transcription of the table
  stopifnot(identical(z, qnorm(u)))
  central <- abs(u - AS_HALF) <= AS_SPLIT
  err <- rel_err(z, zref)
  rows <- lapply(seq_along(pts), function(i) list(name = pts[[i]]$name, why = pts[[i]]$why, u = hex_fmt(u[i]), z = hex_fmt(z[i]),
                                                  z_bit_exact = central[i], z_ref = mp_dec(zref[i], 50)))
  # the two edges named by the spec must be present exactly as the doubles it gives
  req <- c("u_0.075", "u_0.925", "u_exp_minus_25", "u_one_minus_exp_minus_25", "u_2_pow_minus_53", "u_one_minus_2_pow_minus_53", "u_worst_1", "u_worst_2")
  stopifnot(all(req %in% vapply(pts, function(p) p$name, "")))
  stopifnot(as241_branch(0.925) == "tail1", as241_branch(0.075) == "central")
  i53 <- match("u_2_pow_minus_53", vapply(pts, function(p) p$name, ""))
  stopifnot(abs(z[i53] / (-8.2095361516013856) - 1) < 1e-15, abs(as.numeric(zref[i53] / mpfr("-8.20953615160138685563076877867", BITS)) - 1) < 1e-25)

  # ---- measurement of tol_z
  key <- seed_key("20261108")
  gen_u <- function(label, r, p, n) word_uniform(stream_words(0:(n - 1), key, r, p, label_key(label)))
  sample_gen <- gen_u("1", 0, 1, 3000)                                  # what the engine will feed the transform
  tail_lo <- exp(-seq(2.7, 36.7, length.out = 3000))                    # log-spaced lower tail, deterministic
  sample_tail <- c(tail_lo, 1 - tail_lo)
  measure <- function(uu, ln = log, zr = qnorm_mp(uu)) {
    zz <- as241(uu, ln = ln)
    list(err = rel_err(zz, zr), branch = as241_branch(uu, ln = ln))
  }
  ln_up <- function(x) { v <- log(x); ulp_up(v) }                       # |ln| larger by one unit in the last place
  ln_dn <- function(x) { v <- log(x); ulp_dn(v) }
  groups <- list(grid = u, generator_3000 = sample_gen, tail_log_spaced_6000 = sample_tail)
  summary <- list()
  for (g in names(groups)) {
    uu <- groups[[g]]
    zr <- if (g == "grid") zref else qnorm_mp(uu)
    base <- measure(uu, zr = zr); up <- measure(uu, ln_up, zr); dn <- measure(uu, ln_dn, zr)
    summary[[g]] <- list(n = length(uu),
                         max_rel_err = max(base$err),
                         max_rel_err_ln_plus_1ulp = max(up$err), max_rel_err_ln_minus_1ulp = max(dn$err),
                         max_rel_err_central = if (any(abs(uu - 0.5) <= AS_SPLIT)) max(base$err[abs(uu - 0.5) <= AS_SPLIT]) else NA,
                         max_rel_err_tail1 = if (any(base$branch == "tail1")) max(base$err[base$branch == "tail1"]) else NA,
                         max_rel_err_tail2 = if (any(base$branch == "tail2")) max(base$err[base$branch == "tail2"]) else NA,
                         at_max = hex_fmt(uu[which.max(base$err)]))
  }
  # the edge t = 5 with the logarithm moved by up to 2 ulps either way: value within tolerance, branch irrelevant
  e25 <- exp(-25); edge_u <- c(e25, 1 - e25, ulp_up(e25), ulp_dn(e25))
  edge <- list()
  for (shift in -2:2) {
    ln_s <- function(x) { v <- log(x); if (shift == 0) v else if (shift > 0) { for (i in seq_len(shift)) v <- ulp_up(v); v } else { for (i in seq_len(-shift)) v <- ulp_dn(v); v } }
    m <- measure(edge_u, ln_s)
    edge[[as.character(shift)]] <- list(max_rel_err = max(m$err), branches = m$branch)
  }
  all_max <- max(vapply(summary, function(s) max(s$max_rel_err, s$max_rel_err_ln_plus_1ulp, s$max_rel_err_ln_minus_1ulp), 0),
                 vapply(edge, function(s) s$max_rel_err, 0))
  measure_out <- list(
    rule = "SIM-ORA-02, OS-05", method = "relative error of the double implementation against the 256-bit inverse",
    RNGkind = paste(RNGkind(), collapse = "/"),
    groups = summary, edge_t5_with_ln_shifted_by_ulps = edge,
    max_rel_err_anything = all_max, tol_z_proposed = 1e-14, margin = 1e-14 / all_max,
    note = "the ln columns move the logarithm by one unit in the last place; the t = 5 edge shows the two branches agree there")
  write_json_lf(list(rng = "philox4x32-10/as241/v1", rule = "SIM-ORA-02",
                     note = paste("u and z hexadecimal floats (compared as parsed values); z_bit_exact true: the engine's z equals z exactly;",
                                  "false: relative error to z_ref (the 256-bit inverse) at most tol_z; at the t = 5 edges never the branch"),
                     tol_z = 1e-14, points = rows), file.path(out_dir, "transform", "grid.json"))
  write_json_lf(measure_out, file.path(out_dir, "transform", "measure.json"), digits = 6)
  cat(sprintf("sim_transform: %d grid points; max relative error %.3g (all samples, ln shifted by 1 ulp included); tol_z 1e-14 margin %.0fx\n",
              length(pts), all_max, 1e-14 / all_max))
  invisible(measure_out)
}
