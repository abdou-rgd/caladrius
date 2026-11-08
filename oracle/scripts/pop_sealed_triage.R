# Caladrius population-fit oracle, card T-174: triage of the cl bias that the engine's FOCEI fit shows on the sealed
# design A (`accurate`, oracle/expected/pop/sealed/accurate.json).
#
# INFORMATIONAL: nothing here is a pass/fail reference and no test reads the outputs. Three steps, all in R with open
# tools only (base R + saemix), written from specs/population.md without reading any engine code:
#   1. an independent FOCEI fit (pop_sealed_triage_lib.R) on each of the 200 replicates, started at the truth, and a
#      second fit from a different start; its objective is first checked against oracle/expected/pop/ofv/ (to 1e-8
#      relative, all 20 cases) and the evidence that it reaches the minimum is recorded per replicate;
#   2. saemix (the settings of pop_saemix.R, a fixed seed) on the same replicates, as a second look (POP-ORA-03);
#   3. the replicate-by-replicate comparison with the engine's numbers (a CSV written by the T-154 engine agent, given as
#      argument 1 or in TRIAGE_ENGINE_CSV; it is data) and the aggregates of cl, v and ka under the three fits.
#
# Run: Rscript oracle/scripts/pop_sealed_triage.R [engine_csv]
#   writes oracle/expected/pop/second_look/sealed_accurate_focei_r.csv, sealed_accurate_saemix.csv and
#   sealed_accurate_triage.json (LF endings). Environment: TRIAGE_WORKERS (default 6, PSOCK workers), TRIAGE_REPS
#   (a range such as 1:6, development only; the outputs are then not written), TRIAGE_CACHE (a folder where the
#   stages are kept so that a stopped run resumes), TRIAGE_WAIT_MIN (minutes to wait for the engine CSV, default 90).
suppressMessages({ library(jsonlite); library(parallel) })
src <- sub("^--file=", "", grep("^--file=", commandArgs(FALSE), value = TRUE)[1])
SCRIPTS <- normalizePath(dirname(src), winslash = "/")
source(file.path(SCRIPTS, "pop_sealed_triage_lib.R"))
ROOT <- normalizePath(file.path(SCRIPTS, "..", ".."), winslash = "/")
OUTDIR <- file.path(ROOT, "oracle", "expected", "pop", "second_look")
args <- commandArgs(trailingOnly = TRUE)
ENGINE_CSV <- if (length(args) >= 1L) args[1] else Sys.getenv("TRIAGE_ENGINE_CSV", "")
WORKERS <- as.integer(Sys.getenv("TRIAGE_WORKERS", "6"))
CACHE <- Sys.getenv("TRIAGE_CACHE", "")
WAIT_MIN <- as.numeric(Sys.getenv("TRIAGE_WAIT_MIN", "90"))
reps_env <- Sys.getenv("TRIAGE_REPS", "")
DEV <- nzchar(reps_env)
say <- function(...) { cat(format(Sys.time(), "%H:%M:%S"), sprintf(...), "\n", sep = " "); flush.console() }
cached <- function(name, expr_fun) {
  if (!nzchar(CACHE)) return(expr_fun())
  dir.create(CACHE, recursive = TRUE, showWarnings = FALSE); p <- file.path(CACHE, paste0(name, ".rds"))
  if (file.exists(p)) { say("stage %s: from cache", name); return(readRDS(p)) }
  v <- expr_fun(); saveRDS(v, p); v
}
timed <- function(expr_fun) { t0 <- proc.time()[[3]]; v <- expr_fun(); list(value = v, seconds = proc.time()[[3]] - t0) }

sealed <- fromJSON(file.path(ROOT, "oracle", "expected", "pop", "sealed", "accurate.json"), simplifyVector = FALSE)
K_ALL <- sealed$design$replicates
REPS <- if (DEV) eval(parse(text = reps_env)) else seq_len(K_ALL)
TRUTH <- list(theta = list(cl = as.numeric(sealed$truth$typical_values$cl), v = as.numeric(sealed$truth$typical_values$v),
                           ka = as.numeric(sealed$truth$typical_values$ka)),
              omega = vapply(sealed$truth$omega_diagonal, as.numeric, 0)[c("cl", "v", "ka")],
              a = as.numeric(sealed$truth$xi$a), b = as.numeric(sealed$truth$xi$b))
