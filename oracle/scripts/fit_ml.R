#!/usr/bin/env Rscript
# Caladrius maximum-likelihood fitting oracle (task T-148, brick 2).
#
# Reference fits of one-compartment models by Gaussian maximum likelihood with an estimated
# error model (specs/fit.md section 13, FIT-ML-01 to FIT-ML-21; open items OF-12 to OF-18):
#   additive      sigma = a
#   proportional  sigma = b |yhat|
#   combined      sigma^2 = a^2 + b^2 yhat^2      (variance sum, the default combined form)
#   combined_sd   sigma = a + b |yhat|            (standard-deviation sum)
# The objective is -2LL = sum( ln(2 pi sigma^2) + r^2 / sigma^2 ), constant kept (FIT-ML-04).
#
# How a reference value is obtained (independent of the scoring iteration of FIT-ML-06/07 that the
# engine uses; nothing here is a port of it):
#   1. stats::optim (L-BFGS-B, then Nelder-Mead on the best point) on -2LL written directly, with an
#      analytic gradient derived from the formula above, from a fixed set of starts (a base start
#      and seeded random perturbations of it). The lowest value is kept; the number of starts that
#      reach it, and the other optima seen, are recorded.
#   2. The best point is polished in Rmpfr at 256 bits: Newton's method on the analytic gradient with
#      a Hessian by central differences of the gradient (step 1e-25), with the active-set rule for a
#      variable on its bound (error variables >= 0, a parameter with a user bound) and the KKT test
#      (a variable on its bound stays there only if the gradient points inward). The estimates,
#      -2LL, AIC_ML, BIC_ML, the expected information (sum of u u' + 2 l l', FIT-ML-06), the
#      observed information (half the Hessian of -2LL) and the IWRES are computed at 256 bits from
#      the polished point and rounded to 15 significant digits when written.
#   3. Gradient check: at the polished point and at the 15-digit rounded point, the largest
#      |g_j| / sum_i |term_ij| over the free variables, with the analytic gradient evaluated at 256
#      bits, and the largest relative difference between the analytic gradient and the 256-bit
#      central differences of -2LL (which checks the derivation itself). Written to the options file.
#   4. nlme::gnls, informational only (OF-14): its own -2LL and estimates are written beside the
#      reference, never as a pass/fail value.
# Moved variables as in FIT-ML-05: A = a^2 (additive), B = b^2 (proportional), (A, B) (combined),
# (a, b) (combined_sd), all bounded at 0. The estimates are written in the user's units a, b.
#
# Data: the F data of specs/fit.md section 11 (data/fit/spec.csv), seeded synthetic profiles
# generated here with the generator of example ML4 (written to oracle/data/fit/ml/), Theoph and
# Indometh (all subjects). Profiles: iv_s1 (the profile of ML4), iv_s2, iv_s3, iv_s4, oral_s1 ...
# oral_s4: set.seed(1) ... set.seed(4), true error (a, b) = (0.05, 0.10) variance sum, (0.20, 0),
# (0, 0.10) and (0.05, 0.10) standard-deviation sum.
#
# Usage, from the repository root:
#   Rscript oracle/scripts/fit_ml.R
# Writes (deterministically, no timestamps):
#   oracle/data/fit/ml/<profile>.csv                       generated profiles (15 significant digits)
#   oracle/expected/fit/ml/<case>.csv                      long table: subject,parameter,value
#   oracle/expected/fit/ml/<case>.options.json             what was fitted, how, status, checks, versions
#   oracle/expected/fit/ml/<case>.gnls.csv                 informational gnls values (never a reference)
#   oracle/expected/fit/ml/errors.json                     refusal cases (ML6, OF-18)

suppressPackageStartupMessages({
  library(Rmpfr)
  library(jsonlite)
})

args_all <- commandArgs(trailingOnly = FALSE)
file_arg <- sub("^--file=", "", args_all[grepl("^--file=", args_all)])
root <- if (length(file_arg) == 1L) {
  normalizePath(file.path(dirname(file_arg), "..", ".."), winslash = "/", mustWork = TRUE)
} else {
  normalizePath(".", winslash = "/", mustWork = TRUE)
}
data_dir <- file.path(root, "oracle", "data")
exp_dir <- file.path(root, "oracle", "expected", "fit", "ml")
gen_dir <- file.path(data_dir, "fit", "ml")
dir.create(exp_dir, showWarnings = FALSE, recursive = TRUE)
dir.create(gen_dir, showWarnings = FALSE, recursive = TRUE)

# Optional filter for development: Rscript fit_ml.R only=<regex on the case name>
only_arg <- sub("^only=", "", commandArgs(trailingOnly = TRUE)[grepl("^only=", commandArgs(trailingOnly = TRUE))])
only <- if (length(only_arg) == 1L) only_arg else NULL

PREC <- 256L
STEP <- mpfr("1e-25", PREC) # central-difference step of the Hessian (relative to the scale)
write_lines_lf <- function(lines, path) {
  con <- file(path, open = "wb")
  on.exit(close(con))
  writeLines(lines, con, sep = "\n", useBytes = TRUE)
}
num <- function(x) ifelse(is.na(x), "", sprintf("%.15g", x))
mp <- function(x) mpfr(x, PREC)
is_mp <- function(x) inherits(x, "mpfr")
vec <- function(lst) if (is_mp(lst[[1]])) do.call(c, lst) else unlist(lst)
dbl <- function(x) as.numeric(asNumeric(x))
two_pi <- function(x) if (is_mp(x)) 2 * Const("pi", PREC) else 2 * pi

# ---------------------------------------------------------------- models (specs/models.md)
MODELS <- list(
  "pk1.iv_bolus" = list(
    par = c("v", "k"),
    f = function(t, D, th) D / th$v * exp(-th$k * t),
    grad = function(t, D, th) {
      f <- D / th$v * exp(-th$k * t)
      list(v = -f / th$v, k = -t * f)
    }
  ),
  "pk1.oral_1" = list(
    par = c("v", "k", "ka"),
    f = function(t, D, th) D * th$ka / (th$v * (th$ka - th$k)) * (exp(-th$k * t) - exp(-th$ka * t)),
    grad = function(t, D, th) {
      v <- th$v; k <- th$k; ka <- th$ka
      B <- D * ka / (v * (ka - k))
      f <- B * (exp(-k * t) - exp(-ka * t))
      list(v = -f / v,
           k = -B * t * exp(-k * t) + f / (ka - k),
           ka = B * t * exp(-ka * t) - f * k / (ka * (ka - k)))
    }
  )
)
theta_of <- function(model, z) {
  th <- lapply(seq_along(model$par), function(i) z[i])
  names(th) <- model$par
  th
}

