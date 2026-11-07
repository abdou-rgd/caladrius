# The lagged routes and the unbounded likelihood (FIT-ML-13c, decision of the orchestrator on the
# review of T-149). Sourced by fit_ml.R; it needs PREC, mp(), num(), write_lines_lf() from there.
#
# On a route with a lag (pk1.oral_1_lag, pk1.oral_0_lag) the prediction is exactly 0 for every t <= tlag,
# and tlag has no upper bound by default (FIT-BND-03). So an observation y = 0 at a time t0 > 0 can be
# predicted exactly by putting tlag at t0 (or just below it). Under `combined` and `combined_sd` the
# standard deviation of that row is then a (> 0 only through a), the residual is 0, and -2LL -> -infinity
# as a -> 0 while the other rows stay bounded. Under `proportional` the standard deviation is b |yhat| and
# the prediction at the zero row is yhat = f(t0 - tlag): with tlag = t0 - eps both the residual and sigma
# are of order eps, z^2 = 1/b^2 stays finite, ln(sigma^2) -> -infinity. A zero that follows a nonzero
# observation does not do this: with tlag at its time the nonzero row before it is predicted 0 with a
# residual y1 != 0, so -2LL -> +infinity instead. A user bound on tlag below t0 keeps the zero row's
# prediction away from 0.
#
# The rule that results (what the engine must refuse before fitting, code unbounded_likelihood), on a
# route with a lag and under combined, combined_sd or proportional: a row with y = 0 at a time t0 > 0 such
# that (1) every observation at a time <= t0 is also 0, and (2) the domain of tlag meets [t0, t_nz), where
# t_nz is the time of the first nonzero observation (infinity if there is none): upper bound >= t0 and
# lower bound < t_nz, a fixed tlag counting as both bounds, the default lower bound being 0. With tlag at
# or above t_nz the nonzero sample is predicted 0 and -2LL goes to +infinity as a -> 0 (kind
# tlag_above_first_nonzero below), so a fixed tlag or a lower bound >= t_nz does not make it unbounded.

LAG_TIMES <- c(0.5, 1, 2, 4, 6, 8, 12, 24)  # the times of the oral_s1 profile (generator of ML4)
LAG_TRUTH <- list(v = 10, k = 0.2, ka = 1, dur = 2, tlag = 0.5, a = 0.05, b = 0.10, dose = 100)

# the prediction of the lagged models at 256 bits; th holds v, k and ka (oral_1_lag) or v, k, dur (oral_0_lag)
lag_pred <- function(model, t, D, th, tlag) {
  out <- mpfr(rep(0, length(t)), PREC)
  for (i in seq_along(t)) {
    s <- mp(t[i]) - tlag
    if (s > 0) {
      out[i] <- if (model == "pk1.oral_1_lag") {
        D * th$ka / (th$v * (th$ka - th$k)) * (exp(-th$k * s) - exp(-th$ka * s))
      } else {
        R0CL <- (D / th$dur) / (th$v * th$k)
        if (s <= th$dur) R0CL * (1 - exp(-th$k * s)) else R0CL * (1 - exp(-th$k * th$dur)) * exp(-th$k * (s - th$dur))
      }
    }
  }
  out
}

# the terms of -2LL, one per row: ln(2 pi sigma^2) + r^2 / sigma^2
lag_terms <- function(model, em, t, y, D, th, tlag, a, b) {
  yh <- lag_pred(model, t, D, th, tlag)
  r <- mp(y) - yh
  v <- switch(em,
              proportional = b^2 * yh^2,
              combined = a^2 + b^2 * yh^2,
              combined_sd = (a + b * abs(yh))^2)
  log(2 * Const("pi", PREC) * v) + r^2 / v
}

lag_profile <- function(model) {
  th <- if (model == "pk1.oral_1_lag") list(v = mp(LAG_TRUTH$v), k = mp(LAG_TRUTH$k), ka = mp(LAG_TRUTH$ka))
        else list(v = mp(LAG_TRUTH$v), k = mp(LAG_TRUTH$k), dur = mp(LAG_TRUTH$dur))
  mu <- as.numeric(asNumeric(lag_pred(model, LAG_TIMES, LAG_TRUTH$dose, th, mp(LAG_TRUTH$tlag))))
  set.seed(1)
  z <- rnorm(length(LAG_TIMES))
  sd <- sqrt(LAG_TRUTH$a^2 + LAG_TRUTH$b^2 * mu^2)
  y <- as.numeric(sprintf("%.15g", mu + sd * z))
  base <- y; base[1] <- 0                  # the first sample, at the lag, observed as exactly 0
  ctrl <- base; ctrl[1] <- y[2]; ctrl[2] <- 0  # the first two samples swapped: the sample at the second time (1) is 0, preceded by a nonzero one
  list(model = model, theta = th, time = LAG_TIMES, base = base, ctrl = ctrl, mu = mu, z = z)
}

write_lag_dataset <- function(prof, y, file) {
  write_lines_lf(c("subject,time,conc,dose",
                   sprintf("1,%s,%s,%s", num(prof$time), num(y), num(LAG_TRUTH$dose))), file)
}

