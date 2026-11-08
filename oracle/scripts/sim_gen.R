# Caladrius population-simulation oracle, card T-158 item 1 (SIM-ORA-01): the generator, bit for bit.
#
# Order of work, as the card asks: the three known-answer vectors of SIM-RNG-01 first, then the words of SX1
# (the keys of the labels, the blocks, the words, the uniforms, the z), then the big-seed words. Every check
# below stops the run on a mismatch (stop), before any file is written. Only then the case files are
# written: kat.json (the vectors and some blocks, for the engine's `philox` entry point) and streams.json
# (the words, uniforms and z of a grid of streams that exercises every field of the counter and the key).
#
# Everything is integer arithmetic on doubles in 16-bit limbs (sim_lib.R), written from the spec text.
# Called by sim_oracle.R; defines gen_stage(out_dir).

# ------------------------------------------------------------------------------------------ the checks
run_kat <- function() {
  kat <- list(
    list(c = c(0, 0, 0, 0), k = c(0, 0), out = c("6627e8d5", "e169c58d", "bc57ac4c", "9b00dbd8")),
    list(c = rep(TWO32 - 1, 4), k = rep(TWO32 - 1, 2), out = c("408f276d", "41c83b0e", "a20bc7c6", "6d5451fd")),
    list(c = c(0x243f6a88, 0x85a308d3, 0x13198a2e, 0x03707344), k = c(0xa4093822, 0x299f31d0),
         out = c("d16cfe09", "94fdcceb", "5001e420", "24126ea1")))
  for (i in seq_along(kat)) {
    v <- kat[[i]]
    o <- philox(v$c[1], v$c[2], v$c[3], v$c[4], v$k[1], v$k[2])
    got <- c(hex32(o$r0), hex32(o$r1), hex32(o$r2), hex32(o$r3))
    if (!identical(got, v$out)) stop("KNOWN-ANSWER VECTOR ", i, " DOES NOT MATCH: got ", paste(got, collapse = " "), ", expected ", paste(v$out, collapse = " "))
  }
  kat
}