# ---------------------------------------------------------------- error models (FIT-ML-02, 05)
EM <- list(
  additive = list(q = 1L, user = c("error_a"), moved = c("A"), kind = "var"),
  proportional = list(q = 1L, user = c("error_b"), moved = c("B"), kind = "var"),
  combined = list(q = 2L, user = c("error_a", "error_b"), moved = c("A", "B"), kind = "var"),
  combined_sd = list(q = 2L, user = c("error_a", "error_b"), moved = c("a", "b"), kind = "sd")
)
# user value of a moved variable: sqrt for the variance-scale variables
to_user <- function(em, j, x) if (EM[[em]]$kind == "var") sqrt(x) else x
# derivative of the user value with respect to the moved variable (delta method)
d_user <- function(em, j, x) if (EM[[em]]$kind == "var") 1 / (2 * sqrt(x)) else 1 + 0 * x

# One evaluation context: model, error model, data, dose, bounds, scales.
make_ctx <- function(model_id, em, t, y, D, lower_theta = NULL) {
  m <- MODELS[[model_id]]
  P <- length(m$par)
  Q <- EM[[em]]$q
  lo <- c(rep(0, P), rep(0, Q))
  names(lo) <- c(m$par, EM[[em]]$user)
  if (!is.null(lower_theta)) for (nm in names(lower_theta)) lo[[nm]] <- lower_theta[[nm]]
  ysq <- mean(y^2)
  xi_scale <- switch(em, additive = 0.01 * ysq, proportional = 0.01,
                     combined = c(0.01 * ysq, 0.01), combined_sd = c(0.1 * sqrt(ysq), 0.1))
  list(model_id = model_id, model = m, em = em, t = t, y = y, D = D, P = P, Q = Q, n = P + Q,
       lo = lo, xi_scale = xi_scale)
}

# Pieces of the likelihood at z = (theta, moved error variables); generic in double and Rmpfr.
parts <- function(ctx, z) {
  P <- ctx$P
  th <- theta_of(ctx$model, z)
  xi <- z[(P + 1):ctx$n]
  yh <- ctx$model$f(ctx$t, ctx$D, th)
  dy <- ctx$model$grad(ctx$t, ctx$D, th)
  r <- ctx$y - yh
  one <- 0 * yh + 1
  if (ctx$em == "additive") {
    v <- xi[1] * one; dvy <- 0 * yh; dvx <- list(one)
  } else if (ctx$em == "proportional") {
    v <- xi[1] * yh^2; dvy <- 2 * xi[1] * yh; dvx <- list(yh^2)
  } else if (ctx$em == "combined") {
    v <- xi[1] + xi[2] * yh^2; dvy <- 2 * xi[2] * yh; dvx <- list(one, yh^2)
  } else {
    ay <- abs(yh); s <- xi[1] + xi[2] * ay
    v <- s^2; dvy <- 2 * s * xi[2] * sign(yh); dvx <- list(2 * s * one, 2 * s * ay)
  }
  list(yh = yh, dy = dy, r = r, v = v, dvy = dvy, dvx = dvx)
}
m2ll_of <- function(ctx, z, p = parts(ctx, z)) sum(log(two_pi(p$v) * p$v) + p$r^2 / p$v)
# Analytic gradient of -2LL: d/dz_j sum( ln v + r^2/v ) = sum( dv_j/v (1 - r^2/v) - 2 r/v dyh_j )
# with dv_j = dvy dyh_j for a model parameter and dv_j = dvx_j for an error variable.
grad_of <- function(ctx, z, p = parts(ctx, z)) {
  w <- (1 - p$r^2 / p$v) / p$v
  g <- vector("list", ctx$n)
  for (j in seq_len(ctx$P)) g[[j]] <- sum(w * p$dvy * p$dy[[j]] - 2 * p$r / p$v * p$dy[[j]])
  for (j in seq_len(ctx$Q)) g[[ctx$P + j]] <- sum(w * p$dvx[[j]])
  vec(g)
}
# Per-observation terms of the gradient, for the relative size of |g_j| / sum |term|.
grad_abs_terms <- function(ctx, z, p = parts(ctx, z)) {
  w <- (1 - p$r^2 / p$v) / p$v
  s <- vector("list", ctx$n)
  for (j in seq_len(ctx$P)) s[[j]] <- sum(abs(w * p$dvy * p$dy[[j]]) + abs(2 * p$r / p$v * p$dy[[j]]))
  for (j in seq_len(ctx$Q)) s[[ctx$P + j]] <- sum(abs(w * p$dvx[[j]]))
  vec(s)
}
# Expected information of the log-likelihood (FIT-ML-06): sum( u u' + 2 l l' ) with
# u_j = dyh_j / sigma (model parameters only) and l_j = d ln sigma / d z_j = dv_j / (2 v).
info_exp <- function(ctx, z, p = parts(ctx, z)) {
  n <- ctx$n; P <- ctx$P
  u <- vector("list", n); l <- vector("list", n)
  for (j in seq_len(P)) {
    u[[j]] <- p$dy[[j]] / sqrt(p$v)
    l[[j]] <- p$dvy * p$dy[[j]] / (2 * p$v)
  }
  for (j in seq_len(ctx$Q)) {
    u[[P + j]] <- 0 * p$v
    l[[P + j]] <- p$dvx[[j]] / (2 * p$v)
  }
  M <- matrix(vector("list", n * n), n, n)
  for (i in seq_len(n)) for (j in i:n) {
    val <- sum(u[[i]] * u[[j]] + 2 * l[[i]] * l[[j]])
    M[[i, j]] <- val; M[[j, i]] <- val
  }
  M
}
# Hessian of -2LL by central differences of the analytic gradient at 256 bits, columns idx only.
hess_of <- function(ctx, z, idx) {
  n <- ctx$n
  scale <- c(rep(1, ctx$P), ctx$xi_scale)
  H <- matrix(vector("list", n * n), n, n)
  for (i in seq_len(n)) for (j in seq_len(n)) H[[i, j]] <- mp(0)
  for (j in idx) {
    h <- STEP * max(abs(z[j]), mp(scale[j]))
    zp <- z; zm <- z
    zp[j] <- z[j] + h; zm[j] <- z[j] - h
    col <- (grad_of(ctx, zp) - grad_of(ctx, zm)) / (2 * h)
    for (i in seq_len(n)) H[[i, j]] <- col[i]
  }
  for (i in idx) for (j in idx) if (i < j) {
    s <- (H[[i, j]] + H[[j, i]]) / 2
    H[[i, j]] <- s; H[[j, i]] <- s
  }
  H
}
# ---------------------------------------------------------------- small mpfr linear algebra
sub_mat <- function(M, idx) {
  k <- length(idx)
  S <- matrix(vector("list", k * k), k, k)
  for (i in seq_len(k)) for (j in seq_len(k)) S[[i, j]] <- M[[idx[i], idx[j]]]
  S
}
scale_mat <- function(M, f) { for (i in seq_len(nrow(M))) for (j in seq_len(ncol(M))) M[[i, j]] <- M[[i, j]] * f; M }
# Gauss-Jordan inverse with partial pivoting; NULL if a pivot is zero.
mat_inv <- function(M) {
  k <- nrow(M)
  A <- M
  B <- matrix(vector("list", k * k), k, k)
  for (i in seq_len(k)) for (j in seq_len(k)) B[[i, j]] <- mp(if (i == j) 1 else 0)
  for (c in seq_len(k)) {
    piv <- c
    for (r in c:k) if (abs(A[[r, c]]) > abs(A[[piv, c]])) piv <- r
    if (A[[piv, c]] == 0) return(NULL)
    if (piv != c) for (j in seq_len(k)) {
      tmp <- A[[c, j]]; A[[c, j]] <- A[[piv, j]]; A[[piv, j]] <- tmp
      tmp <- B[[c, j]]; B[[c, j]] <- B[[piv, j]]; B[[piv, j]] <- tmp
    }
    d <- A[[c, c]]
    for (j in seq_len(k)) { A[[c, j]] <- A[[c, j]] / d; B[[c, j]] <- B[[c, j]] / d }
    for (r in seq_len(k)) if (r != c) {
      f <- A[[r, c]]
      if (f != 0) for (j in seq_len(k)) {
        A[[r, j]] <- A[[r, j]] - f * A[[c, j]]
        B[[r, j]] <- B[[r, j]] - f * B[[c, j]]
      }
    }
  }
  B
}
mat_vec <- function(M, v) {
  k <- nrow(M)
  vec(lapply(seq_len(k), function(i) sum(vec(lapply(seq_len(k), function(j) M[[i, j]] * v[j])))))
}
# Cholesky pivots all positive: the matrix is positive definite.
is_pd <- function(M) {
  k <- nrow(M)
  L <- matrix(vector("list", k * k), k, k)
  for (i in seq_len(k)) for (j in seq_len(k)) L[[i, j]] <- mp(0)
  for (i in seq_len(k)) {
    for (j in seq_len(i)) {
      s <- M[[i, j]]
      if (j > 1) for (q in seq_len(j - 1)) s <- s - L[[i, q]] * L[[j, q]]
      if (i == j) {
        if (!(s > 0)) return(FALSE)
        L[[i, j]] <- sqrt(s)
      } else L[[i, j]] <- s / L[[j, j]]
    }
  }
  TRUE
}
mat_dbl <- function(M) matrix(vapply(M, function(x) dbl(x), 0), nrow(M), ncol(M))

