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
ABSREG <- new.env() # case names that have scalar_abs rows (adjacent poles, MOD-3C-22)

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

# Zone of a macro input at 256 bits (MOD-3C-08, T-165): the exact conversion of MOD-3C-04 applied to the doubles
# as given. n1 = k21 + k31, n0 = k21 k31, Delta = n1^2 - 4 n0, tau = sqrt(|Delta|) / n1; for Delta >= 0 rho is the
# smallest signed relative offset of a root to its pole (alpha over d_max, gamma under d_min, beta to the nearest
# pole, negative when beta lies outside [d_min, d_max]). Zones, in this order: (0) tau < 2^-20 free; (i) Delta < 0
# or rho < 2^-55 refused; (iii) rho tau < 2^-48 free; (ii) accepted. Needs the domain checks to have passed.
Z_TAU <- 2^-20
Z_RHO <- 2^-55
Z_RHOTAU <- 2^-48
macro_zone <- function(p) {
  P <- lapply(p, mp)
  s <- P$a + P$b + P$c
  w <- c(P$a, P$b, P$c) / s
  l <- c(P$alpha, P$beta, P$gamma)
  S <- sum(l)
  n1 <- sum(w * (S - l))
  n0 <- w[1] * l[2] * l[3] + w[2] * l[1] * l[3] + w[3] * l[1] * l[2]
  disc <- n1 * n1 - 4 * n0
  tau <- sqrt(abs(disc)) / n1
  out <- list(disc = asNumeric(disc), tau = asNumeric(tau), log2tau = asNumeric(log2(tau)),
              rho = NA_real_, rho_alpha = NA_real_, rho_beta = NA_real_, rho_gamma = NA_real_,
              log2rho = NA_real_, log2rho_tau = NA_real_)
  if (disc >= 0) {
    r <- sqrt(disc); dmax <- (n1 + r) / 2; dmin <- n0 / dmax
    ra <- (l[1] - dmax) / dmax; rg <- (dmin - l[3]) / dmin
    rb <- if (l[2] < dmin) -(dmin - l[2]) / dmin else if (l[2] > dmax) -(l[2] - dmax) / dmax
          else min((l[2] - dmin) / dmin, (dmax - l[2]) / dmax)
    rho <- min(ra, rb, rg)
    out$rho <- asNumeric(rho); out$rho_alpha <- asNumeric(ra); out$rho_beta <- asNumeric(rb); out$rho_gamma <- asNumeric(rg)
    out$log2rho <- if (rho > 0) asNumeric(log2(rho)) else -Inf
    out$log2rho_tau <- if (rho > 0 && tau > 0) asNumeric(log2(rho * tau)) else -Inf
    out$rho_tau <- asNumeric(rho * tau)
    out$limit <- c("alpha", "beta", "gamma")[which.min(c(out$rho_alpha, out$rho_beta, out$rho_gamma))]
  }
  out$zone <- if (out$tau < Z_TAU) "0" else if (disc < 0 || !(rho >= Z_RHO)) "i" else if (!(out$rho_tau >= Z_RHOTAU)) "iii" else "ii"
  out
}

