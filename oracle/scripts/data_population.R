#!/usr/bin/env Rscript
# Caladrius population dataset oracle (task T-148, brick 2; open items OD-01 to OD-05 of specs/data.md).
#
# What this script does, in order:
#   1. Exports Theoph (the `datasets` package, 12 subjects, 132 rows, the columns Subject, Wt, Dose,
#      Time, conc, Dose in mg/kg as R documents it) to oracle/data/population/theoph_nlme.csv.
#   2. Reads the definitions in oracle/data/population/cases.json: for each file, the roles and units
#      the specification gives to its columns (specs/data.md DAT-REC-01), the options of the import,
#      the notes and the check codes expected, and, for a file that must be refused, the code, row,
#      subject, column and words of the sentence (DAT-ERR-01 to 06).
#   3. Computes every data fact INDEPENDENTLY of Caladrius with base R (read.table on character
#      columns, then the rules of specs/data.md applied here in R): subjects in order of first
#      appearance, the doses of each subject (bolus, or infusion with rate and duration from
#      duration = amount / rate), the observations (value, status observed / missing / blq), the counts
#      n_missing and n_blq, the covariates (one value per subject), the time order and the duplicated
#      times (the checks of the reading).
#   4. Cross-checks the subject list, the group sizes and the constancy of the covariates with
#      nlme::groupedData, nlme::getGroups and nlme::gsummary(invariantsOnly = TRUE) (OD-04), and for
#      Theoph with the groupedData object of the datasets package itself. A disagreement stops the
#      script: no expected file is written from facts that two implementations do not share.
#   5. For a refusal, finds in R the row that carries the problem and stops if it is not the row the
#      definition claims.
#   6. Writes oracle/expected/data/<case>.json (deterministic: no timestamps) and the list of cases
#      crates/caladrius-project/tests/oracle_data.cases for the test file.
#
# The expected `notes_exact` of a legacy file (DAT-LEG-02, OD-03) is not computed here: it is the notes
# list that the importer of the commit before the brick gave, pinned by the tests in
# crates/caladrius-project/tests/oracle_data.rs through oracle/expected/data/legacy_notes.json.
#
# Usage, from the repository root:   Rscript oracle/scripts/data_population.R

suppressPackageStartupMessages({
  library(jsonlite)
  library(nlme)
})

args_all <- commandArgs(trailingOnly = FALSE)
file_arg <- sub("^--file=", "", args_all[grepl("^--file=", args_all)])
root <- if (length(file_arg) == 1L) {
  normalizePath(file.path(dirname(file_arg), "..", ".."), winslash = "/", mustWork = TRUE)
} else {
  normalizePath(".", winslash = "/", mustWork = TRUE)
}
data_dir <- file.path(root, "oracle", "data")
out_dir <- file.path(root, "oracle", "expected", "data")
cases_file <- file.path(root, "crates", "caladrius-project", "tests", "oracle_data.cases")
dir.create(out_dir, recursive = TRUE, showWarnings = FALSE)
dir.create(dirname(cases_file), recursive = TRUE, showWarnings = FALSE)

write_lines_lf <- function(lines, path) {
  con <- file(path, open = "wb")
  on.exit(close(con))
  writeLines(lines, con, sep = "\n", useBytes = TRUE)
}
fail <- function(...) stop(paste0(...), call. = FALSE)

# ---- 1. Theoph, as R documents it ------------------------------------------------------------
th <- datasets::Theoph
theoph_df <- data.frame(
  Subject = as.character(th$Subject), Wt = th$Wt, Dose = th$Dose, Time = th$Time, conc = th$conc,
  stringsAsFactors = FALSE
)
g15 <- function(x) sprintf("%.15g", x)
write_lines_lf(c(
  "Subject,Wt,Dose,Time,conc",
  paste(theoph_df$Subject, g15(theoph_df$Wt), g15(theoph_df$Dose), g15(theoph_df$Time),
        g15(theoph_df$conc), sep = ",")
), file.path(data_dir, "population", "theoph_nlme.csv"))