# ---------------------------------------------------------------- Rmpfr Newton with active set
feasible <- function(ctx, z) {
  p <- tryCatch(parts(ctx, z), error = function(e) NULL, warning = function(w) NULL)
  if (is.null(p)) return(FALSE)
  all(is.finite(p$yh)) && all(is.finite(p$v)) && all(p$v > 0)
}
polish <- function(ctx, z_dbl, tol_active = 1e-6) {
  n <- ctx$n; P <- ctx$P
  scale <- c(rep(1, P), ctx$xi_scale)
  z <- mp(z_dbl)
  active <- rep(FALSE, n)
  for (j in seq_len(n)) {
    near <- if (j > P) z_dbl[j] - ctx$lo[j] <= tol_active * scale[j] else
      (ctx$lo[j] > 0 && z_dbl[j] - ctx$lo[j] <= 1e-7 * ctx$lo[j])
    if (near) { active[j] <- TRUE; z[j] <- mp(ctx$lo[j]) }
  }
  total_it <- 0L
  for (outer in 1:12) {
    converged <- FALSE
    for (it in 1:150) {
      total_it <- total_it + 1L
      free <- which(!active)
      p <- parts(ctx, z)
      g <- grad_of(ctx, z, p)
      f0 <- m2ll_of(ctx, z, p)
      H <- hess_of(ctx, z, free)
      Hf <- sub_mat(H, free)
      if (!is_pd(Hf)) Hf <- scale_mat(sub_mat(info_exp(ctx, z, p), free), 2) # Fisher scoring fallback
      Hi <- mat_inv(Hf)
      if (is.null(Hi)) stop("singular Hessian in the polish")
      d <- mat_vec(Hi, g[free])
      tau <- mp(1)
      # largest step that keeps the free variables above their lower bounds
      newly <- integer(0)
      for (q in seq_along(free)) {
        j <- free[q]
        if (z[j] - d[q] < ctx$lo[j]) {
          if (j <= P) { tau <- min(tau, (z[j] - ctx$lo[j]) / d[q] * 0.5) } else {
            tau <- min(tau, (z[j] - ctx$lo[j]) / d[q]); newly <- c(newly, j)
          }
        }
      }
      accepted <- FALSE
      for (h in 0:60) {
        zn <- z
        zn[free] <- z[free] - tau * d
        if (length(newly) > 0 && h == 0) for (j in newly) zn[j] <- mp(ctx$lo[j])
        if (feasible(ctx, zn)) {
          fn <- m2ll_of(ctx, zn)
          if (fn <= f0 + abs(f0) * mp("1e-60")) { accepted <- TRUE; break }
        }
        tau <- tau / 2
        newly <- integer(0)
      }
      if (!accepted) break
      rel <- max(abs(tau * d) / pmax(abs(z[free]), mp(scale[free])))
      z <- zn
      if (length(newly) > 0) {
        for (j in newly) if (z[j] <= ctx$lo[j]) { active[j] <- TRUE; z[j] <- mp(ctx$lo[j]) }
        next
      }
      if (rel < mp("1e-45") && tau == 1) { converged <- TRUE; break }
    }
    if (!converged) stop("Newton polish did not converge")
    # KKT: a variable held on its lower bound needs a gradient pointing inward (>= 0)
    g <- grad_of(ctx, z)
    release <- which(active & g < 0)
    if (length(release) == 0) break
    active[release[which.min(dbl(g[release]) / scale[release])]] <- FALSE
  }
  list(z = z, active = active, iterations = total_it)
}