# Relative offsets of alpha above max(k21, k31) and of gamma below min(k21, k31), 256 bits (MOD-3C-08):
# the engine must refuse when either rounds onto the pole in doubles. For the macro set (T-165) the offsets are
# those of the exact conversion of the macro doubles, with beta's signed offset as well (macro_zone).
rel_offsets <- function(set, p) {
  if (set == "macro") {
    z <- macro_zone(p)
    return(c(alpha = z$rho_alpha, beta = z$rho_beta, gamma = z$rho_gamma))
  }
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

# ---------------------------------------------------------------- the naive textbook forms in doubles (T-165)
# A port of crates/caladrius-testkit/src/naive3c.rs (trigonometric roots, weights (k21 - l)(k31 - l) / prod(l_j - l),
# the plain sum of exponentials, powi by repeated squaring as in compiler-rt), used to MEASURE textbook_double_ok:
# the flag is true exactly when the naive forms pass at the testkit's tolerance, |v - want| <= 1e-8 |want| +
# 1e-11 max|want| (the maximum over the times of the case, per quantity). The largest ratio of the two sides is
# written into the options (textbook_worst_ratio). The testkit test recomputes it in Rust.
npowi <- function(a, b) { r <- 1; while (TRUE) { if (bitwAnd(b, 1L) == 1L) r <- r * a; b <- b %/% 2L; if (b == 0L) break; a <- a * a }; r }
nfact <- function(n) { r <- 1; if (n >= 1L) for (j in 1:n) r <- r * j; r }
nipow <- function(p, l, x) {
  if (l == 0) return(npowi(x, p + 1L) / (p + 1))
  y <- l * x
  partial <- 0; for (j in 0:p) partial <- partial + npowi(y, j) / nfact(j)
  nfact(p) / npowi(l, p + 1L) * (1 - exp(-y) * partial)
}
naive_micro <- function(set, p) {
  g <- function(n) if (is.null(p[[n]])) NA_real_ else as.numeric(p[[n]])
  if (set == "clearance") { vc <- g("vc"); list(vc = vc, k10 = g("cl") / vc, k12 = g("q2") / vc, k21 = g("q2") / g("vp2"), k13 = g("q3") / vc, k31 = g("q3") / g("vp3")) }
  else if (set == "micro") list(vc = g("vc"), k10 = g("k10"), k12 = g("k12"), k21 = g("k21"), k13 = g("k13"), k31 = g("k31"))
  else NULL
}
naive_roots <- function(m) {
  a2 <- m$k10 + m$k12 + m$k13 + m$k21 + m$k31
  a1 <- m$k10 * (m$k21 + m$k31) + m$k21 * m$k31 + m$k12 * m$k31 + m$k13 * m$k21
  a0 <- m$k10 * m$k21 * m$k31
  pp <- a1 - a2 * a2 / 3
  qq <- -2 * a2 * a2 * a2 / 27 + a2 * a1 / 3 - a0
  radius <- 2 * sqrt(-pp / 3)
  arg <- 3 * qq / (2 * pp) * sqrt(-3 / pp)
  if (is.finite(arg)) arg <- min(1, max(-1, arg))
  phi <- acos(arg) / 3
  sort(radius * cos(phi - 2 * pi * (0:2) / 3) + a2 / 3, decreasing = TRUE)
}
# conc, auc, aumc of one model at t; NULL when the naive code has no such model or set
naive3c <- function(model, set, p, dose, t) {
  m <- naive_micro(set, p)
  if (is.null(m)) return(NULL)
  l <- naive_roots(m)
  w <- c((m$k21 - l[1]) * (m$k31 - l[1]) / ((l[2] - l[1]) * (l[3] - l[1])),
         (m$k21 - l[2]) * (m$k31 - l[2]) / ((l[1] - l[2]) * (l[3] - l[2])),
         (m$k21 - l[3]) * (m$k31 - l[3]) / ((l[1] - l[3]) * (l[2] - l[3])))
  a <- dose * w / m$vc
  tau <- if (is.null(p$tlag)) 0 else as.numeric(p$tlag)
  tm <- function(c, l, p) list(c = c, l = l, p = p)
  bolus <- model == "pk3.iv_bolus"
  segs <- NULL
  if (bolus) segs <- list(list(s0 = 0, len = NA_real_, terms = lapply(1:3, function(i) tm(a[i], l[i], 0L))))
  else if (model %in% c("pk3.iv_infusion", "pk3.oral_0", "pk3.oral_0_lag")) {
    if (is.null(p$dur)) return(NULL)
    T <- as.numeric(p$dur); during <- list(); after <- list()
    for (i in 1:3) {
      x <- a[i] / (l[i] * T)
      during <- c(during, list(tm(x, 0, 0L), tm(-x, l[i], 0L)))
      after <- c(after, list(tm(x * (1 - exp(-l[i] * T)), l[i], 0L)))
    }
    segs <- list(list(s0 = tau, len = T, terms = during), list(s0 = tau + T, len = NA_real_, terms = after))
  } else if (model %in% c("pk3.oral_1", "pk3.oral_1_lag")) {
    if (is.null(p$ka)) return(NULL)
    ka <- as.numeric(p$ka); terms <- list(); sm <- 0
    for (i in 1:3) { cc <- ka * a[i] / (ka - l[i]); sm <- sm + cc; terms <- c(terms, list(tm(cc, l[i], 0L))) }
    terms <- c(terms, list(tm(-sm, ka, 0L)))
    segs <- list(list(s0 = tau, len = NA_real_, terms = terms))
  } else return(NULL)
  conc <- 0
  for (sg in segs) {
    inside <- t >= sg$s0 && (is.na(sg$len) || t <= sg$s0 + sg$len)
    if (inside) {
      u <- t - sg$s0
      conc <- if (!bolus && u == 0) 0 else { acc <- 0; for (x in sg$terms) acc <- acc + x$c * npowi(u, x$p) * exp(-x$l * u); acc }
      break
    }
  }
  auc <- 0; aumc <- 0
  for (sg in segs) {
    if (t <= sg$s0) next
    upper <- if (is.na(sg$len)) t - sg$s0 else min(sg$len, t - sg$s0)
    for (x in sg$terms) {
      auc <- auc + x$c * nipow(x$p, x$l, upper)
      aumc <- aumc + x$c * (sg$s0 * nipow(x$p, x$l, upper) + nipow(x$p + 1L, x$l, upper))
    }
  }
  c(conc, auc, aumc)
}
# largest ratio |v - want| / (1e-8 |want| + 1e-11 max|want|) over the times and the three quantities (Inf: refused or not finite)
naive_worst_ratio <- function(model, set, p, dose, times, conc, auc, aumc) {
  big <- c(max(abs(conc)), max(abs(auc)), max(abs(aumc)))
  worst <- 0
  for (i in seq_along(times)) {
    v <- naive3c(model, set, p, dose, times[i])
    if (is.null(v) || any(!is.finite(v))) return(Inf)
    want <- c(conc[i], auc[i], aumc[i])
    den <- 1e-8 * abs(want) + 1e-11 * big
    d <- abs(v - want)
    r <- ifelse(den > 0, d / den, ifelse(d > 0, Inf, 0))
    worst <- max(worst, r)
  }
  worst
}
# k21 and k31 are adjacent doubles (no double strictly between them, MOD-3C-22)
nextup <- function(x) x + 2^(floor(log2(x)) - 52)
adjacent_poles <- function(q) {
  a <- asNumeric(q$k21); b <- asNumeric(q$k31)
  lo <- min(a, b); hi <- max(a, b)
  lo != hi && nextup(lo) == hi
}

# ---------------------------------------------------------------- one value case
POLE_CLAIMS <- list()   # lc_<name> -> the pole claim of that point, filled by check_pole_claim (models_3c_closed_form.R)
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
  # adjacent poles (MOD-3C-22): w_beta is held to 1e-12 ABSOLUTE (0 is allowed where the exact weight is below it),
  # whatever its condition number: its own group, scalar_abs, outside scalar and scalar_ill
  adjacent <- adjacent_poles(q)
  if (adjacent) groups[names(scn) == "w_beta"] <- "scalar_abs"
  ill_out <- setdiff(ill, if (adjacent) "w_beta" else character(0))
  tkeys <- if (length(times)) paste0("t=", num17(times)) else character(0)
  rows <- data.frame(subject = c(rep(tkeys, 3), groups), parameter = c(rep(c("conc", "auc", "aumc"), each = length(times)), names(scn)),
                     value = c(conc, auc, aumc, unname(scn)), stringsAsFactors = FALSE)
  ord <- order(match(rows$subject, c(tkeys, "scalar", "scalar_ill", "scalar_abs")),
               match(rows$parameter, c("conc", "auc", "aumc", names(scn))))
  rows <- rows[ord, ]
  base <- paste0("model_pk3_", gsub("[^A-Za-z0-9_]", "_", name))
  if (length(ill_out) > 0L) assign(base, TRUE, envir = ILLREG)
  if (adjacent) assign(base, TRUE, envir = ABSREG)
  write_lines_lf(c("subject,parameter,value", sprintf("%s,%s,%s", rows$subject, rows$parameter, num17(rows$value))),
                 file.path(exp_dir, paste0(base, ".csv")))
  if (is.null(textbook_ok)) textbook_ok <- textbook_flag(q, extra)
  textbook_ok <- textbook_ok && length(ill) == 0L
  # T-165: the flag is MEASURED where the testkit's naive code can run (clearance and micro sets, a positive dose,
  # a time grid); the declared rule above stays for the others (the macro set, dose 0, the conditioning cases)
  textbook_basis <- "declared"; worst_ratio <- NA_real_
  if (!conditioning && dose > 0 && set != "macro" && length(times) > 0L) {
    worst_ratio <- suppressWarnings(naive_worst_ratio(model, set, c(p, extra), dose, times, conc, auc, aumc))
    textbook_ok <- is.finite(worst_ratio) && worst_ratio <= 1
    textbook_basis <- "measured"
  }
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
    textbook_basis = textbook_basis,
    textbook_worst_ratio = if (is.finite(worst_ratio)) signif(worst_ratio, 4) else if (is.na(worst_ratio)) NULL else "Infinity",
    quantities = list(
      conc = "concentration in the central compartment (apparent for extravascular input), 0 before the dose and before the lag; the bolus at t = 0 is D / vc",
      auc = "area under the concentration curve from the dose time to the time",
      aumc = "area under the first moment curve from the dose time to the time",
      scalar = "alpha > beta > gamma; k10, k12, k21, k13, k31; cl, vc, q2, vp2, q3, vp3; a, b, c (INTRAVENOUS coefficients D*w/vc for every route); w_alpha, w_beta, w_gamma; half_life = ln2/gamma, half_life_alpha, half_life_beta, half_life_gamma; vss, vz = cl/gamma, v_extrap = vc/w_gamma; auc_inf = D/cl; aumc_inf; mrt_system = vss/cl; mrt = mrt_system + m; c0 (bolus); tmax_pred, cmax_pred (not the bolus; absent for a zero dose with first-order input); k21_ge_k31 (1 when k21 >= k31)",
      scalar_ill = "the scalars whose condition number (conditions) exceeds 1e3 with respect to the parameters of the case: written, with their condition, but outside the group the engine is held to at MODEL_VALUES; the decision is the orchestrator's",
      scalar_abs = "only for adjacent poles (k21, k31 adjacent doubles, MOD-3C-22): w_beta, held to 1e-12 ABSOLUTE (0 is allowed where the exact weight is below 1e-12), whatever its condition number"
    ),
    conditions = as.list(kap),
    scalar_ill_names = arr(ill_out),
    method = list(
      closed_form = "independent implementation of the explicit sums of exponentials (MOD-3C-10 to 14), 256-bit arithmetic (Rmpfr); the exponents are the roots of the cubic found by Newton inside the brackets given by the critical points of the cubic, the weights the product-of-differences form (checked against the sum-of-positive-terms form), AUC and AUMC exact integrals of the terms; the confluent limit is used when |ka - lambda| is below 2^-150",
      cross_checks = list("matrix exponential (expm) of the 7-state system on intervals of constant input", "adaptive ODE solver (deSolve bdf with an analytic Jacobian, rtol 1e-13, atol 1e-18 of the dose, any solver warning or early return is an error) with AUC and first-moment states", "eigenvalues of the 3 x 3 system (alpha always; beta and gamma where the solver's absolute error, eig_abs_tol, is below the tolerance), and for beta and gamma, when they are apart, the stable identities alpha beta gamma = k10 k21 k31 and the sum of products of pairs (no subtraction of like numbers); a 256-bit scaling-and-squaring matrix exponential for the times that neither double precision check covers"),
      cross_check_tolerance_finite_times = 1e-8,
      cross_check_floor = "agreement is relative (1e-8) down to a floor times the largest value of the quantity in the case. The search order is: all times first, then (if that fails) only the times with ||A|| t <= 1e8; for each of the two the floors in increasing order (expm: 1e-9, 1e-8, 1e-7, 1e-6; ODE: 1e-9 up to 1e-3); the first that holds is used and written in cross_check_results (floor, number of times checked, limit). ODE to infinity (AUC, AUMC, MRT): 1e-8, or 1e-7 when times were dropped (ode_inf_tol, with the measured ode_inf_gap). Times covered by neither: expm256_times_checked, held to 1e-20",
      eigenvalue_note = "beta and gamma are pinned by the Vieta identities (the product of the exponents = k10 k21 k31 and the sum of their products in pairs = k10 k21 + k10 k31 + k21 k31 + k12 k31 + k13 k21, both sums of positive terms, with alpha from the eigenvalue solver) wherever eig_abs_tol (1e-14 of the largest matrix entry, the absolute error of a backward-stable eigenvalue solver) is larger than 1e-9 of the value of the exponent, because the eigenvalue solver cannot resolve it there; where the gaps of the exponents allow it the identities are applied to every case, and eig_checks lists the checks that ran",
      cross_check_tolerance_to_infinity = "see ode_inf_tol of cross_check_results",
      kappa_max = KAPPA_MAX
    ),
    cross_check_results = if (is.null(GAPS[[name]])) list(none = "dose 0 or no time: nothing to integrate") else GAPS[[name]],
    versions = versions,
    n_values = nrow(rows),
    quantities_per_time = 3L,
    n_times = length(times)
  )
  # the pole claim of a large-coupling case (T-165): the root, the pole, the side and the distance in half-spacings of the doubles
  claim <- NULL
  for (k in names(POLE_CLAIMS)) if (endsWith(name, k)) claim <- POLE_CLAIMS[[k]]
  if (!is.null(claim)) meta <- c(meta, claim)
  write_lines_lf(jenc(meta), file.path(exp_dir, paste0(base, ".options.json")))
  register(base, if (conditioning) "conditioning" else "values")
  cat(sprintf("%-52s %3d times %4d values %d ill\n", base, length(times), nrow(rows), length(ill)))
  nrow(rows)
}

