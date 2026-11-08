# Caladrius population-simulation oracle, card T-158 (specs/simulation.md SIM-ORA-01 to 06).
#
# Runs every stage and writes oracle/expected/sim/:
#   gen/        Philox known answers and the blocks of SX1; words, uniforms and normals of streams  (SIM-ORA-01)
#   transform/  AS 241 against a 256-bit inverse at the required grid points                          (SIM-ORA-02)
#   exact/      the exact part with the draws given, 22 cases                                          (SIM-ORA-03)
#   study/      whole studies: tables, summary, dataset, counts                                       (SIM-ORA-03, -05)
#   dist/       the distribution of the draws with stated bounds and recorded statistics              (SIM-ORA-04)
#   quantile/   the quantile of type 7                                                                 (SIM-SUM-01)
#   invariance/ pairs of requests whose answers agree or differ                                        (SIM-ORA-05)
#   roundtrip/  simulate-then-fit requests, waiting for T-154                                          (SIM-ORA-06)
#   errors.json the refusals with their controls                                                       (SIM-REF-*)
#   manifest.json  SHA-256 of every file, the combined hash and the versions
#
# The script stops at once if the three known answers of Philox or the words of SX1 do not match.
# Every number that the engine must match bit for bit is written as a hexadecimal float; decimals are written
# only where Rust reads them correctly rounded (30 or 50 digits) and never as an exact expectation.
#
# The 64-bit label-key collision (SIM-REF, label_key_collision) was found by a search that took about 1.2e10
# evaluations of SHA-256; the pair is fixed here and checked below.
#
# Run: Rscript oracle/scripts/sim_oracle.R      (about a minute; byte-identical output at every run)
suppressMessages({ library(jsonlite); library(Rmpfr); library(gmp); library(digest) })
src <- sub("^--file=", "", grep("^--file=", commandArgs(FALSE), value = TRUE)[1])
ROOT <- normalizePath(file.path(dirname(src), "..", ".."), winslash = "/", mustWork = TRUE)
setwd(ROOT)
for (f in c("sim_lib", "sim_gen", "sim_transform", "sim_exact", "sim_study", "sim_dist", "sim_errors", "sim_extras"))
  source(file.path("oracle", "scripts", paste0(f, ".R")))

OUT <- file.path(ROOT, "oracle", "expected", "sim")
COLLISION <- c("abe6fa9f1cdab0fa", "3df62dbc87d8de8f")   # 64-bit SHA-256 prefixes of two labels (see sim_errors.R)

# Start from nothing so that a removed case does not leave a stale file behind.
unlink(OUT, recursive = TRUE)
dir.create(OUT, recursive = TRUE, showWarnings = FALSE)
for (d in c("gen", "transform", "exact", "study", "dist", "quantile", "invariance", "roundtrip"))
  dir.create(file.path(OUT, d), recursive = TRUE, showWarnings = FALSE)

cat("RNGkind:", paste(RNGkind(), collapse = "/"), "\n")
cat("== gen\n");        gen_stage(OUT)
cat("== transform\n");  transform_stage(OUT)
cat("== exact\n");      exact_stage(OUT)
cat("== study\n");      study_stage(OUT)
cat("== dist\n");       dist_stage(OUT)
cat("== errors\n");     errors_stage(OUT, collision = COLLISION)
cat("== quantile\n");   quantile_stage(OUT)
cat("== invariance\n"); invariance_stage(OUT)
cat("== roundtrip\n");  roundtrip_stage(OUT, ROOT)

# Manifest: relative path and SHA-256 of every file in byte order, then one hash of the whole list.
files <- sort(list.files(OUT, recursive = TRUE, pattern = "\\.json$"), method = "radix")
files <- setdiff(files, "manifest.json")
hashes <- vapply(files, function(f) sha256_file(file.path(OUT, f)), "")
combined <- digest::digest(paste0(files, "  ", hashes, "\n", collapse = ""), algo = "sha256", serialize = FALSE)
write_json_lf(list(
  task = "T-158", spec = "specs/simulation.md SIM-ORA-01 to SIM-ORA-06",
  script = "oracle/scripts/sim_oracle.R",
  files = lapply(seq_along(files), function(i) list(path = files[i], sha256 = unname(hashes[i]))),
  combined_sha256 = combined,
  versions = versions_list()), file.path(OUT, "manifest.json"))
cat(sprintf("files: %d\ncombined sha256: %s\n", length(files), combined))