# ---------------------------------------------------------------- double precision search
# u-variables: log(theta) for the model parameters (log(lower) shift if bounded), xi / scale >= 0.
fit_double <- function(ctx, starts) {
  P <- ctx$P; n <- ctx$n
  scale_xi <- ctx$xi_scale
  lo_theta <- ctx$lo[seq_len(P)]
  to_z <- function(u) c(lo_theta + exp(u[seq_len(P)]), scale_xi * u[(P + 1):n])
  obj <- function(u) {
    z <- to_z(u)
    val <- tryCatch(m2ll_of(ctx, z), error = function(e) NA_real_, warning = function(w) NA_real_)
    if (!is.finite(val)) 1e300 else val
  }
  gr <- function(u) {
    z <- to_z(u)
    g <- tryCatch(grad_of(ctx, z), error = function(e) rep(NA_real_, n), warning = function(w) rep(NA_real_, n))
    if (any(!is.finite(g))) return(rep(0, n))
    c(g[seq_len(P)] * exp(u[seq_len(P)]), g[(P + 1):n] * scale_xi)
  }
  lower <- c(rep(-Inf, P), rep(0, n - P))
  out <- list()
  for (s in seq_along(starts)) {
    u0 <- starts[[s]]
    r <- tryCatch(optim(u0, obj, gr, method = "L-BFGS-B", lower = lower,
                        control = list(maxit = 5000, factr = 1, pgtol = 0)),
                  error = function(e) NULL)
    if (is.null(r) || !is.finite(r$value) || r$value >= 1e299) next
    # Nelder-Mead polish of the best point of this start (reltol 1e-15), kept only if lower
    r2 <- tryCatch(optim(r$par, obj, method = "Nelder-Mead",
                         control = list(maxit = 4000, reltol = 1e-15)), error = function(e) NULL)
    if (!is.null(r2) && is.finite(r2$value) && r2$value < r$value && all(r2$par[(P + 1):n] >= 0)) r <- r2
    out[[length(out) + 1L]] <- list(u = r$par, z = to_z(r$par), value = r$value)
  }
  out
}

# ---------------------------------------------------------------- starts
theta_center <- function(model_id, t, y, D) {
  pos <- y > 0
  tt <- t[pos]; yy <- y[pos]
  tail_n <- max(3L, floor(length(tt) / 2))
  idx <- tail(seq_along(tt), tail_n)
  cf <- coef(lm(log(yy[idx]) ~ tt[idx]))
  k0 <- max(-cf[[2]], 0.01)
  if (model_id == "pk1.iv_bolus") {
    c(v = D / exp(cf[[1]]), k = k0)
  } else {
    c(v = D / (3 * max(y)), k = k0, ka = max(3 * k0, 1))
  }
}
make_starts <- function(ctx, seed) {
  set.seed(seed)
  P <- ctx$P; n <- ctx$n
  th <- theta_center(ctx$model_id, ctx$t, ctx$y, ctx$D)
  lo_theta <- ctx$lo[seq_len(P)]
  base <- log(pmax(th - lo_theta, 1e-3 * pmax(th, 1e-3)))
  starts <- list()
  starts[[1]] <- c(base, rep(1, n - P))
  starts[[2]] <- c(base, rep(0.1, n - P))
  starts[[3]] <- c(base, rep(10, n - P))
  for (s in 1:24) {
    ub <- base + rnorm(P, 0, 0.4)
    ux <- exp(rnorm(n - P, 0, 1.5))
    if (s %% 8 == 0 && n - P == 2) ux[sample(2, 1)] <- 0
    starts[[length(starts) + 1L]] <- c(ub, ux)
  }
  if (ctx$model_id == "pk1.oral_1") {
    for (m in c(0.5, 2, 6)) {
      b2 <- base; b2[3] <- log(max(th[["k"]] * m, 0.05) - lo_theta[3] + 1e-9)
      starts[[length(starts) + 1L]] <- c(b2, rep(1, n - P))
    }
  }
  list(starts = starts, base_z = c(lo_theta + exp(base), ctx$xi_scale))
}

# ---------------------------------------------------------------- one reference fit
flip_flop <- function(ctx, z) {
  if (ctx$model_id == "pk1.oral_1" && z[2] > z[3]) {
    v <- z[1] * z[2] / z[3]; k <- z[3]; ka <- z[2]
    z[1] <- v; z[2] <- k; z[3] <- ka
  }
  z
}
reference_fit <- function(ctx, seed) {
  st <- make_starts(ctx, seed)
  runs <- fit_double(ctx, st$starts)
  if (length(runs) == 0) stop("no start converged")
  vals <- vapply(runs, function(r) r$value, 0)
  best <- runs[[which.min(vals)]]
  best$z <- dbl(flip_flop(ctx, mp(best$z)))
  pol <- polish(ctx, best$z)
  zf <- flip_flop(ctx, pol$z)
  p <- parts(ctx, zf)
  n <- ctx$n; P <- ctx$P
  N <- length(ctx$y)
  m2 <- m2ll_of(ctx, zf, p)
  active <- pol$active
  free <- which(!active)
  Pfree <- sum(!active[seq_len(P)]); Q <- ctx$Q
  # user-unit estimates
  est <- vector("list", n)
  for (j in seq_len(P)) est[[j]] <- zf[j]
  for (j in seq_len(Q)) est[[P + j]] <- if (active[P + j]) mp(0) else to_user(ctx$em, j, zf[P + j])
  est <- vec(est)
  # covariance (moved variables on the free block), expected and observed, then the delta method
  fct <- vec(lapply(seq_len(n), function(j) if (j <= P) mp(1) else if (active[j]) mp(1) else d_user(ctx$em, j - P, zf[j])))
  I <- sub_mat(info_exp(ctx, zf, p), free)
  Cexp <- mat_inv(I)
  Hs <- sub_mat(hess_of(ctx, zf, free), free)
  Cobs <- mat_inv(scale_mat(Hs, mp(1) / 2))
  userize <- function(C) {
    k <- length(free)
    for (i in seq_len(k)) for (j in seq_len(k)) C[[i, j]] <- C[[i, j]] * fct[free[i]] * fct[free[j]]
    C
  }
  stats_of <- function(C) {
    if (is.null(C)) return(NULL)
    C <- userize(C)
    k <- length(free)
    se <- vec(lapply(seq_len(k), function(i) sqrt(C[[i, i]])))
    cor <- matrix(vector("list", k * k), k, k)
    for (i in seq_len(k)) for (j in seq_len(k)) cor[[i, j]] <- C[[i, j]] / (se[i] * se[j])
    list(se = se, cor = cor)
  }
  sexp <- stats_of(Cexp); sobs <- stats_of(Cobs)
  # IWRES
  sigma <- sqrt(p$v); iw <- p$r / sigma
  iwd <- dbl(iw)
  # gradient checks
  gA <- grad_of(ctx, zf, p); tA <- grad_abs_terms(ctx, zf, p)
  rel_polished <- max(dbl(abs(gA[free]) / tA[free]))
  # the written point: user units rounded to 15 significant digits, converted back
  ur <- vapply(seq_len(n), function(j) as.numeric(sprintf("%.15g", dbl(est[j]))), 0)
  zr <- zf
  for (j in seq_len(Q)) zr[P + j] <- if (active[P + j]) mp(0) else if (EM[[ctx$em]]$kind == "var") mp(ur[P + j])^2 else mp(ur[P + j])
  for (j in seq_len(P)) zr[j] <- mp(ur[j])
  gB <- grad_of(ctx, zr); tB <- grad_abs_terms(ctx, zr)
  rel_written <- max(dbl(abs(gB[free]) / tB[free]))
  m2_written <- m2ll_of(ctx, zr)
  # derivation check: analytic gradient against 256-bit central differences of -2LL at the base start
  zb <- mp(st$base_z)
  gb <- grad_of(ctx, zb); tb <- grad_abs_terms(ctx, zb)
  gnum <- vec(lapply(seq_len(n), function(j) {
    h <- mpfr("1e-30", PREC) * max(abs(zb[j]), mp(c(rep(1, P), ctx$xi_scale)[j]))
    zp <- zb; zm <- zb; zp[j] <- zb[j] + h; zm[j] <- zb[j] - h
    (m2ll_of(ctx, zp) - m2ll_of(ctx, zm)) / (2 * h)
  }))
  deriv_err <- max(dbl(abs(gb - gnum) / tb))
  # agreement of the double-precision optimum with the 256-bit polish
  dpol <- abs(dbl(m2) - best$value)
  near6 <- sum(vals <= best$value + 1e-6)
  near3 <- sum(vals <= best$value + 1e-3)
  sv <- sort(vals)
  distinct <- 1L + sum(diff(sv) > 1e-3)
  list(ctx = ctx, N = N, P = P, Pfree = Pfree, Q = Q, active = active, free = free,
       z = zf, est = est, m2 = m2, p = p,
       aic = m2 + 2 * (Pfree + Q), bic = m2 + (Pfree + Q) * log(mp(N)),
       sexp = sexp, sobs = sobs, sigma = sigma, iwres = iw, iwd = iwd,
       rss = sum(p$r^2),
       checks = list(
         starts = length(st$starts), converged_starts = length(runs),
         starts_within_1e6_of_best = near6, starts_within_1e3_of_best = near3,
         distinct_optima_gap_1e3 = distinct, worst_start_minus_best = max(vals) - best$value,
         polish_iterations = pol$iterations,
         m2ll_double_minus_polished = dbl(best$value - m2),
         rel_gradient_at_polished_point = rel_polished,
         rel_gradient_at_written_point = rel_written,
         m2ll_written_minus_polished = dbl(m2_written - m2),
         rel_gradient_vs_central_difference = deriv_err
       ))
}

