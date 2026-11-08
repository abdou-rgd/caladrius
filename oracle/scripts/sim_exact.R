# Caladrius population-simulation oracle, card T-158 item 3 (SIM-ORA-03): the exact part, draws given.
#
# With the standard normals z^eta and z^eps GIVEN (hexadecimal doubles in the case file), eta, P, PRED, IPRED,
# sigma, y_raw, y, cens and dv are a deterministic function of (theta, Omega, xi, design). The reference is
# computed at 256 bits (Rmpfr) from the exact doubles of the z and from the DECIMAL STRINGS of theta, Omega, xi,
# dose, times and LLOQ, parsed by Rmpfr (not the doubles the engine holds); the difference is at most 2^-53
# relative per input. The same code is also run in double precision (the decimals converted to the nearest
# double by Rmpfr, R's exp), to measure tol_exact: how far a correct double implementation is from 256 bits.
#
# One generic implementation serves both: every operation is an arithmetic operator, exp, sqrt or abs, which
# behave the same on doubles and on mpfr vectors; `mk` converts a double vector to the working type.
# The structural models are written from specs/models.md: MOD-IVB-01, MOD-IVI-01, MOD-AB1-01, MOD-AB1-05 (lag,
# t - tlag) and MOD-2C-03/-06/-07 (the textbook roots of the quadratic, the weights of MOD-2C-03).
#
# The error model, the negative rule (before the BLQ rule) and the BLQ rule (strictly below the LLOQ, the limit
# written as the value of a censored row) are SIM-ORD-03, -04, -05. Defines exact_stage(out_dir).

MODEL_PARAMS <- list("pk1.iv_bolus" = c("cl", "v"), "pk1.iv_infusion" = c("cl", "v", "dur"), "pk1.oral_1" = c("cl", "v", "ka"),
                     "pk1.oral_1_lag" = c("cl", "v", "ka", "tlag"), "pk2.iv_bolus" = c("cl", "vc", "q", "vp"))

# concentration at times t (vector of the working type); a time before the dose gives 0 (MOD-GEN-02)
conc_generic <- function(model, P, D, t, mk) {
  n <- length(t); C <- mk(rep(0, n))
  on <- as.logical(t >= 0)
  if (!any(on)) return(C)
  oral <- function(s) {                                                  # MOD-AB1-01 for s >= 0
    k <- P$cl / P$v
    if (abs(as.numeric(P$ka - k)) < 1e-6 * as.numeric(k)) stop("ka too close to k for the textbook form")
    D * P$ka / (P$v * (P$ka - k)) * (exp(-k * s) - exp(-P$ka * s))
  }
  switch(model,
    "pk1.iv_bolus" = { k <- P$cl / P$v; C[on] <- D / P$v * exp(-k * t[on]) },
    "pk1.iv_infusion" = {
      k <- P$cl / P$v; T <- P$dur; R0 <- D / T
      d <- on & as.logical(t <= T); a <- on & !d
      if (any(d)) C[d] <- R0 / P$cl * (1 - exp(-k * t[d]))
      if (any(a)) C[a] <- R0 / P$cl * (1 - exp(-k * T)) * exp(-k * (t[a] - T))
    },
    "pk1.oral_1" = C[on] <- oral(t[on]),
    "pk1.oral_1_lag" = { s <- t - P$tlag; ons <- as.logical(s > 0); if (any(ons)) C[ons] <- oral(s[ons]) },
    "pk2.iv_bolus" = {
      k10 <- P$cl / P$vc; k12 <- P$q / P$vc; k21 <- P$q / P$vp
      S <- k10 + k12 + k21; r <- sqrt(S * S - 4 * k10 * k21)
      al <- (S + r) / 2; be <- k10 * k21 / al
      wa <- (al - k21) / (al - be); wb <- (k21 - be) / (al - be)
      C[on] <- D / P$vc * (wa * exp(-al * t[on]) + wb * exp(-be * t[on]))
    },
    stop("model ", model, " is not in the oracle"))
  C
}

