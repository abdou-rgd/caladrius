# The collapse property of the maximum-likelihood references (FIT-ML-13c, FIT-ML-15), checked in R.
#
# A combined or combined_sd fit that ends with one error component at 0 is the one-parameter fit of
# the other component, on THE SAME ROWS: same -2LL, same theta, same remaining component, and an
# AIC_ML larger by 2 (P + Q counts both error parameters). This function reads the files that
# fit_ml.R has written (oracle/expected/fit/ml/<case>.csv and .options.json) and, for every subject of
# every combined case without a user bound that has one component on its bound, finds the
# one-parameter case with the same dataset, model and `observations` rule (and no bound and no
# error_init), then compares at the tolerances of the Rust test (-2LL: absolute 1e-7; estimates: relative to the
# one-parameter value, as Tolerance::accepts(actual, expected) does, with expected = the one-parameter fit;
# rel 1e-4, 1e-6 for the F data, abs 1e-12 for an expected 0). It stops on a mismatch, or when there is no such one-parameter case (or several).
#
# Sourced at the end of fit_ml.R, or run by itself:  Rscript oracle/scripts/ml_collapse_check.R
# Setting COLLAPSE_PARTNER_BY_NAME=1 pairs by the case name only (ml_<data>_<one-parameter model>),
# which is how the first version of the test paired them; it was used once to show that the check
# catches the Theoph mismatch (10 rows against 11).

collapse_check <- function(exp_dir, by_name = FALSE, m2ll_abs = 1e-7, est_abs = 1e-12) {
  suppressPackageStartupMessages(library(jsonlite))
  files <- list.files(exp_dir, pattern = "[.]options[.]json$", full.names = TRUE)
  opts <- lapply(files, function(f) fromJSON(f, simplifyVector = FALSE))
  names(opts) <- vapply(opts, function(o) o$case, "")
  table_of <- function(case) {
    t <- read.csv(file.path(exp_dir, paste0(case, ".csv")), stringsAsFactors = FALSE, colClasses = "character")
    setNames(as.numeric(t$value), paste(t$subject, t$parameter, sep = "|"))
  }
  tables <- list()
  val <- function(case, s, p) {
    if (is.null(tables[[case]])) tables[[case]] <<- table_of(case)
    v <- tables[[case]][paste(s, p, sep = "|")]
    if (is.na(v)) stop(case, " subject ", s, ": no value ", p, call. = FALSE)
    unname(v)
  }
  fail <- function(...) stop(paste0(...), call. = FALSE)
  checked <- 0L
  for (o in opts) {
    if (!(o$error_model %in% c("combined", "combined_sd")) || !is.null(o$bounds)) next
    for (s in unlist(o$subjects)) {
      bound <- unlist(o$at_bound[[s]])
      if (length(bound) == 0L) next
      if (length(bound) != 1L) fail(o$case, " subject ", s, ": ", length(bound), " components on the bound")
      alone <- if (bound == "error_a") "proportional" else "additive"
      other <- if (bound == "error_a") "error_b" else "error_a"
      same_rows <- function(q) identical(q$observations, o$observations)
      if (by_name) {
        cand <- opts[names(opts) == paste0("ml_", o$dataset, "_", alone)]
      } else {
        cand <- Filter(function(q) q$error_model == alone && q$dataset == o$dataset && q$model == o$model &&
                         is.null(q$bounds) && is.null(q$error_init) && same_rows(q) &&
                         s %in% unlist(q$subjects), opts)
      }
      if (length(cand) == 0L) fail(o$case, " subject ", s, ": no ", alone, " case on the same rows (",
                                   toJSON(o$observations, auto_unbox = TRUE), ") to compare the collapse with")
      if (length(cand) > 1L) fail(o$case, " subject ", s, ": several ", alone, " cases on the same rows: ",
                                  paste(names(cand), collapse = ", "))
      q <- cand[[1]]
      d <- val(o$case, s, "m2ll") - val(q$case, s, "m2ll")
      msg_rows <- if (same_rows(q)) "" else paste0(" (the rows differ: ", toJSON(o$observations, auto_unbox = TRUE),
                                                    " against ", toJSON(q$observations, auto_unbox = TRUE), ")")
      if (abs(d) > m2ll_abs)
        fail(o$case, " subject ", s, ": -2LL ", format(val(o$case, s, "m2ll"), digits = 15), " against ", q$case, " ",
             format(val(q$case, s, "m2ll"), digits = 15), msg_rows)
      pars <- c(setdiff(names(o$initial_estimates[[s]]), character(0)), if (alone == "additive") "error_a" else "error_b")
      for (p in pars) {
        a <- val(o$case, s, paste0("estimate.", p)); b <- val(q$case, s, paste0("estimate.", p))
        # the rule of the Rust test (Tolerance::accepts(actual = combined, expected = one-parameter)):
        # |a - b| <= rel * |b|, and |a| <= abs when b is exactly 0; rel is 1e-4 (FIT_PARAMETERS), 1e-6 on
        # the five-point F data (FIT_PARAMETERS_SMALL_CASE), abs is 1e-12
        est_rel <- if (identical(o$dataset, "f")) 1e-6 else 1e-4
        if (!(if (b == 0) abs(a) <= est_abs else abs(a - b) <= est_rel * abs(b)))
          fail(o$case, " subject ", s, ": estimate.", p, " ", format(a, digits = 15), " against ", q$case, " ",
               format(b, digits = 15), msg_rows)
      }
      if (val(o$case, s, paste0("estimate.", bound)) != 0) fail(o$case, " subject ", s, ": ", bound, " is not 0")
      if (abs(val(o$case, s, "aic_ml") - val(q$case, s, "aic_ml") - 2) > m2ll_abs)
        fail(o$case, " subject ", s, ": AIC_ML is not larger by 2 than in ", q$case, msg_rows)
      if (val(o$case, s, "n") != val(q$case, s, "n")) fail(o$case, " subject ", s, ": n differs from ", q$case)
      checked <- checked + 1L
    }
  }
  if (checked == 0L) fail("collapse check: no collapsed subject found")
  cat(sprintf("collapse check: %d collapsed subjects agree with their one-parameter fit on the same rows\n", checked))
  invisible(checked)
}

if (!exists("COLLAPSE_CHECK_SOURCED")) {
  args_all <- commandArgs(trailingOnly = FALSE)
  file_arg <- sub("^--file=", "", args_all[grepl("^--file=", args_all)])
  if (length(file_arg) == 1L && grepl("ml_collapse_check[.]R$", file_arg)) {
    root <- normalizePath(file.path(dirname(file_arg), "..", ".."), winslash = "/", mustWork = TRUE)
    collapse_check(file.path(root, "oracle", "expected", "fit", "ml"),
                   by_name = nzchar(Sys.getenv("COLLAPSE_PARTNER_BY_NAME")))
  }
}
