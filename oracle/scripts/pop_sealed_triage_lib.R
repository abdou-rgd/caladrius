# Caladrius population-fit oracle, card T-174: helpers of the triage of the cl bias of the sealed design A.
#
# An independent FOCEI fit in plain R, double precision, written from specs/population.md (POP-MOD-01,
# POP-ERR-01, POP-OBJ-02, POP-INN-01/02, POP-EST-01/03) and from nothing else. No engine code was read.
# Open tools only: base R (stats::nlminb) and, in pop_sealed_triage.R, saemix.
#
#   * the prediction and its derivatives with respect to eta by the complex step (exact to machine
#     precision, no finite difference): G_ijk = d yhat_ij / d eta_ik;
#   * the inner problem POP-INN-01: for each subject the minimiser of
#       h_i(eta) = sum_j [ln s_ij + r_ij^2 / s_ij] + eta' Omega^-1 eta,   s = sigma^2(yhat(eta))   (interaction),
#     by Fisher scoring with the expected information of POP-INN-02 (M = sum_j [1/s + s'^2 / (2 s^2)] G G' + Omega^-1)
#     and a backtracking line search on h_i, to a gradient of h_i of at most `inner_tol` (absolute);
#   * the objective of POP-OBJ-02, convention `full`:
#       OFV_i = n_i ln 2pi + ln|C_i| + (y_i - E_i)' C_i^-1 (y_i - E_i),
#       E_i = yhat_i - G eta_hat,  C_i = G Omega G' + diag(s_i),  s_i at the individual prediction;
#   * the outer fit POP-EST-01: phi = (ln theta_k, ln omega_k, A/A_SCALE, B/B_SCALE), with A = a^2 and B = b^2
#     on the variance scale and the bound 0 handled as a bound (nlminb `lower`), then a projected Newton
#     polish with a finite-difference Hessian, and the evidence that the minimum was reached.

suppressPackageStartupMessages(library(jsonlite))

A_SCALE <- 1e-3   # the outer optimiser moves A / A_SCALE and B / B_SCALE (a linear rescaling, it changes no result)
B_SCALE <- 1e-2

# ------------------------------------------------------------------ structural models (complex-step safe)
# P: named list of parameter vectors (length N, possibly complex); D, t: numeric vectors of length N.
pk_f <- function(model, P, D, t) {
  switch(model,
    `pk1.iv_bolus` = (D / P$v) * exp(-(P$cl / P$v) * t),
    `pk1.oral_1` = { k <- P$cl / P$v; D * P$ka / (P$v * (P$ka - k)) * (exp(-k * t) - exp(-P$ka * t)) },
    `pk2.iv_bolus` = {
      k10 <- P$cl / P$vc; k12 <- P$q / P$vc; k21 <- P$q / P$vp; S <- k10 + k12 + k21
      al <- (S + sqrt((k10 - k21)^2 + k12 * (k12 + 2 * k10 + 2 * k21))) / 2; be <- k10 * k21 / al
      (D / P$vc) * (((al - k21) / (al - be)) * exp(-al * t) + ((k21 - be) / (al - be)) * exp(-be * t))
    },
    stop("pk_f: unknown model ", model))
}

# data: list(model, re (names of the parameters with a random effect, in the order of Omega), id (integer 1..m per
# observation), t, y, D (dose per observation), m)
make_data <- function(model, re, id, t, y, D) {
  m <- max(id)
  list(model = model, re = re, q = length(re), id = id, t = t, y = y, D = D, m = m, N = length(y),
       rows = split(seq_along(id), factor(id, levels = seq_len(m))))
}

# prediction yhat (N) and G (N x q) at eta (m x q) for typical values theta (named numeric)
predict_G <- function(dat, theta, eta, h = 1e-20) {
  q <- dat$q; N <- dat$N; id <- dat$id
  P <- lapply(names(theta), function(nm) {
    k <- match(nm, dat$re)
    if (is.na(k)) rep(theta[[nm]], N) else theta[[nm]] * exp(eta[id, k])
  }); names(P) <- names(theta)
  yhat <- Re(pk_f(dat$model, P, dat$D, dat$t))
  G <- matrix(0, N, q)
  for (k in seq_len(q)) {
    Pk <- P; Pk[[dat$re[k]]] <- Pk[[dat$re[k]]] * exp(1i * h)   # eta_k + i h
    G[, k] <- Im(pk_f(dat$model, Pk, dat$D, dat$t)) / h
  }
  list(yhat = yhat, G = G)
}

