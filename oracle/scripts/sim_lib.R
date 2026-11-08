# Caladrius population-simulation oracle, card T-158: the generator, the stream layout, the uniform and the
# normal transform, in R, from the text of specs/simulation.md (SIM-RNG-01 to SIM-RNG-07) and nothing else.
#
# Sourced by sim_oracle.R and the other sim_*.R scripts; it writes nothing by itself. No engine code was read
# and none was used: every function below is written from the spec text. The checks that must hold before
# anything else (the three known-answer vectors of SIM-RNG-01, the words of SX1) are in sim_gen.R and stop
# the run on a mismatch; this file only defines.
#
# Representation. An unsigned 32-bit integer is an R double in [0, 2^32) (exact), so that every operation
# below is exact: sums and products are split into 16-bit limbs (a product of two 32-bit numbers needs 64
# bits, a double holds 53). A 64-bit word is a pair (hi, lo) of such doubles, written as 16 hexadecimal digits.
# Doubles that leave R are written as hexadecimal floats (sprintf("%a")) and checked to read back to the same
# bits by hex_parse() below, never as decimals read by R's parser, which is not correctly rounded
# (as.numeric("2.58859887947042") is one ulp off in R 4.5.2).

suppressPackageStartupMessages({
  library(Rmpfr)
  library(jsonlite)
})

BITS <- 256L
TWO32 <- 4294967296
TWO16 <- 65536

# ------------------------------------------------------------------ exact 32-bit arithmetic on doubles
u32_add <- function(a, b) (a + b) %% TWO32
u32_xor <- function(a, b) {
  hi <- bitwXor(as.integer(a %/% TWO16), as.integer(b %/% TWO16))
  lo <- bitwXor(as.integer(a %% TWO16), as.integer(b %% TWO16))
  hi * TWO16 + lo
}
# the 64-bit product a*b of two 32-bit numbers, split into (hi, lo)
mulhilo <- function(a, b) {
  ah <- a %/% TWO16; al <- a %% TWO16; bh <- b %/% TWO16; bl <- b %% TWO16
  t <- al * bl                        # < 2^32
  mid <- ah * bl + al * bh            # < 2^33
  lo_full <- t + (mid %% TWO16) * TWO16   # < 2^33
  list(hi = ah * bh + mid %/% TWO16 + lo_full %/% TWO32, lo = lo_full %% TWO32)
}

# ------------------------------------------------------------------ hexadecimal floats
hex_fmt <- function(x) {
  s <- sprintf("%a", x)
  back <- hex_parse(s)
  if (!identical(back, x, num.eq = FALSE)) stop("hex_fmt: ", s, " does not read back to the same bits")
  s
}
# exact parser of "[-]0x1.hhhhp[+-]e" and "[-]0x0p+0"; at most 53 significant bits (no rounding is ever needed)
hex_parse <- function(s) {
  vapply(s, function(one) {
    neg <- startsWith(one, "-")
    body <- sub("^[-+]", "", one)
    body <- sub("^0[xX]", "", body)
    parts <- strsplit(body, "[pP]")[[1]]
    mant <- parts[1]; ex <- as.integer(parts[2])
    ip <- strsplit(mant, ".", fixed = TRUE)[[1]]
    int_digits <- ip[1]; frac_digits <- if (length(ip) > 1L) ip[2] else ""
    digits <- strsplit(paste0(int_digits, frac_digits), "")[[1]]
    n <- 0
    for (d in digits) {
      n <- n * 16 + strtoi(d, 16L)
      if (n >= 2^53) stop("hex_parse: more than 53 bits in ", one)
    }
    v <- n * 2^(ex - 4L * nchar(frac_digits))
    if (neg) -v else v
  }, 0, USE.NAMES = FALSE)
}
# the 16 hexadecimal digits of the IEEE bit pattern of a double
bits_hex <- function(x) {
  r <- writeBin(as.double(x), raw(), endian = "big")
  paste(sprintf("%02x", as.integer(r)), collapse = "")
}
# a 17-digit decimal that is only for reading (never parsed back by R as a bit-exact value)
dec17 <- function(x) sprintf("%.17g", x)

# ------------------------------------------------------------------ Philox4x32-10 (SIM-RNG-01)
PHILOX_M0 <- 3528531795   # 0xD2511F53
PHILOX_M1 <- 3449720151   # 0xCD9E8D57
PHILOX_W0 <- 2654435769   # 0x9E3779B9
PHILOX_W1 <- 3144134277   # 0xBB67AE85
stopifnot(PHILOX_M0 == 0xD2511F53, PHILOX_M1 == 0xCD9E8D57, PHILOX_W0 == 0x9E3779B9, PHILOX_W1 == 0xBB67AE85)

