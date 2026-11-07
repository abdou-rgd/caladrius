# Caladrius three-compartment oracle (T-142), part 2: derivatives (OM-21), then refusals (OM-26),
# multiple dosing and steady state (OM-28) and the case list for the engine tests (models_3c_more.R).
# Sourced by models_3c_closed_form.R (needs exp_dir, the helpers, REG, points, ext, to_set).

# ================================================================ derivatives (OM-21)
# C = (1 / V) * sum_i w_i * Phi(lambda_i, t) is a function of psi = (V, l1, l2, l3, w1, w2), w3 = 1 - w1 - w2.
# dC/dtheta = sum_j (dC/dpsi_j)(dpsi_j/dtheta) for the six native parameters theta of the set, both
# factors by 256-bit central differences with relative step 1e-25 (the roots of every perturbed set
# re-solved in 256 bits). The condition number of an entry is sum_j |term_j| / |sum_j term_j|: an
# entry above KAPPA_MAX is omitted and counted (T-032). ka, tlag and dur are differentiated directly.
step_now <- function() mpfr(10, PREC)^-25
psi_vec <- function(q) c(q$V, q$l, q$w[1], q$w[2])
q_from_psi <- function(q0, psi) {
  q <- q0
  q$V <- psi[1]; q$l <- psi[2:4]
  w <- c(psi[5], psi[6], 1 - psi[5] - psi[6])
  q$w <- w; q$A <- q$D * w / q$V
  q
}
deriv_times <- function(q, extra) {
  ta <- 1 / num(q$l[1]); tb <- 1 / num(q$l[2]); tg <- 1 / num(q$l[3])
  tl <- extra$tlag %||% 0
  ts <- c(ta * c(1e-3, 0.1, 1), tb * c(0.5, 2), tg * c(0.5, 2, 8, 40, 800))
  if (!is.null(extra$ka)) ts <- c(ts, 1 / extra$ka)
  exact <- c(-1, 0)
  ts <- ts + tl
  if (tl > 0) exact <- c(exact, tl / 2, tl)
  if (!is.null(extra$dur)) { ts <- c(ts, tl + extra$dur / 2, tl + 3 * extra$dur); exact <- c(exact, tl + extra$dur) }
  sort(unique(c(signif(ts, 6), exact, 1, 12)))
}
ifelse_mp <- function(cond, a, b) { r <- rep(0, length(cond)); r[cond] <- a; if (any(!cond)) r[!cond] <- b[!cond]; r }
deriv_chain <- function(model, set, p, D, extra, tm) {
  bolus <- model == "pk3.iv_bolus"
  st <- step_now()
  q0 <- derive_q3(set, p, D, extra)
  psi0 <- psi_vec(q0)
  cfun <- function(psi) conc_vec(segments(model, q_from_psi(q0, psi)), tm, bolus)
  cps <- vector("list", 6)
  for (j in 1:6) {
    h <- if (psi0[j] == 0) st else st * abs(psi0[j])
    pp <- psi0; pp[j] <- pp[j] + h
    pm <- psi0; pm[j] <- pm[j] - h
    cps[[j]] <- (cfun(pp) - cfun(pm)) / (2 * h)
  }
  dps <- list()
  for (n in names(p)) {
    h <- st * abs(mp(p[[n]]))
    ph <- p; pl <- p
    ph[[n]] <- mp(p[[n]]) + h; pl[[n]] <- mp(p[[n]]) - h
    dps[[n]] <- (psi_vec(derive_q3(set, ph, D, extra)) - psi_vec(derive_q3(set, pl, D, extra))) / (2 * h)
  }
  ent <- list(); kap <- list()
  for (n in names(p)) {
    S <- tm * 0; Ab <- tm * 0
    for (j in 1:6) { tj <- cps[[j]] * dps[[n]][j]; S <- S + tj; Ab <- Ab + abs(tj) }
    ent[[n]] <- S
    kk <- rep(0, length(tm))
    nz <- S != 0
    if (any(nz)) kk[nz] <- asNumeric(Ab[nz] / abs(S[nz]))
    kk[!nz & Ab != 0] <- Inf
    kap[[n]] <- kk
  }
  list(ent = ent, kap = kap, cps = cps, psi0 = psi0, conc = cfun(psi0))
}
extra_deriv <- function(model, q0, which, tm) {
  bolus <- model == "pk3.iv_bolus"
  fld <- switch(which, ka = "ka", tlag = "tau", dur = "dur")
  base <- q0[[fld]]
  h <- step_now() * abs(base)
  qp <- q0; qm <- q0
  qp[[fld]] <- base + h; qm[[fld]] <- base - h
  (conc_vec(segments(model, qp), tm, bolus) - conc_vec(segments(model, qm), tm, bolus)) / (2 * h)
}
N_OMIT <- 0L; N_STRUCT <- 0L; N_RECOMP <- 0L; N_KINK <- 0L
write_deriv_case <- function(name, model, set, p, dose, extra, note) {
  q0 <- derive_q3(set, p, dose, extra)
  times <- deriv_times(q0, extra)
  tm <- mp(times)
  tl <- extra$tlag %||% 0
  ch <- deriv_chain(model, set, p, dose, extra, tm)
  # noise guard: a central difference within 1e-12 of the 256-bit floor is recomputed at 2048 bits
  Cabs <- abs(ch$conc)
  flag <- rep(FALSE, length(times))
  for (j in 1:6) {
    sc <- abs(ch$cps[[j]]) * abs(ch$psi0[j])
    flag <- flag | (sc != 0 & Cabs != 0 & sc < mpfr(10, 256L)^-38 * Cabs)
  }
  ent <- ch$ent; kap <- ch$kap
  if (any(flag)) {
    idx <- which(flag)
    hi <- at_prec(2048L, deriv_chain(model, set, p, dose, extra, mpfr(times[idx], 2048L)))
    for (n in names(p)) {
      for (i in seq_along(idx)) ent[[n]][idx[i]] <- mpfr(hi$ent[[n]][i], 256L)
      kap[[n]][idx] <- hi$kap[[n]]
    }
    N_RECOMP <<- N_RECOMP + length(idx)
  }
  cols <- ent; kapp <- kap
  names(cols) <- paste0("d_", names(cols)); names(kapp) <- names(cols)
  for (which in c("ka", "tlag", "dur")) {
    present <- switch(which, ka = !is.null(extra$ka), tlag = !is.null(extra$tlag), dur = !is.null(extra$dur))
    if (!present) next
    d <- extra_deriv(model, q0, which, tm)
    base <- mp(extra[[which]])
    k <- rep(1, length(times))
    k[(d != 0) & (abs(base * d) < mpfr(10, 256L)^-12 * Cabs)] <- Inf
    cols[[paste0("d_", which)]] <- d; kapp[[paste0("d_", which)]] <- k
  }
  keys_t <- character(0); keys_p <- character(0); vals <- numeric(0)
  tkeys <- paste0("t=", num17(times))
  omitted <- 0L; structural <- 0L
  for (nm in names(cols)) {
    v <- cols[[nm]]; k <- kapp[[nm]]
    for (i in seq_along(times)) {
      kink <- (nm == "d_tlag" && (times[i] == tl || (!is.null(extra$dur) && times[i] == tl + extra$dur))) ||
        (nm == "d_dur" && times[i] == tl + (extra$dur %||% 0))
      if (kink) { N_KINK <<- N_KINK + 1L; next }
      if (v[i] == 0) { structural <- structural + 1L; keys_t <- c(keys_t, tkeys[i]); keys_p <- c(keys_p, nm); vals <- c(vals, 0); next }
      if (k[i] > KAPPA_MAX) { omitted <- omitted + 1L; next }
      keys_t <- c(keys_t, tkeys[i]); keys_p <- c(keys_p, nm); vals <- c(vals, tonum(v[i]))
    }
  }
  N_OMIT <<- N_OMIT + omitted; N_STRUCT <<- N_STRUCT + structural
  rows <- data.frame(subject = keys_t, parameter = keys_p, value = vals, stringsAsFactors = FALSE)
  rows <- rows[order(match(rows$subject, tkeys), match(rows$parameter, names(cols))), ]
  base_n <- paste0("model_pk3_deriv_", short(model), "_", gsub("[^A-Za-z0-9_]", "_", name))
  write_lines_lf(c("subject,parameter,value", sprintf("%s,%s,%s", rows$subject, rows$parameter, num17(rows$value))),
                 file.path(exp_dir, paste0(base_n, ".csv")))
  meta <- list(
    schema = 1L, kind = "model_derivatives", case = base_n,
    generated_by = "oracle/scripts/models_3c_closed_form.R",
    model = model, parameterisation = set, dose = dose,
    parameters = c(p, extra), times = arr(times),
    units = list(dose = "mg", time = "h", concentration = "mg/L"),
    note = paste("derivatives,", note),
    textbook_double_ok = FALSE,
    quantities = list(
      derivative = "d_<name> at a time is the partial derivative of the concentration with respect to the parameter <name> of this case (the dose fixed; for the macro set vc = D / (a + b + c) moves with a, b and c); 0 before the dose and before the lag; d_tlag is not written at the kinks t = tlag and t = tlag + dur, d_dur not at t = tlag + dur (the derivative has a jump there)",
      omitted = "an entry whose condition number (sum of the absolute terms of the chain rule through V, the three exponents and two weights, divided by the absolute sum; for ka, tlag, dur a derivative below 1e-12 of the concentration per unit of the parameter) exceeds 1e3 is not written: no closed form in double precision can be asked for 1e-12 relative accuracy there"
    ),
    method = list(
      derivatives = "central differences in 256-bit arithmetic (Rmpfr), relative step 1e-25, of the independent explicit form of the concentration, the roots of every perturbed parameter set re-solved; chain rule through psi = (V, alpha, beta, gamma, w_alpha, w_beta); rounded once to double",
      kappa_max = KAPPA_MAX,
      n_omitted_ill_conditioned = omitted,
      n_structural_zero_written_as_zero = structural,
      n_recomputed_at_2048_bits = sum(flag)
    ),
    versions = versions,
    n_values = nrow(rows), n_times = length(times)
  )
  write_lines_lf(jenc(meta), file.path(exp_dir, paste0(base_n, ".options.json")))
  register(base_n, "derivatives")
  cat(sprintf("%-52s %3d values, %2d omitted, %2d zeros\n", base_n, nrow(rows), omitted, structural))
  nrow(rows)
}
deriv_specs <- list()
dadd <- function(model, name, pname, set = NULL, extra = NULL, note = "") {
  pt <- points[[pname]]
  set <- set %||% pt$set
  ex <- extra %||% ext(model, pname)
  if (model == "pk3.iv_infusion") ex$tlag <- NULL
  deriv_specs[[length(deriv_specs) + 1L]] <<- list(model = model, name = name, set = set, p = to_set(pt, set),
                                                    dose = pt$dose, extra = ex, note = paste(note, "point", pname, "set", set))
}
for (id in IDS) {
  ex <- list()
  if (is_first_order(id)) ex$ka <- 1.5
  if (is_zero_order(id)) ex$dur <- 2
  if (id %in% c("pk3.oral_0_lag", "pk3.oral_1_lag")) ex$tlag <- 0.5
  dadd(id, "base", "base", extra = ex)
  dadd(id, "base_micro", "base", set = "micro", extra = ex)
  dadd(id, "base_macro", "base", set = "macro", extra = ex)
  for (pn in c("fast_dist", "small2_6", "small3_9", "tie_4", "tie_9", "tie_exact", "close_exp"))
    dadd(id, pn, pn)
}
dadd("pk3.oral_1", "ka_eq_alpha", "base_macro", extra = list(ka = 3), note = "ka = alpha exactly (confluent limit)")
dadd("pk3.oral_1", "ka_eq_beta", "base_macro", extra = list(ka = 0.5), note = "ka = beta exactly")
dadd("pk3.oral_1", "ka_eq_gamma", "base_macro", extra = list(ka = 0.05), note = "ka = gamma exactly")
dadd("pk3.oral_1_lag", "ka_eq_beta", "base_macro", extra = list(ka = 0.5, tlag = 0.5), note = "ka = beta exactly")
dadd("pk3.oral_1", "ka_near_alpha", "base", extra = list(ka = 3 * (1 + 1e-9)), note = "ka = alpha (1 + 1e-9)")
dadd("pk3.oral_1", "ka_500_alpha", "base", extra = list(ka = 1500), note = "ka / alpha = 500")
dadd("pk3.iv_infusion", "dur_short", "base", extra = list(dur = 0.01), note = "very short input")
cat(sprintf("derivative cases: %d\n", length(deriv_specs)))
total_deriv <- 0L
for (s in deriv_specs) total_deriv <- total_deriv + write_deriv_case(s$name, s$model, s$set, s$p, s$dose, s$extra, s$note)
cat(sprintf("derivatives: %d values, %d omitted (kappa > %g), %d structural zeros, %d recomputed at 2048 bits, %d kink entries not written\n",
            total_deriv, N_OMIT, KAPPA_MAX, N_STRUCT, N_RECOMP, N_KINK))

if (file.exists(file.path(script_dir, "models_3c_more.R"))) source(file.path(script_dir, "models_3c_more.R"))