# Cholesky-Banachiewicz, SIM-RNG-07; om(i, j) is the matrix entry, returns the lower factor as a list q*q (row-major)
chol_generic <- function(om, q, mk) {
  L <- vector("list", q * q); ix <- function(i, j) (i - 1L) * q + j
  for (i in seq_len(q)) for (j in seq_len(q)) L[[ix(i, j)]] <- mk(0)
  for (i in seq_len(q)) for (j in seq_len(i)) {
    tsum <- mk(0)
    for (l in seq_len(j - 1L)) tsum <- tsum + L[[ix(i, l)]] * L[[ix(j, l)]]
    s <- om(i, j) - tsum
    if (i == j) {
      if (!(as.numeric(s) > 0)) stop("Omega is not positive definite at effect ", i)
      L[[ix(i, i)]] <- sqrt(s)
    } else L[[ix(i, j)]] <- s / L[[ix(j, j)]]
  }
  L
}

# the case, parsed in one working type: parse(string) -> number, mk(double vector) -> vector
run_case <- function(cs, parse, mk) {
  re <- unlist(cs$random_effects); q <- length(re)
  theta <- lapply(cs$theta, parse); xi <- lapply(cs$xi, parse); D <- parse(cs$dose)
  lloq <- if (is.null(cs$lloq)) NULL else parse(cs$lloq)
  om_entries <- lapply(cs$omega, parse)
  om <- function(i, j) {
    if (cs$omega_shape == "diagonal") { if (i == j) om_entries[[i]] else mk(0) }
    else { a <- max(i, j); b <- min(i, j); om_entries[[a * (a - 1L) / 2L + b]] }
  }
  L <- chol_generic(om, q, mk)
  subjects <- lapply(cs$subjects, function(s) {
    zeta <- mk(vapply(s$z_eta, function(h) hex_parse(h), 0)); zeps <- mk(vapply(s$z_eps, function(h) hex_parse(h), 0))
    t <- if (identical(parse, nearest_double)) vapply(s$time, nearest_double, 0, USE.NAMES = FALSE) else mpfr(unlist(s$time), BITS)
    eta <- vector("list", q); eta_scale <- vector("list", q)
    for (i in seq_len(q)) {
      acc <- mk(0); sc <- mk(0)
      for (j in seq_len(i)) { term <- L[[(i - 1L) * q + j]] * zeta[j]; acc <- acc + term; sc <- sc + abs(term) }
      eta[[i]] <- acc; eta_scale[[i]] <- sc
    }
    P <- theta
    for (k in seq_len(q)) P[[re[k]]] <- theta[[re[k]]] * exp(eta[[k]])
    ipred <- conc_generic(cs$model, P, D, t, mk)
    pred <- conc_generic(cs$model, theta, D, t, mk)
    a <- if (is.null(xi$a)) mk(0) else xi$a; b <- if (is.null(xi$b)) mk(0) else xi$b
    sigma <- switch(cs$error_model,
      additive = ipred * 0 + a,
      proportional = b * abs(ipred),
      combined = sqrt(a * a + b * b * ipred * ipred),
      combined_sd = a + b * abs(ipred),
      stop("error model"))
    y_raw <- ipred + sigma * zeps
    neg <- as.logical(y_raw < 0)
    y <- y_raw
    if (cs$negative == "zero" && any(neg)) y[neg] <- mk(0)
    blq <- if (is.null(lloq)) rep(FALSE, length(t)) else as.logical(y < lloq)
    dv <- y
    if (any(blq)) dv[blq] <- lloq
    list(id = s$id, eta = eta, eta_scale = eta_scale, P = P, pred = pred, ipred = ipred, sigma = sigma, y_raw = y_raw,
         y_scale = abs(ipred) + sigma * abs(zeps), y = y, cens = as.integer(blq), dv = dv,
         n_zero_sigma = sum(as.logical(sigma == 0)), n_negative = sum(neg), n_blq = sum(blq))
  })
  list(L = L, q = q, subjects = subjects)
}
run_mp <- function(cs) run_case(cs, function(s) mpfr(s, BITS), function(x) mpfr(x, BITS))
run_dbl <- function(cs) run_case(cs, nearest_double, function(x) as.numeric(x))

