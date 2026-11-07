# Caladrius three-compartment oracle (T-142), part 4: multiple dosing and steady state (OM-28), then the
# case list for the engine tests. Sourced by models_3c_more.R.
#
# Every expected value is a plain sum of SINGLE-DOSE functions (MOD-3C-10..14, the lag as a delay) at
# 256 bits; steady state is the sum over j = 0..N of f(s + j tau) with N chosen so that the tail is
# below 2^-70 of the smallest value (checked by a longer sum). The geometric-series forms of the
# steady-state rules are not used to produce a value; the area over one interval is checked against
# D/CL. Cross-check: a long train of dosing events in deSolve (lsoda, rtol 1e-12), 1e-8.

TAILBITS <- 70L
MD_ATOL <- function(recs) 1e-14 * max(1, max(recs$dose))

# ---------------------------------------------------------------- single-dose building blocks
dose_segments <- function(model, q, D, dur) {
  qd <- with_dose(q, D, if (is.null(dur) || is.na(dur)) NULL else dur)
  segments(model, qd)
}
dconc_vec <- function(segs, tv) {
  out <- tv * 0
  done <- rep(FALSE, length(tv))
  for (sg in segs) {
    inside <- tv >= sg$s0
    if (!is.null(sg$len)) inside <- inside & (tv <= sg$s0 + sg$len)
    idx <- inside & !done
    if (any(idx)) { out[idx] <- eval_dterms(sg$terms, tv[idx] - sg$s0); done <- done | idx }
  }
  out
}
sched_eval <- function(model, q, recs, tv) {
  bolus <- model == "pk3.iv_bolus"
  conc <- tv * 0
  auc <- tv * 0
  cache <- list()
  for (r in seq_len(nrow(recs))) {
    key <- paste(sprintf("%.17g", recs$dose[r]), sprintf("%.17g", recs$dur[r]))
    if (is.null(cache[[key]])) cache[[key]] <- dose_segments(model, q, recs$dose[r], recs$dur[r])
    u <- tv - mp(recs$time[r])
    conc <- conc + conc_vec(cache[[key]], u, bolus)
    auc <- auc + area_vec(cache[[key]], u, 0)
  }
  list(conc = conc, auc = auc)
}
fmod_mp <- function(x, tau) x - floor(x / tau) * tau

# steady state as a plain sum over j = 0..N
ss_conc <- function(segs, bolus, tau, s, N) sum(conc_vec(segs, s + mpfr(0:N, PREC) * tau, bolus))
ss_dconc <- function(segs, tau, s, N) sum(dconc_vec(segs, s + mpfr(0:N, PREC) * tau))
ss_area <- function(segs, tau, s, N) {
  j <- mpfr(0:N, PREC) * tau
  sum(area_vec(segs, s + j, 0) - area_vec(segs, j, 0))
}
slowest <- function(q, model) {
  lm <- asNumeric(q$l[3])
  if (is_first_order(model)) lm <- min(lm, asNumeric(q$ka))
  lm
}
ss_terms <- function(q, model, tau) {
  late <- asNumeric(q$tau) + (if (is_zero_order(model)) asNumeric(q$dur) else 0)
  as.integer(ceiling((TAILBITS + 25L) * log(2) / (slowest(q, model) * asNumeric(tau))) + ceiling(late / asNumeric(tau)) + 5L)
}