# ---------------------------------------------------------------- rows of the expected table
rows_of <- function(subject, ref) {
  ctx <- ref$ctx; m <- ctx$model
  names_all <- c(m$par, EM[[ctx$em]]$user)
  n <- ctx$n; free <- ref$free
  out <- list()
  add <- function(name, value) out[[length(out) + 1L]] <<- sprintf("%s,%s,%s", subject, name, num(value))
  add("n", ref$N); add("p", ref$Pfree); add("q", ref$Q)
  add("m2ll", dbl(ref$m2)); add("aic_ml", dbl(ref$aic)); add("bic_ml", dbl(ref$bic))
  add("rss_uniform", dbl(ref$rss))
  for (j in seq_len(n)) add(paste0("estimate.", names_all[j]), dbl(ref$est[j]))
  pos <- function(j) match(j, free)
  for (j in seq_len(n)) {
    ie <- pos(j)
    add(paste0("se.", names_all[j]), if (is.na(ie)) NA_real_ else dbl(ref$sexp$se[ie]))
    add(paste0("cv_percent.", names_all[j]),
        if (is.na(ie) || dbl(ref$est[j]) == 0) NA_real_ else 100 * dbl(ref$sexp$se[ie]) / abs(dbl(ref$est[j])))
    add(paste0("se_obs.", names_all[j]), if (is.na(ie)) NA_real_ else dbl(ref$sobs$se[ie]))
  }
  for (a in seq_len(n - 1L)) for (b in (a + 1L):n) {
    ia <- pos(a); ib <- pos(b)
    if (!is.na(ia) && !is.na(ib)) {
      add(sprintf("correlation.%s.%s", names_all[a], names_all[b]), dbl(ref$sexp$cor[[ia, ib]]))
      add(sprintf("correlation_obs.%s.%s", names_all[a], names_all[b]), dbl(ref$sobs$cor[[ia, ib]]))
    }
  }
  sg <- dbl(ref$sigma)
  for (i in seq_len(ref$N)) {
    add(sprintf("sigma.%d", i), sg[i]); add(sprintf("iwres.%d", i), ref$iwd[i])
  }
  add("iwres_mean", mean(ref$iwd)); add("iwres_sd", sd(ref$iwd))
  add("iwres_sum_sq", sum(ref$iwd^2))
  add("n_iwres_gt2", sum(abs(ref$iwd) > 2)); add("n_iwres_gt3", sum(abs(ref$iwd) > 3))
  unlist(out)
}

# ---------------------------------------------------------------- nlme::gnls (informational, OF-14)
# Never a pass/fail value: gnls aims at the same Gaussian likelihood but its variance-function
# iteration often stops above the minimum or does not converge for the non-additive models.
gnls_info <- function(model_id, em, t, y, D, start) {
  res <- c(converged = 0)
  try_fit <- function() {
    df <- data.frame(t = t, y = y)
    form <- if (model_id == "pk1.iv_bolus") {
      as.formula(sprintf("y ~ %.17g / v * exp(-k * t)", D))
    } else {
      as.formula(sprintf("y ~ %.17g * ka / (v * (ka - k)) * (exp(-k * t) - exp(-ka * t))", D))
    }
    w <- switch(em, additive = NULL,
                proportional = nlme::varPower(form = ~ fitted(.), fixed = 1),
                combined = nlme::varConstProp(form = ~ fitted(.)),
                combined_sd = nlme::varConstPower(form = ~ fitted(.), fixed = list(power = 1)))
    ctl <- if (em == "combined") nlme::gnlsControl(sigma = 1) else nlme::gnlsControl()
    fit <- suppressWarnings(nlme::gnls(form, data = df, start = as.list(start), weights = w, control = ctl))
    vs <- if (is.null(w)) numeric(0) else coef(fit$modelStruct$varStruct, unconstrained = FALSE)
    c(converged = 1, m2ll = -2 * as.numeric(logLik(fit)), coef(fit), sigma = fit$sigma,
      setNames(as.numeric(vs), if (length(vs)) paste0("varstruct.", seq_along(vs)) else character(0)))
  }
  out <- tryCatch(try_fit(), error = function(e) res)
  out
}