RE <- unlist(sealed$model$random_effects)             # cl, v, ka: the order of the columns of Omega
C_BIAS <- sealed$aggregate_checks$constants$c_bias
STARTS <- list(
  truth = list(theta = TRUTH$theta, omega_sd = sqrt(TRUTH$omega[RE]), A = TRUTH$a^2, B = TRUTH$b^2),
  # a different start (card: two starts on at least 10 replicates): typical values 25% above, 20% below and 30% above,
  # standard deviations 1.5, 0.6 and 1.4 times the truth, error variances 10 and 3 times the truth
  other = list(theta = list(cl = TRUTH$theta$cl * 1.25, v = TRUTH$theta$v * 0.8, ka = TRUTH$theta$ka * 1.3),
               omega_sd = sqrt(TRUTH$omega[RE]) * c(1.5, 0.6, 1.4), A = TRUTH$a^2 * 10, B = TRUTH$b^2 * 3))
say("T-174 triage: %d replicates, %d workers, root %s", length(REPS), WORKERS, ROOT)

# ============================================================ step 0: the objective against oracle/expected/pop/ofv/
ofv_check <- cached("ofv_check", function() {
  ids <- setdiff(sub("[.]json$", "", list.files(file.path(ROOT, "oracle", "expected", "pop", "ofv"), pattern = "[.]json$")), "options")
  rows <- lapply(ids, function(id) {
    cs <- load_ofv_dat(ROOT, id)
    r <- ofv_focei(cs$dat, cs$theta, cs$Omega, cs$err, cs$xi, inner_tol = 1e-12, per_subject = TRUE)
    # the subjects of a case with a hard inner problem may need more iterations: report the inner gradient too
    ex_ps <- vapply(cs$expected$per_subject, function(s) s$ofv_full$linearised, 0)
    list(id = id, error_model = cs$err, n_subjects = cs$dat$m, n_random_effects = cs$dat$q, ofv_r = r$ofv,
         ofv_expected = cs$expected$total_full$linearised,
         rel_error_total = abs(r$ofv - cs$expected$total_full$linearised) / abs(cs$expected$total_full$linearised),
         max_rel_error_subject = max(abs(r$per_subject - ex_ps) / abs(ex_ps)), inner_gradmax = r$gradmax)
  })
  rows
})
ofv_check_df <- do.call(rbind, lapply(ofv_check, as.data.frame))
say("OFV cross-check: %d cases, max relative error of the total %.2e (median %.2e), cases above 1e-8: %s",
    nrow(ofv_check_df), max(ofv_check_df$rel_error_total), median(ofv_check_df$rel_error_total),
    paste(ofv_check_df$id[ofv_check_df$rel_error_total > 1e-8], collapse = ","))

# ============================================================ worker pool
cl <- makeCluster(WORKERS)

clusterExport(cl, c("ROOT", "SCRIPTS", "RE", "STARTS", "TRUTH"), envir = environment())
invisible(clusterEvalQ(cl, { source(file.path(SCRIPTS, "pop_sealed_triage_lib.R")); NULL }))
fit_one <- function(k, which) {
  d <- read_sealed_rep(ROOT, "accurate", k)
  dat <- sealed_dat(d, "pk1.oral_1", RE)
  r <- tryCatch(fit_focei(dat, STARTS[[which]], "combined"), error = function(e) list(error = conditionMessage(e)))
  r$replicate <- k
  r
}
clusterExport(cl, "fit_one", envir = environment())