# counter columns c0..c3 and key k0, k1: numeric vectors (recycled to a common length), returns the four output words
philox <- function(c0, c1, c2, c3, k0, k1) {
  n <- max(length(c0), length(c1), length(c2), length(c3), length(k0), length(k1))
  c0 <- rep_len(c0, n); c1 <- rep_len(c1, n); c2 <- rep_len(c2, n); c3 <- rep_len(c3, n)
  k0 <- rep_len(k0, n); k1 <- rep_len(k1, n)
  for (rho in 0:9) {
    kk0 <- u32_add(k0, rho * PHILOX_W0); kk1 <- u32_add(k1, rho * PHILOX_W1)
    p0 <- mulhilo(PHILOX_M0, c0); p1 <- mulhilo(PHILOX_M1, c2)
    n0 <- u32_xor(u32_xor(p1$hi, c1), kk0)
    n1 <- p1$lo
    n2 <- u32_xor(u32_xor(p0$hi, c3), kk1)
    n3 <- p0$lo
    c0 <- n0; c1 <- n1; c2 <- n2; c3 <- n3
  }
  list(r0 = c0, r1 = c1, r2 = c2, r3 = c3)
}

# ------------------------------------------------------------------ the seed and the label (SIM-RNG-02, -03)
# a seed is a decimal string of digits (any seed up to 2^64 - 1); the key is (seed mod 2^32, floor(seed / 2^32))
seed_key <- function(seed_string) {
  stopifnot(is.character(seed_string), grepl("^[0-9]+$", seed_string))
  s <- gmp::as.bigz(seed_string)
  stopifnot(s < gmp::as.bigz(2)^64)
  c(k0 = as.numeric(s %% gmp::as.bigz(2)^32), k1 = as.numeric(s %/% gmp::as.bigz(2)^32))
}
# SHA-256 of the label's UTF-8 bytes, first 8 bytes: (c2, c3) as the two 32-bit halves of a big-endian integer
label_bytes <- function(label) charToRaw(enc2utf8(label))
label_hash_hex <- function(label) {
  substr(digest::digest(label_bytes(label), algo = "sha256", serialize = FALSE), 1, 16)
}
label_key <- function(label) {
  h <- label_hash_hex(label)
  c(c2 = strtoi(substr(h, 1, 4), 16L) * TWO16 + strtoi(substr(h, 5, 8), 16L),
    c3 = strtoi(substr(h, 9, 12), 16L) * TWO16 + strtoi(substr(h, 13, 16), 16L))
}

# the blocks b (a numeric vector) of the stream (seed key, replicate r, purpose p, label key): returns r0..r3
stream_blocks <- function(b, key, r, p, lkey) {
  stopifnot(r >= 0, r < 2^24, p >= 0, p < 256, all(b >= 0 & b < TWO32))
  philox(b, p * 2^24 + r, lkey[["c2"]], lkey[["c3"]], key[["k0"]], key[["k1"]])
}
# words n (numeric vector, 0 <= n < 2^33): returns hi and lo 32-bit halves, word = r_{2h} + 2^32 * r_{2h+1}
stream_words <- function(n, key, r, p, lkey) {
  stopifnot(all(n >= 0 & n < 2^33))
  b <- n %/% 2; h <- n %% 2
  o <- stream_blocks(b, key, r, p, lkey)
  list(lo = ifelse(h == 0, o$r0, o$r2), hi = ifelse(h == 0, o$r1, o$r3))
}
hex32 <- function(x) sprintf("%04x%04x", as.integer(x %/% TWO16), as.integer(x %% TWO16))
word_hex <- function(w) paste0(hex32(w$hi), hex32(w$lo))

# ------------------------------------------------------------------ the uniform (SIM-RNG-04)
# u = (m + 0.5) * 2^-52 with m = hi * 2^20 + floor(lo / 2^12), the top 52 bits of the word
word_uniform <- function(w) (w$hi * 1048576 + w$lo %/% 4096 + 0.5) * 2^-52