# ---------------------------------------------------------------- data
gen_profile <- function(seed, model_id, truth, kind) {
  set.seed(seed)
  if (model_id == "pk1.iv_bolus") {
    t <- c(0.25, 0.5, 1, 2, 3, 4, 6, 8, 12, 16, 24)
    mu <- 10 * exp(-0.2 * t)
  } else {
    t <- c(0.5, 1, 2, 4, 6, 8, 12, 24)
    mu <- 100 * 1 / (10 * (1 - 0.2)) * (exp(-0.2 * t) - exp(-1 * t))
  }
  sd <- if (kind == "var") sqrt(truth[1]^2 + truth[2]^2 * mu^2) else truth[1] + truth[2] * mu
  y <- mu + sd * rnorm(length(t))
  data.frame(subject = 1L, time = t, conc = as.numeric(sprintf("%.15g", y)), dose = 100)
}
write_dataset <- function(df, path) {
  write_lines_lf(c("subject,time,conc,dose",
                   sprintf("%d,%s,%s,%s", df$subject, num(df$time), num(df$conc), num(df$dose))), path)
}
PROFILES <- list(
  iv_s1 = list(seed = 1L, model = "pk1.iv_bolus", truth = c(0.05, 0.10), kind = "var"),
  iv_s2 = list(seed = 2L, model = "pk1.iv_bolus", truth = c(0.20, 0.00), kind = "var"),
  iv_s3 = list(seed = 3L, model = "pk1.iv_bolus", truth = c(0.00, 0.10), kind = "var"),
  iv_s4 = list(seed = 4L, model = "pk1.iv_bolus", truth = c(0.05, 0.10), kind = "sd"),
  oral_s1 = list(seed = 1L, model = "pk1.oral_1", truth = c(0.05, 0.10), kind = "var"),
  oral_s2 = list(seed = 2L, model = "pk1.oral_1", truth = c(0.20, 0.00), kind = "var"),
  oral_s3 = list(seed = 3L, model = "pk1.oral_1", truth = c(0.00, 0.10), kind = "var"),
  oral_s4 = list(seed = 4L, model = "pk1.oral_1", truth = c(0.05, 0.10), kind = "sd")
)
for (nm in names(PROFILES)) {
  pr <- PROFILES[[nm]]
  write_dataset(gen_profile(pr$seed, pr$model, pr$truth, pr$kind), file.path(gen_dir, paste0(nm, ".csv")))
}
spec_df <- read.csv(file.path(data_dir, "fit", "spec.csv"), stringsAsFactors = FALSE)
write_dataset(spec_df[1:4, ], file.path(gen_dir, "f4.csv"))

old_init <- function(file) fromJSON(file.path(root, "oracle", "expected", "fit", file), simplifyVector = FALSE)$initial_estimates
ALL_EM <- names(EM)
dataset_list <- list(
  list(name = "f", file = "oracle/data/fit/spec.csv", model = "pk1.iv_bolus", ems = ALL_EM,
       init = list("1" = c(v = 12, k = 0.15)),
       note = "F data of specs/fit.md section 11: IV bolus, D = 100, N = 5 (examples ML1 to ML3)"),
  list(name = "f4", file = "oracle/data/fit/ml/f4.csv", model = "pk1.iv_bolus", ems = c("additive", "proportional"),
       init = list("1" = c(v = 12, k = 0.15)),
       note = "the first four F observations: N = 4 > 3 so additive and proportional run; combined forms are refused (ML6c)"),
  list(name = "indometh", file = "oracle/data/indometh.csv", model = "pk1.iv_bolus", ems = ALL_EM,
       init = old_init("fit_indometh_uniform.options.json"),
       note = "Indometh, six subjects, one-compartment IV bolus, D = 25 (test constant of F5); example ML5"),
  list(name = "theoph", file = "oracle/data/theoph.csv", model = "pk1.oral_1", ems = ALL_EM,
       init = old_init("fit_theoph_uniform.options.json"),
       note = "Theoph, twelve subjects, first-order absorption; the proportional and combined models are fitted without the sample at time 0, where the prediction is 0 (ML6b; a combined model refuses a subject whose sample at time 0 is 0: cases ml_theoph_t0_combined and ml_theoph_t0_combined_sd, refusals ml6e_*)")
)
for (nm in names(PROFILES)) {
  pr <- PROFILES[[nm]]
  dataset_list[[length(dataset_list) + 1L]] <- list(
    name = nm, file = paste0("oracle/data/fit/ml/", nm, ".csv"), model = pr$model, ems = ALL_EM,
    init = list("1" = if (pr$model == "pk1.iv_bolus") c(v = 12, k = 0.15) else c(v = 12, k = 0.15, ka = 1.5)),
    note = sprintf("seeded synthetic profile, set.seed(%d), true error (a, b) = (%g, %g), %s form (generator of ML4)",
                   pr$seed, pr$truth[1], pr$truth[2], if (pr$kind == "var") "variance-sum" else "standard-deviation-sum"))
}
# Extra cases: a user bound on a model parameter, and an error_init at the collapse.
extra_cases <- list()
for (em in ALL_EM) {
  extra_cases[[length(extra_cases) + 1L]] <- list(
    case = paste0("ml_f_kbound_", em), base = "f", em = em,
    bounds = list(k = list(lower = 0.25)), init = list("1" = c(v = 12, k = 0.3)),
    note = "F data with a lower bound k >= 0.25 above the unconstrained estimate 0.204: k ends on its bound (FIT-BND-02), statistics of the free block")
}
extra_cases[[length(extra_cases) + 1L]] <- list(
  case = "ml_f_combined_start_at_collapse", base = "f", em = "combined",
  error_init = c(1e-9, 0.0325), init = list("1" = c(v = 10.08, k = 0.196)),
  note = "F data, combined: theta started at the proportional optimum and error_init a = 1e-9, so the start already sits at the collapse of ML3")
# The one-parameter fit that the collapsed Theoph combined fits are compared with: the combined cases of
# Theoph fit the rows with time > 0 (the sample at time 0 is a 0 observed where the prediction is 0), so
# the additive model must be fitted to the same rows (ml_theoph_additive uses all 11). The proportional
# case of Theoph already uses time > 0. Checked in R by ml_collapse_check.R.
extra_cases[[length(extra_cases) + 1L]] <- list(
  case = "ml_theoph_gt0_additive", base = "theoph", em = "additive", init = NULL,
  rule = list(use = "time_greater_than", time = 0),
  note = "Theoph, additive error, on the rows with time > 0 (10 per subject): the one-parameter fit that a collapsed combined fit of ml_theoph_combined and ml_theoph_combined_sd is compared with (same rows)")
for (em in c("combined", "combined_sd")) {
  extra_cases[[length(extra_cases) + 1L]] <- list(
    case = paste0("ml_theoph_t0_", em), base = "theoph", em = em, init = NULL,
    note = paste0("Theoph with the sample at time 0, ", em, " error: the subjects whose sample at time 0 is exactly 0 are refused before fitting ",
                  "(code unbounded_likelihood; the cases ml6e_y0_at_t0_", em, "_theoph<subject> of errors.json) and are not in this case; ",
                  "the others (a nonzero observation where the prediction is 0) fit as in any other case"))
}

