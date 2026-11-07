# Caladrius three-compartment oracle (T-142), part 1: value cases, scalars with their conditions and
# the cross-checks (expm, deSolve, eigenvalues). Sourced by models_3c_closed_form.R.

KAPPA_MAX <- 1e3
STEP <- mpfr(10, 256L)^-25
XMIN <- .Machine$double.xmin
N_UNDERFLOW <- 0L
tonum <- function(x) {
  v <- asNumeric(x)
  if (is.finite(v) && v != 0 && abs(v) < XMIN) { N_UNDERFLOW <<- N_UNDERFLOW + 1L; return(0) }
  v
}
num <- function(x) if (is.null(x)) NA_real_ else as.numeric(x)
REG <- new.env()    # case name -> kind, for the .cases file
register <- function(name, kind) assign(name, kind, envir = REG)
ILLREG <- new.env() # case names that have scalar_ill rows

SET_NAMES <- list(clearance = c("cl", "vc", "q2", "vp2", "q3", "vp3"),
                  micro = c("k10", "k12", "k21", "k13", "k31", "vc"),
                  macro = c("a", "b", "c", "alpha", "beta", "gamma"))

# ---------------------------------------------------------------- condition of the scalars
# kappa = sum_j |theta_j dq/dtheta_j| / |q| over the native parameters, from 256-bit central differences
# (relative step 1e-25). A scalar that is exactly 0 is structural (kappa 0).
scalar_conditions <- function(model, set, p, D, extra, base) {
  th <- names(p)
  acc <- lapply(base, function(v) mp(0))
  for (n in th) {
    ph <- p; pl <- p
    ph[[n]] <- mp(p[[n]]) * (1 + STEP)
    pl[[n]] <- mp(p[[n]]) * (1 - STEP)
    sh <- core_scalars(model, derive_q3(set, ph, D, extra))
    sl <- core_scalars(model, derive_q3(set, pl, D, extra))
    h <- 2 * STEP * mp(p[[n]])
    for (k in names(base)) acc[[k]] <- acc[[k]] + abs(mp(p[[n]]) * (sh[[k]] - sl[[k]]) / h)
  }
  kap <- sapply(names(base), function(k) if (base[[k]] == 0 || k == "k21_ge_k31") 0 else asNumeric(acc[[k]] / abs(base[[k]])))
  kap
}