# ------------------------------------------------------------------------------------------ the cases
SEED <- "20261108"
sx2_times <- c("0.5", "1", "2", "4", "8", "12")
subject_draws <- function(labels, times, q, r = 0) lapply(labels, function(l)
  list(id = l, time = arr(times), z_eta = arr(vapply(draw_z(SEED, r, 1, l, q), hex_fmt, "")), z_eps = arr(vapply(draw_z(SEED, r, 2, l, length(times)), hex_fmt, ""))))
subject_given <- function(id, times, z_eta, z_eps) list(id = id, time = arr(times), z_eta = arr(vapply(z_eta, hex_fmt, "")), z_eps = arr(vapply(z_eps, hex_fmt, "")))

case <- function(name, why, model, theta, re, omega_shape, omega, error_model, xi, dose, subjects, negative = "keep", lloq = NULL, rules = "SIM-ORA-03")
  list(name = name, why = why, rules = rules, model = model, theta = theta, random_effects = arr(re), omega_shape = omega_shape,
       omega = arr(omega), error_model = error_model, xi = xi, dose = dose, negative = negative, lloq = lloq, subjects = subjects)

sx2_base <- function(name, why, error_model, xi, ...) case(name, why, "pk1.iv_bolus", list(cl = "5", v = "50"), c("cl", "v"), "block",
  c("0.09", "0.03", "0.04"), error_model, xi, "100", subject_draws(c("1", "2", "3"), sx2_times, 2), ...)