# ------------------------------------------------------------------ AS 241 (SIM-RNG-05)
# The coefficients are the decimals of the table of the spec, converted to the nearest double by Rmpfr
# (correctly rounded), not by R's parser.
nearest_double <- function(decimal) as.numeric(mpfr(decimal, BITS))
AS241_TEXT <- list(
  A = c("3.387132872796366608", "133.14166789178437745", "1971.5909503065514427", "13731.693765509461125",
        "45921.953931549871457", "67265.770927008700853", "33430.575583588128105", "2509.0809287301226727"),
  B = c("1", "42.313330701600911252", "687.1870074920579083", "5394.1960214247511077",
        "21213.794301586595867", "39307.89580009271061", "28729.085735721942674", "5226.495278852854561"),
  C = c("1.42343711074968357734", "4.6303378461565452959", "5.7694972214606914055", "3.64784832476320460504",
        "1.27045825245236838258", "0.24178072517745061177", "0.0227238449892691845833", "7.7454501427834140764e-4"),
  D = c("1", "2.05319162663775882187", "1.6763848301838038494", "0.68976733498510000455",
        "0.14810397642748007459", "0.0151986665636164571966", "5.475938084995344946e-4", "1.05075007164441684324e-9"),
  E = c("6.6579046435011037772", "5.4637849111641143699", "1.7848265399172913358", "0.29656057182850489123",
        "0.026532189526576123093", "0.0012426609473880784386", "2.71155556874348757815e-5", "2.01033439929228813265e-7"),
  F = c("1", "0.59983220655588793769", "0.13692988092273580531", "0.0148753612908506148525",
        "7.868691311456132591e-4", "1.8463183175100546818e-5", "1.4215117583164458887e-7", "2.04426310338993978564e-15"))
AS241 <- lapply(AS241_TEXT, function(v) vapply(v, nearest_double, 0, USE.NAMES = FALSE))
AS_SPLIT <- nearest_double("0.425")      # |q| <= 0.425: the central region
AS_CONST1 <- nearest_double("0.180625")
AS_HALF <- 0.5
AS_T_EDGE <- 1.6
AS_T_SPLIT <- 5

# Horner from the highest degree down, every product and sum rounded separately (R does no fused multiply-add)
poly <- function(x, cf) {
  r <- rep(cf[8], length(x))
  for (i in 7:1) r <- r * x + cf[i]
  r
}
# which branch the algorithm takes for each u: "central", "tail1" (t <= 5) or "tail2" (t > 5)
as241_branch <- function(u, ln = log) {
  q <- u - AS_HALF
  out <- rep("central", length(u))
  tail <- !(abs(q) <= AS_SPLIT)
  if (any(tail)) {
    pt <- ifelse(q[tail] < 0, u[tail], 1 - u[tail])
    t <- sqrt(-ln(pt))
    out[tail] <- ifelse(t <= AS_T_SPLIT, "tail1", "tail2")
  }
  out
}
# the transform; `ln` can be replaced to measure how much a different logarithm moves the result
as241 <- function(u, ln = log) {
  q <- u - AS_HALF
  z <- numeric(length(u))
  cen <- abs(q) <= AS_SPLIT
  if (any(cen)) {
    qc <- q[cen]
    rho <- AS_CONST1 - qc * qc
    z[cen] <- qc * poly(rho, AS241$A) / poly(rho, AS241$B)
  }
  if (any(!cen)) {
    qt <- q[!cen]
    pt <- ifelse(qt < 0, u[!cen], 1 - u[!cen])
    t <- sqrt(-ln(pt))
    v <- numeric(length(t))
    near <- t <= AS_T_SPLIT
    if (any(near)) { tp <- t[near] - AS_T_EDGE; v[near] <- poly(tp, AS241$C) / poly(tp, AS241$D) }
    if (any(!near)) { tp <- t[!near] - AS_T_SPLIT; v[!near] <- poly(tp, AS241$E) / poly(tp, AS241$F) }
    z[!cen] <- ifelse(qt < 0, -v, v)
  }
  z
}

# ------------------------------------------------------------------ the 256-bit inverse of the normal distribution
# Solves Phi(z) = u by Newton iteration on erfc, from the double result as the start; the lower tail is
# solved for min(u, 1 - u) (1 - u is exact in 256 bits) and the sign applied after.
PI_MP <- Const("pi", BITS)
SQRT2_MP <- sqrt(mpfr(2, BITS))
SQRT_2PI_MP <- sqrt(2 * PI_MP)
phi_lower <- function(z) erfc(-z / SQRT2_MP) / 2        # Phi(z)
qnorm_mp <- function(u, iterations = 9L) {
  lower <- u <= 0.5
  pd <- ifelse(lower, u, 1 - u)                           # min(u, 1 - u), exact in double
  p <- mpfr(pd, BITS)
  z <- mpfr(as241(pd), BITS)                              # the start: the double result, lower tail
  for (it in seq_len(iterations)) {
    f <- phi_lower(z) - p
    dens <- exp(-(z * z) / 2) / SQRT_2PI_MP
    z <- z - f / dens
  }
  resid <- abs(phi_lower(z) / p - 1)
  stopifnot(all(resid < mpfr(2, BITS)^-230))
  z[!lower] <- -z[!lower]
  z
}
# decimal text of a 256-bit number (50 significant digits)
mp_dec <- function(x, digits = 50) vapply(seq_along(x), function(i) format(x[i], digits = digits), "")
# nearest double of a 256-bit number
mp_dbl <- function(x) as.numeric(x)

