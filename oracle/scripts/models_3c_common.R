# Caladrius three-compartment model oracle (task T-142), part 0: helpers and the independent forms.
# Sourced by models_3c_closed_form.R; not run on its own.
#
# Independence from the engine (specs/models.md OM-19): the exponents are the roots of the cubic of
# MOD-3C-03 found in 256-bit arithmetic by a bracketed Newton iteration on the cubic itself (the
# brackets are the critical points of the cubic), NOT by the trigonometric seed, the deflation or the
# secular offsets of MOD-3C-07; the weights are the product-of-differences form of MOD-3C-03 (the
# second form, a sum of positive terms, is only used as a check); the reverse conversion is the
# partial-fraction numerator of MOD-3C-04; AUC(0, t) and AUMC(0, t) are the exact integrals of the
# terms c * u^p * exp(-lambda * u) on each stretch of constant input. The concentration is the
# explicit sum of exponentials, with the confluent term u * exp(-lambda * u) when ka equals an
# exponent exactly.

suppressPackageStartupMessages({
  library(Rmpfr)
  library(expm)
  library(deSolve)
})

write_lines_lf <- function(lines, path) {
  con <- file(path, open = "wb")
  on.exit(close(con))
  writeLines(lines, con, sep = "\n", useBytes = TRUE)
}
num17 <- function(x) sprintf("%.17g", x)
rt <- function(x) {   # shortest decimal text that reads back as the same double
  s <- ""
  for (d in 15:17) {
    s <- sprintf(paste0("%.", d, "g"), x)
    if (as.numeric(s) == x) return(s)
  }
  s
}
jstr <- function(s) {
  s <- gsub("\\", "\\\\", s, fixed = TRUE)
  s <- gsub("\"", "\\\"", s, fixed = TRUE)
  paste0("\"", s, "\"")
}
jnum1 <- function(v) {
  if (is.finite(v)) rt(v) else jstr(if (is.nan(v)) "NaN" else if (v > 0) "Infinity" else "-Infinity")
}
jenc <- function(x, ind = 0L) {
  pad <- strrep("  ", ind)
  pad1 <- strrep("  ", ind + 1L)
  if (is.null(x)) return("null")
  if (is.list(x)) {
    if (length(x) == 0L) return(if (is.null(names(x))) "[]" else "{}")
    if (!is.null(names(x))) {
      items <- mapply(function(k, v) paste0(pad1, jstr(k), ": ", jenc(v, ind + 1L)),
                      names(x), x, USE.NAMES = FALSE)
      return(paste0("{\n", paste(items, collapse = ",\n"), "\n", pad, "}"))
    }
    return(paste0("[", paste(vapply(x, jenc, "", ind = ind + 1L), collapse = ", "), "]"))
  }
  if (is.character(x)) {
    if (length(x) == 1L) return(jstr(x))
    return(paste0("[", paste(jstr(x), collapse = ", "), "]"))
  }
  if (is.logical(x)) {
    s <- ifelse(x, "true", "false")
    return(if (length(x) == 1L) s else paste0("[", paste(s, collapse = ", "), "]"))
  }
  if (is.numeric(x)) {
    s <- if (is.integer(x)) as.character(x) else vapply(x, jnum1, "")
    return(if (length(x) == 1L) s else paste0("[", paste(s, collapse = ", "), "]"))
  }
  stop("jenc: unsupported value")
}
arr <- function(x) as.list(unname(x))
`%||%` <- function(a, b) if (is.null(a)) b else a
check <- function(cond, ...) {
  if (!isTRUE(cond)) stop(paste0(..., collapse = ""), call. = FALSE)
}

PREC <- 256L
mp <- function(x) mpfr(x, PREC)
zero <- function() mp(0)
TINY <- mpfr(2, 256L)^-150   # |ka - lambda| below this: the confluent limit of MOD-3C-12
at_prec <- function(prec, expr) {
  old <- PREC
  PREC <<- prec
  on.exit(PREC <<- old)
  expr
}
versions <- list(
  R = R.version.string,
  Rmpfr = as.character(packageVersion("Rmpfr")),
  expm = as.character(packageVersion("expm")),
  deSolve = as.character(packageVersion("deSolve")),
  gmp = as.character(packageVersion("gmp"))
)