# the SX1 words: (seed, replicate, purpose, label) -> expected blocks and words, copied from specs/simulation.md section 9
run_sx1 <- function() {
  key <- seed_key("20261108")
  if (!identical(unname(key), c(0x013528f4, 0))) stop("SX1 SEED KEY MISMATCH")
  keys <- c("1" = "6b86b273ff34fce1", "2" = "d4735e3a265e16ee", "3" = "4e07408562bedb8b", "S1" = "3696ad59777e09d5")
  for (l in names(keys)) if (!identical(label_hash_hex(l), unname(keys[l]))) stop("SX1 LABEL KEY MISMATCH for ", l)
  eta1 <- label_key("1")
  # blocks 0 and 1 of the eta stream of subject "1" (output r0..r3)
  b <- stream_blocks(c(0, 1), key, 0, 1, eta1)
  if (!identical(hex32(b$r0), c("bc2fc714", "e0c78126")) || !identical(hex32(b$r1), c("04bc16cb", "c0b96feb")) ||
      !identical(hex32(b$r2), c("0f6e941d", "dbcc1bce")) || !identical(hex32(b$r3), c("b0671d12", "af56fc7b"))) stop("SX1 ETA BLOCKS MISMATCH")
  # the four words, uniforms and z of the table (u as hexadecimal floats, z as the 17-digit decimals converted by Rmpfr)
  w <- stream_words(0:3, key, 0, 1, eta1)
  want_word <- c("04bc16cbbc2fc714", "b0671d120f6e941d", "c0b96febe0c78126", "af56fc7bdbcc1bce")
  if (!identical(word_hex(w), want_word)) stop("SX1 ETA WORDS MISMATCH: ", paste(word_hex(w), collapse = " "))
  u <- word_uniform(w)
  want_u <- hex_parse(c("0x1.2f05b2ef0bf2p-6", "0x1.60ce3a241edd3p-1", "0x1.8172dfd7c18f1p-1", "0x1.5eadf8f7b7983p-1"))
  if (!identical(u, want_u)) stop("SX1 ETA UNIFORMS MISMATCH")
  z <- as241(u)
  want_z <- vapply(c("-2.0858741749246219", "0.49322555372575094", "0.68342091773486213", "0.48150463001583299"), nearest_double, 0, USE.NAMES = FALSE)
  # z0 is in the first tail branch (a tolerance target), the others are central (bit-exact targets)
  if (!identical(z[2:4], want_z[2:4])) stop("SX1 ETA CENTRAL Z MISMATCH")
  if (abs(z[1] / want_z[1] - 1) > 1e-15) stop("SX1 ETA TAIL Z MISMATCH")
  # the eps stream (purpose 2) of subject "1": blocks 0 and 1
  e <- stream_blocks(c(0, 1), key, 0, 2, eta1)
  if (!identical(c(hex32(e$r0), hex32(e$r1), hex32(e$r2), hex32(e$r3)),
                 c("77e3c473", "f336bfd1", "5abb8cc4", "65955cd3", "c6d6da9d", "9e9f9e92", "702ddb8d", "960ccb3c")))
    stop("SX1 EPS BLOCKS MISMATCH")
  ew <- stream_words(0:3, key, 0, 2, eta1)
  ze <- as241(word_uniform(ew))
  want_ze <- vapply(c("-0.37340294508690741", "-0.15553512657995675", "-0.26161187860580409", "0.21760798641501161"), nearest_double, 0, USE.NAMES = FALSE)
  if (!identical(ze, want_ze)) stop("SX1 EPS Z MISMATCH")
  # the first word of the eta stream: replicate 0 and 1, subjects "1" and "2"
  first <- list(list(1, 0, "1", "04bc16cb", "bc2fc714"), list(1, 0, "2", "11c354cd", "b72ea848"),
                list(1, 1, "1", "0c6839e0", "16c75d60"), list(1, 1, "2", "e69c8696", "0edcc929"))
  for (f in first) {
    ww <- stream_words(0, key, f[[2]], f[[1]], label_key(f[[3]]))
    if (!identical(c(hex32(ww$hi), hex32(ww$lo)), c(f[[4]], f[[5]]))) stop("SX1 FIRST WORD MISMATCH for ", f[[3]], " replicate ", f[[2]])
  }
  # the seed above 2^53, as a decimal string, and the largest replicate
  big <- seed_key("81985529216486895")
  if (!identical(unname(big), c(0x89abcdef, 0x01234567))) stop("BIG-SEED KEY MISMATCH")
  w1 <- stream_words(0, big, 0, 1, label_key("1"))
  if (!identical(c(hex32(w1$hi), hex32(w1$lo)), c("f5623047", "2f8d0529"))) stop("BIG-SEED WORD MISMATCH")
  w2 <- stream_words(5, big, 16777215, 2, label_key("S1"))
  if (!identical(c(hex32(w2$hi), hex32(w2$lo)), c("15481cf8", "2c2455f2"))) stop("LARGEST-REPLICATE WORD MISMATCH")
  invisible(TRUE)
}

# ------------------------------------------------------------------------------------------ the case files
# one word of a stream, with its uniform and normal; z_ref is the 256-bit value (50 digits)
word_entry <- function(n, key, r, p, lkey) {
  w <- stream_words(n, key, r, p, lkey)
  u <- word_uniform(w)
  z <- as241(u)
  central <- abs(u - AS_HALF) <= AS_SPLIT
  list(n = n, block = n %/% 2, half = n %% 2, word = word_hex(w), u = hex_fmt(u), z = hex_fmt(z),
       z_bit_exact = central, z_ref = mp_dec(qnorm_mp(u)))
}
stream_case <- function(name, seed, r, p, label, idx, why) {
  key <- seed_key(seed); lkey <- label_key(label)
  bytes <- label_bytes(label)
  list(name = name, why = why, seed = seed, replicate = r, purpose = p, label = label,
       label_utf8_hex = paste(sprintf("%02x", as.integer(bytes)), collapse = ""),
       label_hash = label_hash_hex(label),
       key = c(hex32(key[["k0"]]), hex32(key[["k1"]])),
       c1 = hex32(p * 2^24 + r),
       words = lapply(idx, function(n) word_entry(n, key, r, p, lkey)))
}