# ============================================================ step 1: the FOCEI fits (start at the truth, and another start)
run_focei <- function(which, reps = REPS) cached(paste0("focei_", which), function() {
  say("FOCEI fit from the %s start on %d replicates", which, length(reps))
  tm <- timed(function() parLapplyLB(cl, reps, fit_one, which = which))
  say("FOCEI %s start done in %.0f s wall", which, tm$seconds)
  list(fits = tm$value, wall_seconds = tm$seconds)
})
f1 <- run_focei("truth")
OTHER_EVERY <- as.integer(Sys.getenv("TRIAGE_OTHER_EVERY", "5"))   # the other start runs on every 5th replicate (40 of 200)
f2 <- run_focei("other", REPS[REPS %% OTHER_EVERY == 0L])

to_df <- function(fits, prefix) {
  do.call(rbind, lapply(fits, function(r) {
    if (!is.null(r$error)) return(data.frame(replicate = r$replicate, error = r$error))
    data.frame(replicate = r$replicate, cl = r$theta[["cl"]], v = r$theta[["v"]], ka = r$theta[["ka"]],
               omega_cl = r$omega[1], omega_v = r$omega[2], omega_ka = r$omega[3], A = r$A, B = r$B,
               a = sqrt(r$A), b = sqrt(r$B), ofv = r$ofv, grad_free_max = r$grad_free_max, newton_decrement = r$newton_decrement,
               hess_min_eig = r$hess_min_eig, n_on_bound = r$n_on_bound, kkt_multiplier_min = r$kkt_multiplier_min,
               inner_gradmax = r$inner_gradmax, nlminb_convergence = r$nlminb_convergence, polish_steps = r$polish_steps,
               evals = r$evals, seconds = r$seconds)
  }))
}
d1 <- to_df(f1$fits, ""); d2 <- to_df(f2$fits, "")
if (!is.null(d1$error) || !is.null(d2$error)) say("WARNING: fits with an error: start1 %d, start2 %d", sum(!is.na(d1$error)), sum(!is.na(d2$error)))
ok1 <- is.null(d1$error) ; ok2 <- is.null(d2$error)
say("FOCEI truth start: max free gradient %.2e, max Newton decrement %.2e, min Hessian eigenvalue %.3g, on the A bound %d/%d, KKT multiplier min %.3g",
    max(d1$grad_free_max), max(d1$newton_decrement, na.rm = TRUE), min(d1$hess_min_eig), sum(d1$n_on_bound > 0), nrow(d1), min(d1$kkt_multiplier_min, na.rm = TRUE))

# ============================================================ step 2: saemix
sx_one <- function(k) {
  suppressMessages(library(saemix))
  wd <- file.path(tempdir(), paste0("sx", k)); dir.create(wd, showWarnings = FALSE); old <- setwd(wd); on.exit(setwd(old))
  d <- read_sealed_rep(ROOT, "accurate", k)
  dd <- data.frame(id = as.integer(factor(d$subject, levels = unique(d$subject))), time = d$time, conc = d$conc, dose = d$dose)
  model <- function(psi, id, xidep) { cl <- psi[id, 1]; v <- psi[id, 2]; ka <- psi[id, 3]; kk <- cl / v
    (xidep[, 2] * ka / (v * (ka - kk))) * (exp(-kk * xidep[, 1]) - exp(-ka * xidep[, 1])) }
  sd_ <- saemixData(name.data = dd, name.group = "id", name.predictors = c("time", "dose"), name.response = "conc", verbose = FALSE)
  psi0 <- matrix(c(TRUTH$theta$cl, TRUTH$theta$v, TRUTH$theta$ka), nrow = 1, dimnames = list(NULL, RE))
  sm <- saemixModel(model = model, psi0 = psi0, transform.par = rep(1, 3), covariance.model = diag(3),
                    omega.init = diag(TRUTH$omega[RE]), error.model = "combined", error.init = c(TRUTH$a, TRUTH$b), verbose = FALSE)
  # the settings of pop_saemix.R (map, 300 + 100 iterations); the seed is fixed per replicate (SEED + k); the Fisher
  # matrix and the likelihood by importance sampling are not computed, they are not used here
  opt <- saemixControl(seed = 632545L + k, map = TRUE, fim = FALSE, ll.is = FALSE, ll.gq = FALSE, nbiter.saemix = c(300, 100),
                       displayProgress = FALSE, save = FALSE, save.graphs = FALSE, print = FALSE, warnings = FALSE)
  t0 <- proc.time()[[3]]
  fit <- tryCatch(suppressWarnings(saemix(sm, sd_, opt)), error = function(e) conditionMessage(e))
  if (is.character(fit)) return(data.frame(replicate = k, converged = FALSE, error = fit, seconds = proc.time()[[3]] - t0))
  r <- fit@results
  th <- as.numeric(r@fixed.effects); om <- diag(r@omega)[1:3]; rp <- as.numeric(r@respar)
  data.frame(replicate = k, converged = TRUE, cl = th[1], v = th[2], ka = th[3], omega_cl = om[1], omega_v = om[2], omega_ka = om[3],
             a = rp[1], b = rp[2], seconds = proc.time()[[3]] - t0)
}
clusterExport(cl, c("sx_one"), envir = environment())
sx <- cached("saemix", function() {
  say("saemix on %d replicates", length(REPS))
  tm <- timed(function() parLapplyLB(cl, REPS, sx_one))
  say("saemix done in %.0f s wall", tm$seconds)
  list(rows = tm$value, wall_seconds = tm$seconds)
})
dsx <- do.call(rbind, lapply(sx$rows, function(r) { if (is.null(r$cl)) r$cl <- r$v <- r$ka <- r$omega_cl <- r$omega_v <- r$omega_ka <- r$a <- r$b <- NA_real_; r[, c("replicate", "converged", "cl", "v", "ka", "omega_cl", "omega_v", "omega_ka", "a", "b", "seconds")] }))
say("saemix: %d of %d runs returned an estimate", sum(dsx$converged), nrow(dsx))