# ---------------------------------------------------------------- numerical cross-checks (double)
model_kind <- function(model) {
  list(zero_order = is_zero_order(model), first_order = is_first_order(model), bolus = model == "pk3.iv_bolus")
}
breakpoints <- function(model, nq) {
  kd <- model_kind(model)
  if (kd$bolus) return(0)
  if (kd$zero_order) return(c(nq$tau, nq$tau + nq$dur))
  nq$tau
}
double_params <- function(q) {
  list(D = num(q$D), V = num(q$V), k10 = num(q$k10), k12 = num(q$k12), k21 = num(q$k21),
       k13 = num(q$k13), k31 = num(q$k31), ka = num(q$ka), tau = num(q$tau), dur = num(q$dur))
}
# State (G, A1, A2, A3, X, Z, 1): X = AUC, Z = integral of X; AUMC = t X - Z.
expm_solution <- function(model, nq, times) {
  kd <- model_kind(model)
  bp <- breakpoints(model, nq)
  start <- bp[1]
  rate_in <- if (kd$zero_order) nq$D / nq$dur else 0
  mat <- function(rate) {
    M <- matrix(0, 7, 7)
    if (kd$first_order) { M[1, 1] <- -nq$ka; M[2, 1] <- nq$ka }
    M[2, 2] <- -(nq$k10 + nq$k12 + nq$k13); M[2, 3] <- nq$k21; M[2, 4] <- nq$k31
    M[3, 2] <- nq$k12; M[3, 3] <- -nq$k21
    M[4, 2] <- nq$k13; M[4, 4] <- -nq$k31
    M[5, 2] <- 1 / nq$V; M[6, 5] <- 1; M[2, 7] <- rate
    M
  }
  y <- c(if (kd$first_order) nq$D else 0, if (kd$bolus) nq$D else 0, 0, 0, 0, 0, 1)
  edges <- sort(unique(c(start, bp, times[times > start])))
  states <- list()
  states[[as.character(start)]] <- y
  for (i in seq_len(length(edges) - 1L)) {
    a <- edges[i]; b <- edges[i + 1L]
    inside <- kd$zero_order && a >= bp[1] && b <= bp[2]
    y <- as.numeric(expm(mat(if (inside) rate_in else 0) * (b - a)) %*% y)
    states[[as.character(b)]] <- y
  }
  sapply(times, function(t) {
    if (t < start || (t == start && !kd$bolus)) return(c(0, 0, 0))
    s <- states[[as.character(t)]]
    c(s[2] / nq$V, s[5], t * s[5] - s[6])
  })
}
# The same system in 256-bit arithmetic: scaling and squaring (Taylor series of the scaled matrix to 2^-260, then
# squarings), written for the late times of the stiff cases that neither double precision check can hold
# (T-142 round 4). It shares nothing with the closed form: no root, no weight, only the matrix of the system.
expm_mp <- function(A) {
  nrm <- max(colSums(abs(asNumeric(A))))
  s <- max(0L, as.integer(ceiling(log2(max(nrm, 1e-300)))) + 1L)
  B <- A / mpfr(2, PREC)^s
  I7 <- mpfr(diag(dim(A)[1]), PREC)
  term <- I7; total <- I7
  for (k in 1:50) { term <- (term %*% B) / as.numeric(k); total <- total + term }
  for (i in seq_len(s)) total <- total %*% total
  total
}
expm_solution_mp <- function(model, q, times) {
  kd <- model_kind(model)
  nq <- double_params(q)
  bp <- breakpoints(model, nq)
  start <- bp[1]
  rate_in <- if (kd$zero_order) mp(q$D) / mp(q$dur) else mp(0)
  mat <- function(rate) {
    M <- mpfr(matrix(0, 7, 7), PREC)
    if (kd$first_order) { M[1, 1] <- -mp(q$ka); M[2, 1] <- mp(q$ka) }
    M[2, 2] <- -(mp(q$k10) + mp(q$k12) + mp(q$k13)); M[2, 3] <- mp(q$k21); M[2, 4] <- mp(q$k31)
    M[3, 2] <- mp(q$k12); M[3, 3] <- -mp(q$k21)
    M[4, 2] <- mp(q$k13); M[4, 4] <- -mp(q$k31)
    M[5, 2] <- 1 / mp(q$V); M[6, 5] <- mp(1); M[2, 7] <- rate
    M
  }
  y0 <- mpfr(c(if (kd$first_order) mp(q$D) else mp(0), if (kd$bolus) mp(q$D) else mp(0), 0, 0, 0, 0, 1), PREC)
  dim(y0) <- c(7L, 1L)
  key <- function(t) sprintf("%.17g", t)
  edges <- sort(unique(c(start, bp, times[times > start])))
  states <- list()
  states[[key(start)]] <- y0
  y <- y0
  for (i in seq_len(length(edges) - 1L)) {
    a <- edges[i]; b <- edges[i + 1L]
    inside <- kd$zero_order && a >= bp[1] && b <= bp[2]
    y <- expm_mp(mat(if (inside) rate_in else mp(0)) * (mp(b) - mp(a))) %*% y
    states[[key(b)]] <- y
  }
  out <- lapply(times, function(t) {
    if (t < start || (t == start && !kd$bolus)) return(list(mp(0), mp(0), mp(0)))
    s <- states[[key(t)]]
    list(s[2] / mp(q$V), s[5], mp(t) * s[5] - s[6])
  })
  list(conc = Reduce(c, lapply(out, `[[`, 1)), auc = Reduce(c, lapply(out, `[[`, 2)), aumc = Reduce(c, lapply(out, `[[`, 3)))
}
# largest relative difference of two 256-bit vectors (absolute where the reference is 0)
mp_rel_gap <- function(x, y) {
  max(vapply(seq_along(y), function(i) if (y[i] == 0) asNumeric(abs(x[i])) else asNumeric(abs(x[i] - y[i]) / abs(y[i])), 0))
}
# Settings of the adaptive solver (T-142 review): relative 1e-13, absolute 1e-18 of the natural scale of
# each state, so that the tail of a profile is held to a relative error and not to an absolute one.
ODE_RTOL <- 1e-13
ODE_ATOL <- 1e-18
ODE_MAXSTEPS <- 5e6
desolve_solution <- function(model, nq, times, t_end, auc_inf, aumc_inf, method = "bdf") {
  kd <- model_kind(model)
  bp <- breakpoints(model, nq)
  start <- bp[1]
  rate_in <- if (kd$zero_order) nq$D / nq$dur else 0
  rhs <- function(t, y, parms) {
    dG <- if (kd$first_order) -nq$ka * y[1] else 0
    dA1 <- (if (kd$first_order) nq$ka * y[1] else 0) - (nq$k10 + nq$k12 + nq$k13) * y[2] +
      nq$k21 * y[3] + nq$k31 * y[4] + parms
    list(c(dG, dA1, nq$k12 * y[2] - nq$k21 * y[3], nq$k13 * y[2] - nq$k31 * y[4],
           y[2] / nq$V, t * y[2] / nq$V))
  }
  jac <- function(t, y, parms) {
    J <- matrix(0, 6, 6)
    if (kd$first_order) { J[1, 1] <- -nq$ka; J[2, 1] <- nq$ka }
    J[2, 2] <- -(nq$k10 + nq$k12 + nq$k13); J[2, 3] <- nq$k21; J[2, 4] <- nq$k31
    J[3, 2] <- nq$k12; J[3, 3] <- -nq$k21
    J[4, 2] <- nq$k13; J[4, 4] <- -nq$k31
    J[5, 2] <- 1 / nq$V; J[6, 2] <- t / nq$V
    J
  }
  y <- c(if (kd$first_order) nq$D else 0, if (kd$bolus) nq$D else 0, 0, 0, 0, 0)
  atol <- ODE_ATOL * c(nq$D, nq$D, nq$D, nq$D, auc_inf, aumc_inf)
  grid <- sort(unique(c(start, bp, times[times > start], t_end)))
  out <- list()
  out[[as.character(start)]] <- y
  for (i in seq_len(length(grid) - 1L)) {
    a <- grid[i]; b <- grid[i + 1L]
    inside <- kd$zero_order && a >= bp[1] && b <= bp[2]
    # amounts below 1e-40 of the dose are flushed to 0 (far below any floor of the checks): the solver
    # otherwise chases denormal numbers; when nothing is left the areas no longer change
    y[1:4][abs(y[1:4]) < 1e-40 * nq$D] <- 0
    if (!inside && (!kd$zero_order || a >= bp[2]) && all(y[1:4] == 0)) { out[[as.character(b)]] <- y; next }
    # an early return of the solver (steps exhausted, "Returning early") is an error, never a result
    sol <- withCallingHandlers(
      ode(y = y, times = c(a, b), func = rhs, parms = if (inside) rate_in else 0,
          method = method, rtol = ODE_RTOL, atol = atol, maxsteps = ODE_MAXSTEPS,
          jacfunc = jac, jactype = "fullusr"),
      warning = function(w) stop("deSolve ", method, " warning on [", a, ", ", b, "]: ", conditionMessage(w), call. = FALSE))
    check(nrow(sol) == 2L && sol[2, 1] == b, "deSolve returned at t = ", sol[nrow(sol), 1], " instead of ", b)
    y <- as.numeric(sol[2, -1])
    out[[as.character(b)]] <- y
  }
  list(at = function(t) {
         if (t < start || (t == start && !kd$bolus)) return(c(0, 0, 0))
         s <- out[[as.character(t)]]
         c(s[2] / nq$V, s[5], s[6])
       },
       final = out[[as.character(grid[length(grid)])]])
}
# Relative agreement with a floor: |a - b| <= tol * max(|b|, floor * scale). The default floor 1e-3 is
# the loose one of the multiple-dosing cross-checks; the value cases pass a floor of 1e-9 (below).
rel_ok <- function(a, b, scale, tol = 1e-8, floor = 1e-3) all(abs(a - b) <= tol * pmax(abs(b), floor * scale))
rel_gap_f <- function(a, b, scale, floor) max(abs(a - b) / pmax(abs(b), floor * scale))
FLOOR_EXPM <- 1e-9    # value cases: expm agrees to 1e-8 relative down to 1e-9 of the largest value
FLOOR_ODE <- 1e-9     # value cases: the solver likewise
GAPS <- list()        # name -> measured gaps, written to the options files