ss_extrema <- function(model, q, segs, tau, N) {
  bolus <- model == "pk3.iv_bolus"
  bp <- c(zero(), tau)
  if (!bolus && q$tau > 0) bp <- c(bp, fmod_mp(q$tau, tau))
  if (is_zero_order(model)) bp <- c(bp, fmod_mp(q$tau + q$dur, tau))
  bp <- bp[order(asNumeric(bp))]
  bp <- bp[!duplicated(asNumeric(bp))]
  xs <- sort(unique(c(10^seq(-9, -1, length.out = 9), seq(0.1, 0.9, length.out = 17), 1 - 10^seq(-1, -9, length.out = 9))))
  cand <- c(bp, (bp[-length(bp)] + bp[-1]) / 2)
  cm <- asNumeric(ss_conc(segs, bolus, tau, bp[1], N))
  for (i in seq_len(length(bp) - 1L)) {
    b0 <- bp[i]; b1 <- bp[i + 1L]
    g <- b0 + mpfr(xs, PREC) * (b1 - b0)
    d <- vapply(seq_along(g), function(k) asNumeric(ss_dconc(segs, tau, g[k], N)), 0)
    sc <- max(abs(d))
    if (sc < 1e-20 * cm / asNumeric(tau)) next
    for (k in seq_len(length(g) - 1L)) {
      if (d[k] == 0 || (d[k] > 0) != (d[k + 1L] > 0)) {
        if (abs(d[k]) < 1e-12 * sc && abs(d[k + 1L]) < 1e-12 * sc) next
        lo <- g[k]; hi <- g[k + 1L]
        slo <- ss_dconc(segs, tau, lo, N) > 0
        for (it in 1:70) {
          mid <- (lo + hi) / 2
          if ((ss_dconc(segs, tau, mid, N) > 0) == slo) lo <- mid else hi <- mid
        }
        cand <- c(cand, (lo + hi) / 2)
      }
    }
  }
  vals <- do.call(c, lapply(seq_along(cand), function(k) ss_conc(segs, bolus, tau, cand[k], N)))
  imax <- which(vals == max(vals))[1]
  imin <- which(vals == min(vals))[1]
  wrap <- function(x) if (bolus || x < tau) x else x - tau
  near <- which(abs(vals - max(vals)) <= mpfr(2, PREC)^-60 * max(vals))
  tie <- length(unique(vapply(near, function(k) sprintf("%.12g", asNumeric(wrap(cand[k]))), ""))) > 1L
  list(cmax = vals[imax], tmax = wrap(cand[imax]), cmin = vals[imin], tied = tie)
}

# ---------------------------------------------------------------- ODE cross-check (double)
# States (G, A1, A2, A3, AUC, R); doses as events (amount added to the depot or the central
# compartment, or the input rate switched on and off).
md_rhs <- function(nq, kd) {
  function(t, y, parms) {
    dG <- if (kd$first_order) -nq$ka * y[1] else 0
    dA1 <- (if (kd$first_order) nq$ka * y[1] else 0) + y[6] - (nq$k10 + nq$k12 + nq$k13) * y[2] +
      nq$k21 * y[3] + nq$k31 * y[4]
    list(c(dG, dA1, nq$k12 * y[2] - nq$k21 * y[3], nq$k13 * y[2] - nq$k31 * y[4], y[2] / nq$V, 0))
  }
}
md_events <- function(nq, kd, recs) {
  ev <- data.frame(var = character(0), time = numeric(0), value = numeric(0), method = character(0))
  add <- function(var, time, value) ev <<- rbind(ev, data.frame(var = var, time = time, value = value, method = "add"))
  for (r in seq_len(nrow(recs))) {
    tj <- recs$time[r]; Dj <- recs$dose[r]
    if (kd$bolus) add("A1", tj, Dj)
    else if (kd$first_order) add("G", tj + nq$tau, Dj)
    else { R0 <- Dj / recs$dur[r]; add("R", tj + nq$tau, R0); add("R", tj + nq$tau + recs$dur[r], -R0) }
  }
  if (nrow(ev) > 0) { ev <- aggregate(value ~ var + time, data = ev, FUN = sum); ev$method <- "add" }
  ev[order(ev$time), , drop = FALSE]
}
md_ode <- function(model, q, recs, times) {
  nq <- double_params(q); kd <- model_kind(model)
  ev <- md_events(nq, kd, recs)
  t0 <- min(c(times, ev$time)) - 1
  grid <- sort(unique(c(t0, times, ev$time)))
  sol <- withCallingHandlers(
    ode(y = c(G = 0, A1 = 0, A2 = 0, A3 = 0, AUC = 0, R = 0), times = grid, func = md_rhs(nq, kd), parms = NULL,
        method = "lsoda", rtol = 1e-12, atol = MD_ATOL(recs), maxsteps = 1e6,
        events = if (nrow(ev) > 0) list(data = ev) else NULL),
    warning = function(w) stop("deSolve warning (an early return is an error): ", conditionMessage(w), call. = FALSE))
  check(nrow(sol) == length(grid), "deSolve returned ", nrow(sol), " rows for ", length(grid), " times")
  rows <- match(times, sol[, "time"])
  a1 <- sol[rows, "A1"]
  if (kd$bolus) for (i in seq_len(nrow(ev))) { hit <- which(times == ev$time[i]); if (length(hit)) a1[hit] <- a1[hit] + ev$value[i] }
  list(conc = a1 / nq$V, auc = sol[rows, "AUC"])
}
rel_gap <- function(a, b, scale) max(abs(a - b) / pmax(abs(b), 1e-3 * scale))
sig2 <- function(x) as.numeric(sprintf("%.2g", x))