# ------------------------------------------------------------------ error model (POP-ERR-01)
# xi = c(A, B) on the variance scale for additive / proportional / combined; c(a, b) for combined_sd.
# Returns the variance s and ds = d s / d yhat.
err_var <- function(err, xi, yhat) {
  A <- xi[1]; B <- xi[2]
  switch(err,
    additive = list(s = rep(A, length(yhat)), ds = numeric(length(yhat))),
    proportional = list(s = B * yhat^2, ds = 2 * B * yhat),
    combined = list(s = A + B * yhat^2, ds = 2 * B * yhat),
    combined_sd = { sd <- A + B * abs(yhat); list(s = sd^2, ds = 2 * sd * B * sign(yhat)) },
    stop("err_var: unknown error model ", err))
}

# ------------------------------------------------------------------ the inner problem (POP-INN-01, -02)
# returns eta_hat (m x q), plus yhat, G, s, per-subject h and the final max |grad h| per subject.
inner_modes <- function(dat, theta, Omega, err, xi, eta0 = NULL, inner_tol = 1e-10, maxit = 300L) {
  q <- dat$q; m <- dat$m; id <- dat$id; y <- dat$y
  Oi <- solve(Omega)
  eta <- if (is.null(eta0)) matrix(0, m, q) else eta0
  evalh <- function(eta) {
    p <- predict_G(dat, theta, eta); v <- err_var(err, xi, p$yhat)
    if (any(!is.finite(v$s)) || any(v$s <= 0)) return(NULL)
    r <- y - p$yhat
    hobs <- log(v$s) + r^2 / v$s
    hsub <- as.numeric(rowsum(hobs, id)) + rowSums((eta %*% Oi) * eta)
    c(p, v, list(r = r, h = hsub))
  }
  cur <- evalh(eta)
  if (is.null(cur)) stop("inner_modes: invalid start")
  iters <- 0L
  for (it in seq_len(maxit)) {
    G <- cur$G; s <- cur$s; ds <- cur$ds; r <- cur$r
    w <- (ds / s - 2 * r / s - r^2 * ds / s^2)
    grad <- rowsum(G * w, id) + 2 * (eta %*% Oi)               # m x q: gradient of h_i
    gmax <- apply(abs(grad), 1, max)
    if (max(gmax) <= inner_tol) break
    wm <- 1 / s + 0.5 * ds^2 / s^2
    step <- matrix(0, m, q)
    for (i in seq_len(m)) {
      if (gmax[i] <= inner_tol) next
      ri <- dat$rows[[i]]; Gi <- G[ri, , drop = FALSE]
      M <- crossprod(Gi * sqrt(wm[ri])) + Oi
      step[i, ] <- solve(M, grad[i, ]) / 2                      # scoring step on h_i: (2M)^-1 grad
    }
    lam <- rep(1, m); todo <- gmax > inner_tol
    new <- cur; neweta <- eta
    for (bt in 1:40) {
      trial <- eta - lam * step
      tr <- tryCatch(evalh(trial), error = function(e) NULL)
      if (is.null(tr)) { lam[todo] <- lam[todo] / 2; next }
      ok <- todo & (tr$h <= cur$h + 1e-12 * abs(cur$h))
      if (any(ok)) neweta[ok, ] <- trial[ok, ]
      todo <- todo & !ok
      if (!any(todo)) break
      lam[todo] <- lam[todo] / 2
    }
    # subjects that could not decrease h keep their eta (they are at the floating-point floor)
    if (!any(neweta != eta)) break   # no subject can decrease h any more: the floating-point floor
    eta <- neweta
    cur <- evalh(eta); iters <- it
  }
  G <- cur$G; s <- cur$s; ds <- cur$ds; r <- cur$r
  w <- (ds / s - 2 * r / s - r^2 * ds / s^2)
  grad <- rowsum(G * w, id) + 2 * (eta %*% Oi)
  c(list(eta = eta, gradmax = apply(abs(grad), 1, max), iterations = iters), cur[c("yhat", "G", "s", "ds", "h")])
}

