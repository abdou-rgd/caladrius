# Caladrius population-simulation oracle, card T-158 item 4 (SIM-ORA-04): the distribution of the draws.
#
# Each check has a bound derived from the null distribution (qchisq, qbinom, qnorm and the large-sample
# standard errors of the moments), evaluated at the fixed seed 20261108 on N = 10^6 words, so a failure is a
# change of the stream and never bad luck. The statistics of the reference are RECORDED in the file; they must
# match the numbers printed in SX3 (this script stops if they do not), and the engine's own statistics, which
# come from the same words, must equal the recorded ones within tol_stat (measured below: the difference
# between a sequential double sum and R's extended-precision sum, and the effect of moving the logarithm of
# the transform by one unit in the last place). Counts are exact: the script checks that no z lies within 1e-12
# of a threshold.
#
# Defines dist_stage(out_dir). Needs sim_lib.R.

N_DIST <- 1000000L
DIST_SEED <- "20261108"

dist_streams <- function() {
  key <- seed_key(DIST_SEED); n <- 0:(N_DIST - 1L)
  mk <- function(label, r, p) { u <- word_uniform(stream_words(n, key, r, p, label_key(label))); list(u = u, z = as241(u)) }
  list(eta1 = mk("1", 0, 1), eps1 = mk("1", 0, 2), eta2 = mk("2", 0, 1), eta1_rep1 = mk("1", 1, 1))
}

moments <- function(z, seq_sum = FALSE) {
  n <- length(z)
  ssum <- if (seq_sum) function(x) { s <- 0; for (v in x) s <- s + v; s } else sum
  m <- ssum(z) / n
  d <- z - m
  m2 <- ssum(d * d) / n; m3 <- ssum(d^3) / n; m4 <- ssum(d^4) / n
  list(mean = m, variance = m2 * n / (n - 1), variance_n = m2, skewness = m3 / m2^1.5, excess_kurtosis = m4 / m2^2 - 3)
}
chisq_bins <- function(u, bins = 100L) { o <- tabulate(floor(u * bins) + 1L, bins); e <- length(u) / bins; sum((o - e)^2 / e) }

dist_stats <- function(S, ln_note = NULL) {
  u <- S$eta1$u; z <- S$eta1$z
  um <- moments(u); zm <- moments(z)
  list(u_min = min(u), u_max = max(u), u_mean = um$mean, u_variance = um$variance, u_chisq100 = chisq_bins(u),
       z_mean = zm$mean, z_variance = zm$variance, z_skewness = zm$skewness, z_excess_kurtosis = zm$excess_kurtosis, z_max_abs = max(abs(z)),
       corr_eta_eps = cor(S$eta1$z, S$eps1$z), corr_subject1_subject2 = cor(S$eta1$z, S$eta2$z), corr_rep0_rep1 = cor(S$eta1$z, S$eta1_rep1$z),
       lag1 = cor(z[-length(z)], z[-1]), lag2 = cor(z[1:(length(z) - 2L)], z[3:length(z)]))
}