# ---------------------------------------------------------------- time grids
md_sched_grid <- function(model, recs, tl) {
  tj <- sort(recs$time); n <- length(tj)
  k <- max(1L, ceiling(n / 10)); sel <- seq(1L, n, by = k)
  pts <- c(min(tj) - 1, tj, (tj + tl)[sel])
  if (is_zero_order(model)) {
    d <- recs$dur[order(recs$time)]
    pts <- c(pts, (tj + tl + d)[sel], signif((tj + tl + d / 2)[sel], 6))
  }
  if (n >= 2L) {
    g <- c(diff(tj), diff(tj)[n - 1L])
    pts <- c(pts, signif(tj[sel] + 0.3 * g[sel], 6), signif(tj[sel] + 0.9 * g[sel], 6))
  } else pts <- c(pts, tj + c(0.5, 2))
  sort(unique(c(pts, tj[n] + c(1, 5, 20, 80, 300), 4, 12)))
}
md_ss_grid <- function(model, tau, tl, dur) {
  tn <- as.numeric(tau)
  pts <- c(0, signif(tn * c(1e-6, 0.01, 0.05, 0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9, 0.99, 1 - 1e-6), 6), tn)
  if (model != "pk3.iv_bolus" && tl > 0) { e <- tl - floor(tl / tn) * tn; pts <- c(pts, e, e + 1e-3 * tn) }
  if (is_zero_order(model)) {
    e1 <- (tl + dur) - floor((tl + dur) / tn) * tn
    pts <- c(pts, e1, e1 + 1e-3 * tn, e1 - 1e-3 * tn, tl + dur / 2)
  }
  pts <- c(pts, 0.2, 1, 2, 3.5)
  sort(unique(pts[pts >= 0 & pts <= tn]))
}