# ---------------------------------------------------------------- the three roots
# Roots of P(x) = x^3 - a2 x^2 + a1 x - a0 (three positive simple roots). The critical points c1 < c2
# of P separate them: gamma in (0, c1), beta in (c1, c2), alpha in (c2, a2). Newton inside the
# bracket, bisection whenever a step leaves it.
cubic_roots <- function(a2, a1, a0) {
  # work with 128 extra bits: near a multiple root the noise of P(x) / P'(x) is far above the last bit
  WP <- PREC + 128L
  a2 <- mpfr(a2, WP); a1 <- mpfr(a1, WP); a0 <- mpfr(a0, WP)
  Pf <- function(x) ((x - a2) * x + a1) * x - a0
  dPf <- function(x) (3 * x - 2 * a2) * x + a1
  d <- sqrt(a2 * a2 - 3 * a1)
  c1 <- (a2 - d) / 3
  c2 <- (a2 + d) / 3
  lo <- c(mpfr(0, WP), c1, c2)
  hi <- c(c1, c2, a2)
  sg <- c(1, -1, 1)
  x <- (lo + hi) / 2
  tol <- mpfr(2, WP)^-(PREC + 8L)
  for (it in 1:20000) {
    fx <- Pf(x)
    up <- (fx * sg) < 0
    lo[up] <- x[up]
    hi[!up] <- x[!up]
    xn <- x - fx / dPf(x)
    bad <- !(xn >= lo & xn <= hi)
    if (any(bad)) xn[bad] <- ((lo + hi) / 2)[bad]
    step <- abs(xn - x) / abs(xn)
    x <- xn
    if (all(step < tol)) break
    check(it < 20000L, "cubic_roots: no convergence")
  }
  mpfr(c(x[3], x[2], x[1]), PREC)
}
# Weights (MOD-3C-03, first form) for the poles k21, k31 and the exponents l (alpha, beta, gamma).
weights3 <- function(k21, k31, l) {
  nm <- (k21 - l) * (k31 - l)
  den <- c((l[2] - l[1]) * (l[3] - l[1]), (l[1] - l[2]) * (l[3] - l[2]), (l[1] - l[3]) * (l[2] - l[3]))
  nm / den
}
# Micro-constants to exponents and weights; the exact tie k21 = k31 is the merged two-compartment model.
micro_to_exp <- function(m) {
  if (m$k21 == m$k31) {
    S <- m$k10 + m$k12 + m$k13 + m$k21
    Pr <- m$k10 * m$k21
    r <- sqrt(S * S - 4 * Pr)
    al <- (S + r) / 2
    l <- c(al, m$k21, Pr / al)
  } else {
    a2 <- m$k10 + m$k12 + m$k13 + m$k21 + m$k31
    a1 <- m$k10 * m$k21 + m$k10 * m$k31 + m$k21 * m$k13 + m$k31 * m$k12 + m$k21 * m$k31
    a0 <- m$k10 * m$k21 * m$k31
    l <- cubic_roots(a2, a1, a0)
  }
  if (m$k21 == m$k31) {
    # merged: weights of the two-compartment model with k12 + k13 as exchange rate
    kk <- m$k12 + m$k13
    w1 <- (m$k21 - l[1]) / (l[3] - l[1])   # weight of alpha
    w1 <- (l[1] - m$k21) / (l[1] - l[3]) ; w3 <- 1 - w1
    w <- c(w1, zero(), w3)
    w <- c((m$k21 - l[1]) / (l[3] - l[1]), zero(), (m$k21 - l[3]) / (l[1] - l[3]))
  } else {
    w <- weights3(m$k21, m$k31, l)
    w2 <- 1 / (1 + m$k12 * m$k21 / (l - m$k21)^2 + m$k13 * m$k31 / (l - m$k31)^2)
    check(all(abs(w - w2) <= mpfr(2, PREC)^-(PREC - 120L) * w2), "the two weight forms differ")
  }
  check(abs(sum(w) - 1) < mpfr(2, PREC)^-(PREC - 40L), "weights do not sum to 1")
  list(l = l, w = w)
}
# Exponents and weights to micro-constants (MOD-3C-04, from the partial fractions).
exp_to_micro <- function(l, w, V) {
  S <- sum(l)
  n1 <- sum(w * (S - l))
  n0 <- w[1] * l[2] * l[3] + w[2] * l[1] * l[3] + w[3] * l[1] * l[2]
  disc <- n1 * n1 - 4 * n0
  check(disc > 0, "reverse conversion: k21 = k31 or non-real")
  r <- sqrt(disc)
  k21 <- (n1 + r) / 2      # the larger one is compartment 2 (MOD-3C-05)
  k31 <- n0 / k21
  k10 <- l[1] * l[2] * l[3] / (k21 * k31)
  Pp <- function(x) ((x - S) * x + (l[1] * l[2] + l[1] * l[3] + l[2] * l[3])) * x - l[1] * l[2] * l[3]
  k12 <- -Pp(k21) / ((k21 - k31) * k21)
  k13 <- -Pp(k31) / ((k31 - k21) * k31)
  check(k10 > 0 && k12 > 0 && k13 > 0 && k31 > 0, "reverse conversion: a non-positive constant")
  list(vc = V, k10 = k10, k12 = k12, k21 = k21, k13 = k13, k31 = k31)
}