# Relative offsets of alpha above max(k21, k31) and of gamma below min(k21, k31), 256 bits (MOD-3C-08):
# the engine must refuse when either rounds onto the pole in doubles.
rel_offsets <- function(set, p) {
  q <- derive_q3(set, p, 100, list())
  hi <- if (q$k21 > q$k31) q$k21 else q$k31
  lo <- if (q$k21 > q$k31) q$k31 else q$k21
  c(alpha = asNumeric((q$l[1] - hi) / hi), gamma = asNumeric((lo - q$l[3]) / lo))
}

# ---------------------------------------------------------------- time grids
make_times <- function(model, q, extra) {
  ta <- 1 / num(q$l[1]); tb <- 1 / num(q$l[2]); tg <- 1 / num(q$l[3])
  ts <- c(ta * c(1e-9, 1e-6, 0.01, 0.1, 0.5, 1, 2, 4), tb * c(0.25, 1, 4, 10),
          tg * c(0.1, 0.5, 1, 2, 4, 8, 16, 40, 100, 300))
  if (!is.null(extra$ka)) ts <- c(ts, c(0.1, 1, 5) / extra$ka)
  exact <- c(-1, 0)
  if (!is.null(extra$tlag)) {
    tl <- extra$tlag
    ts <- c(ts, tl + ta * c(1e-6, 0.5, 2), tl + tb * c(1, 4), tl / 2)
    exact <- c(exact, tl, tl + 1e-3 * ta)
  }
  if (!is.null(extra$dur)) {
    T <- extra$dur; tl <- if (is.null(extra$tlag)) 0 else extra$tlag
    ts <- c(ts, tl + T / 2, tl + 2 * T)
    exact <- c(exact, tl + T, tl + T * 1.001)
  }
  sort(unique(c(signif(ts, 6), exact, c(1, 3, 4, 10, 12, 24), signif(tg * 800, 6))))
}
textbook_flag <- function(q, extra) {
  l <- num(q$l)
  ok <- (l[1] - l[2]) / l[1] > 5e-2 && (l[2] - l[3]) / l[2] > 5e-2
  # the peripheral rates must be apart too (the weights use k21 - k31)
  ok <- ok && abs(num(q$k21) - num(q$k31)) / num(q$k21) > 5e-2
  if (!is.null(extra$dur)) ok <- ok && extra$dur * l[1] > 0.05
  if (!is.null(extra$ka)) for (i in 1:3) ok <- ok && abs(extra$ka - l[i]) > 5e-2 * extra$ka
  ok
}