gen_stage <- function(out_dir) {
  kat <- run_kat()
  run_sx1()
  cat("sim_gen: known-answer vectors, SX1 words and big-seed words all match\n")
  # ---- kat.json: the vectors, then blocks that move one field at a time, for the engine's `philox` entry point
  kat_rows <- lapply(kat, function(v) list(counter = vapply(v$c, hex32, ""), key = vapply(v$k, hex32, ""), out = v$out))
  extra <- list()
  addb <- function(name, c, k) {
    o <- philox(c[1], c[2], c[3], c[4], k[1], k[2])
    extra[[length(extra) + 1L]] <<- list(name = name, counter = vapply(c, hex32, ""), key = vapply(k, hex32, ""),
                                          out = c(hex32(o$r0), hex32(o$r1), hex32(o$r2), hex32(o$r3)))
  }
  l1 <- label_key("1"); k0 <- seed_key("20261108")
  addb("sx1_eta_1_block0", c(0, 0x01000000, l1[["c2"]], l1[["c3"]]), k0)
  addb("sx1_eta_1_block1", c(1, 0x01000000, l1[["c2"]], l1[["c3"]]), k0)
  addb("counter_c0_alone", c(1, 0, 0, 0), c(0, 0))
  addb("counter_c1_alone", c(0, 1, 0, 0), c(0, 0))
  addb("counter_c2_alone", c(0, 0, 1, 0), c(0, 0))
  addb("counter_c3_alone", c(0, 0, 0, 1), c(0, 0))
  addb("key_k0_alone", c(0, 0, 0, 0), c(1, 0))
  addb("key_k1_alone", c(0, 0, 0, 0), c(0, 1))
  addb("high_bits", c(TWO32 - 1, 0, 0, 0), c(0, TWO32 - 1))
  write_json_lf(list(rng = "philox4x32-10/as241/v1", rule = "SIM-RNG-01, SIM-ORA-01 item 1",
                     note = "counter, key and output are 8-digit hexadecimal 32-bit words; compare parsed integers",
                     kat = kat_rows, blocks = extra), file.path(out_dir, "gen", "kat.json"))

  # ---- streams.json
  cases <- list()
  add <- function(...) cases[[length(cases) + 1L]] <<- stream_case(...)
  first8 <- 0:7
  for (lab in c("1", "2", "3", "S1")) {
    add(sprintf("sx1_%s_r0_eta", lab), "20261108", 0, 1, lab, first8, "SX1: the eta stream, replicate 0")
    add(sprintf("sx1_%s_r0_eps", lab), "20261108", 0, 2, lab, first8, "SX1: the eps stream, replicate 0")
  }
  add("sx1_1_r1_eta", "20261108", 1, 1, "1", first8, "SX1: replicate 1 enters the counter")
  add("sx1_2_r1_eta", "20261108", 1, 1, "2", first8, "SX1: replicate 1 enters the counter")
  add("big_seed_1_eta", "81985529216486895", 0, 1, "1", first8, "SX1: a seed above 2^53 as a decimal string, key (0x89abcdef, 0x01234567)")
  add("big_seed_S1_r_max_eps", "81985529216486895", 16777215, 2, "S1", first8, "SX1: the largest replicate 2^24 - 1; word 5 is the one the spec records")
  for (s in c("0", "1", "4294967295", "4294967296", "9007199254740991", "9007199254740992", "18446744073709551615"))
    add(sprintf("seed_%s", s), s, 0, 1, "1", first8, "the seed key: low and high halves, 2^53 - 1 and 2^53, the largest seed")
  labels <- list("empty" = "", "leading_zero" = "01", "fullwidth_one" = "１", "space_before" = " 1", "space_after" = "1 ",
                 "e_acute" = "é", "cjk" = "日本語", "mixed" = "Subject-007/ü",
                 "long_200" = paste(rep("abcdefghij", 20), collapse = ""), "lower_s1" = "s1")
  for (nm in names(labels))
    add(sprintf("label_%s", nm), "20261108", 0, 1, labels[[nm]], 0:3, "the label is hashed as held: no trimming, case folding or normalisation")
  for (r in c(0, 1, 255, 256, 65535, 65536, 16777215))
    add(sprintf("replicate_%d", r), "20261108", r, 2, "1", 0:3, "the replicate is the low 24 bits of c1; the purpose is the top 8 bits")
  far <- c(0, 1, 99, 100, 1000, 65535, 65536, 2^31, 2^32 - 1, 2^32, 2^33 - 2, 2^33 - 1)
  add("far_words", "20261108", 0, 1, "1", far, "block index in c0 up to 2^32 - 1 (word 2^33 - 1 is the last of a stream)")
  write_json_lf(list(rng = "philox4x32-10/as241/v1", rule = "SIM-RNG-03, -04, -05, SIM-ORA-01 items 2 and 3",
                     note = paste("word: 16 hexadecimal digits of x = r_{2h} + 2^32 r_{2h+1}; u and z are hexadecimal floats, compared as parsed values;",
                                  "u is bit-exact; z is bit-exact where z_bit_exact is true (central region, |u - 0.5| <= 0.425),",
                                  "else within tol_z (relative) of z_ref, the 256-bit inverse, and never compared as a branch"),
                     cases = cases), file.path(out_dir, "gen", "streams.json"))
  invisible(length(cases))
}