# The same data as oracle/data/theoph.csv and indometh.csv in the event layout (OD-05): one dose row at
# time 0 (the amount of the profile file, text copied unchanged), then the observations of the profile
# file in order. `event_theoph_dose0.csv` is the Theoph table with the dose of subject 5 set to 0.
to_event_layout <- function(profile_file, with_wt) {
  p <- read.csv(file.path(data_dir, profile_file), colClasses = "character", check.names = FALSE)
  wt <- if (with_wt) tapply(g15(theoph_df$Wt), theoph_df$Subject, function(x) x[1L]) else NULL
  lines <- c(if (with_wt) "ID,TIME,DV,AMT,EVID,WT" else "ID,TIME,DV,AMT,EVID")
  for (s in unique(p$subject)) {
    rows <- p[p$subject == s, ]
    w <- if (with_wt) paste0(",", wt[[s]]) else ""
    lines <- c(lines, paste0(s, ",0,,", rows$dose[1L], ",1", w))
    lines <- c(lines, paste0(s, ",", rows$time, ",", rows$conc, ",,0", w))
  }
  lines
}
ev_theoph <- to_event_layout("theoph.csv", TRUE)
write_lines_lf(ev_theoph, file.path(data_dir, "population", "event_theoph.csv"))
write_lines_lf(to_event_layout("indometh.csv", FALSE), file.path(data_dir, "population", "event_indometh.csv"))
zero <- ev_theoph
zero[grepl("^5,0,,", zero)] <- sub("^5,0,,[^,]*,1", "5,0,,0,1", zero[grepl("^5,0,,", zero)])
write_lines_lf(zero, file.path(data_dir, "population", "event_theoph_dose0.csv"))

# ---- 2/3. reading a file and applying the rules of specs/data.md ---------------------------------
BLQ_WORDS <- c("BLQ", "BQL", "BLOQ", "<LOQ", "<LLOQ", "<QL", "<LQ", "<LIQ", "NQ")
MISSING_WORDS <- c("", "na", "n/a", "nan", ".", "-", "null")

read_cells <- function(path, sep) {
  read.table(path, header = TRUE, sep = sep, quote = "\"", colClasses = "character",
             na.strings = character(0), check.names = FALSE, comment.char = "",
             stringsAsFactors = FALSE, strip.white = TRUE, fill = FALSE, blank.lines.skip = TRUE)
}
to_num <- function(x, dec_comma) {
  x <- trimws(x)
  if (dec_comma) x <- ifelse(grepl("\\.", x), x, sub(",", ".", x, fixed = TRUE))
  suppressWarnings(as.numeric(x))
}
is_blank <- function(x) trimws(x) == ""
is_missing_cell <- function(x) tolower(trimws(x)) %in% MISSING_WORDS
# value cell: list(kind = missing | number | blq, value, limit)
value_cell <- function(txt, dec_comma) {
  t <- trimws(txt)
  if (is_missing_cell(t)) return(list(kind = "missing", value = NA_real_, limit = NA_real_))
  x <- to_num(t, dec_comma)
  if (!is.na(x) && is.finite(x)) return(list(kind = "number", value = x, limit = NA_real_))
  compact <- toupper(gsub("[[:space:]]", "", t))
  if (compact %in% BLQ_WORDS) return(list(kind = "blq", value = 0, limit = NA_real_))
  if (startsWith(compact, "<")) {
    lim <- to_num(substring(compact, 2L), dec_comma)
    if (!is.na(lim) && lim >= 0) return(list(kind = "blq", value = 0, limit = lim))
  }
  fail("cannot classify the value cell `", txt, "`")
}
parse_cols <- function(spec) {
  parts <- strsplit(spec, "|", fixed = TRUE)[[1]]
  do.call(rbind, lapply(parts, function(p) {
    at <- regexpr(":[^:]*$", p)
    header <- substr(p, 1L, at - 1L)
    role <- substring(p, at + 1L)
    m <- regmatches(header, regexec("^(.*?)\\s*[\\(\\[]([^\\)\\]]+)[\\)\\]]$", header, perl = TRUE))[[1]]
    if (length(m) == 3L) {
      data.frame(header = header, name = trimws(m[2]), unit = m[3], role = role, stringsAsFactors = FALSE)
    } else {
      data.frame(header = header, name = header, unit = NA_character_, role = role, stringsAsFactors = FALSE)
    }
  }))
}
col_of <- function(cols, role) {
  i <- which(cols$role == role)
  if (length(i) == 0L) NULL else cols$header[i[1L]]
}