exact_cases <- function() {
  cs <- list()
  add <- function(x) cs[[length(cs) + 1L]] <<- x
  add(sx2_base("sx2_proportional", "SX2: three subjects, six times, proportional b = 0.1, block Omega with correlation 0.5", "proportional", list(b = "0.1")))
  add(sx2_base("sx2_additive", "SX2: the same eps with additive a = 0.2", "additive", list(a = "0.2")))
  add(sx2_base("sx4_combined", "SX4: combined error (variance sum) a = 0.2, b = 0.1", "combined", list(a = "0.2", b = "0.1")))
  add(sx2_base("sx4_combined_sd", "combined_sd a = 0.2, b = 0.1 (SD sum)", "combined_sd", list(a = "0.2", b = "0.1")))
  add(sx2_base("sx4_negative_keep", "SX4: additive a = 1.0, negative = keep (rows with y < 0 stay)", "additive", list(a = "1.0"), negative = "keep"))
  add(sx2_base("sx4_negative_zero", "SX4: additive a = 1.0, negative = zero (y = max(y, 0))", "additive", list(a = "1.0"), negative = "zero"))
  add(sx2_base("sx4_blq", "SX4: additive a = 0.2, LLOQ 0.8: subject 3 has two BLQ rows (cens 0 0 0 1 0 1)", "additive", list(a = "0.2"), lloq = "0.8"))
  add(sx2_base("negative_zero_and_blq", "negative zero then BLQ: every y < 0 is below any LLOQ > 0 (same cens and dv as keep, only y differs)", "additive", list(a = "1.0"), negative = "zero", lloq = "0.8"))
  add(sx2_base("negative_keep_and_blq", "the same with keep", "additive", list(a = "1.0"), negative = "keep", lloq = "0.8"))
  # ties at the limit: v is fixed (50), t = 0 and eta does not enter, so IPRED = D/V * exp(0) = 2 exactly in any arithmetic
  tie_t <- c("0", "0", "0")
  add(case("tie_at_lloq", "BLQ is strict: y = LLOQ is quantified, y a few ulps below is BLQ, a few ulps above is not", "pk1.iv_bolus",
           list(cl = "5", v = "50"), "cl", "diagonal", "0.09", "additive", list(a = "1e-15"), "100",
           list(subject_given("equal", tie_t, c(0.25), c(0, -1, 1)), subject_given("below", tie_t, c(-0.5), c(-1, -1, -1)),
                subject_given("above", tie_t, c(1.5), c(1, 1, 1))), lloq = "2"))
  add(case("tie_at_zero", "negative rule: y = 0 exactly is not negative; one ulp below zero is (additive a = 2, ipred 2, z = -1)", "pk1.iv_bolus",
           list(cl = "5", v = "50"), "cl", "diagonal", "0.09", "additive", list(a = "2"), "100",
           list(subject_given("exact_zero", c("0", "0"), c(0.1), c(-1, -1)),
                subject_given("ulp_below", c("0", "0"), c(0.1), c(-1 - 2^-52, -1 - 2^-52)),
                subject_given("ulp_above", c("0", "0"), c(0.1), c(-1 + 2^-53, -1 + 2^-53))), negative = "zero"))
  add(case("tie_at_zero_keep", "the same ties with keep: the raw negative value is kept", "pk1.iv_bolus",
           list(cl = "5", v = "50"), "cl", "diagonal", "0.09", "additive", list(a = "2"), "100",
           list(subject_given("exact_zero", c("0", "0"), c(0.1), c(-1, -1)),
                subject_given("ulp_below", c("0", "0"), c(0.1), c(-1 - 2^-52, -1 - 2^-52))), negative = "keep"))
  add(case("iv_infusion_before_during_after", "pk1.iv_infusion with dur fixed, times before the dose, during, at the end and after, combined error", "pk1.iv_infusion",
           list(cl = "5", v = "50", dur = "2"), c("cl", "v"), "diagonal", c("0.09", "0.04"), "combined", list(a = "0.2", b = "0.1"), "100",
           subject_draws(c("1", "2"), c("-1", "0", "1", "2", "2.5", "6"), 2)))
  add(case("oral_1_q3_block", "pk1.oral_1, three random effects with a full block Omega (3x3 Cholesky), proportional error: t = 0 gives sigma = 0", "pk1.oral_1",
           list(cl = "5", v = "50", ka = "1.5"), c("cl", "v", "ka"), "block", c("0.09", "0.02", "0.04", "0.01", "0.015", "0.0625"), "proportional", list(b = "0.15"), "100",
           subject_draws(c("1", "2", "3"), c("0", "0.25", "0.5", "1", "2", "4", "8", "24"), 3)))
  add(case("oral_1_lag_fixed_tlag", "pk1.oral_1_lag with tlag fixed in theta (no random effect on it): zero up to and including the lag", "pk1.oral_1_lag",
           list(cl = "5", v = "50", ka = "1.5", tlag = "0.5"), c("cl", "v"), "diagonal", c("0.09", "0.04"), "combined_sd", list(a = "0.1", b = "0.1"), "100",
           subject_draws(c("1", "2"), c("0", "0.25", "0.5", "0.75", "1", "2", "6"), 2)))
  add(case("oral_1_lag_proportional_zero_sigma", "lag with proportional error: sigma = 0 before the lag, counted", "pk1.oral_1_lag",
           list(cl = "5", v = "50", ka = "1.5", tlag = "0.5"), c("cl", "v", "ka"), "diagonal", c("0.09", "0.04", "0.0625"), "proportional", list(b = "0.1"), "100",
           subject_draws(c("1", "2"), c("0", "0.25", "0.5", "0.75", "1", "2", "6"), 3)))
  add(case("pk2_iv_bolus_q3", "pk2.iv_bolus in the clearance set, effects on cl, vc and vp (q = 3, diagonal)", "pk2.iv_bolus",
           list(cl = "2", vc = "10", q = "4", vp = "8"), c("cl", "vc", "vp"), "diagonal", c("0.09", "0.04", "0.16"), "additive", list(a = "0.5"), "100",
           subject_draws(c("1", "2", "3"), c("0", "0.25", "0.5", "1", "2", "4", "8", "24"), 3)))
  add(case("high_correlation", "Omega with correlation 0.99 (pivot 0.0008): the factor L is a bit-exact target", "pk1.iv_bolus",
           list(cl = "5", v = "50"), c("cl", "v"), "block", c("0.09", "0.0594", "0.04"), "proportional", list(b = "0.1"), "100",
           subject_draws(c("1", "2"), sx2_times, 2)))
  add(case("single_effect_block", "q = 1 given as a block with one number; the effect is on v only", "pk1.iv_bolus",
           list(cl = "5", v = "50"), "v", "block", "0.0625", "proportional", list(b = "0.1"), "100",
           subject_draws(c("1", "2"), sx2_times, 1)))
  add(case("extreme_z", "the largest |z| the generator can make (8.2095...), zero, and a mixed pair; z^eps at the same extremes", "pk1.iv_bolus",
           list(cl = "5", v = "50"), c("cl", "v"), "diagonal", c("0.04", "0.09"), "proportional", list(b = "0.1"), "100",
           list(subject_given("max", sx2_times, c(-8.2095361516013856, 8.2095361516013856), c(-8.2095361516013856, 8.2095361516013856, 0, 1e-300, -1e-300, 3)),
                subject_given("zero", sx2_times, c(0, 0), c(0, 0, 0, 0, 0, 0)),
                subject_given("mixed", sx2_times, c(-3.5, 2.25), c(1.5, -1.5, 0.5, -0.5, 2.5, -2.5)))))
  add(case("all_error_parameters_zero", "all error parameters 0: y = IPRED exactly and every row is counted in n_zero_sigma", "pk1.iv_bolus",
           list(cl = "5", v = "50"), c("cl", "v"), "diagonal", c("0.09", "0.04"), "combined", list(a = "0", b = "0"), "100",
           subject_draws(c("1", "2"), sx2_times, 2)))
  add(case("predose_times", "observation rows before the dose: IPRED = PRED = 0, sigma = 0 under proportional", "pk1.iv_bolus",
           list(cl = "5", v = "50"), c("cl", "v"), "diagonal", c("0.09", "0.04"), "proportional", list(b = "0.1"), "100",
           subject_draws(c("1", "2"), c("-2", "-1", "-0.5", "0", "1", "2"), 2)))
  cs
}