# ---------------------------------------------------------------- one case
MD_COUNT <- list()
write_md_case <- function(name, model, set, p, dose, extra, regimen, note) {
  q <- derive_q3(set, p, dose, extra)
  bolus <- model == "pk3.iv_bolus"
  base <- paste0("model_pk3_", gsub("[^A-Za-z0-9_]", "_", name))
  tl <- asNumeric(q$tau)
  dur <- extra[["dur"]]
  chk <- function(cond, ...) if (!isTRUE(cond)) stop(base, ": ", paste0(..., collapse = ""), call. = FALSE)
  scalars <- list(); per_time <- list(); cross <- list(); ss_info <- NULL
  if (regimen$kind %in% c("regular", "schedule")) {
    recs <- if (regimen$kind == "regular")
      data.frame(time = (0:(regimen$n - 1L)) * regimen$tau, dose = dose, dur = dur %||% NA_real_) else regimen$records
    times <- md_sched_grid(model, recs, tl)
    ev <- sched_eval(model, q, recs, mpfr(times, PREC))
    conc <- sapply(ev$conc, tonum); auc <- sapply(ev$auc, tonum)
    cl <- q$cl
    scalars$auc_inf <- tonum(sum(mpfr(recs$dose, PREC)) / cl)
    ai <- zero()
    for (r in seq_len(nrow(recs))) ai <- ai + area_vec(dose_segments(model, q, recs$dose[r], recs$dur[r]), NULL, 0)
    chk(abs(ai - sum(mpfr(recs$dose, PREC)) / cl) <= mpfr(2, PREC)^-150 * ai, "area to infinity differs from sum(D) / CL")
    od <- md_ode(model, q, recs, times)
    sc <- max(abs(conc)); sa <- max(abs(auc), 1e-300)
    chk(rel_ok(od$conc, conc, sc), "deSolve concentration, gap ", rel_gap(od$conc, conc, sc))
    chk(rel_ok(od$auc, auc, sa), "deSolve AUC, gap ", rel_gap(od$auc, auc, sa))
    cross <- list(ode_events = list(conc_max_rel_gap = sig2(rel_gap(od$conc, conc, sc)), auc_max_rel_gap = sig2(rel_gap(od$auc, auc, sa))))
    per_time <- list(conc = conc, auc = auc)
  } else {
    tau <- mp(regimen$tau); tn <- regimen$tau
    chk(is.null(dur) || dur <= tn, "tau must be at least the duration")
    segs <- dose_segments(model, q, dose, dur)
    s_num <- md_ss_grid(model, tau, tl, dur %||% 0)
    sv <- mpfr(s_num, PREC)
    N <- ss_terms(q, model, tau)
    vals <- do.call(c, lapply(seq_along(sv), function(i) ss_conc(segs, bolus, tau, sv[i], N)))
    # the tail: a 50 % longer sum changes no value by more than 2^-70 of the smallest
    N2 <- as.integer(ceiling(N * 1.5)) + 3L
    for (i in unique(c(1L, ceiling(length(sv) / 2), length(sv)))) {
      chk(abs(ss_conc(segs, bolus, tau, sv[i], N2) - vals[i]) <= mpfr(2, PREC)^-70 * min(vals), "the steady-state sum has not converged")
    }
    chk(min(vals) > 0, "a steady-state value is not positive")
    areas <- do.call(c, lapply(seq_along(sv), function(i) ss_area(segs, tau, sv[i], N)))
    d_over_cl <- mp(dose) / q$cl
    a_tau <- ss_area(segs, tau, tau, N)
    chk(abs(a_tau - d_over_cl) <= mpfr(2, PREC)^-60 * d_over_cl, "area over one interval differs from D / CL")
    ex <- ss_extrema(model, q, segs, tau, N)
    sp_c <- if (bolus) mp(dose) / q$V else if (is_zero_order(model)) conc_vec(segs, q$tau + q$dur, FALSE) else peak_first_order(with_dose(q, dose), segs)$cmax
    area1 <- area_vec(segs, tau, 0)
    c1 <- conc_vec(segs, sv, bolus)
    accum_c <- rep(NA_real_, length(s_num))
    for (i in seq_along(s_num)) if (c1[i] != 0) accum_c[i] <- tonum(vals[i] / c1[i])
    conc <- sapply(vals, tonum); auc <- sapply(areas, tonum)
    # grid check of the extrema (double precision, no refinement)
    gv <- sapply(seq(0, tn, length.out = 301), function(s) asNumeric(ss_conc(segs, bolus, tau, mp(s), N)))
    chk(max(gv) <= asNumeric(ex$cmax) * (1 + 1e-12) && min(gv) >= asNumeric(ex$cmin) * (1 - 1e-12), "grid value outside [cmin, cmax]")
    # ODE: a long train of doses, read after the last dose
    sc <- max(abs(conc)); sa <- max(abs(auc))
    m <- ceiling(33 / (slowest(q, model) * tn)) + 3
    if (m <= 3000) {
      recs <- data.frame(time = (0:m) * tn, dose = dose, dur = dur %||% NA_real_)
      last <- m * tn
      od <- md_ode(model, q, recs, c(last, last + s_num))
      tc <- od$conc[-1]; ta <- od$auc[-1] - od$auc[1]
      chk(rel_ok(tc, conc, sc), "deSolve long train, concentration, gap ", rel_gap(tc, conc, sc))
      chk(rel_ok(ta, auc, sa), "deSolve long train, AUC, gap ", rel_gap(ta, auc, sa))
      cross <- list(ode_long_train = list(doses = as.integer(m + 1), conc_max_rel_gap = sig2(rel_gap(tc, conc, sc)),
                                          auc_max_rel_gap = sig2(rel_gap(ta, auc, sa))))
    } else cross <- list(ode_long_train = list(skipped = paste0(as.integer(m + 1), " doses would be needed")))
    times <- s_num
    per_time <- list(conc = conc, auc = auc, accum_c = accum_c)
    scalars$cmax_ss <- tonum(ex$cmax)
    scalars$tmax_ss <- if (ex$tied) NA_real_ else tonum(ex$tmax)
    scalars$cmin_ss <- tonum(ex$cmin)
    scalars$cav_ss <- tonum(d_over_cl / tau)
    scalars$auc_tau_ss <- tonum(d_over_cl)
    scalars$accum_cmax <- tonum(ex$cmax / sp_c)
    scalars$accum_auc <- if (area1 == 0) NA_real_ else tonum(d_over_cl / area1)
    ss_info <- list(terms_in_sum = as.integer(N + 1L), tmax_ss_is_not_unique = ex$tied)
  }
  tk <- paste0("t=", sprintf("%.17g", times))
  rows <- list()
  for (i in seq_along(times)) for (qn in names(per_time)) rows[[length(rows) + 1L]] <- c(tk[i], qn, ifelse(is.na(per_time[[qn]][i]), "", num17(per_time[[qn]][i])))
  for (sn in names(scalars)) rows[[length(rows) + 1L]] <- c("scalar", sn, ifelse(is.na(scalars[[sn]]), "", num17(scalars[[sn]])))
  rows <- do.call(rbind, rows)
  write_lines_lf(c("subject,parameter,value", paste(rows[, 1], rows[, 2], rows[, 3], sep = ",")), file.path(exp_dir, paste0(base, ".csv")))
  reg_json <- switch(regimen$kind,
    steady_state = list(kind = "steady_state", tau = regimen$tau),
    regular = list(kind = "regular", tau = regimen$tau, n_doses = as.integer(regimen$n)),
    schedule = list(kind = "schedule", records = lapply(seq_len(nrow(regimen$records)), function(i) {
      r <- list(time = regimen$records$time[i], dose = regimen$records$dose[i])
      if (!is.na(regimen$records$dur[i])) r$dur <- regimen$records$dur[i]
      r
    })))
  is_ss <- regimen$kind == "steady_state"
  l <- asNumeric(q$l)
  tex <- set == "clearance" && (l[1] - l[2]) / l[1] > 5e-2 && (l[2] - l[3]) / l[2] > 5e-2 &&
    abs(asNumeric(q$k21) - asNumeric(q$k31)) / asNumeric(q$k21) > 5e-2 &&
    (is.null(q$ka) || all(abs(asNumeric(q$ka) - l) > 5e-2 * asNumeric(q$ka)))
  quantities <- list(
    conc = if (is_ss) "concentration of the central compartment at the time s since the last dose, 0 <= s <= tau, at steady state: the sum over j >= 0 of the single-dose function at s + j*tau (a bolus at s = 0 includes the dose; at s = tau it is the value just before the next one)"
           else "concentration of the central compartment at the time t of the origin shared by the dose times: the sum of the single-dose functions of every dose (0 before the first dose; a bolus exactly at its dose time counts)",
    auc = if (is_ss) "area under the steady-state profile from s = 0 to s (D/CL at s = tau)" else "sum of the single-dose areas, each from its own dose time (not from its lag), to the time")
  if (is_ss) {
    quantities$accum_c <- "C_ss(s) / C_1(s), the ratio to the single-dose concentration at the same time since the dose; not available where C_1(s) = 0"
    quantities$scalar <- "cmax_ss, tmax_ss (time since the dose in [0, tau); not available when the maximum is attained at several times), cmin_ss, cav_ss = D/(CL*tau), auc_tau_ss = D/CL, accum_cmax = cmax_ss / (single-dose Cmax), accum_auc = (D/CL) / AUC(0, tau) of one dose (not available when that area is 0)"
  } else quantities$scalar <- "auc_inf = sum of the doses / CL"
  meta <- list(
    schema = 1L, kind = "model_md", case = base, generated_by = "oracle/scripts/models_3c_closed_form.R",
    model = model, group = regimen$kind, parameterisation = set, dose = dose,
    parameters = c(p, extra), regimen = reg_json, times = arr(times),
    units = list(dose = "mg", time = "h", concentration = "mg/L"), note = note,
    textbook_double_ok = isTRUE(tex), quantities = quantities,
    method = list(
      closed_form = "plain sum of the single-dose closed forms of the model (MOD-3C-10..14, the lag as a delay), 256-bit arithmetic (Rmpfr), rounded once to double; areas are the exact integrals of the terms; steady state is the sum over j = 0..N with the tail below 2^-70 of the smallest value; the area over one interval is checked against D/CL",
      cross_checks = list("adaptive ODE solver (deSolve lsoda, rtol 1e-12) with the doses as events and a state for the AUC", "steady state: a long train of dosing events read after the last dose"),
      cross_check_tolerance = 1e-8),
    results_of_checks = c(cross, if (!is.null(ss_info)) list(steady_state = ss_info)),
    versions = versions, n_values = nrow(rows), n_times = length(times))
  write_lines_lf(jenc(meta), file.path(exp_dir, paste0(base, ".options.json")))
  register(base, "md")
  cat(sprintf("%-60s %3d times %5d values\n", base, length(times), nrow(rows)))
  nrow(rows)
}