# ------------------------------------------------------------------ Cholesky factor and eta (SIM-RNG-07), doubles
# omega: the lower triangle by rows (q(q+1)/2 numbers); returns L (q x q, lower) in the convention of the spec
omega_matrix <- function(lower_by_rows, q) {
  stopifnot(length(lower_by_rows) == q * (q + 1) / 2)
  M <- matrix(0, q, q); k <- 1L
  for (i in seq_len(q)) for (j in seq_len(i)) { M[i, j] <- lower_by_rows[k]; M[j, i] <- lower_by_rows[k]; k <- k + 1L }
  M
}
omega_diag_matrix <- function(variances) diag(variances, nrow = length(variances))
# Cholesky-Banachiewicz row by row; T accumulated from 0 left to right in l, then subtracted once
chol_spec <- function(Om) {
  q <- nrow(Om); L <- matrix(0, q, q)
  for (i in seq_len(q)) for (j in seq_len(i)) {
    tsum <- 0
    for (l in seq_len(j - 1L)) tsum <- tsum + L[i, l] * L[j, l]
    s <- Om[i, j] - tsum
    if (i == j) {
      if (!(is.finite(s) && s > 0)) return(list(ok = FALSE, index = i, pivot = s))
      L[i, i] <- sqrt(s)
    } else L[i, j] <- s / L[j, j]
  }
  list(ok = TRUE, L = L)
}
# eta = L z, accumulated from 0, left to right in j
eta_spec <- function(L, z) {
  q <- nrow(L); eta <- numeric(q)
  for (i in seq_len(q)) { s <- 0; for (j in seq_len(i)) s <- s + L[i, j] * z[j]; eta[i] <- s }
  eta
}

# ------------------------------------------------------------------ type 7 quantile (SIM-SUM-01)
quantile7 <- function(x, p) {
  x <- sort(x); n <- length(x)
  vapply(p, function(pp) {
    h <- 1 + (n - 1) * pp; lo <- floor(h); hi <- ceiling(h); g <- h - lo
    (1 - g) * x[lo] + g * x[hi]
  }, 0)
}

# ------------------------------------------------------------------ files
write_lines_lf <- function(lines, path) {
  dir.create(dirname(path), recursive = TRUE, showWarnings = FALSE)
  con <- file(path, open = "wb"); on.exit(close(con)); writeLines(lines, con, sep = "\n", useBytes = TRUE)
}
# JSON with the doubles as they are (never the carrier of a bit-exact double: those are hex strings)
write_json_lf <- function(obj, path, digits = NA) {
  write_lines_lf(toJSON(obj, auto_unbox = TRUE, pretty = TRUE, digits = digits, null = "null", na = "null"), path)
}
sha256_file <- function(path) digest::digest(file = path, algo = "sha256", serialize = FALSE)
repo_root <- function() {
  args_all <- commandArgs(trailingOnly = FALSE)
  file_arg <- sub("^--file=", "", args_all[grepl("^--file=", args_all)])
  if (length(file_arg) == 1L) normalizePath(file.path(dirname(file_arg), "..", ".."), winslash = "/", mustWork = TRUE)
  else normalizePath(".", winslash = "/", mustWork = TRUE)
}
versions_list <- function() {
  list(R = R.version.string, Rmpfr = as.character(packageVersion("Rmpfr")), gmp = as.character(packageVersion("gmp")),
       jsonlite = as.character(packageVersion("jsonlite")), digest = as.character(packageVersion("digest")),
       RNGkind = paste(RNGkind(), collapse = "/"), precision_bits = BITS)
}

# ------------------------------------------------------------------ draws of a stream, as uniforms and normals
# n words from the start of a stream (seed as a decimal string)
draw_u <- function(seed, r, p, label, n) word_uniform(stream_words(seq_len(n) - 1, seed_key(seed), r, p, label_key(label)))
draw_z <- function(seed, r, p, label, n) as241(draw_u(seed, r, p, label, n))

# a vector as a JSON array (a list of scalars)
arr <- function(x) as.list(x)