# ------------------------------------------------------------------ the objective (POP-OBJ-02)
ofv_focei <- function(dat, theta, Omega, err, xi, eta0 = NULL, inner_tol = 1e-10, per_subject = FALSE) {
  mo <- inner_modes(dat, theta, Omega, err, xi, eta0, inner_tol)
  q <- dat$q; id <- dat$id; y <- dat$y
  Geta <- rowSums(mo$G * mo$eta[id, , drop = FALSE])
  E <- mo$yhat - Geta
  per <- numeric(dat$m)
  for (i in seq_len(dat$m)) {
    ri <- dat$rows[[i]]; Gi <- mo$G[ri, , drop = FALSE]
    C <- Gi %*% Omega %*% t(Gi); diag(C) <- diag(C) + mo$s[ri]
    L <- chol(C); w <- backsolve(L, y[ri] - E[ri], transpose = TRUE)
    per[i] <- length(ri) * log(2 * pi) + 2 * sum(log(diag(L))) + sum(w^2)
  }
  out <- list(ofv = sum(per), eta = mo$eta, gradmax = max(mo$gradmax), iterations = mo$iterations)
  if (per_subject) out$per_subject <- per
  out
}

# ------------------------------------------------------------------ the outer vector (POP-EST-01)
# layout: ln theta (all parameters of `theta` with a random effect; others natural), ln omega (diagonal Omega), A/A_SCALE, B/B_SCALE
phi_pack <- function(dat, theta, omega_sd, A, B) {
  th <- vapply(names(theta), function(nm) if (nm %in% dat$re) log(theta[[nm]]) else theta[[nm]], 0)
  c(th, log(omega_sd), A / A_SCALE, B / B_SCALE)
}
phi_unpack <- function(dat, phi, theta_names) {
  nt <- length(theta_names); q <- dat$q
  th <- setNames(vapply(seq_len(nt), function(i) if (theta_names[i] %in% dat$re) exp(phi[i]) else phi[i], 0), theta_names)
  list(theta = as.list(th), omega_sd = exp(phi[nt + seq_len(q)]), A = phi[nt + q + 1L] * A_SCALE, B = phi[nt + q + 2L] * B_SCALE)
}

# objective in phi; a cache keeps the last modes as the start of the next inner solves (the inner problem is
# solved to inner_tol in every call, so the cache changes the result at the 1e-12 level only)
make_objective <- function(dat, theta_names, err, inner_tol = 1e-11) {
  cache <- new.env(); cache$eta <- NULL
  f <- function(phi) {
    u <- phi_unpack(dat, phi, theta_names)
    if (u$A < -0.5 * A_SCALE || u$B < -0.5 * B_SCALE) return(1e30)   # far below the bound: never evaluated by the optimiser
    th <- unlist(u$theta); Omega <- diag(u$omega_sd^2, length(u$omega_sd))
    r <- tryCatch(ofv_focei(dat, th, Omega, err, c(u$A, u$B), eta0 = cache$eta, inner_tol = inner_tol),
                  error = function(e) NULL)
    if (is.null(r) || !is.finite(r$ofv)) return(1e30)
    cache$eta <- r$eta   # the next evaluation starts from the modes of this one
    r$ofv
  }
  list(f = f, cache = cache, set_start = function(eta) cache$eta <- eta)
}