# ============================================================ step 3: the engine, replicate by replicate
wait_for_engine <- function() {
  if (!nzchar(ENGINE_CSV)) return(NULL)
  t0 <- Sys.time()
  repeat {
    if (file.exists(ENGINE_CSV)) {
      e <- tryCatch(read.csv(ENGINE_CSV, comment.char = "#", stringsAsFactors = FALSE), error = function(e) NULL)
      if (!is.null(e) && nrow(e) >= length(REPS) && all(REPS %in% e$replicate)) return(e)
    }
    if (as.numeric(difftime(Sys.time(), t0, units = "mins")) > WAIT_MIN) return(if (file.exists(ENGINE_CSV)) tryCatch(read.csv(ENGINE_CSV, comment.char = "#", stringsAsFactors = FALSE), error = function(e) NULL) else NULL)
    say("waiting for the engine CSV (%s)", ENGINE_CSV); Sys.sleep(60)
  }
}
eng <- wait_for_engine()
have_engine <- !is.null(eng)
if (have_engine) { eng <- eng[eng$replicate %in% REPS, ]; eng <- eng[order(eng$replicate), ]; say("engine CSV: %d rows, statuses: %s", nrow(eng), paste(names(table(eng$status)), table(eng$status), collapse = ", ")) }

# R OFV at the engine's estimate (Omega diagonal from omega_<re> = variances, A = a^2, B = b^2), strict inner solve
ofv_at_engine <- function(k, row) {
  d <- read_sealed_rep(ROOT, "accurate", k); dat <- sealed_dat(d, "pk1.oral_1", RE)
  th <- c(cl = row$estimate.cl, v = row$estimate.v, ka = row$estimate.ka)
  Om <- diag(c(row$omega_cl, row$omega_v, row$omega_ka))
  r <- ofv_focei(dat, th, Om, "combined", c(row$xi.a^2, row$xi.b^2), inner_tol = 1e-12)
  c(ofv = r$ofv, gradmax = r$gradmax)
}
clusterExport(cl, "ofv_at_engine", envir = environment())
eng_ofv <- if (have_engine) cached("engine_ofv", function() {
  rows <- split(eng, seq_len(nrow(eng)))
  parLapplyLB(cl, seq_along(rows), function(i, rows) ofv_at_engine(rows[[i]]$replicate, rows[[i]]), rows = rows)
}) else NULL