# ---------------------------------------------------------------- the cases
md_rows <- 0L
sched3 <- function(zero) data.frame(time = c(0, 1.5, 11.5), dose = c(100, 50, 75), dur = if (zero) c(2, 1, 3) else NA_real_)
sched40 <- function(zero) {
  n <- 40L
  data.frame(time = cumsum(c(0, rep(c(6, 8, 10, 12, 7), length.out = n - 1L))),
             dose = 50 + 10 * ((0:(n - 1L)) %% 5), dur = if (zero) 1 + 0.5 * ((0:(n - 1L)) %% 3) else NA_real_)
}
for (pn in c("base", "tie_9")) {
  pt <- points[[pn]]
  for (id in IDS) {
    ex <- list()
    if (is_first_order(id)) ex$ka <- 1.5
    if (is_zero_order(id)) ex$dur <- 2
    if (id %in% c("pk3.oral_0_lag", "pk3.oral_1_lag")) ex$tlag <- 0.5
    ex_s <- ex; ex_s$dur <- NULL                 # a schedule carries the duration in each record
    z <- is_zero_order(id)
    regs <- list(
      list(nm = "regular_n3", reg = list(kind = "regular", tau = 12, n = 3L), ex = ex),
      list(nm = "regular_n10", reg = list(kind = "regular", tau = 12, n = 10L), ex = ex),
      list(nm = "schedule_n3", reg = list(kind = "schedule", records = sched3(z)), ex = ex_s),
      list(nm = "schedule_n40", reg = list(kind = "schedule", records = sched40(z)), ex = ex_s),
      list(nm = "steady_tau12", reg = list(kind = "steady_state", tau = 12), ex = ex),
      list(nm = "steady_tau3", reg = list(kind = "steady_state", tau = 3), ex = ex))
    for (r in regs)
      md_rows <- md_rows + write_md_case(paste0("md_", short(id), "_", pn, "_", r$nm), id, pt$set, pt$p, pt$dose, r$ex, r$reg,
                                          paste("multiple dosing,", r$nm, ", point", pn))
  }
}
cat(sprintf("multiple-dosing cases: %d values\n", md_rows))