# ---------------------------------------------------------------- run
status_of <- function(ref) if (any(ref$active)) "at_bound" else "converged"
versions <- list(
  R = R.version.string,
  Rmpfr = as.character(packageVersion("Rmpfr")),
  gmp = as.character(packageVersion("gmp")),
  nlme = as.character(packageVersion("nlme")),
  jsonlite = as.character(packageVersion("jsonlite")),
  rng = paste(RNGkind(), collapse = "/")
)
METHOD <- list(
  reference = "stats::optim L-BFGS-B (analytic gradient) then Nelder-Mead from seeded starts, lowest -2LL kept; Newton polish at 256 bits (Rmpfr) with active set and KKT test",
  precision_bits = PREC,
  hessian_step = "1e-25 times max(|z_j|, scale_j), central differences of the analytic gradient",
  expected_information = "sum_i (u_i u_i' + 2 l_i l_i') on the moved variables, inverse over the free block, delta method to user units (FIT-ML-06, FIT-ML-17)",
  observed_information = "half the Hessian of -2LL at 256 bits on the moved variables, inverse over the free block, delta method to user units",
  flip_flop = "pk1.oral_1: the point with ka > k is reported (V, k, ka) == (V k / ka, ka, k)",
  gnls = "nlme::gnls, informational only (OF-14): see <case>.gnls.csv"
)
FIT_OPTIONS <- list(convergence = 1e-10, criterion = "decrement", max_iterations = 500L,
                    confidence_level = 0.95, information = "expected")

# The observation of exactly 0 where the prediction is 0 for every parameter value: the sample at the
# dose time of an oral dose, under a combined error model.
is_unbounded <- function(model, em, d) {
  model == "pk1.oral_1" && em %in% c("combined", "combined_sd") && any(d$time == 0 & d$conc == 0)
}

run_case <- function(case, ds, em, init, obs_rule, bounds = NULL, error_init = NULL, note = NULL) {
  if (!is.null(only) && !grepl(only, case)) return(invisible(NULL))
  df <- read.csv(file.path(root, ds$file), stringsAsFactors = FALSE)
  subjects <- unique(df$subject)
  lower_theta <- if (is.null(bounds)) NULL else lapply(bounds, function(b) b$lower)
  rows <- character(0); status <- list(); atb <- list(); chk <- list(); gn <- character(0)
  fitted_subjects <- character(0)
  for (s in subjects) {
    d <- df[df$subject == s, ]
    if (obs_rule$use == "time_greater_than") d <- d[d$time > obs_rule$time, ]
    D <- d$dose[1]
    # An oral model predicts 0 at t = 0 whatever theta is. With a combined error model an observation
    # y = 0 there has residual 0 and sigma = a (or sqrt(A)), so -2LL -> -infinity as a -> 0: the
    # likelihood is unbounded. The engine refuses such a subject before fitting (code
    # unbounded_likelihood, decided by the orchestrator for FIT-ML-13c); it has no reference value here
    # and its refusal rows are written below (ml6e_*), from the same rule (is_unbounded).
    if (is_unbounded(ds$model, em, d)) {
      cat(sprintf("%-32s subject %-3s %-12s refused (unbounded likelihood: y = 0 at t = 0)\n", case, as.character(s), em))
      next
    }
    fitted_subjects <- c(fitted_subjects, as.character(s))
    ctx <- make_ctx(ds$model, em, d$time, d$conc, D, lower_theta)
    seed <- 1000L + 10L * as.integer(s) + match(em, ALL_EM)
    ref <- reference_fit(ctx, seed)
    sk <- as.character(s)
    rows <- c(rows, rows_of(sk, ref))
    status[[sk]] <- status_of(ref)
    names_all <- c(ctx$model$par, EM[[em]]$user)
    atb[[sk]] <- I(names_all[ref$active])
    chk[[sk]] <- ref$checks
    g <- gnls_info(ds$model, em, d$time, d$conc, D, unlist(init[[sk]])[ctx$model$par])
    gn <- c(gn, sprintf("%s,%s,%s", sk, gsub("^\\(Intercept\\)$", "intercept", names(g)), num(unname(g))))
    cat(sprintf("%-32s subject %-3s %-12s m2ll %14.8f  %s\n", case, sk, em, dbl(ref$m2), status[[sk]]))
  }
  header <- "subject,parameter,value"
  write_lines_lf(c(header, rows), file.path(exp_dir, paste0(case, ".csv")))
  write_lines_lf(c(header, gn), file.path(exp_dir, paste0(case, ".gnls.csv")))
  init_list <- lapply(init, function(x) as.list(x))
  names(init_list) <- names(init)
  opt <- list(
    schema = 1L, kind = "fit_ml", case = case, generated_by = "oracle/scripts/fit_ml.R",
    dataset = ds$name, data_file = sub("^oracle/", "", ds$file), model = ds$model,
    objective = "ml", error_model = em, weighting = "uniform",
    note = if (is.null(note)) ds$note else note,
    units = list(dose = "mg", time = "h", concentration = "mg/L"),
    observations = if (obs_rule$use == "all") list(use = "all") else list(use = "time_greater_than", time = obs_rule$time),
    initial_estimates = init_list,
    fit_options = FIT_OPTIONS,
    subjects = I(fitted_subjects),
    status = status, at_bound = atb,
    method = METHOD, checks = chk,
    versions = versions,
    n_values = length(rows)
  )
  if (!is.null(bounds)) opt$bounds <- bounds
  if (!is.null(error_init)) opt$error_init <- I(error_init)
  write_lines_lf(toJSON(opt, auto_unbox = TRUE, pretty = TRUE, digits = NA, null = "null"),
                 file.path(exp_dir, paste0(case, ".options.json")))
  invisible(NULL)
}

for (ds in dataset_list) for (em in ds$ems) {
  rule <- if (ds$name == "theoph" && em != "additive") list(use = "time_greater_than", time = 0) else list(use = "all")
  run_case(paste0("ml_", ds$name, "_", em), ds, em, ds$init, rule)
}
for (ex in extra_cases) {
  ds <- Filter(function(d) d$name == ex$base, dataset_list)[[1]]
  run_case(ex$case, ds, ex$em, if (is.null(ex$init)) ds$init else ex$init, if (is.null(ex$rule)) list(use = "all") else ex$rule, bounds = ex$bounds, error_init = ex$error_init, note = ex$note)
}