# ============================================================ outputs and summaries
dir.create(OUTDIR, recursive = TRUE, showWarnings = FALSE)
write_lf <- function(df, path) { con <- file(path, open = "wb"); on.exit(close(con)); write.table(df, con, sep = ",", row.names = FALSE, quote = FALSE, na = "NA", eol = "\n") }
fmt17 <- function(df) { for (nm in names(df)) if (is.numeric(df[[nm]]) && !is.integer(df[[nm]])) df[[nm]] <- ifelse(is.na(df[[nm]]), NA, sprintf("%.17g", df[[nm]])); df }
names2 <- setdiff(names(d1), "replicate")
focei_out <- merge(d1, setNames(d2, c("replicate", paste0("other_", names2))), by = "replicate", all.x = TRUE)
if (have_engine) {
  focei_out$engine_ofv_r <- vapply(eng_ofv, function(x) x[["ofv"]], 0)[match(focei_out$replicate, eng$replicate)]
}
if (!DEV) {
  write_lf(fmt17(focei_out), file.path(OUTDIR, "sealed_accurate_focei_r.csv"))
  write_lf(fmt17(dsx), file.path(OUTDIR, "sealed_accurate_saemix.csv"))
}

smry <- function(x) list(max_abs = max(abs(x), na.rm = TRUE), median = median(x, na.rm = TRUE), median_abs = median(abs(x), na.rm = TRUE), n = sum(!is.na(x)))
agg <- function(x, truth) { x <- x[is.finite(x)]; K <- length(x); sdx <- sd(x); lim <- C_BIAS * sdx / sqrt(K)
  list(n = K, mean = mean(x), bias = mean(x) - truth, limit_3sd_sqrtK = lim, bias_over_limit = abs(mean(x) - truth) / lim,
       z_bias_se_mean = (mean(x) - truth) / (sdx / sqrt(K)), exceeds_limit = abs(mean(x) - truth) > lim, sd = sdx, median = median(x), median_bias = median(x) - truth) }
th_names <- c("cl", "v", "ka")
aggs <- list(R_focei = lapply(setNames(th_names, th_names), function(p) agg(d1[[p]], TRUTH$theta[[p]])),
             R_focei_other_start = lapply(setNames(th_names, th_names), function(p) agg(d2[[p]], TRUTH$theta[[p]])),
             saemix = lapply(setNames(th_names, th_names), function(p) agg(dsx[[p]], TRUTH$theta[[p]])))
if (have_engine) {
  aggs$engine_all <- lapply(setNames(th_names, th_names), function(p) agg(eng[[paste0("estimate.", p)]], TRUTH$theta[[p]]))
  aggs$engine_converged_only <- lapply(setNames(th_names, th_names), function(p) agg(eng[[paste0("estimate.", p)]][eng$status == "converged"], TRUTH$theta[[p]]))
  aggs$R_focei_converged_engine_reps <- lapply(setNames(th_names, th_names), function(p) agg(d1[[p]][d1$replicate %in% eng$replicate[eng$status == "converged"]], TRUTH$theta[[p]]))
  aggs$R_focei_at_bound_engine_reps <- lapply(setNames(th_names, th_names), function(p) agg(d1[[p]][d1$replicate %in% eng$replicate[eng$status == "at_bound"]], TRUTH$theta[[p]]))
}

# two starts of the R fit (card: agreement in theta and OFV)
two <- merge(d1, d2, by = "replicate", suffixes = c("_1", "_2"))
start_cmp <- list(n_replicates = nrow(two),
  max_abs_ofv = max(abs(two$ofv_1 - two$ofv_2)),
  max_rel_theta = lapply(setNames(th_names, th_names), function(p) max(abs(two[[paste0(p, "_1")]] - two[[paste0(p, "_2")]]) / two[[paste0(p, "_1")]])),
  max_rel_omega = lapply(setNames(c("omega_cl", "omega_v", "omega_ka"), c("omega_cl", "omega_v", "omega_ka")), function(p) max(abs(two[[paste0(p, "_1")]] - two[[paste0(p, "_2")]]) / two[[paste0(p, "_1")]])),
  max_abs_A = max(abs(two$A_1 - two$A_2)), max_abs_B = max(abs(two$B_1 - two$B_2)),
  n_other_start_lower_ofv_by_more_than_1e_6 = sum(two$ofv_2 < two$ofv_1 - 1e-6),
  n_other_start_higher_ofv_by_more_than_1e_6 = sum(two$ofv_2 > two$ofv_1 + 1e-6))