# ---------------------------------------------------------------- case list for the engine tests
case_names <- sort(ls(REG))
kinds <- unlist(mget(case_names, envir = REG))
lst <- function(kind) sub("^model_pk3_", "", case_names[kinds == kind])
emit <- function(const, items) c(sprintf("const %s: &[&str] = &[", const), sprintf("    \"%s\",", items), "];")
invoke <- function(macro, items) c(sprintf("%s! {", macro), sprintf("    %s,", items), "}")
# the cases that have scalar_ill rows (the others have no ignored test)
ill <- function(kind) { x <- lst(kind); x[paste0("model_pk3_", x) %in% ls(ILLREG)] }
ill_invoke <- function(module, kind, what) c(sprintf("ill_tests! {"), sprintf("    %s, %s;", module, kind), sprintf("    %s,", ill(what)), "}")
lines <- c("// Generated by oracle/scripts/models_3c_closed_form.R; do not edit by hand.",
           emit("VALUE_CASES", lst("values")), emit("CONDITIONING_CASES", lst("conditioning")),
           emit("CONVERSION_CASES", lst("conversion")), emit("DERIVATIVE_CASES", lst("derivatives")),
           emit("MD_CASES", lst("md")),
           invoke("value_tests", lst("values")), invoke("conditioning_tests", lst("conditioning")),
           invoke("conversion_tests", lst("conversion")), invoke("derivative_tests", lst("derivatives")),
           invoke("md_tests", lst("md")),
           emit("ILL_VALUE_CASES", ill("values")), emit("ILL_CONDITIONING_CASES", ill("conditioning")),
           emit("ILL_CONVERSION_CASES", ill("conversion")),
           ill_invoke("values_ill", "Values", "values"), ill_invoke("conditioning_ill", "Conditioning", "conditioning"),
           ill_invoke("conversions_ill", "Conversion", "conversion"))
write_lines_lf(lines, file.path(root, "crates", "caladrius-models", "tests", "oracle_models_3c.cases"))
cat(sprintf("case list: values %d, conditioning %d, conversion %d, derivatives %d, multiple dosing %d\n",
            length(lst("values")), length(lst("conditioning")), length(lst("conversion")), length(lst("derivatives")), length(lst("md"))))