# central-difference gradient with a step proportional to max(|phi_j|, 1) (POP-EST-03); at a lower bound the
# step is still central (the objective is defined for a slightly negative A because s = A + B yhat^2 stays > 0)
fd_grad <- function(f, phi, h = 1e-4) {
  n <- length(phi); g <- numeric(n)
  for (j in seq_len(n)) {
    hj <- h * max(abs(phi[j]), 1); e <- numeric(n); e[j] <- hj
    g[j] <- (f(phi + e) - f(phi - e)) / (2 * hj)
  }
  g
}
# Richardson-extrapolated central difference (step h and 2h) for the final gradient: error O(h^4)
fd_grad_r <- function(f, phi, h = 1e-3) {
  n <- length(phi); g <- numeric(n)
  for (j in seq_len(n)) {
    hj <- h * max(abs(phi[j]), 1); e <- numeric(n); e[j] <- hj
    d1 <- (f(phi + e) - f(phi - e)) / (2 * hj)
    d2 <- (f(phi + 2 * e) - f(phi - 2 * e)) / (4 * hj)
    g[j] <- (4 * d1 - d2) / 3
  }
  g
}
# finite-difference Hessian of f (central second differences, step proportional to max(|phi_j|, 1))
fd_hess <- function(f, phi, h = 2e-3) {
  n <- length(phi); H <- matrix(0, n, n); f0 <- f(phi)
  hs <- h * pmax(abs(phi), 1)
  fp <- fm <- numeric(n)
  for (j in seq_len(n)) { e <- numeric(n); e[j] <- hs[j]; fp[j] <- f(phi + e); fm[j] <- f(phi - e); H[j, j] <- (fp[j] - 2 * f0 + fm[j]) / hs[j]^2 }
  for (j in seq_len(n)) for (k in seq_len(n)) if (k < j) {
    ej <- numeric(n); ej[j] <- hs[j]; ek <- numeric(n); ek[k] <- hs[k]
    H[j, k] <- H[k, j] <- (f(phi + ej + ek) - f(phi + ej - ek) - f(phi - ej + ek) + f(phi - ej - ek)) / (4 * hs[j] * hs[k])
  }
  H
}

# ------------------------------------------------------------------ the outer fit
# start: list(theta (named list), omega_sd (vector), A, B). Variables: theta (no bound), ln omega (no bound), A >= 0, B >= 0.
fit_focei <- function(dat, start, err, inner_tol = 1e-11, polish_iters = 8L, tag = "") {
  theta_names <- names(start$theta); nt <- length(theta_names); q <- dat$q; n <- nt + q + 2L
  phi0 <- phi_pack(dat, unlist(start$theta), start$omega_sd, start$A, start$B)
  obj <- make_objective(dat, theta_names, err, inner_tol)
  f <- obj$f
  lower <- c(rep(-Inf, nt + q), 0, 0)
  t0 <- proc.time()[[3]]
  nevals <- 0L
  fc <- function(p) { nevals <<- nevals + 1L; f(p) }
  gr <- function(p) fd_grad(fc, p)
  op <- tryCatch(nlminb(phi0, fc, gr, lower = lower, control = list(eval.max = 400L, iter.max = 200L, rel.tol = 1e-10, x.tol = 1e-12, abs.tol = 0)),
                 error = function(e) NULL)
  phi <- if (is.null(op)) phi0 else op$par
  phi <- pmax(phi, lower)
  conv_nlminb <- if (is.null(op)) NA_integer_ else op$convergence
  # projected Newton polish on the free variables with a finite-difference Hessian; the evidence (gradient, Newton
  # decrement, smallest eigenvalue, multipliers of the active bounds) is taken at the last point, once
  fcur <- fc(phi); pol <- 0L
  repeat {
    g <- fd_grad_r(fc, phi)
    atb <- is.finite(lower) & (phi <= lower + 1e-7) & (g > 0)    # on a bound, with the multiplier of the right sign
    phi[atb] <- lower[atb]
    onb <- is.finite(lower) & (phi <= lower)                      # on a bound whatever the sign of its multiplier
    free <- !atb
    H <- fd_hess(fc, phi)
    Hf <- H[free, free, drop = FALSE]; gf <- g[free]
    ev <- eigen((Hf + t(Hf)) / 2, symmetric = TRUE, only.values = TRUE)$values
    dec <- if (min(ev) > 0) 0.5 * sum(gf * solve(Hf, gf)) else NA_real_
    if ((!is.na(dec) && dec < 1e-9) || pol >= polish_iters) break
    Hs <- Hf
    if (min(ev) <= 0) Hs <- Hf + diag(abs(min(ev)) + 1e-6 * max(abs(ev)), nrow(Hf))
    step <- numeric(n); step[free] <- -solve(Hs, gf)
    lam <- 1; ok <- FALSE
    for (bt in 1:30) {
      trial <- pmax(phi + lam * step, lower)
      ft <- fc(trial)
      if (is.finite(ft) && ft <= fcur + 1e-14 * abs(fcur)) { ok <- TRUE; break }
      lam <- lam / 2
    }
    if (!ok) break
    phi <- trial; fcur <- ft; pol <- pol + 1L
  }
  mult <- g[atb]                                                   # KKT: the gradient at an active lower bound is >= 0
  u <- phi_unpack(dat, phi, theta_names)
  th <- unlist(u$theta); Omega <- diag(u$omega_sd^2, q)
  fin <- ofv_focei(dat, th, Omega, err, c(u$A, u$B), eta0 = NULL, inner_tol = 1e-12)
  list(theta = th, omega_sd = u$omega_sd, omega = u$omega_sd^2, A = u$A, B = u$B, ofv = fin$ofv,
       ofv_search = fcur, inner_gradmax = fin$gradmax, inner_iterations = fin$iterations,
       grad_free_max = if (length(gf)) max(abs(gf)) else 0, grad_free_norm = sqrt(sum(gf^2)),
       newton_decrement = dec, hess_min_eig = min(ev), hess_cond = max(ev) / min(ev),
       n_on_bound = sum(onb), bound_names = c("A", "B")[onb[(n - 1L):n]], kkt_multiplier_min = if (length(mult)) min(mult) else NA_real_,
       nlminb_convergence = conv_nlminb, polish_steps = pol, evals = nevals, seconds = proc.time()[[3]] - t0)
}