dist_stage <- function(out_dir) {
  S <- dist_streams(); N <- N_DIST
  z <- S$eta1$z
  st <- dist_stats(S)
  # ---- the targets of SX3 (printed digits): stop on a mismatch
  want <- c(u_mean = 0.5005576930, u_variance = 0.0832617157, u_chisq100 = 99.6822, z_mean = 0.001852, z_variance = 0.999919, z_skewness = 0.000541,
            z_excess_kurtosis = 0.011713, z_max_abs = 5.099343, u_min = 1.986594e-06, corr_eta_eps = -0.001538, corr_subject1_subject2 = -0.001217,
            corr_rep0_rep1 = 0.000933, lag1 = -0.002105, lag2 = -0.000380)
  for (nm in names(want)) {
    got <- st[[nm]]
    ok <- if (nm == "u_min") abs(got / want[[nm]] - 1) < 1e-6 else abs(got - want[[nm]]) < dist_print_tol(want[[nm]])
    if (!ok) stop("SX3 MISMATCH: ", nm, " reference ", sprintf("%.10g", got), " spec ", want[[nm]])
  }
  stopifnot(abs(st$u_max - 0.999999830) < 5e-10)
  # ---- bounds from the null distributions
  b_u_mean <- 5 * sqrt(1 / (12 * N)); b_chi <- c(qchisq(1e-6, 99), qchisq(1 - 1e-6, 99))
  b_mean <- 5 / sqrt(N); b_var <- 5 * sqrt(2 / N); b_skew <- 5 * sqrt(6 / N); b_kurt <- 5 * sqrt(24 / N); b_corr <- 5 / sqrt(N)
  stopifnot(round(b_u_mean, 6) == 0.001443, round(b_chi, 4) == c(45.8344, 180.7920), b_mean == 0.005, round(b_var, 6) == 0.007071,
            round(b_skew, 6) == 0.012247, round(b_kurt, 6) == 0.024495)
  # ---- quantile counts
  ps <- c(0.001, 0.025, 0.1587, 0.5, 0.84, 0.975, 0.999)
  thr <- qnorm(ps)
  counts <- vapply(thr, function(t) sum(z < t), 0L)
  lo <- qbinom(5e-7, N, ps); hi <- qbinom(1 - 5e-7, N, ps)
  spec_counts <- c(1010, 24891, 158075, 498631, 840291, 974834, 998928)
  spec_lo <- c(849, 24240, 156915, 497554, 838204, 974233, 998842); spec_hi <- c(1158, 25767, 160490, 502446, 841791, 975760, 999151)
  stopifnot(counts == spec_counts, lo == spec_lo, hi == spec_hi)
  p3 <- 2 * pnorm(-3); p4 <- 2 * pnorm(-4)
  c3 <- sum(abs(z) > 3); c4 <- sum(abs(z) > 4)
  lo3 <- qbinom(5e-7, N, p3); hi3 <- qbinom(1 - 5e-7, N, p3); lo4 <- qbinom(5e-7, N, p4); hi4 <- qbinom(1 - 5e-7, N, p4)
  stopifnot(c3 == 2784, c4 == 61, lo3 == 2450, hi3 == 2957, lo4 == 29, hi4 == 106)
  gaps <- min(vapply(c(thr, 3, -3, 4, -4), function(t) min(abs(z - t)), 0))
  stopifnot(gaps > 1e-12)
  # ---- correlated eta, m = 1e5 subjects, labels "1".."100000", replicate 0, the first two words of each eta stream
  m <- 100000L; key <- seed_key(DIST_SEED)
  lk <- t(vapply(as.character(seq_len(m)), function(l) label_key(l), c(c2 = 0, c3 = 0), USE.NAMES = FALSE))
  o <- philox(0, 1 * 2^24 + 0, lk[, 1], lk[, 2], key[["k0"]], key[["k1"]])
  u1 <- word_uniform(list(hi = o$r1, lo = o$r0)); u2 <- word_uniform(list(hi = o$r3, lo = o$r2))
  # these are the words 0 and 1 of the stream: check against stream_words for the first labels
  for (i in c(1L, 2L, 777L, 100000L)) {
    w <- stream_words(0:1, key, 0, 1, label_key(as.character(i)))
    stopifnot(identical(word_uniform(w), c(u1[i], u2[i])))
  }
  z1 <- as241(u1); z2 <- as241(u2)
  Om <- matrix(c(0.09, 0.03, 0.03, 0.04), 2); ch <- chol_spec(Om); stopifnot(ch$ok)
  e1 <- ch$L[1, 1] * z1; e2 <- ch$L[2, 1] * z1 + ch$L[2, 2] * z2
  se_mean <- sqrt(c(0.09, 0.04) / m)
  Skl <- c(S11 = 0.09, S21 = 0.03, S22 = 0.04)
  var_S <- c((0.09 * 0.09 + 0.09^2), (0.09 * 0.04 + 0.03^2), (0.04 * 0.04 + 0.04^2)) / (m - 1)
  sd_S <- sqrt(var_S)
  Scov <- cov(cbind(e1, e2))
  samp <- c(S11 = Scov[1, 1], S21 = Scov[2, 1], S22 = Scov[2, 2])
  stopifnot(round(5 * sd_S, 6) == c(0.002012, 0.001061, 0.000894), abs(samp - c(0.090762, 0.030313, 0.040040)) < 6e-7,
            abs(c(mean(e1), mean(e2)) - c(0.000432, 0.000720)) < 6e-7, abs(cor(e1, e2) - 0.50284) < 6e-6)

  # ---- tolerance of the recorded statistics: a sequential double sum against R's extended sum, and the logarithm moved by 1 ulp
  seq_m <- moments(z, TRUE); ext_m <- moments(z)
  d_seq <- max(abs(unlist(seq_m) - unlist(ext_m)))
  ulp <- function(x, s) x + s * 2^(floor(log2(abs(x))) - 52)
  d_ln <- 0
  for (s in c(-1, 1)) {
    z_s <- as241(S$eta1$u, ln = function(x) ulp(log(x), s))
    mm <- moments(z_s)
    d_ln <- max(d_ln, max(abs(unlist(mm) - unlist(ext_m))), abs(max(abs(z_s)) - max(abs(z))), abs(sum(z_s < thr[1]) - counts[1]))
    stopifnot(all(vapply(thr, function(t) sum(z_s < t), 0L) == counts), sum(abs(z_s) > 3) == c3, sum(abs(z_s) > 4) == c4)
  }
  tol_stat <- 1e-9
  hexed <- function(x) vapply(x, hex_fmt, "")
  out <- list(
    rule = "SIM-ORA-04", rng = "philox4x32-10/as241/v1", RNGkind = paste(RNGkind(), collapse = "/"), seed = DIST_SEED, n_words = N, false_alarm_per_check = "about 1e-6",
    note = paste("each bound is derived from the null distribution at a fixed seed; recorded = the reference statistic on the same words (must equal SX3);",
                 "tol_stat is the tolerance for comparing the engine's own statistic with the recorded one; counts are exact"),
    tol_stat = tol_stat,
    streams = list(eta_subject1 = list(label = "1", replicate = 0, purpose = 1), eps_subject1 = list(label = "1", replicate = 0, purpose = 2),
                   eta_subject2 = list(label = "2", replicate = 0, purpose = 1), eta_subject1_replicate1 = list(label = "1", replicate = 1, purpose = 1)),
    uniform = list(mean = list(recorded = sprintf("%.17g", st$u_mean), bound_around = 0.5, bound = b_u_mean),
                   chisq100 = list(recorded = sprintf("%.17g", st$u_chisq100), lower = b_chi[1], upper = b_chi[2], degrees_of_freedom = 99),
                   variance_recorded = sprintf("%.17g", st$u_variance), min_recorded = hex_fmt(st$u_min), max_recorded = hex_fmt(st$u_max)),
    normal = list(mean = list(recorded = sprintf("%.17g", st$z_mean), bound = b_mean),
                  variance = list(recorded = sprintf("%.17g", st$z_variance), bound_around = 1, bound = b_var, divisor = "N - 1"),
                  skewness = list(recorded = sprintf("%.17g", st$z_skewness), bound = b_skew),
                  excess_kurtosis = list(recorded = sprintf("%.17g", st$z_excess_kurtosis), bound = b_kurt),
                  max_abs_recorded = sprintf("%.17g", st$z_max_abs)),
    quantile_counts = lapply(seq_along(ps), function(i) list(p = ps[i], threshold = hex_fmt(thr[i]), count_below = counts[i], interval = arr(c(lo[i], hi[i])))),
    tails = list(abs_gt_3 = list(count = c3, interval = arr(c(lo3, hi3))), abs_gt_4 = list(count = c4, interval = arr(c(lo4, hi4)))),
    min_distance_of_any_z_to_a_threshold = gaps,
    correlations = list(bound = b_corr, eta_vs_eps_subject1 = st$corr_eta_eps, subject1_vs_subject2 = st$corr_subject1_subject2, replicate0_vs_1 = st$corr_rep0_rep1,
                        lag1 = st$lag1, lag2 = st$lag2),
    correlated_eta = list(m = m, omega_lower_by_rows = arr(c("0.09", "0.03", "0.04")), first_words_of_each_eta_stream = 2, replicate = 0,
                          mean_bound = arr(5 * se_mean), mean_recorded = arr(c(mean(e1), mean(e2))),
                          covariance_recorded = arr(unname(samp)), covariance_target = arr(unname(Skl)), covariance_bound = arr(unname(5 * sd_S)),
                          correlation_recorded = cor(e1, e2)),
    measured_tol_stat = list(sequential_vs_extended_sum_max_abs = d_seq, ln_plus_minus_1ulp_max_abs = d_ln, tol_stat = tol_stat,
                             margin = tol_stat / max(d_seq, d_ln, 1e-300)))
  write_json_lf(out, file.path(out_dir, "dist", "sx3.json"), digits = NA)
  cat(sprintf("sim_dist: SX3 statistics reproduced (N = %d, m = %d); sequential-sum difference %.3g, ln +-1 ulp %.3g; tol_stat %.0e\n", N, m, d_seq, d_ln, tol_stat))
  invisible(out)
}

# the spec prints each statistic to a fixed number of decimals; match to half a unit of the last printed digit
dist_print_tol <- function(x) {
  s <- format(abs(x), scientific = FALSE, digits = 12); s <- sub("0+$", "", s)
  dec <- nchar(sub("^[^.]*\\.", "", s)); 0.51 * 10^-dec
}