# Reads the file of a case; returns the per-row records and the facts of every subject.
analyse <- function(def, strict = TRUE) {
  path <- file.path(data_dir, def$file)
  dec <- isTRUE(def$decimal_comma)
  cols <- if (!is.null(def$cols)) parse_cols(def$cols) else NULL
  df <- read_cells(path, def$delimiter %||% ",")
  if (!is.null(cols) && !identical(cols$header, names(df))) {
    fail(def$case, ": the header ", paste(names(df), collapse = "|"), " is not the one defined ",
         paste(cols$header, collapse = "|"))
  }
  n <- nrow(df)
  get <- function(role) { h <- col_of(cols, role); if (is.null(h)) NULL else df[[h]] }
  subj <- get("subject"); tm <- get("time"); val <- get("conc"); amt <- get("amt"); evt <- get("event")
  rate <- get("rate"); dur <- get("dur"); cmt <- get("cmt"); mdv <- get("mdv")
  cens <- get("cens"); lim <- get("limit")
  layout <- if (is.null(evt)) "profile" else "event"
  label <- if (is.null(subj)) rep("1", n) else trimws(subj)
  cov_cols <- cols$header[cols$role == "covariate"]
  rec <- vector("list", n)
  for (i in seq_len(n)) {
    r <- list(row = i, subject = label[i], time = if (is.null(tm)) NA_real_ else to_num(tm[i], dec),
              kind = "observation", status = NA_character_, value = NA_real_, limit = NA_real_,
              amount = NA_real_, rate = NA_real_, duration = NA_real_)
    kind <- "observation"
    if (layout == "event") {
      e <- tolower(trimws(evt[i]))
      kind <- if (e %in% c("0", "obs", "observation")) "observation"
              else if (e %in% c("1", "dose")) "dose" else "other"
    }
    r$kind <- kind
    if (kind == "dose") {
      a <- if (is.null(amt)) NA_real_ else to_num(amt[i], dec)
      rt <- if (is.null(rate)) NA_real_ else to_num(rate[i], dec)
      du <- if (is.null(dur)) NA_real_ else to_num(dur[i], dec)
      rt0 <- if (is.na(rt)) 0 else rt
      du0 <- if (is.na(du)) 0 else du
      r$amount <- a
      if (!is.na(a) && rt0 > 0 && du0 > 0) {
        if (abs(rt0 * du0 - a) > 1e-6 * a && strict) fail(def$case, ": row ", i, " has an inconsistent rate and duration")
        r$rate <- rt0; r$duration <- du0
      } else if (!is.na(a) && rt0 > 0) {
        r$rate <- rt0; r$duration <- a / rt0
      } else if (!is.na(a) && du0 > 0) {
        r$duration <- du0; r$rate <- a / du0
      }
    } else if (kind == "observation") {
      vc <- if (is.null(val)) list(kind = "missing", value = NA_real_, limit = NA_real_) else value_cell(val[i], dec)
      status <- switch(vc$kind, number = "observed", missing = "missing", blq = "blq")
      value <- vc$value; limit <- vc$limit
      if (!is.null(cens) && !is_blank(cens[i]) && to_num(cens[i], FALSE) == 1 && !is.na(vc$value) && vc$kind == "number") {
        status <- "blq"; limit <- vc$value; value <- 0
      }
      if (!is.null(mdv) && !is_blank(mdv[i]) && to_num(mdv[i], FALSE) == 1) status <- "missing"
      r$status <- status; r$value <- value; r$limit <- limit
    }
    rec[[i]] <- r
  }
  list(def = def, df = df, cols = cols, layout = layout, label = label, rec = rec, dec = dec,
       cov_cols = cov_cols, get = get)
}
strip_unit <- function(h) sub("\\s*[\\(\\[].*$", "", h, perl = TRUE)
`%||%` <- function(a, b) if (is.null(a)) b else a

subjects_of <- function(a) { s <- a$label; unique(s[s != ""]) }