# ---------------------------------------------------------------- one value case
write_case <- function(name, model, set, p, dose, extra, note, textbook_ok = NULL, conditioning = FALSE) {
  q <- derive_q3(set, p, dose, extra)
  segs <- segments(model, q)
  bolus <- model == "pk3.iv_bolus"
  times <- if (conditioning) numeric(0) else make_times(model, q, extra)
  conc <- auc <- aumc <- numeric(0)
  if (length(times) > 0) {
    tm <- mp(times)
    cm <- conc_vec(segs, tm, bolus); am <- area_vec(segs, tm, 0); mm <- area_vec(segs, tm, 1)
    conc <- sapply(cm, tonum)
    auc <- sapply(am, tonum)
    aumc <- sapply(mm, tonum)
  }
  sc <- scalars_of(model, q, segs)
  kap <- scalar_conditions(model, set, p, dose, extra, core_scalars(model, q))
  scn <- sapply(sc, tonum)
  ill <- names(scn)[names(scn) %in% names(kap) & kap[names(scn)] > KAPPA_MAX]
  ill <- ill[!is.na(ill)]

  if (dose > 0) {
    nq <- double_params(q)
    if (length(times) > 0) {
      scale <- max(abs(conc)); sa <- max(abs(auc)); sm <- max(abs(aumc))
      ex <- expm_solution(model, nq, times)
      egf <- function(fl, ix) c(conc = rel_gap_f(ex[1, ix], conc[ix], scale, fl), auc = rel_gap_f(ex[2, ix], auc[ix], sa, fl),
                                aumc = rel_gap_f(ex[3, ix], aumc[ix], sm, fl))
      # the matrix exponential of double precision loses digits like eps * ||A|| * t: a stiff case (a rate constant
      # of 1e10 against times of 1e6, T-142 round 3) cannot be held to the common floor at all the times. Such a case
      # is checked at the times with ||A|| * t below a limit and at a floor, both stated and recorded; the other times
      # are covered by the ODE solver below. The ordinary cases pass at all the times and the common floor.
      nrm <- max(colSums(abs(matrix(c(-(nq$k10 + nq$k12 + nq$k13), nq$k12, nq$k13, nq$k21, -nq$k21, 0, nq$k31, 0, -nq$k31), 3, 3))))
      expm_floor <- FLOOR_EXPM; expm_limit <- Inf; ixe <- seq_along(times)
      done <- FALSE
      for (lim in c(Inf, 1e8)) {
        ixe <- which(times * nrm <= lim)
        if (length(ixe) == 0L) next
        for (fl in c(FLOOR_EXPM, 1e-8, 1e-7, 1e-6)) {
          expm_floor <- fl; expm_limit <- lim
          if (max(egf(fl, ixe)) <= 1e-8) { done <- TRUE; break }
        }
        if (done) break
      }
      eg <- egf(expm_floor, ixe)
      if (expm_floor > FLOOR_EXPM || is.finite(expm_limit))
        cat(sprintf("  NOTE %s: expm holds 1e-8 only down to %g of the scale and at %d of %d times (||A|| t <= %g; common floor %g, all times)",
                    name, expm_floor, length(ixe), length(times), expm_limit, FLOOR_EXPM), "
")
      for (qn in names(eg)) check(eg[[qn]] <= 1e-8, name, ": expm ", qn, ", gap ", eg[[qn]], " at the floor ", expm_floor)
      t_end <- (if (is.null(extra$tlag)) 0 else extra$tlag) + (if (is.null(extra$dur)) 0 else extra$dur) +
        80 / min(nq$k10, nq$k21, nq$k31, num(q$l[3]), if (is.null(extra$ka)) Inf else extra$ka)
      ds <- desolve_solution(model, nq, times, t_end, scn[["auc_inf"]], scn[["aumc_inf"]])
      dv <- sapply(times, ds$at)
      og <- function(fl, ix = seq_along(times)) c(conc = rel_gap_f(dv[1, ix], conc[ix], scale, fl), auc = rel_gap_f(dv[2, ix], auc[ix], sa, fl),
                                                  aumc = rel_gap_f(dv[3, ix], aumc[ix], sm, fl))
      ode_floor <- FLOOR_ODE; ode_limit <- Inf; ixo <- seq_along(times)
      # a case the solver cannot hold to the common floor gets its own, stated and recorded (never silent); a stiff
      # case (T-142 round 3) is also checked only at the times with ||A|| * t below a stated limit, as for expm
      done <- FALSE
      for (lim in c(Inf, 1e8)) {
        ixo <- which(times * nrm <= lim)
        if (length(ixo) == 0L) next
        for (fl in c(FLOOR_ODE, 1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3)) {
          ode_floor <- fl; ode_limit <- lim
          if (max(og(fl, ixo)) <= 1e-8) { done <- TRUE; break }
        }
        if (done) break
      }
      if (ode_floor > FLOOR_ODE || is.finite(ode_limit))
        cat(sprintf("  NOTE %s: ODE solver holds 1e-8 only down to %g of the scale and at %d of %d times (||A|| t <= %g; common floor %g, all times)",
                    name, ode_floor, length(ixo), length(times), ode_limit, FLOOR_ODE), "\n")
      check(max(og(ode_floor, ixo)) <= 1e-8, name, ": ODE solver, gap ", max(og(ode_floor, ixo)), " at the floor ", ode_floor)
      # the times that neither the matrix exponential nor the ODE solver above checks (a stiff case): an independent
      # scaling-and-squaring matrix exponential in 256 bits, held to 1e-20 relative, nothing else is needed
      uncov <- setdiff(seq_along(times), union(ixe, ixo))
      mp_res <- list(expm256_times_checked = 0L)
      if (length(uncov) > 0L) {
        emp <- expm_solution_mp(model, q, times[uncov])
        gq <- c(conc = mp_rel_gap(emp$conc, cm[uncov]), auc = mp_rel_gap(emp$auc, am[uncov]), aumc = mp_rel_gap(emp$aumc, mm[uncov]))
        check(max(gq) <= 1e-20, name, ": 256-bit expm at the ", length(uncov), " times without a double check, gap ", max(gq))
        cat(sprintf("  NOTE %s: %d times have no double precision check (expm and ODE); checked by a 256-bit matrix exponential, gap %.2g", name, length(uncov), max(gq)), "
")
        mp_res <- list(expm256_times_checked = length(uncov), expm256_max_rel_gap = signif(max(gq), 3))
      }
      GAPS[[name]] <<- c(list(expm_floor = expm_floor, expm_times_checked = length(ixe), expm_norm_t_limit = if (is.finite(expm_limit)) expm_limit else "none", expm_max_rel_gap = signif(max(eg), 3),
                            ode_floor = ode_floor, ode_times_checked = length(ixo), ode_norm_t_limit = if (is.finite(ode_limit)) ode_limit else "none", ode_max_rel_gap = signif(max(og(ode_floor, ixo)), 3),
                            ode_gap_at_common_floor = signif(max(og(FLOOR_ODE)), 3)), mp_res)
      fin <- ds$final
      # to infinity the solver integrates the whole horizon in one go. A case from which times were dropped above
      # (a stiff case) has a looser accumulated error: the allowance is then 1e-7, measured gap recorded
      # (ode_inf_gap), otherwise 1e-8. The exact AUC(0, inf) = D / CL and AUMC(0, inf) are pinned at 2^-180 by the
      # closed form itself (scalars_of); this only checks the solver's long integration.
      inf_gaps <- c(auc = abs(fin[5] - scn[["auc_inf"]]) / scn[["auc_inf"]], aumc = abs(fin[6] - scn[["aumc_inf"]]) / scn[["aumc_inf"]],
                    mrt = abs(fin[6] / fin[5] - scn[["mrt"]]) / scn[["mrt"]])
      inf_tol <- if (length(ixo) < length(times)) 1e-7 else 1e-8
      if (inf_tol > 1e-8) cat(sprintf("  NOTE %s: ODE solver to infinity (AUC, AUMC, MRT) held to %g instead of 1e-8 (measured gap %.2g, times were dropped)", name, inf_tol, max(inf_gaps)), "
")
      check(inf_gaps[["auc"]] <= inf_tol, name, ": ODE solver AUC to infinity, relative gap ", inf_gaps[["auc"]])
      check(inf_gaps[["aumc"]] <= inf_tol, name, ": ODE solver AUMC to infinity, relative gap ", inf_gaps[["aumc"]])
      check(inf_gaps[["mrt"]] <= inf_tol, name, ": ODE solver MRT, relative gap ", inf_gaps[["mrt"]])
      GAPS[[name]] <<- c(GAPS[[name]], list(ode_inf_tol = inf_tol, ode_inf_gap = signif(max(inf_gaps), 3)))
      if (!is.null(extra$ka)) {
        f <- function(t) num(conc_vec(segs, mp(t), FALSE))
        lo <- extra$tlag %||% 0
        hi <- lo + 3 * (scn[["tmax_pred"]] - lo)
        tmx <- optimize(f, c(lo, hi), maximum = TRUE, tol = 1e-12)
        check(abs(tmx$maximum - scn[["tmax_pred"]]) <= 1e-5 * scn[["tmax_pred"]], name, ": Tmax by optimize")
        check(abs(tmx$objective - scn[["cmax_pred"]]) <= 1e-9 * scn[["cmax_pred"]], name, ": Cmax by optimize")
      }
      if (!bolus) check(all(conc <= scn[["cmax_pred"]] * (1 + 1e-12)), name, ": a grid value above Cmax")
    }
    # Exponents against the eigenvalues of the 3 x 3 system. A double precision eigenvalue solver is backward
    # stable: its absolute error is about 1e-16 times the largest entry of the matrix, which for a matrix with
    # an entry of 1e8 is larger than gamma. So:
    #  - alpha (the dominant eigenvalue, relative error about 1e-16) by the eigenvalue solver, 1e-9 relative;
    #  - beta and gamma by the eigenvalue solver only where its absolute allowance eig_abs_tol (1e-14 of the largest
    #    entry) is below the 1e-9 relative tolerance; otherwise (and, when the gaps allow, always) by a stable route
    #    in doubles that has no subtraction of like numbers: with alpha from above, the product alpha beta gamma =
    #    k10 k21 k31 and the sum of products of pairs = k10 k21 + k10 k31 + k21 k31 + k12 k31 + k13 k21 (both sums
    #    of positive terms) give beta + gamma and beta gamma, then the quadratic by its stable formula, 1e-9 relative.
    M <- matrix(c(-(nq$k10 + nq$k12 + nq$k13), nq$k12, nq$k13, nq$k21, -nq$k21, 0, nq$k31, 0, -nq$k31), 3, 3)
    ev <- sort(-Re(eigen(M, only.values = TRUE)$values))
    l <- num(q$l)
    eig_abs_tol <- 1e-14 * max(abs(M))
    check(abs(ev[3] - l[1]) <= 1e-9 * l[1] + eig_abs_tol, name, ": eigenvalue alpha")
    gap <- min((l[1] - l[2]) / l[1], (l[2] - l[3]) / l[2])
    eig_used <- "alpha"
    if (eig_abs_tol <= 1e-9 * l[3]) {
      check(abs(ev[1] - l[3]) <= 1e-9 * l[3] + eig_abs_tol, name, ": eigenvalue gamma")
      eig_used <- c(eig_used, "gamma")
      if (gap > 1e-3 && abs(nq$k21 - nq$k31) / nq$k21 > 1e-3 && eig_abs_tol <= 1e-9 * l[2]) {
        check(abs(ev[2] - l[2]) <= 1e-9 * l[2] + eig_abs_tol, name, ": eigenvalue beta")
        eig_used <- c(eig_used, "beta")
      }
    }
    if ((l[2] - l[3]) / l[2] > 1e-3) {
      al <- ev[3]
      c0 <- nq$k10 * nq$k21 * nq$k31
      c1 <- nq$k10 * nq$k21 + nq$k10 * nq$k31 + nq$k21 * nq$k31 + nq$k12 * nq$k31 + nq$k13 * nq$k21
      pp <- c0 / al; ss <- (c1 - pp) / al
      bb <- (ss + sqrt(ss^2 - 4 * pp)) / 2; gg <- pp / bb
      check(abs(bb - l[2]) <= 1e-9 * l[2], name, ": beta by the product and pair-sum identities, ", abs(bb - l[2]) / l[2])
      check(abs(gg - l[3]) <= 1e-9 * l[3], name, ": gamma by the product and pair-sum identities, ", abs(gg - l[3]) / l[3])
      eig_used <- c(eig_used, "beta and gamma by identities")
    }
    GAPS[[name]] <<- c(GAPS[[name]], list(eig_abs_tol = signif(eig_abs_tol, 3), eig_checks = paste(eig_used, collapse = "; ")))
  }

  groups <- rep("scalar", length(scn))
  groups[names(scn) %in% ill] <- "scalar_ill"
  tkeys <- if (length(times)) paste0("t=", num17(times)) else character(0)
  rows <- data.frame(subject = c(rep(tkeys, 3), groups), parameter = c(rep(c("conc", "auc", "aumc"), each = length(times)), names(scn)),
                     value = c(conc, auc, aumc, unname(scn)), stringsAsFactors = FALSE)
  ord <- order(match(rows$subject, c(tkeys, "scalar", "scalar_ill")),
               match(rows$parameter, c("conc", "auc", "aumc", names(scn))))
  rows <- rows[ord, ]
  base <- paste0("model_pk3_", gsub("[^A-Za-z0-9_]", "_", name))
  if (length(ill) > 0L) assign(base, TRUE, envir = ILLREG)
  write_lines_lf(c("subject,parameter,value", sprintf("%s,%s,%s", rows$subject, rows$parameter, num17(rows$value))),
                 file.path(exp_dir, paste0(base, ".csv")))
  if (is.null(textbook_ok)) textbook_ok <- textbook_flag(q, extra)
  textbook_ok <- textbook_ok && length(ill) == 0L
  kap_kept <- kap[!(names(kap) %in% ill)]
  meta <- list(
    schema = 1L, kind = if (conditioning) "model_conditioning" else "model", case = base,
    generated_by = "oracle/scripts/models_3c_closed_form.R",
    model = model, parameterisation = set, dose = dose,
    parameters = c(p, extra),
    times = arr(times),
    units = list(dose = "mg", time = "h", concentration = "mg/L"),
    note = note,
    textbook_double_ok = textbook_ok,
    quantities = list(
      conc = "concentration in the central compartment (apparent for extravascular input), 0 before the dose and before the lag; the bolus at t = 0 is D / vc",
      auc = "area under the concentration curve from the dose time to the time",
      aumc = "area under the first moment curve from the dose time to the time",
      scalar = "alpha > beta > gamma; k10, k12, k21, k13, k31; cl, vc, q2, vp2, q3, vp3; a, b, c (INTRAVENOUS coefficients D*w/vc for every route); w_alpha, w_beta, w_gamma; half_life = ln2/gamma, half_life_alpha, half_life_beta, half_life_gamma; vss, vz = cl/gamma, v_extrap = vc/w_gamma; auc_inf = D/cl; aumc_inf; mrt_system = vss/cl; mrt = mrt_system + m; c0 (bolus); tmax_pred, cmax_pred (not the bolus; absent for a zero dose with first-order input); k21_ge_k31 (1 when k21 >= k31)",
      scalar_ill = "the scalars whose condition number (conditions) exceeds 1e3 with respect to the parameters of the case: written, with their condition, but outside the group the engine is held to at MODEL_VALUES; the decision is the orchestrator's"
    ),
    conditions = as.list(kap),
    scalar_ill_names = arr(ill),
    method = list(
      closed_form = "independent implementation of the explicit sums of exponentials (MOD-3C-10 to 14), 256-bit arithmetic (Rmpfr); the exponents are the roots of the cubic found by Newton inside the brackets given by the critical points of the cubic, the weights the product-of-differences form (checked against the sum-of-positive-terms form), AUC and AUMC exact integrals of the terms; the confluent limit is used when |ka - lambda| is below 2^-150",
      cross_checks = list("matrix exponential (expm) of the 7-state system on intervals of constant input", "adaptive ODE solver (deSolve bdf with an analytic Jacobian, rtol 1e-13, atol 1e-18 of the dose, any solver warning or early return is an error) with AUC and first-moment states", "eigenvalues of the 3 x 3 system (alpha always; beta and gamma where the solver's absolute error, eig_abs_tol, is below the tolerance), and for beta and gamma, when they are apart, the stable identities alpha beta gamma = k10 k21 k31 and the sum of products of pairs (no subtraction of like numbers); a 256-bit scaling-and-squaring matrix exponential for the times that neither double precision check covers"),
      cross_check_tolerance = 1e-8,
      cross_check_floor = "agreement is relative (1e-8) down to a floor times the largest value of the quantity in the case. The search order is: all times first, then (if that fails) only the times with ||A|| t <= 1e8; for each of the two the floors in increasing order (expm: 1e-9, 1e-8, 1e-7, 1e-6; ODE: 1e-9 up to 1e-3); the first that holds is used and written in cross_check_results (floor, number of times checked, limit). ODE to infinity (AUC, AUMC, MRT): 1e-8, or 1e-7 when times were dropped (ode_inf_tol, with the measured ode_inf_gap). Times covered by neither: expm256_times_checked, held to 1e-20",
      cross_check_tolerance_to_infinity = "see ode_inf_tol of cross_check_results",
      kappa_max = KAPPA_MAX
    ),
    cross_check_results = if (is.null(GAPS[[name]])) list(none = "dose 0 or no time: nothing to integrate") else GAPS[[name]],
    versions = versions,
    n_values = nrow(rows),
    quantities_per_time = 3L,
    n_times = length(times)
  )
  write_lines_lf(jenc(meta), file.path(exp_dir, paste0(base, ".options.json")))
  register(base, if (conditioning) "conditioning" else "values")
  cat(sprintf("%-52s %3d times %4d values %d ill\n", base, length(times), nrow(rows), length(ill)))
  nrow(rows)
}