# ------------------------------------------------------------------ data readers
read_sealed_rep <- function(root, name, k) {
  d <- read.csv(file.path(root, "oracle", "data", "pop", "sealed", name, sprintf("rep_%03d.csv", k)), stringsAsFactors = FALSE)
  d
}
sealed_dat <- function(d, model, re) {
  id <- as.integer(factor(d$subject, levels = unique(d$subject)))
  make_data(model, re, id, d$time, d$conc, d$dose)
}

# an oracle case of oracle/expected/pop/ofv/ as a `dat`, with its parameters
load_ofv_dat <- function(root, id) {
  j <- fromJSON(file.path(root, "oracle", "expected", "pop", "ofv", paste0(id, ".json")), simplifyVector = FALSE)
  mdl <- j$model; q <- length(mdl$random_effects)
  Omega <- matrix(0, q, q); for (a in seq_len(q)) for (b in seq_len(q)) Omega[a, b] <- as.numeric(mdl$omega[[a]][[b]])
  sub <- j$subjects
  ids <- rep(seq_along(sub), vapply(sub, function(s) length(s$time), 0L))
  dat <- make_data(mdl$name, unlist(mdl$random_effects), ids, as.numeric(unlist(lapply(sub, function(s) unlist(s$time)))),
                   as.numeric(unlist(lapply(sub, function(s) unlist(s$y)))),
                   rep(vapply(sub, function(s) as.numeric(s$dose), 0), vapply(sub, function(s) length(s$time), 0L)))
  xi <- c(as.numeric(mdl$xi$a %||% 0), as.numeric(mdl$xi$b %||% 0))
  err <- mdl$error_model
  # engine/oracle xi are (a, b) on the standard-deviation scale: A = a^2, B = b^2 except for combined_sd
  xi_v <- if (err == "combined_sd") xi else xi^2
  list(dat = dat, theta = vapply(mdl$theta, as.numeric, 0), Omega = Omega, err = err, xi = xi_v, expected = j$expected, json = j)
}
`%||%` <- function(a, b) if (is.null(a)) b else a