evidence <- list(
  n_fits = nrow(d1), max_grad_free_max = max(d1$grad_free_max), median_grad_free_max = median(d1$grad_free_max),
  max_newton_decrement = max(d1$newton_decrement, na.rm = TRUE), n_decrement_na = sum(is.na(d1$newton_decrement)),
  min_hessian_eigenvalue = min(d1$hess_min_eig), n_on_A_bound = sum(d1$n_on_bound > 0), min_kkt_multiplier_at_bound = min(d1$kkt_multiplier_min, na.rm = TRUE),
  max_inner_gradmax = max(d1$inner_gradmax), median_polish_steps = median(d1$polish_steps), max_polish_steps = max(d1$polish_steps),
  nlminb_convergence_codes = as.list(table(d1$nlminb_convergence)),
  seconds_per_fit = list(median = median(d1$seconds), max = max(d1$seconds), sum = sum(d1$seconds)),
  wall_seconds_truth_start = f1$wall_seconds, wall_seconds_other_start = f2$wall_seconds,
  two_starts = start_cmp)

cmp <- NULL; verdict <- NULL
if (have_engine) {
  m <- match(eng$replicate, d1$replicate)
  rr <- d1[m, ]
  dth <- lapply(setNames(th_names, th_names), function(p) eng[[paste0("estimate.", p)]] - rr[[p]])
  drel <- lapply(setNames(th_names, th_names), function(p) (eng[[paste0("estimate.", p)]] - rr[[p]]) / rr[[p]])
  dom <- lapply(setNames(c("omega_cl", "omega_v", "omega_ka"), c("omega_cl", "omega_v", "omega_ka")), function(p) (eng[[p]] - rr[[p]]) / rr[[p]])
  dA <- eng$xi.a^2 - rr$A; dB <- eng$xi.b^2 - rr$B
  ofv_eng_at <- vapply(eng_ofv, function(x) x[["ofv"]], 0)
  gap <- ofv_eng_at - rr$ofv                    # R OFV at the engine's estimate minus R OFV at the R estimate
  eng_rep_minus_r_at <- eng$ofv - ofv_eng_at    # the engine's reported OFV minus the R OFV at the same point
  eng_minus_r_fit <- eng$ofv - rr$ofv
  # the R fit's own tolerance: the largest of its Newton decrements, of the disagreement of its two starts in OFV, and 1e-9
  tol_ofv <- max(1e-9, max(d1$newton_decrement, na.rm = TRUE), start_cmp$max_abs_ofv)
  tol_theta_rel <- max(1e-9, unlist(start_cmp$max_rel_theta))
  cnt <- function(x, t) sum(abs(x) > t, na.rm = TRUE)
  cmp <- list(
    n = nrow(eng), status_counts = as.list(table(eng$status)),
    theta_engine_minus_R = lapply(th_names, function(p) c(list(name = p), smry(dth[[p]]),
      list(max_abs_rel = max(abs(drel[[p]])), median_abs_rel = median(abs(drel[[p]])), n_rel_above_1e_4 = cnt(drel[[p]], 1e-4), n_rel_above_1e_3 = cnt(drel[[p]], 1e-3),
           n_rel_above_R_tolerance = cnt(drel[[p]], tol_theta_rel)))),
    omega_engine_minus_R_rel = lapply(names(dom), function(p) c(list(name = p), smry(dom[[p]]), list(n_above_1e_3 = cnt(dom[[p]], 1e-3)))),
    A_engine_minus_R = c(smry(dA), list(n_above_1e_6 = cnt(dA, 1e-6))), B_engine_minus_R = smry(dB),
    ofv_R_at_engine_minus_R_at_R = c(smry(gap), list(R_tolerance = tol_ofv, n_above_R_tolerance = cnt(gap, tol_ofv), n_above_1e_8 = cnt(gap, 1e-8), n_above_1e_6 = cnt(gap, 1e-6), n_above_1e_4 = cnt(gap, 1e-4),
      n_above_1e_2 = cnt(gap, 1e-2), n_negative_below_minus_R_tolerance = sum(gap < -tol_ofv), min = min(gap), max = max(gap))),
    engine_reported_ofv_minus_R_ofv_at_engine_point = smry(eng_rep_minus_r_at),
    engine_reported_ofv_minus_R_ofv_at_R_point = smry(eng_minus_r_fit),
    r_tolerances = list(ofv = tol_ofv, theta_rel = tol_theta_rel),
    by_status = lapply(setNames(unique(eng$status), unique(eng$status)), function(s) { i <- eng$status == s
      list(n = sum(i), theta_cl_engine_minus_R_median = median(dth$cl[i]), theta_cl_engine_minus_R_max_abs = max(abs(dth$cl[i])), ofv_gap_max = max(gap[i]), ofv_gap_median = median(gap[i])) }))
  # the rule of the card
  engine_agrees <- cmp$ofv_R_at_engine_minus_R_at_R$n_above_R_tolerance == 0 &&
    all(vapply(cmp$theta_engine_minus_R, function(x) x$n_rel_above_1e_3 == 0, TRUE))
  r_biased <- any(vapply(aggs$R_focei, function(x) x$exceeds_limit, TRUE))
  verdict <- list(rule = "(a) FOCEI itself is biased: the engine agrees with the R fit replicate by replicate (no OFV gap above the R tolerance, theta within 1e-3 relative) and the R FOCEI aggregate exceeds the limit; (b) engine defect: the R fit finds a lower OFV or different theta; (c) undecided",
                  engine_agrees_with_R = engine_agrees, R_focei_exceeds_bias_limit = r_biased,
                  suggested = if (engine_agrees && r_biased) "a" else if (!engine_agrees) "b" else "c")
  say("engine vs R: OFV gap max %.3g median %.3g, count above R tolerance (%.2g) = %d; theta cl diff max %.3g", cmp$ofv_R_at_engine_minus_R_at_R$max_abs, cmp$ofv_R_at_engine_minus_R_at_R$median, tol_ofv, cmp$ofv_R_at_engine_minus_R_at_R$n_above_R_tolerance, cmp$theta_engine_minus_R[[1]]$max_abs)
}