# ------------------------------------------------------------------------------------------ cross-checks with the spec
close_to <- function(a, b, tol, what) {
  if (!all(abs(a / b - 1) < tol | (a == 0 & b == 0))) stop("SPEC CROSS-CHECK FAILED: ", what, ": ", paste(format(a, digits = 12), collapse = " "), " vs ", paste(format(b, digits = 12), collapse = " "))
}
spec_cross_checks <- function(res) {
  s2p <- res$sx2_proportional; s2a <- res$sx2_additive
  # L, bit-exact (the spec gives the hexadecimal values)
  Ld <- sapply(res$sx2_proportional$dbl$L, function(x) x)
  if (!identical(unlist(Ld[c(1, 3, 4)]), hex_parse(c("0x1.3333333333333p-2", "0x1.999999999999ap-4", "0x1.62b9586ad0a22p-3")))) stop("SPEC CROSS-CHECK FAILED: the Cholesky factor of SX2")
  # PRED at the six times (9 digits)
  close_to(as.numeric(s2p$mp$subjects[[1]]$pred), c(1.90245885, 1.80967484, 1.63746151, 1.34064009, 0.898657928, 0.602388424), 1e-8, "SX2 PRED")
  # subject "1": z^eta, eta, IPRED, y
  p <- s2p$mp$subjects[[1]]
  close_to(vapply(p$eta, as.numeric, 0), c(-0.62576225247738659, -0.12315824562803285), 1e-14, "SX2 eta of subject 1")
  close_to(as.numeric(p$ipred), c(2.194727210, 2.129335791, 2.004339871, 1.775929778, 1.394230685, 1.094569857), 1e-8, "SX2 IPRED of subject 1")
  close_to(as.numeric(p$y), c(2.112775450, 2.096217140, 1.951903959, 1.814575428, 1.138332645, 1.104341218), 1e-8, "SX2 y_prop of subject 1")
  close_to(as.numeric(s2a$mp$subjects[[1]]$y), c(2.120046621, 2.098228765, 1.952017496, 1.819451375, 1.027149336, 1.112424105), 1e-8, "SX2 y_add of subject 1")
  # the 25-digit 256-bit values of the spec for subject 1
  spec256 <- function(x) mpfr(x, BITS)
  ref_eta <- c(spec256("-0.6257622524773865713143550"), spec256("-0.1231582456280328309522156"))
  stopifnot(all(abs(c(p$eta[[1]], p$eta[[2]]) - ref_eta) < 1e-22))
  ref_ipred <- spec256(c("2.194727210157184274350938", "2.129335790734342907644516", "2.004339871342818624880866",
                         "1.775929778143120224479069", "1.394230684798553405713860", "1.094569856509939153226976"))
  stopifnot(all(abs(p$ipred - ref_ipred) < 1e-22))
  ref_yp <- spec256(c("2.112775449763677816107288", "2.096217139560033076543666", "1.951903959432167578775889",
                      "1.814575428446738493601467", "1.138332644515781219552664", "1.104341217751047252699768"))
  stopifnot(all(abs(p$y - ref_yp) < 1e-22))
  ref_ya <- spec256(c("2.120046621139802791986894", "2.098228765418351558631882", "1.952017495621657806185200",
                      "1.819451375426122546810738", "1.027149335818279269873542", "1.112424105250653257509089"))
  stopifnot(all(abs(s2a$mp$subjects[[1]]$y - ref_ya) < 1e-22))
  # subjects 2 and 3 (10 digits)
  close_to(as.numeric(s2p$mp$subjects[[2]]$y), c(2.428668661, 1.636176679, 2.280870511, 1.927016446, 1.541526962, 1.109067726), 1e-8, "SX2 y_prop of subject 2")
  close_to(as.numeric(s2a$mp$subjects[[3]]$y), c(1.317906144, 0.8394963479, 0.9228417933, 0.7477272597, 0.9110844407, 0.1679000294), 1e-8, "SX2 y_add of subject 3")
  # SX4 combined, subject 1
  c1 <- res$sx4_combined$mp$subjects[[1]]
  close_to(as.numeric(c1$sigma), c(0.296931432, 0.292131322, 0.283149754, 0.267468252, 0.243800722, 0.227993052), 1e-8, "SX4 combined sigma")
  close_to(as.numeric(c1$y), c(2.083852139, 2.083899109, 1.930264532, 1.834133006, 0.946757195, 1.114923080), 1e-8, "SX4 combined y")
  # SX4 negative values and BLQ
  nk <- res$sx4_negative_keep$mp$subjects; nz <- res$sx4_negative_zero$mp$subjects
  close_to(as.numeric(nk[[1]]$y), c(1.821324, 1.973801, 1.742728, 1.993538, -0.441176, 1.183841), 1e-5, "SX4 negative keep, subject 1")
  close_to(as.numeric(nk[[2]]$y), c(3.084171, -0.258999, 3.392267, 2.895062, 3.320092, 2.621724), 1e-5, "SX4 negative keep, subject 2")
  close_to(as.numeric(nk[[3]]$y), c(1.235443, -0.892719, 0.013403, -0.020013, 2.046844, -0.834763), 1e-4, "SX4 negative keep, subject 3")
  stopifnot(identical(vapply(nk, function(s) s$n_negative, 0L), c(1L, 1L, 3L)), identical(vapply(nz, function(s) s$n_negative, 0L), c(1L, 1L, 3L)))
  stopifnot(as.numeric(nz[[3]]$y)[4] == 0, as.numeric(nz[[3]]$y)[6] == 0, as.numeric(nz[[2]]$y)[2] == 0)
  b <- res$sx4_blq$mp$subjects
  stopifnot(identical(b[[1]]$cens, rep(0L, 6)), identical(b[[2]]$cens, rep(0L, 6)), identical(b[[3]]$cens, c(0L, 0L, 0L, 1L, 0L, 1L)), b[[3]]$n_blq == 2L)
  invisible(TRUE)
}