# ---------------------------------------------------------------- free-zone macro inputs (MOD-3C-08 zones (0) and (iii), T-165)
# A macro input whose zone leaves the outcome to the engine: it may refuse (DegenerateExponents) or accept. When it
# accepts, the quantities that follow from the macro input alone are those of the exact model within the tolerance of
# the model oracle: the concentration, the areas and the moments on a time grid, the exponents, the weights, the
# half-lives, the intravenous coefficients and vc; and the micro constants it reports, fed back as a micro input,
# are accepted. No micro-derived constant, no clearance and no scalar built from them is written (no contract).
FREE_SCALARS <- c("alpha", "beta", "gamma", "w_alpha", "w_beta", "w_gamma", "a", "b", "c", "vc",
                  "half_life", "half_life_alpha", "half_life_beta", "half_life_gamma")
write_free_case <- function(name, p, dose, note) {
  z <- macro_zone(p)
  check(z$zone %in% c("0", "iii"), name, ": a free-zone case is in zone (0) or (iii), not ", z$zone)
  check(z$disc >= 0, name, ": the free cases written here have real poles")
  model <- "pk3.iv_bolus"
  q <- derive_q3("macro", p, dose, list())
  segs <- segments(model, q)
  times <- make_times(model, q, list())
  tm <- mp(times)
  cm <- conc_vec(segs, tm, TRUE); am <- area_vec(segs, tm, 0); mm <- area_vec(segs, tm, 1)
  conc <- sapply(cm, tonum); auc <- sapply(am, tonum); aumc <- sapply(mm, tonum)
  sc <- scalars_of(model, q, segs)
  scn <- sapply(sc[FREE_SCALARS], tonum)
  # the macro-alone quantities by the plain double precision sums of the macro form (a, b, c, alpha, beta, gamma)
  A <- c(p$a, p$b, p$c); L <- c(p$alpha, p$beta, p$gamma)
  big <- c(max(abs(conc)), max(abs(auc)), max(abs(aumc)))
  gaps <- c(0, 0, 0)
  for (i in seq_along(times)) {
    t <- times[i]
    if (t < 0) next
    v <- c(sum(A * exp(-L * t)), sum(A * (1 - exp(-L * t)) / L), sum(A * (1 / L^2 - exp(-L * t) * (t / L + 1 / L^2))))
    gaps <- pmax(gaps, abs(v - c(conc[i], auc[i], aumc[i])) / big)
  }
  check(all(gaps < 1e-10), name, ": the macro form in doubles disagrees with the 256-bit value, gaps ", paste(signif(gaps, 3), collapse = " "))
  tkeys <- paste0("t=", num17(times))
  rows <- data.frame(subject = c(rep(tkeys, 3), rep("scalar", length(scn))),
                     parameter = c(rep(c("conc", "auc", "aumc"), each = length(times)), names(scn)),
                     value = c(conc, auc, aumc, unname(scn)), stringsAsFactors = FALSE)
  base <- paste0("model_pk3_", name)
  write_lines_lf(c("subject,parameter,value", sprintf("%s,%s,%s", rows$subject, rows$parameter, num17(rows$value))),
                 file.path(exp_dir, paste0(base, ".csv")))
  meta <- list(
    schema = 1L, kind = "model_macro_free", case = base,
    generated_by = "oracle/scripts/models_3c_closed_form.R",
    model = model, parameterisation = "macro", dose = dose, parameters = p, times = arr(times),
    units = list(dose = "mg", time = "h", concentration = "mg/L"),
    note = note,
    textbook_double_ok = FALSE,
    zone = list(zone = z$zone, tau = z$tau, log2_tau = signif(z$log2tau, 6), rho = z$rho, log2_rho = signif(z$log2rho, 6),
                rho_tau = z$rho_tau, log2_rho_tau = signif(z$log2rho_tau, 6), limiting_root = z$limit,
                rho_alpha = z$rho_alpha, rho_beta = z$rho_beta, rho_gamma = z$rho_gamma),
    contract = "MOD-3C-08 zones (0) and (iii): the engine either refuses with DegenerateExponents or accepts; when it accepts, conc, auc, aumc at the times and the scalars written here (macro-alone quantities) are held to MODEL_VALUES, and the micro constants it reports (k10, k12, k21, k13, k31, vc) must be accepted when supplied as a micro input to the same engine",
    quantities = list(
      conc = "concentration in the central compartment, 0 before the dose", auc = "AUC(0, t)", aumc = "AUMC(0, t)",
      scalar = paste("the quantities that follow from the macro input alone:", paste(FREE_SCALARS, collapse = ", "))),
    method = list(closed_form = "256-bit arithmetic (Rmpfr) on the exact conversion of the macro doubles (MOD-3C-04); the exponents and weights are the macro input itself",
                  cross_check = "the macro form summed in double precision, agreement 1e-10 of the largest value of each quantity",
                  macro_alone_cross_check_gaps = signif(gaps, 3)),
    versions = versions, n_values = nrow(rows), quantities_per_time = 3L, n_times = length(times)
  )
  write_lines_lf(jenc(meta), file.path(exp_dir, paste0(base, ".options.json")))
  register(base, "free")
  cat(sprintf("%-52s zone %-3s log2 tau %7.2f rho %.3g (free) %3d values\n", base, z$zone, z$log2tau, z$rho, nrow(rows)))
  nrow(rows)
}