out <- list(schema = 1L, kind = "pop_sealed_triage", card = "T-174", informational = TRUE, generated_by = "oracle/scripts/pop_sealed_triage.R",
  design = "accurate", replicates = length(REPS),
  versions = list(R = R.version.string, saemix = as.character(packageVersion("saemix")), jsonlite = as.character(packageVersion("jsonlite")),
                  RNGkind = paste(RNGkind(), collapse = "/"), saemix_seed = "632545 + replicate", workers = WORKERS),
  ofv_check_vs_oracle = list(tolerance_relative = 1e-8, cases = ofv_check, max_rel_error_total = max(ofv_check_df$rel_error_total), n_cases = nrow(ofv_check_df),
                             n_above_1e_8 = sum(ofv_check_df$rel_error_total > 1e-8)),
  focei_r = evidence,
  saemix = list(n_runs = nrow(dsx), n_with_estimate = sum(dsx$converged), wall_seconds = sx$wall_seconds, seconds_per_run = list(median = median(dsx$seconds), max = max(dsx$seconds), sum = sum(dsx$seconds))),
  aggregates = aggs, engine_comparison = cmp, verdict = verdict)
if (!DEV) {
  con <- file(file.path(OUTDIR, "sealed_accurate_triage.json"), open = "wb")
  writeLines(toJSON(out, auto_unbox = TRUE, pretty = TRUE, digits = NA, null = "null", na = "null"), con, sep = "\n", useBytes = TRUE); close(con)
  say("wrote %s", OUTDIR)
}
saveRDS(out, file.path(if (nzchar(CACHE)) CACHE else tempdir(), "triage_out.rds"))
try(stopCluster(cl), silent = TRUE)
say("done")