# ------------------------------------------------------------------------------------------ the stage
exact_stage <- function(out_dir) {
  cases <- exact_cases()
  res <- list()
  for (cs in cases) res[[cs$name]] <- list(cs = cs, mp = run_mp(cs), dbl = run_dbl(cs))
  # the generic Cholesky in double equals the one written first in sim_lib.R (two codings, the same bits)
  for (nm in names(res)) {
    cs <- res[[nm]]$cs; q <- length(unlist(cs$random_effects))
    ent <- vapply(cs$omega, nearest_double, 0)
    Om <- if (cs$omega_shape == "diagonal") omega_diag_matrix(ent) else omega_matrix(ent, q)
    ch <- chol_spec(Om)
    got <- matrix(unlist(res[[nm]]$dbl$L), q, q, byrow = TRUE)
    stopifnot(ch$ok, identical(got, ch$L))
  }
  spec_cross_checks(res)

  # ---- tolerance measurement: the double pipeline against 256 bits
  classes <- c("eta", "P", "pred", "ipred", "sigma", "y_raw", "y", "dv")
  mx <- setNames(numeric(length(classes)), classes); mx_plain <- mx
  nonties_min_gap <- Inf
  for (nm in names(res)) {
    cs <- res[[nm]]$cs
    for (i in seq_along(cs$subjects)) {
      m <- res[[nm]]$mp$subjects[[i]]; d <- res[[nm]]$dbl$subjects[[i]]
      stopifnot(identical(m$cens, d$cens), m$n_zero_sigma == d$n_zero_sigma, m$n_negative == d$n_negative)
      err <- function(dv, mv, scale) { x <- abs(mpfr(dv, BITS) - mv); den <- pmax(abs(as.numeric(mv)), as.numeric(scale)); r <- as.numeric(x) / den; r[den == 0] <- 0; r }
      plain <- function(dv, mv) { x <- abs(mpfr(dv, BITS) - mv); den <- abs(as.numeric(mv)); r <- as.numeric(x) / den; r[den == 0] <- as.numeric(x)[den == 0]; r }
      for (k in seq_along(m$eta)) {
        mx["eta"] <- max(mx["eta"], err(d$eta[[k]], m$eta[[k]], m$eta_scale[[k]])); mx_plain["eta"] <- max(mx_plain["eta"], plain(d$eta[[k]], m$eta[[k]]))
      }
      for (nme in names(m$P)) { mx["P"] <- max(mx["P"], plain(d$P[[nme]], m$P[[nme]])); mx_plain["P"] <- mx["P"] }
      for (cl in c("pred", "ipred", "sigma")) { mx[cl] <- max(mx[cl], plain(d[[cl]], m[[cl]])); mx_plain[cl] <- mx[cl] }
      for (cl in c("y_raw", "y", "dv")) {
        mx[cl] <- max(mx[cl], err(d[[cl]], m[[cl]], m$y_scale)); mx_plain[cl] <- max(mx_plain[cl], plain(d[[cl]], m[[cl]]))
      }
      if (!is.null(cs$lloq) && !grepl("^tie", nm)) {
        gap <- abs(as.numeric(m$y) - as.numeric(mpfr(cs$lloq, BITS))) / pmax(as.numeric(m$y_scale), 1e-300)
        nonties_min_gap <- min(nonties_min_gap, gap)
      }
    }
  }
  stopifnot(nonties_min_gap > 1e-6)   # no non-tie row sits within 1e-6 (relative to its scale) of the limit: the flags are not at risk
  tol_exact <- 1e-12

  # ---- files
  dec <- function(x) arr(mp_dec(x, 50))
  for (nm in names(res)) {
    cs <- res[[nm]]$cs; m <- res[[nm]]$mp; d <- res[[nm]]$dbl; q <- m$q
    Lhex <- lapply(seq_len(q), function(i) arr(vapply(seq_len(q), function(j) hex_fmt(d$L[[(i - 1L) * q + j]]), "")))
    Ldec <- lapply(seq_len(q), function(i) arr(vapply(seq_len(q), function(j) mp_dec(m$L[[(i - 1L) * q + j]], 50), "")))
    subj <- lapply(seq_along(m$subjects), function(i) {
      s <- m$subjects[[i]]
      list(id = s$id,
           eta = arr(vapply(s$eta, mp_dec, "", 50)), eta_scale = arr(vapply(s$eta_scale, function(x) format(as.numeric(x), digits = 17), "")),
           p = lapply(s$P, function(x) mp_dec(x, 50)),
           pred = dec(s$pred), ipred = dec(s$ipred), sigma = dec(s$sigma), y_raw = dec(s$y_raw),
           y_scale = arr(format(as.numeric(s$y_scale), digits = 17)),
           y = dec(s$y), cens = arr(s$cens), dv = dec(s$dv),
           n_zero_sigma = s$n_zero_sigma, n_negative = s$n_negative, n_blq = s$n_blq)
    })
    cs$expected <- list(l = Lhex, l_dec = Ldec, subjects = subj,
                        n_zero_sigma = sum(vapply(m$subjects, function(s) s$n_zero_sigma, 0L)),
                        n_negative = sum(vapply(m$subjects, function(s) s$n_negative, 0L)),
                        n_blq = sum(vapply(m$subjects, function(s) s$n_blq, 0L)))
    cs$subjects <- lapply(cs$subjects, function(s) s)
    cs$tol_exact <- tol_exact
    cs$note <- "z^eta and z^eps are hexadecimal doubles (exact); theta, omega, xi, dose, time, lloq are decimal strings; expected values are 50-digit decimals of the 256-bit result. eta and the y family are compared against max(|expected|, scale); l is bit-exact (hexadecimal doubles of the factor of the double nearest each omega entry)"
    write_json_lf(cs, file.path(out_dir, "exact", paste0(nm, ".json")))
  }
  measure <- list(rule = "SIM-ORA-03, OS-05", method = "the double pipeline (decimals converted by Rmpfr to the nearest double, R's exp) against the 256-bit result",
                  cases = length(res), rows = sum(vapply(res, function(r) sum(vapply(r$mp$subjects, function(s) length(s$ipred), 0L)), 0L)),
                  max_error_vs_scale = as.list(mx), max_relative_error_plain = as.list(mx_plain),
                  max_over_all = max(mx), tol_exact_proposed = tol_exact, margin = tol_exact / max(mx),
                  min_gap_of_nontie_rows_to_lloq_relative_to_scale = nonties_min_gap,
                  note = "error vs scale: |double - 256 bit| / max(|256 bit|, scale), scale = sum |L_ij z_j| for eta and |ipred| + sigma |z| for the y family; plain: relative to the 256-bit value (eta, y can cancel, hence the scale)")
  write_json_lf(measure, file.path(out_dir, "exact", "measure.json"), digits = 6)
  cat(sprintf("sim_exact: %d cases, SX2/SX4 cross-checks with the spec hold; max error vs scale %.3g; tol_exact 1e-12 margin %.0fx\n", length(res), max(mx), tol_exact / max(mx)))
  invisible(measure)
}