# the facts of every subject (the expected `records`)
facts_of <- function(a) {
  recs <- a$rec
  subs <- subjects_of(a)
  out <- list()
  dose_col <- col_of(a$cols, "amt")
  for (s in subs) {
    mine <- Filter(function(r) r$subject == s, recs)
    doses <- list(); obs <- list()
    if (a$layout == "event") {
      for (r in mine) {
        if (r$kind == "dose") {
          doses[[length(doses) + 1L]] <- list(row = r$row, time = r$time, amount = r$amount,
            rate = if (is.na(r$rate)) NULL else r$rate,
            duration = if (is.na(r$duration)) NULL else r$duration,
            kind = if (is.na(r$rate)) "bolus" else "infusion")
        }
      }
    } else if (!is.null(dose_col)) {
      d <- unique(to_num(a$df[[dose_col]][a$label == s], a$dec))
      d <- d[!is.na(d)]
      if (length(d) > 1L) fail(a$def$case, ": subject ", s, " has several doses")
      if (length(d) == 1L) doses[[1L]] <- list(row = NULL, time = 0, amount = d, rate = NULL, duration = NULL, kind = "bolus")
    }
    for (r in mine) {
      if (r$kind == "observation") {
        obs[[length(obs) + 1L]] <- list(row = r$row, time = r$time, status = r$status,
          value = if (r$status == "observed") r$value else NULL)
      }
    }
    cov <- setNames(list(), character(0))
    for (h in a$cov_cols) {
      raw <- a$df[[h]][a$label == s]
      raw <- raw[!is_blank(raw)]
      num <- suppressWarnings(to_num(raw, a$dec))
      u <- if (!anyNA(num)) unique(num) else unique(raw)
      if (length(u) > 1L) fail(a$def$case, ": covariate ", h, " varies in subject ", s)
      cov[[strip_unit(h)]] <- if (length(u) == 1L) u else NULL
    }
    st <- vapply(obs, function(o) o$status, "")
    out[[s]] <- list(n_rows = length(mine), doses = doses, observations = obs,
                     n_obs = length(obs), n_observed = sum(st == "observed"),
                     n_missing = sum(st == "missing"), n_blq = sum(st == "blq"), covariates = cov)
  }
  out
}

# the checks of a reading (time order and duplicates), per specs/data.md DAT-ORD-01
order_checks <- function(a) {
  codes <- character(0)
  for (s in subjects_of(a)) {
    mine <- Filter(function(r) r$subject == s && !is.na(r$time), a$rec)
    tms <- vapply(mine, function(r) r$time, 0)
    if (length(tms) > 1L && any(diff(tms) < 0)) codes <- union(codes, "time_not_increasing")
    ob <- Filter(function(r) r$kind == "observation", mine)
    to <- vapply(ob, function(r) r$time, 0)
    if (length(to) > 1L && any(diff(to) == 0)) codes <- union(codes, "duplicate_times")
  }
  as.list(sort(codes))
}

# ---- 4. nlme cross-check -----------------------------------------------------------------------
nlme_check <- function(a, facts) {
  ob <- Filter(function(r) r$kind == "observation" && r$subject != "", a$rec)
  subs <- subjects_of(a)
  d <- data.frame(id = factor(vapply(ob, function(r) r$subject, ""), levels = subs),
                  time = vapply(ob, function(r) r$time, 0),
                  y = vapply(ob, function(r) if (is.na(r$value)) NA_real_ else r$value, 0))
  for (h in a$cov_cols) {
    raw <- a$df[[h]][vapply(ob, function(r) r$row, 0L)]
    num <- suppressWarnings(to_num(raw, a$dec))
    d[[h]] <- if (!anyNA(num)) num else raw
  }
  gd <- nlme::groupedData(y ~ time | id, data = d, order.groups = FALSE)
  g <- nlme::getGroups(gd)
  if (!identical(levels(g), subs)) fail(a$def$case, ": nlme::getGroups lists ", paste(levels(g), collapse = ","),
                                       " but the facts have ", paste(subs, collapse = ","))
  sizes <- as.integer(table(g)[subs])
  mine <- vapply(facts[subs], function(f) f$n_obs, 0L)
  if (!identical(sizes, as.integer(mine))) fail(a$def$case, ": nlme group sizes ", paste(sizes, collapse = ","),
                                               " differ from ", paste(mine, collapse = ","))
  inv <- names(nlme::gsummary(gd, invariantsOnly = TRUE))
  for (h in a$cov_cols) {
    if (!(h %in% inv)) fail(a$def$case, ": nlme says covariate ", h, " is not constant within the groups")
  }
  list(subjects = as.list(levels(g)), group_sizes = as.list(sizes),
       invariant_columns = as.list(setdiff(inv, "id")))
}
theoph_check <- function(a, facts) {
  g <- nlme::getGroups(datasets::Theoph)
  if (!setequal(as.character(levels(g)), names(facts))) fail("Theoph: the groups of the datasets object differ from the file")
  sizes <- table(g)[names(facts)]
  if (!identical(as.integer(sizes), as.integer(vapply(facts, function(f) f$n_obs, 0L)))) fail("Theoph: group sizes differ")
  inv <- names(nlme::gsummary(datasets::Theoph, invariantsOnly = TRUE))
  list(invariant_columns_of_the_datasets_object = as.list(setdiff(inv, "Subject")))
}