# ---------------------------------------------------------------- refusals (ML6, OF-18)
f_t <- spec_df$time; f_y <- spec_df$conc
exact_y <- as.numeric(sprintf("%.15g", 100 / 10 * exp(-0.2 * f_t)))
theoph <- read.csv(file.path(data_dir, "theoph.csv"), stringsAsFactors = FALSE)
th1 <- theoph[theoph$subject == 1, ]
mk <- function(case, description, model, dose, time, conc, initial, options, code, words, weighting = "uniform", control = NULL) {
  r <- list(case = case, description = description, model = model, dose = dose, time = time, conc = conc,
       initial = initial, weighting = weighting, options = options,
       error = list(code = code, words = I(words)))
  if (!is.null(control)) r$control_options <- control
  r
}
f_init <- list(v = 12, k = 0.15)
errs <- list()
for (em in ALL_EM) {
  errs[[length(errs) + 1L]] <- mk(paste0("ml6a_exact_fit_", em),
    "ML6a: y = (100/10) exp(-0.2 t) at the F times, the uniform fit has RSS = 0 within rounding",
    "pk1.iv_bolus", 100, I(f_t), I(exact_y), f_init, list(objective = "ml", error_model = em),
    "exact_fit", c("reproduces the observations exactly", "grows without limit"))
}
errs[[length(errs) + 1L]] <- mk("ml6b_sigma_zero_proportional",
  "ML6b: Theoph subject 1 with its sample at time 0 (prediction 0), proportional error",
  "pk1.oral_1", th1$dose[1], I(th1$time), I(th1$conc), old_init("fit_theoph_uniform.options.json")[["1"]],
  list(objective = "ml", error_model = "proportional"),
  "non_positive_sigma", c("proportional", "standard deviation of 0", "time 0"))
for (em in c("combined", "combined_sd")) {
  errs[[length(errs) + 1L]] <- mk(paste0("ml6c_too_few_", em),
    "ML6c: the first four F observations, P + Q = 4 = N",
    "pk1.iv_bolus", 100, I(f_t[1:4]), I(f_y[1:4]), f_init, list(objective = "ml", error_model = em),
    "too_few_observations", c("4 parameters", "from 4 observations"))
}
errs[[length(errs) + 1L]] <- mk("ml6c_too_few_additive",
  "ML6c: the first three F observations with the additive model, P + Q = 3 = N",
  "pk1.iv_bolus", 100, I(f_t[1:3]), I(f_y[1:3]), f_init, list(objective = "ml", error_model = "additive"),
  "too_few_observations", c("3 parameters", "from 3 observations"))
opts_cases <- list(
  list("unknown_error_model", "an error_model that is not offered", list(objective = "ml", error_model = "poisson"), "error_model"),
  list("missing_error_model", "objective ml without an error_model", list(objective = "ml"), "error_model"),
  list("error_init_zero", "error_init with an entry equal to 0", list(objective = "ml", error_model = "combined", error_init = c(0.1, 0)), "error_init", list(objective = "ml", error_model = "combined", error_init = c(0.1, 0.2))),
  list("error_init_negative", "error_init with a negative entry", list(objective = "ml", error_model = "additive", error_init = I(-0.5)), "error_init", list(objective = "ml", error_model = "additive", error_init = I(0.5))),
  list("error_init_wrong_length", "error_init with three entries for a two-parameter model", list(objective = "ml", error_model = "combined", error_init = c(0.1, 0.1, 0.1)), "error_init", list(objective = "ml", error_model = "combined", error_init = c(0.1, 0.1))),
  list("unknown_objective", "an objective that is neither wls nor ml", list(objective = "reml", error_model = "additive"), "objective")
)
for (o in opts_cases) {
  errs[[length(errs) + 1L]] <- mk(paste0("ml6d_", o[[1]]), paste0("ML6 (FIT-ML-14c): ", o[[2]]),
    "pk1.iv_bolus", 100, I(f_t), I(f_y), f_init, o[[3]], "invalid_option", o[[4]],
    control = if (length(o) >= 5L) o[[5]] else NULL)
}
errs[[length(errs) + 1L]] <- mk("ml6d_weighting_with_ml",
  "FIT-ML-01: a weighting other than uniform together with objective ml",
  "pk1.iv_bolus", 100, I(f_t), I(f_y), f_init, list(objective = "ml", error_model = "additive"),
  "invalid_option", c("weighting", "error model"), weighting = "inv_y")
# ML6e (decision on FIT-ML-13c): under a combined error model an observation of exactly 0 where the
# prediction is 0 for every parameter value (the sample at the dose time of an oral dose) makes the
# likelihood unbounded. The subject is refused before fitting, with the code unbounded_likelihood; the
# sentence names the rows (time 0) and says to exclude them or to use the additive model. The control
# runs the additive model on the same observations: it must be accepted and fitted.
for (em in c("combined", "combined_sd")) {
  for (s in sort(unique(theoph$subject))) {
    d <- theoph[theoph$subject == s, ]
    if (!is_unbounded("pk1.oral_1", em, d)) next
    errs[[length(errs) + 1L]] <- mk(sprintf("ml6e_y0_at_t0_%s_theoph%d", em, s),
      sprintf("ML6e: Theoph subject %d, observation 0 at time 0 where the prediction is 0, %s error: the likelihood is unbounded", s, em),
      "pk1.oral_1", d$dose[1], I(d$time), I(d$conc), old_init("fit_theoph_uniform.options.json")[[as.character(s)]],
      list(objective = "ml", error_model = em),
      "unbounded_likelihood", c("time 0", "exclude", "additive"),
      control = list(objective = "ml", error_model = "additive"))
  }
}
if (is.null(only)) {
  write_lines_lf(toJSON(list(schema = 1L, kind = "fit_ml_errors", generated_by = "oracle/scripts/fit_ml.R",
                             versions = versions, cases = errs),
                        auto_unbox = TRUE, pretty = TRUE, digits = NA),
                 file.path(exp_dir, "errors.json"))
}

# ---------------------------------------------------------------- the list of cases for the engine test
# crates/caladrius-fit/tests/oracle_fit_ml.rs includes this file: one group of tests per case and per
# refusal. The names are sorted by bytes, as the loader of the testkit sorts them.
case_names <- sort(c(unlist(lapply(dataset_list, function(d) paste0("ml_", d$name, "_", d$ems))),
                     vapply(extra_cases, function(e) e$case, "")), method = "radix")
refusal_names <- sort(vapply(errs, function(e) e$case, ""), method = "radix")
cases_path <- file.path(root, "crates", "caladrius-fit", "tests", "oracle_fit_ml.cases")
dir.create(dirname(cases_path), recursive = TRUE, showWarnings = FALSE)
quoted <- function(x) sprintf("    \"%s\",", x)
write_lines_lf(c(
  "// Generated by oracle/scripts/fit_ml.R; do not edit by hand.",
  "const ML_CASES: &[&str] = &[", quoted(case_names), "];",
  "const ML_REFUSALS: &[&str] = &[", quoted(refusal_names), "];",
  "ml_case_tests!(", sprintf("    %s,", case_names), ");",
  "ml_refusal_tests!(", sprintf("    %s,", refusal_names), ");"
), cases_path)

# ---------------------------------------------------------------- the collapse property, in R
# Every combined subject with a component on its bound must equal the one-parameter fit on the same
# rows (-2LL, estimates, AIC_ML + 2); the script stops otherwise. Not run with a case filter.
if (is.null(only)) {
  source(file.path(root, "oracle", "scripts", "ml_collapse_check.R"), local = TRUE)
  collapse_check(exp_dir)
}