# ---------------------------------------------------------------- parameter sets (256 bits)
#   clearance: cl, vc, q2, vp2, q3, vp3      micro: k10, k12, k21, k13, k31, vc
#   macro: a, b, c, alpha, beta, gamma (V = D / (a + b + c))
derive_q3 <- function(set, p, D, extra = list()) {
  P <- lapply(p, mp)
  D <- mp(D)
  if (set == "macro") {
    s <- P[["a"]] + P[["b"]] + P[["c"]]
    w <- c(P[["a"]], P[["b"]], P[["c"]]) / s
    V <- D / s
    l <- c(P[["alpha"]], P[["beta"]], P[["gamma"]])
    micro <- exp_to_micro(l, w, V)
  } else {
    micro <- switch(set,
      clearance = list(vc = P[["vc"]], k10 = P[["cl"]] / P[["vc"]], k12 = P[["q2"]] / P[["vc"]],
                       k21 = P[["q2"]] / P[["vp2"]], k13 = P[["q3"]] / P[["vc"]], k31 = P[["q3"]] / P[["vp3"]]),
      micro = list(vc = P[["vc"]], k10 = P[["k10"]], k12 = P[["k12"]], k21 = P[["k21"]],
                   k13 = P[["k13"]], k31 = P[["k31"]]),
      stop("unknown set ", set))
    V <- micro$vc
    ew <- micro_to_exp(micro)
    l <- ew$l; w <- ew$w
  }
  q <- list(D = D, V = V, l = l, w = w, A = D * w / V, set = set,
            ka = if (!is.null(extra[["ka"]])) mp(extra[["ka"]]) else NULL,
            tau = if (!is.null(extra[["tlag"]])) mp(extra[["tlag"]]) else zero(),
            dur = if (!is.null(extra[["dur"]])) mp(extra[["dur"]]) else NULL)
  for (nm in c("k10", "k12", "k21", "k13", "k31")) q[[nm]] <- micro[[nm]]
  q$cl <- micro$k10 * V
  q$q2 <- micro$k12 * V; q$vp2 <- q$q2 / micro$k21
  q$q3 <- micro$k13 * V; q$vp3 <- q$q3 / micro$k31
  q
}

# ---------------------------------------------------------------- explicit forms
trm <- function(c, l, p) list(c = c, l = l, p = as.integer(p))
sgm <- function(s0, len, terms) list(s0 = s0, len = len, terms = terms)