# ---- 5. refusals: find the problem in R ----------------------------------------------------------
find_problem <- function(a, code) {
  recs <- a$rec; get <- a$get; dec <- a$dec
  rows_where <- function(pred) which(vapply(recs, pred, NA))
  first_row <- function(idx) if (length(idx) == 0L) NULL else idx[1L]
  num_cell <- function(v, i) if (is.null(v)) NA_real_ else to_num(v[i], dec)
  evt <- get("event")
  switch(code,
    time_not_increasing = {
      for (s in subjects_of(a)) {
        mine <- Filter(function(r) r$subject == s && !is.na(r$time), recs)
        for (k in seq_along(mine)[-1L]) if (mine[[k]]$time < mine[[k - 1L]]$time) return(list(row = mine[[k]]$row, subject = s))
      }
      NULL },
    dose_without_amount = {
      i <- first_row(rows_where(function(r) r$kind == "dose" && is.na(r$amount)))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    amount_on_observation = {
      amt <- get("amt")
      i <- first_row(rows_where(function(r) r$kind == "observation" && !is_blank(amt[r$row])))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    subject_without_observation = {
      for (s in subjects_of(a)) {
        if (!any(vapply(recs, function(r) r$subject == s && r$kind == "observation", NA))) return(list(row = NULL, subject = s))
      }
      NULL },
    covariate_varies = {
      for (h in a$cov_cols) for (s in subjects_of(a)) {
        idx <- which(a$label == s & !is_blank(a$df[[h]]))
        v <- a$df[[h]][idx]
        if (length(unique(v)) > 1L) {
          k <- which(v != v[1L])[1L]
          return(list(row = idx[k], subject = s, first_row = idx[1L], first_value = v[1L], other_value = v[k]))
        }
      }
      NULL },
    unsupported_event = {
      rate <- get("rate")
      i <- first_row(rows_where(function(r) {
        e <- tolower(trimws(evt[r$row]))
        e %in% c("2", "3", "4") || (!is.null(rate) && !is_blank(rate[r$row]) && num_cell(rate, r$row) < 0)
      }))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    unknown_event = {
      i <- first_row(rows_where(function(r) !(tolower(trimws(evt[r$row])) %in% c("0", "obs", "observation", "1", "dose", "2", "3", "4"))))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    rate_duration_mismatch = {
      rate <- get("rate"); dur <- get("dur"); amt <- get("amt")
      i <- first_row(rows_where(function(r) {
        rt <- num_cell(rate, r$row); du <- num_cell(dur, r$row); a <- num_cell(amt, r$row)
        r$kind == "dose" && !is.na(rt) && !is.na(du) && rt > 0 && du > 0 && abs(rt * du - a) > 1e-6 * a
      }))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    non_positive_amount = {
      i <- first_row(rows_where(function(r) r$kind == "dose" && !is.na(r$amount) && r$amount <= 0))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    several_endpoints = {
      cmt <- get("cmt")
      for (s in subjects_of(a)) {
        v <- unique(cmt[vapply(recs, function(r) r$subject == s && r$kind == "observation", NA)])
        if (length(v) > 1L) return(list(row = NULL, subject = s, values = v))
      }
      NULL },
    unsupported_censoring = {
      cens <- get("cens"); val <- get("conc")
      i <- first_row(rows_where(function(r) {
        if (r$kind != "observation" || is_blank(cens[r$row])) return(FALSE)
        c <- to_num(cens[r$row], FALSE)
        !(c %in% c(0, 1)) || (c == 1 && is_missing_cell(val[r$row]))
      }))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    unsupported_history = {
      cols <- intersect(c("addl", "ii", "ss"), tolower(names(a$df)))
      for (cname in cols) {
        h <- names(a$df)[tolower(names(a$df)) == cname]
        i <- first_row(rows_where(function(r) r$kind == "dose" && !is_blank(a$df[[h]][r$row]) && to_num(a$df[[h]][r$row], FALSE) != 0))
        if (!is.null(i)) return(list(row = i, subject = recs[[i]]$subject, column = h))
      }
      NULL },
    empty_subject = {
      i <- first_row(which(a$label == ""))
      if (is.null(i)) NULL else list(row = i, subject = NULL) },
    unknown_mdv = {
      mdv <- get("mdv")
      i <- first_row(which(!is_blank(mdv) & !(trimws(mdv) %in% c("0", "1"))))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    missing_time = {
      tm <- get("time")
      i <- first_row(which(is_blank(tm)))
      if (is.null(i)) NULL else list(row = i, subject = recs[[i]]$subject) },
    different_doses = {
      dc <- col_of(a$cols, "amt")
      for (s in subjects_of(a)) {
        d <- unique(to_num(a$df[[dc]][a$label == s], dec))
        if (length(d) > 1L) return(list(row = NULL, subject = s, values = d))
      }
      NULL },
    fail("no R finder for the code ", code))
}

# ---- 6. run -------------------------------------------------------------------------------------
defs <- fromJSON(file.path(data_dir, "population", "cases.json"), simplifyVector = FALSE)
meta <- list(r_version = R.version.string, nlme = as.character(packageVersion("nlme")),
             jsonlite = as.character(packageVersion("jsonlite")))
as_array <- function(x) if (length(x) == 0L) list() else as.list(x)

load_names <- character(0); refusal_names <- character(0); all_facts <- list()
for (def in defs$loads) {
  a <- analyse(def)
  if (a$layout != def$layout) fail(def$case, ": layout ", a$layout, " but ", def$layout, " defined")
  facts <- facts_of(a)
  computed_checks <- order_checks(a)
  if (!is.null(def$checks) && !identical(unlist(computed_checks), unlist(def$checks))) {
    fail(def$case, ": R finds the checks [", paste(unlist(computed_checks), collapse = ","), "] but [",
         paste(unlist(def$checks), collapse = ","), "] are defined")
  }
  ref <- nlme_check(a, facts)
  if (def$case %in% c("theoph_nlme", "theoph_nlme_wt")) ref <- c(ref, theoph_check(a, facts))
  columns <- lapply(seq_len(nrow(a$cols)), function(i) list(
    name = a$cols$name[i], unit = if (is.na(a$cols$unit[i])) NULL else a$cols$unit[i], role = a$cols$role[i]))
  out <- list(schema = 1L, kind = "data_load", case = def$case, file = def$file, od = def$od,
              rules = as_array(def$rules), generated_by = "oracle/scripts/data_population.R",
              options = def$options, delimiter = def$delimiter, decimal_comma = isTRUE(def$decimal_comma),
              layout = a$layout, legacy = isTRUE(def$legacy), same_data_as = def$same_data_as, columns = columns,
              notes_contain = as_array(def$notes_contain), checks = computed_checks,
              declared_covariates = as_array(unname(strip_unit(a$cov_cols))),
              subjects = as_array(subjects_of(a)), n_rows = nrow(a$df), records = facts,
              reference = c(list(software = meta), ref))
  write_lines_lf(toJSON(out, auto_unbox = TRUE, pretty = TRUE, digits = NA, null = "null", na = "null"),
                 file.path(out_dir, paste0(def$case, ".json")))
  load_names <- c(load_names, def$case)
  all_facts[[def$case]] <- facts
  cat(sprintf("%-34s %-8s %3d subjects %4d rows\n", def$case, a$layout, length(subjects_of(a)), nrow(a$df)))
}

# OD-05: the event layout of Theoph and Indometh holds exactly the data of the profile files
for (def in defs$loads) {
  if (is.null(def$same_data_as)) next
  x <- all_facts[[def$case]]; y <- all_facts[[def$same_data_as]]
  if (!identical(names(x), names(y))) fail(def$case, ": the subjects differ from ", def$same_data_as)
  for (s in names(x)) {
    pick <- function(f, k) vapply(f$observations, function(o) as.numeric(o[[k]] %||% NA_real_), 0)
    if (!identical(pick(x[[s]], "time"), pick(y[[s]], "time")) || !identical(pick(x[[s]], "value"), pick(y[[s]], "value")) ||
        !identical(x[[s]]$doses[[1L]]$amount, y[[s]]$doses[[1L]]$amount)) {
      fail(def$case, ": subject ", s, " differs from ", def$same_data_as)
    }
  }
  cat(sprintf("%-34s same data as %s\n", def$case, def$same_data_as))
}

for (def in defs$refusals) {
  def$delimiter <- ","; def$decimal_comma <- FALSE
  # the roles of the refusal files follow the same key rule: read the header and guess them here
  hdr <- names(read_cells(file.path(data_dir, def$file), ","))
  key <- function(h) gsub("[^a-z0-9]", "", tolower(strip_unit(h)))
  role_of_key <- function(k) {
    if (grepl("^time", k) || k %in% c("t", "hours", "hour", "hr", "h")) "time"
    else if (grepl("^conc", k) || k %in% c("c", "cp", "dv", "cobs")) "conc"
    else if (k %in% c("subject", "subj", "id", "subjectid", "patient", "animal", "individual")) "subject"
    else if (grepl("^dose", k) || k %in% c("amt", "amount")) "amt"
    else if (k %in% c("evid", "event", "eventtype", "evt")) "event"
    else if (k %in% c("cmt", "compartment", "cmpt")) "cmt"
    else if (k %in% c("rate", "infusionrate")) "rate"
    else if (k %in% c("dur", "duration", "tinf")) "dur"
    else if (k %in% c("mdv", "missing")) "mdv"
    else if (k %in% c("cens", "censored", "censoring")) "cens"
    else if (k %in% c("limit", "lloq", "loq")) "limit"
    else "other"
  }
  roles <- vapply(hdr, function(h) role_of_key(key(h)), "")
  declared <- unlist(def$options$covariates)
  roles[hdr %in% declared] <- "covariate"
  def$cols <- paste(paste0(hdr, ":", roles), collapse = "|")
  a <- analyse(def, strict = FALSE)
  subs <- subjects_of(a)
  # the profile layout has no event column: different_doses is read from the dose column
  found <- find_problem(a, def$code)
  if (is.null(found)) fail(def$case, ": R finds no ", def$code)
  claimed_row <- def$row
  if (!identical(found$row, claimed_row)) fail(def$case, ": R finds ", def$code, " at row ", format(found$row),
                                                " but the definition says ", format(claimed_row))
  if (!identical(found$subject, def$subject)) fail(def$case, ": R finds subject ", format(found$subject),
                                                    " but the definition says ", format(def$subject))
  affected <- unlist(def$affected)
  unaffected <- setdiff(subs, affected)
  # sanity: the rows of the unaffected subjects are free of the problem
  out <- list(schema = 1L, kind = "data_refusal", case = def$case, file = def$file, od = def$od,
              rules = as_array(def$rules), generated_by = "oracle/scripts/data_population.R",
              options = def$options, delimiter = ",", decimal_comma = FALSE,
              layout = a$layout, columns = as.list(unname(vapply(seq_along(hdr), function(i)
                paste0(hdr[i], ":", roles[i]), ""))),
              error = list(code = def$code, row = def$row, subject = def$subject, column = def$column,
                           words = as_array(def$words)),
              subjects = as_array(subs), affected = as_array(affected), unaffected = as_array(unaffected),
              n_rows = nrow(a$df),
              reference = list(software = meta, problem_found_in_r = found[c("row", "subject")]))
  write_lines_lf(toJSON(out, auto_unbox = TRUE, pretty = TRUE, digits = NA, null = "null", na = "null"),
                 file.path(out_dir, paste0(def$case, ".json")))
  refusal_names <- c(refusal_names, def$case)
  cat(sprintf("%-34s %-26s row %-4s subject %s\n", def$case, def$code, format(found$row), format(found$subject)))
}

# ---- the list of cases for the test file ---------------------------------------------------------
load_names <- sort(load_names, method = "radix"); refusal_names <- sort(refusal_names, method = "radix")
fmt <- function(v) paste0("    ", paste(v, collapse = ", "), ",\n")
write_lines_lf(c(
  "// Generated by oracle/scripts/data_population.R; do not edit by hand.",
  "const LOAD_CASES: &[&str] = &[",
  paste0("    \"", load_names, "\","),
  "];",
  "const REFUSAL_CASES: &[&str] = &[",
  paste0("    \"", refusal_names, "\","),
  "];",
  "load_tests!(",
  paste0("    ", load_names, ","),
  ");",
  "refusal_tests!(",
  paste0("    ", refusal_names, ","),
  ");"
), cases_file)
cat("cases:", length(load_names), "loads,", length(refusal_names), "refusals\n")