# The paths. Returns a list of records and stops when a path does not behave as the rule says.
lag_paths <- function(prof) {
  m <- prof$model; th <- prof$theta; D <- LAG_TRUTH$dose; t <- prof$time
  b <- mp(LAG_TRUTH$b)
  js <- c(1, 2, 5, 10, 20, 50)  # 10^-50 is still resolved by 256 bits (77 digits); tlag = t0 - 10^-j must differ from t0
  recs <- list()
  add <- function(kind, em, path, j, tlag, a, terms, row) {
    recs[[length(recs) + 1L]] <<- list(kind = kind, error_model = em, path = path, j = j,
      m2ll = as.numeric(asNumeric(sum(terms))), row_term = as.numeric(asNumeric(terms[row])),
      other_rows = as.numeric(asNumeric(sum(terms[-row]))))
  }
  one_over <- function(j) mpfr(10, PREC)^(-j)
  t0 <- mp(t[1])
  for (em in c("combined", "combined_sd", "proportional")) {
    # unbounded: leading zero at t0 = 0.5, tlag at t0 (combined forms) or at t0 - 10^-j (proportional)
    prev <- NULL; prev_other <- NULL
    for (j in js) {
      if (em == "proportional") { tlag <- t0 - one_over(j); a <- mp(0) } else { tlag <- t0; a <- one_over(j) }
      terms <- lag_terms(m, em, t, prof$base, D, th, tlag, a, b)
      cur <- as.numeric(asNumeric(sum(terms)))
      if (!is.null(prev) && !(cur < prev))
        stop(sprintf("%s %s: -2LL does not decrease along the path (j = %g: %.12g after %.12g)", m, em, j, cur, prev), call. = FALSE)
      prev <- cur
      add("leading_zero", em, if (em == "proportional") "tlag = t0 - 10^-j, b fixed" else "tlag = t0, a = 10^-j, b fixed", j, tlag, a, terms, 1L)
    }
    # the other rows stay bounded: their sum changes by less than 1e-9 (relative) between j = 20 and j = 50
    r20 <- Filter(function(r) r$kind == "leading_zero" && r$error_model == em && r$j == 20, recs)[[1]]
    r50 <- Filter(function(r) r$kind == "leading_zero" && r$error_model == em && r$j == 50, recs)[[1]]
    if (abs(r50$other_rows - r20$other_rows) > 1e-9 * max(1, abs(r20$other_rows)))
      stop(sprintf("%s %s: the other rows are not bounded along the path", m, em), call. = FALSE)
    # a zero after a nonzero observation: the same path ends at +infinity (the first row is predicted 0 with residual y1 != 0)
    prev <- NULL
    for (j in js) {
      if (em == "proportional") { tlag <- t0 - one_over(j); a <- mp(0) } else { tlag <- mp(t[2]); a <- one_over(j) }
      terms <- lag_terms(m, em, t, prof$ctrl, D, th, tlag, a, b)
      cur <- as.numeric(asNumeric(sum(terms)))
      if (!is.null(prev) && !(cur > prev))
        stop(sprintf("%s %s: -2LL of the zero after a nonzero observation does not increase (j = %g)", m, em, j), call. = FALSE)
      prev <- cur
      add("zero_after_nonzero", em, if (em == "proportional") "tlag = t0 - 10^-j" else "tlag = time of the zero (1), a = 10^-j", j, tlag, a, terms, 2L)
    }
    # an upper bound on tlag below t0 (0.4): at the bound the zero row keeps a nonzero prediction, the limit a -> 0 is finite
    if (em != "proportional") {
      prev <- NULL
      for (j in js) {
        a <- one_over(j)
        terms <- lag_terms(m, em, t, prof$base, D, th, mp(0.4), a, b)
        add("upper_bound_below_t0", em, "tlag = 0.4, a = 10^-j", j, mp(0.4), a, terms, 1L)
      }
      r20 <- Filter(function(r) r$kind == "upper_bound_below_t0" && r$error_model == em && r$j == 20, recs)[[1]]
      r50 <- Filter(function(r) r$kind == "upper_bound_below_t0" && r$error_model == em && r$j == 50, recs)[[1]]
      if (abs(r50$m2ll - r20$m2ll) > 1e-9 * max(1, abs(r20$m2ll)))
        stop(sprintf("%s %s: -2LL is not bounded when tlag <= 0.4", m, em), call. = FALSE)
    }
    # the domain of tlag above the first nonzero sample (t_nz = 1): a fixed tlag of 1, or a lower bound of 1 (tlag = 1
    # and tlag = 1.5 below). The rows at t0 = 0.5 and t_nz = 1 are then both predicted 0 for every theta, the zero
    # contributes ln(2 pi a^2) but the nonzero one contributes ln(2 pi a^2) + y^2 / a^2 with y = 3.79...: together
    # they are at least 2 ln(pi y^2) + 2 whatever a is (minimum over sigma^2 = a^2 at y^2 / 2), and along a -> 0
    # -2LL increases without limit. The likelihood is not unbounded through the zero.
    if (em != "proportional") {
      y2 <- mp(prof$base[2])
      floor12 <- 2 * log(Const("pi", PREC) * y2^2) + 2
      for (tl in c(1, 1.5)) {
        prev <- NULL
        for (j in js) {
          a <- one_over(j)
          terms <- lag_terms(m, em, t, prof$base, D, th, mp(tl), a, b)
          rows12 <- terms[1] + terms[2]
          if (!(rows12 >= floor12))
            stop(sprintf("%s %s: rows 1 and 2 are below their lower bound at tlag = %g", m, em, tl), call. = FALSE)
          cur <- as.numeric(asNumeric(sum(terms)))
          if (!is.null(prev) && !(cur > prev))
            stop(sprintf("%s %s: -2LL does not increase as a -> 0 at tlag = %g (j = %g)", m, em, tl, j), call. = FALSE)
          prev <- cur
          add("tlag_above_first_nonzero", em, sprintf("tlag = %g, a = 10^-j", tl), j, mp(tl), a, terms, 1L)
          recs[[length(recs)]]$rows_1_2 <- as.numeric(asNumeric(rows12))
          recs[[length(recs)]]$rows_1_2_lower_bound <- as.numeric(asNumeric(floor12))
        }
      }
    }
  }
  recs
}