segments <- function(model, q) {
  A <- q$A; l <- q$l; tau <- q$tau
  idx <- 1:3
  switch(model,
    "pk3.iv_bolus" = list(sgm(zero(), NULL, lapply(idx, function(i) trm(A[i], l[i], 0L)))),
    "pk3.iv_infusion" = , "pk3.oral_0" = , "pk3.oral_0_lag" = {
      Tt <- q$dur
      during <- unlist(lapply(idx, function(i) {
        ci <- A[i] / (l[i] * Tt)
        list(trm(ci, zero(), 0L), trm(-ci, l[i], 0L))
      }), recursive = FALSE)
      after <- lapply(idx, function(i) trm(A[i] / (l[i] * Tt) * (1 - exp(-l[i] * Tt)), l[i], 0L))
      list(sgm(tau, Tt, during), sgm(tau + Tt, NULL, after))
    },
    "pk3.oral_1" = , "pk3.oral_1_lag" = {
      ka <- q$ka
      j <- which(vapply(idx, function(i) abs(ka - l[i]) < TINY, TRUE))
      check(length(j) <= 1L, "ka equal to two exponents")
      others <- setdiff(idx, j)
      Ao <- lapply(others, function(i) ka * A[i] / (ka - l[i]))
      terms <- lapply(seq_along(others), function(m) trm(Ao[[m]], l[others[m]], 0L))
      if (length(j) == 1L) terms <- c(terms, list(trm(A[j] * ka, l[j], 1L)))
      terms <- c(terms, list(trm(-Reduce(`+`, Ao), ka, 0L)))
      list(sgm(tau, NULL, terms))
    },
    stop("unknown model ", model))
}
# The same with a dose and a duration of its own (a record of a schedule).
with_dose <- function(q, D, dur = NULL) {
  q$D <- mp(D)
  q$A <- q$D * q$w / q$V
  if (!is.null(dur)) q$dur <- mp(dur)
  q
}

# Vectorised in the times u (an mpfr vector).
eval_terms <- function(terms, u) {
  total <- u * 0
  for (tm in terms) {
    x <- tm$c * exp(-tm$l * u)
    if (tm$p > 0L) x <- x * u^tm$p
    total <- total + x
  }
  total
}
eval_dterms <- function(terms, u) {
  total <- u * 0
  for (tm in terms) {
    d <- -tm$l * u^tm$p
    if (tm$p > 0L) d <- d + tm$p * u^(tm$p - 1L)
    total <- total + tm$c * d * exp(-tm$l * u)
  }
  total
}
# Concentration at the times t: 0 before the dose and before the lag, and exactly 0 at the start of
# the input of a non-bolus model.
conc_vec <- function(segs, t, bolus) {
  tm <- if (inherits(t, "mpfr")) t else mp(t)
  out <- tm * 0
  done <- rep(FALSE, length(tm))
  for (sg in segs) {
    inside <- (tm >= sg$s0) & if (is.null(sg$len)) rep(TRUE, length(tm)) else (tm <= sg$s0 + sg$len)
    idx <- !done & inside
    if (any(idx)) {
      u <- tm[idx] - sg$s0
      v <- eval_terms(sg$terms, u)
      if (!bolus) v[u == 0] <- zero()
      out[idx] <- v
      done <- done | idx
    }
  }
  out
}
# integral over [0, x] (x NULL: infinity) of u^p * exp(-lam * u)
Ipow <- function(p, lam, x) {
  if (lam == 0) return(x^(p + 1L) / (p + 1L))
  f <- mp(factorial(p))
  if (is.null(x)) return(f / lam^(p + 1L))
  y <- lam * x
  s <- y * 0
  for (j in 0:p) s <- s + y^j / factorial(j)
  f / lam^(p + 1L) * (1 - exp(-y) * s)
}
area_vec <- function(segs, t, m) {   # t NULL: infinity (a scalar)
  inf <- is.null(t)
  tm <- if (inf) mp(0) else if (inherits(t, "mpfr")) t else mp(t)
  total <- tm * 0
  for (sg in segs) {
    if (inf) {
      upper <- sg$len
      idx <- TRUE
    } else {
      idx <- tm > sg$s0
      if (!any(idx)) next
      upper <- tm[idx] - sg$s0
      if (!is.null(sg$len)) { ov <- upper > sg$len; if (any(ov)) upper[ov] <- sg$len }
    }
    acc <- if (inf) zero() else tm[idx] * 0
    for (tmx in sg$terms) {
      v <- if (m == 0) Ipow(tmx$p, tmx$l, upper) else sg$s0 * Ipow(tmx$p, tmx$l, upper) + Ipow(tmx$p + 1L, tmx$l, upper)
      acc <- acc + tmx$c * v
    }
    if (inf) total <- total + acc else total[idx] <- total[idx] + acc
  }
  total
}

input_mean_time <- function(model, q) {
  switch(model,
    "pk3.iv_bolus" = zero(),
    "pk3.iv_infusion" = q$dur / 2,
    "pk3.oral_0" = , "pk3.oral_0_lag" = q$dur / 2 + q$tau,
    "pk3.oral_1" = , "pk3.oral_1_lag" = 1 / q$ka + q$tau)
}
is_zero_order <- function(model) model %in% c("pk3.iv_infusion", "pk3.oral_0", "pk3.oral_0_lag")
is_first_order <- function(model) model %in% c("pk3.oral_1", "pk3.oral_1_lag")

# Scalars that need no peak search (MOD-3C-02, 15, 16, 25), in 256 bits.
core_scalars <- function(model, q) {
  D <- q$D; V <- q$V; l <- q$l; w <- q$w
  vss <- V * (1 + q$k12 / q$k21 + q$k13 / q$k31)
  m <- input_mean_time(model, q)
  ln2 <- log(mp(2))
  list(alpha = l[1], beta = l[2], gamma = l[3],
       k10 = q$k10, k12 = q$k12, k21 = q$k21, k13 = q$k13, k31 = q$k31,
       cl = q$cl, vc = V, q2 = q$q2, vp2 = q$vp2, q3 = q$q3, vp3 = q$vp3,
       a = q$A[1], b = q$A[2], c = q$A[3],
       w_alpha = w[1], w_beta = w[2], w_gamma = w[3],
       half_life = ln2 / l[3], half_life_alpha = ln2 / l[1], half_life_beta = ln2 / l[2], half_life_gamma = ln2 / l[3],
       vss = vss, vz = q$cl / l[3], v_extrap = V / w[3],
       auc_inf = D / q$cl, aumc_inf = D * vss / q$cl^2 + D / q$cl * m,
       mrt_system = vss / q$cl, mrt = vss / q$cl + m,
       k21_ge_k31 = if (q$k21 >= q$k31) mp(1) else mp(0))
}

# Tmax and Cmax of the first-order models by bisection on C'(u) = 0 inside the bracket of MOD-3C-17.
peak_first_order <- function(q, segs) {
  ka <- q$ka
  xof <- function(x) if (abs(ka - x) < TINY) 1 / x else log(ka / x) / (ka - x)
  xs <- c(xof(q$l[1]), xof(q$l[2]), xof(q$l[3]))
  lo <- min(xs); hi <- max(xs)
  terms <- segs[[1]]$terms
  for (i in 1:300) {
    mid <- (lo + hi) / 2
    if (eval_dterms(terms, mid) > 0) lo <- mid else hi <- mid
  }
  u <- (lo + hi) / 2
  list(tmax = u + q$tau, cmax = eval_terms(terms, u))
}
# Number of sign changes of C' on a fine grid of the profile (OM-24: exactly one).
count_sign_changes <- function(q, segs, n = 400L) {
  terms <- segs[[1]]$terms
  mx <- max(8 / q$l[3], 8 / q$ka)
  us <- mx * (seq_len(n) / n)^3
  d <- eval_dterms(terms, us)
  s <- as.numeric(sign(d))
  s <- s[s != 0]
  sum(diff(s) != 0)
}
scalars_of <- function(model, q, segs) {
  out <- core_scalars(model, q)
  D <- q$D
  if (D > 0) {
    ai <- area_vec(segs, NULL, 0)
    am <- area_vec(segs, NULL, 1)
    check(abs(ai - out$auc_inf) <= mpfr(2, PREC)^-180 * out$auc_inf, "AUC(0, inf) differs from D / CL")
    check(abs(am - out$aumc_inf) <= mpfr(2, PREC)^-180 * out$aumc_inf, "AUMC(0, inf) differs from MOD-3C-16")
    check(out$vc < out$vss && out$vss < out$vz && out$vz < out$v_extrap, "order of the volumes")
  }
  if (model == "pk3.iv_bolus") {
    out$c0 <- D / q$V
  } else if (is_zero_order(model)) {
    out$tmax_pred <- q$tau + q$dur
    out$cmax_pred <- conc_vec(segs, q$tau + q$dur, FALSE)
  } else if (D > 0) {
    pk <- peak_first_order(q, segs)
    out$tmax_pred <- pk$tmax
    out$cmax_pred <- pk$cmax
    check(count_sign_changes(q, segs) == 1L, "C' does not change sign exactly once")
  }
  out
}
