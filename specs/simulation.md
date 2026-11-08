# Population simulation: reproducible draws from a population model (SIM-*, card T-157)

Brick 4 of iteration 3 (T-139). `specs/population.md` fits a population model; this file specifies the opposite direction: **given θ, Ω, a residual error model, a design (subjects, doses, times) and a seed, return simulated random effects, individual parameters, predictions and observations**, the same bits on 1 thread, on N threads and in WebAssembly. Written by the `reader` agent on 2026-11-08, in its own words, from the open sources of `specs/sources.md` section 15 (keys S-59 to S-67) and from derivations (D). **When this file was written nothing in it was implemented or exercised by an oracle.** The numbers of the worked examples (section 9) come from throwaway R 4.5.2 scripts kept outside the repository (hand checks H39 to H44), seeded and written out in full: the 256-bit values (`Rmpfr`) are exact to the digits shown, the double-precision values are shown to 17 digits. **Which of them are bit-exact targets and which are tolerance targets is stated at the head of section 9**: bit-exact are the known-answer vectors, the stream words, the uniforms, the Cholesky factor and the z of the central region; every value that follows an `ln` or an `exp` (tail z, exp(η), P, IPRED, σ, y) is a tolerance target (SIM-ORA-02, -03), because the numbers come from R's `log` and `exp`, which are not the `libm` crate of golden rule 10. **Doubles are exchanged between tools as hexadecimal floats or 64-bit patterns, never as decimals read by R** (SIM-ORA-01 item 3). The numbers are targets for the oracle card T-158, not results. Where a number is a size limit or a tolerance, the rule names a symbol and the value is an open item (OS-04, OS-05): `AGENTS.md` section 5 asks that population tolerances come from what the oracle measures.

**Reading guide.** The rules answer, in order, the eight items of the card: the model (section 1), the randomness (2), what is simulated and in what order (3), the outputs and their summaries (4), determinism (5), what can be tested (6), the refusals (7), the VPC, marked `later` (8); the worked examples are section 9 and the open items for Abdou section 10. Three choices are Caladrius's own and each has its reason: a **counter-based generator** whose draw number n of a stream is a pure function of its address (SIM-RNG-01 to -03), **inversion** as the only normal transform so that every draw consumes exactly one uniform (SIM-RNG-05, -06), and **a stream per subject, replicate and purpose, keyed by the subject's label** (SIM-RNG-03).

Notation added to `specs/population.md`:

| symbol | meaning |
|---|---|
| s | the seed, an integer 0 ≤ s < 2⁶⁴ |
| r | the replicate index, 0 ≤ r < R; one replicate is one complete virtual study of the design |
| i, λ_i | a subject and its label, the text exactly as the design holds it |
| q, m | number of random effects (the length of `random_effects`) and number of subjects |
| n_i | number of observation rows of subject i in the design |
| stream(s, r, p, λ) | the sequence of 64-bit words of purpose p (1 = η, 2 = ε) for replicate r and subject label λ (SIM-RNG-03); word n is its n-th word, n = 0, 1, 2, … |
| u_n, z_n | the uniform and the standard normal made from word n (SIM-RNG-04, -05) |
| z_k^η, z_j^ε | z_{k−1} of the η stream (k = 1..q) and z_{j−1} of the ε stream (j = 1..n_i) |
| L | the lower Cholesky factor of Ω (SIM-RNG-07); η = L·z^η |
| PRED_j, IPRED_j | the prediction with every η = 0 (the typical individual) and with the subject's η, at observation j |
| σ_j, y_j | the standard deviation of observation j (POP-ERR-01) and the simulated observation |
| LLOQ | the lower limit of quantification, one number per run (SIM-ORD-05) |

## 1. The model (card item 1)

### SIM-MOD-01 The model that is simulated
The model is the one of `specs/population.md` read forwards. For subject i, replicate r, observation j (POP-MOD-01, POP-ERR-01):

η_i ~ N(0, Ω);  P_ik = θ_k·exp(η_ik) for a parameter with a random effect, P_ik = θ_k otherwise;  IPRED_ij = f(P_i, dose_i, t_ij);  y_ij = IPRED_ij + σ_ij·ε_ij,  ε_ij ~ N(0, 1), independent of η_i and of each other,

with σ_ij = g(IPRED_ij; ξ) one of the four error models `additive`, `proportional`, `combined` (variance sum), `combined_sd` (POP-ERR-01, FIT-ML-02), evaluated **on the individual prediction**. So the simulator is the exact forward counterpart of the FOCEI model: a fit of simulated data estimates the same θ, Ω and ξ that were put in (SIM-ORA-06). θ_k is the median of P_ik, not its mean (the mean is θ_k·exp(Ω_kk/2), POP-MOD-01). Nothing is estimated and no derivative is taken: the structural model is evaluated, not differentiated, which is why the restrictions that POP-MOD-02 and POP-MOD-04 place on the fit do not all apply (SIM-MOD-02, -03).
- Status: `assumed`
- Sources: `specs/population.md` POP-MOD-01, POP-ERR-01; FIT-ML-02; D (the median and the mean of a log-normal).

### SIM-MOD-02 Which structural models, and the decision on `pk3.*`
**Simulated:** the six `pk1.*` ids, the six `pk2.*` ids in the **clearance set** (`cl`, `vc`, `q`, `vp`: MOD-2C-02, as for the fit in POP-MOD-04), and **the six `pk3.*` ids in the clearance set** (`cl`, `vc`, `q2`, `vp2`, `q3`, `vp3`: MOD-3C-25 of `specs/models.md` section 14, which exists on the branch `iter3/pk3` and is not on the base of this branch). A `pk3.*` id is accepted **only once section 14 of `specs/models.md` is on the same branch as the engine**; until then a request for a `pk3.*` id is `InvalidOption` ("model not available in this version"). **Reason for including `pk3.*` here when the fit leaves them out (POP-MOD-04):** the fit's reason was the state of the branches and the absence of an oracle for derivatives and the inner problem; simulation needs only the forward function of a model, which is specified (MOD-3C-10 to -14) and gets its own oracle from the model card T-142. A simulator that handles the three-compartment catalogue costs nothing more than the catalogue's forward function, and it gives the population fit its first three-compartment data for later. The corollary: **data simulated from a `pk3.*` model cannot be refitted by the population fit in v1** (POP-MOD-04), and the result of such a run carries a note saying so. The micro and macro parameter sets are not accepted (a log-normal random effect on each macro constant would not respect their ordering: the reason of POP-MOD-04). Michaelis-Menten elimination and the effect compartment (MOD-LT-02, MOD-LT-03) are `later`.
- Status: `assumed`
- Sources: design choice; MOD-GEN-05, MOD-2C-02, MOD-2C-20, MOD-3C-25 (`iter3/pk3`, read for the ids and parameter names only), MOD-LT-02; POP-MOD-04.

### SIM-MOD-03 Random effects and the shape of Ω
The user names the random effects explicitly (`random_effects`: a list of parameter names, no default, no duplicate: POP-MOD-02, POP-INN-03 item 5); the **order of that list is part of the model** because the k-th name receives the k-th normal of the η stream (SIM-RNG-06) and the k-th row of L. Reordering the list gives different η for the same seed; appending a name at the end leaves the earlier η unchanged when the leading block of Ω is the same (D: the leading block of a Cholesky factor is the Cholesky factor of the leading block).

Rules inherited from POP-MOD-02: a parameter with a random effect has θ_k > 0; a random effect on a parameter the model does not have is `InvalidOption`; q ≥ 1: a simulation of residual error alone (q = 0) is not offered in v1, and `random_effects` empty is `InvalidOption`. As for the fit, **`tlag` and `dur` carry no random effect in a simulation either** (POP-MOD-02 rule 3), so that every simulated dataset can be refitted by the population fit and lifting the restriction later moves no draw (the k-th effect keeps the k-th word whatever the list holds). This is a decision taken meanwhile (2026-11-08), reversible by Abdou (OS-03). The bioavailability F is not a parameter (MOD-GEN-03) and has no random effect, as for the fit.

Ω has the two shapes of POP-MOD-03: `diagonal` (the default; the variances Ω_11..Ω_qq are given) and `block` (the lower triangle **by rows**, q(q+1)/2 numbers: Ω_11, Ω_21, Ω_22, Ω_31, …). Both are accepted whatever is decided for the fit (OP-01): a correlated Ω is needed to test the Cholesky convention (SIM-RNG-07) and for the round trip with a block fit. A full square matrix is not accepted as input (a non-symmetric one would need a rule about which triangle wins).
- Status: `assumed`
- Sources: `specs/population.md` POP-MOD-02, -03, POP-INN-03; D (leading block of a Cholesky factor); design choice for the differences.

### SIM-MOD-04 The design
The design is a table in the layout of `specs/data.md` (DAT-LAY-01), read as the dataset of POP-DAT-01 is: one dose row per subject (dose, dose time and, for an infusion, duration or rate; more than one dose row is refused, as in POP-DAT-01; multiple dosing is `later`, OS-08), the observation rows before or after the dose row, in non-decreasing time, the time axis measured from the dose (DAT-USE-01). A simulated observation before the dose is a legitimate row (item 4), but the population fit leaves pre-dose rows out (`before_dose`, POP-DAT-01). Differences, all because nothing is observed:
1. **The value cell of an observation row is ignored** and the number of rows whose cell was not empty is returned (`n_dv_ignored`), never silently. This lets the user reuse the table of a real study as the design of a simulated one.
2. The `cens`, `limit` and missing-value markers of the table are ignored in the same way (the limit of the simulation is the option LLOQ, SIM-ORD-05).
3. **All observation rows are simulated, in the order of the table**, including rows that were missing or BLQ in the source. The j-th observation row of a subject (j = 1..n_i, counting every observation row of that subject) is the one that uses ε-word j − 1 (SIM-RNG-06). Times that go backwards inside a subject are refused (`time_not_increasing`, DAT-ERR-01), and **two observation rows at the same time for one subject are refused as the dataset reader refuses them (`duplicate_times`, DAT-ORD-01)**, so that every simulated table is a valid dataset (SIM-OUT-02). A dose and an observation at the same time are allowed, as in DAT-ORD-01.
4. A time before the dose gives IPRED = 0 (MOD-GEN-02); it is simulated, with σ = 0 under `proportional`, and counted in `n_zero_sigma` (SIM-ORD-03).
5. **A design without a table**: the user may instead give a list of times, a dose and m, and the subjects are labelled "1", "2", …, "m" (the decimal of the integer, no leading zeros). The label rule matters because the label keys the stream (SIM-RNG-03).
6. Subjects are taken in the order of first appearance of the label (DAT-ORD-01); a label that appears in two separate blocks is one subject whose rows are gathered in table order, as in the dataset rules, and the same time-order rule applies to the gathered rows.
- Status: `assumed`
- Sources: `specs/data.md` DAT-LAY-01, DAT-ORD-01, DAT-ERR-01, DAT-USE-01, DAT-CEN-01, -02; `specs/population.md` POP-DAT-01; MOD-GEN-02; design choice.

### SIM-MOD-05 The inputs of one simulation (the whole call)
`model` (an id of SIM-MOD-02), `theta` (typical values of every parameter of the model; a parameter with a random effect has θ > 0), `random_effects`, `omega` (shape and numbers, SIM-MOD-03), `error_model` and its parameters `a` and/or `b` (non-negative and finite; there is **no default error model**, as in FIT-ML-01), the design (SIM-MOD-04), `seed` (required, SIM-RNG-02), `replicates` R ≥ 1 (default 1), and the options `negative` (`keep` or `zero`, default `keep`, SIM-ORD-04), `lloq` (absent or a number > 0, SIM-ORD-05) and `quantiles` (a list of probabilities for the summary of SIM-SUM-01, default `[0.05, 0.5, 0.95]`). An unknown option name or value is `InvalidOption` naming it. The result repeats every input (seed and options included) so that the call can be replayed and sealed (SIM-DET-02).
- Status: `assumed`
- Sources: design choice; FIT-ML-01 (no default error model); `AGENTS.md` section 7 (errors name what to change).

## 2. Reproducible randomness (card item 2)

**Why a counter-based generator.** The usual choice, one sequential generator such as the Mersenne Twister that R uses by default (S-62), has a state that moves with each draw: to give subject i the draws it would get on a single thread, every thread has to know how many draws the earlier subjects consumed, or the generator has to be split by jumping ahead, and adding a subject to the design shifts the draws of every later subject. A **counter-based** generator has no state: the output for a counter value is a function of the counter and a key. The address of a draw, (seed, replicate, purpose, subject label, position), is then the counter and the key, a subject's draws do not depend on any other subject, on the order of evaluation or on the number of threads, and adding subjects changes no other subject's draws. This is the property the card asks for, and it follows from the choice of generator, not from discipline in the engine.

### SIM-RNG-01 The uniform generator: Philox4x32-10
**The generator is Philox4x32 with 10 rounds** (Salmon, Moraes, Dror and Shaw, *Parallel random numbers: as easy as 1, 2, 3*, SC11, 2011: the "Random123" family; S-59). It is a bijection of a 128-bit counter into 128 bits of output for each 64-bit key, built from 32 × 32 → 64-bit integer multiplications, XORs and additions only: **no floating point, no table, no platform-dependent function**, so it gives the same bits on every target including WebAssembly. Definition (all quantities are unsigned 32-bit integers, additions modulo 2³²; `mulhilo(a, b)` is the 64-bit product a·b split into its high and low 32-bit halves):

- constants: M0 = 0xD2511F53, M1 = 0xCD9E8D57 (multipliers); W0 = 0x9E3779B9, W1 = 0xBB67AE85 (key increments);
- the counter is (c0, c1, c2, c3) and the key is (k0, k1);
- **one round**, with the current key: (hi0, lo0) = mulhilo(M0, c0); (hi1, lo1) = mulhilo(M1, c2); the new counter is (c0′, c1′, c2′, c3′) = (hi1 XOR c1 XOR k0, lo1, hi0 XOR c3 XOR k1, lo0);
- **the key schedule**: round ρ (ρ = 0..9) uses k0 + ρ·W0 and k1 + ρ·W1 modulo 2³² (the first round uses the key as given);
- the output is the counter after round 9, the four words (r0, r1, r2, r3).

**Known-answer vectors** of the published generator, which an implementation must reproduce before anything else (S-60; all in hexadecimal, counter then key then output; reproduced by two independent implementations of mine, H39):

| counter (c0 c1 c2 c3) | key (k0 k1) | output (r0 r1 r2 r3) |
|---|---|---|
| 00000000 00000000 00000000 00000000 | 00000000 00000000 | 6627e8d5 e169c58d bc57ac4c 9b00dbd8 |
| ffffffff ffffffff ffffffff ffffffff | ffffffff ffffffff | 408f276d 41c83b0e a20bc7c6 6d5451fd |
| 243f6a88 85a308d3 13198a2e 03707344 | a4093822 299f31d0 | d16cfe09 94fdcceb 5001e420 24126ea1 |

**Why this generator and not another.** (1) It is counter-based, which is what the stream design needs. (2) Its quality is reported in its paper from the standard batteries of statistical tests (S-59, summary level: the paper's claim is relied on only for the generator's general quality, and SIM-ORA-04 tests the draws it gives). (3) Its operations are integer-only, so reproducibility across targets is a property of integer arithmetic and not of a library. (4) It is published with test vectors, so the generator can be checked bit for bit (SIM-ORA-01). The alternatives that were weighed: a permuted congruential generator (sequential, so it would need jump-ahead to split streams); xoshiro (the same); a counter-based generator built on a cryptographic hash (slower, no gain); Threefry, the other member of the Random123 family (also counter-based and integer-only; Philox was chosen because its 32-bit multiplications are cheap in WebAssembly and in Rust without a 128-bit type). The engine does **not** reproduce R's Mersenne Twister, so a simulation of Caladrius is not expected to equal a simulation in R at the same seed (POP-DET-04).
- Status: `documented, untested`
- Sources: S-59 (the construction: **the paper itself was not read in this card**; the round function and the constants are written from the published definition and are accepted only because they reproduce every known-answer vector of S-60), S-60 (the test vectors, read from the generator's own repository); H39; `AGENTS.md` rule 10 (the same bits on every target).

### SIM-RNG-02 The seed
The seed s is **a required integer 0 ≤ s < 2⁶⁴**. There is no default seed and no clock: a call without `seed` is refused (SIM-REF-01), because a result that cannot be reproduced from its call is not a result (`AGENTS.md` rule 10: no random number except a stream drawn from a seed that is a parameter). The key is (k0, k1) = (s mod 2³², ⌊s / 2³²⌋). A seed is given as an integer or as a decimal string (a string of digits is accepted for **any** seed); **a seed above 2⁵³ − 1 must be given as a decimal string**, because a number that large does not survive a JSON number or a JavaScript double (a number above 2⁵³ − 1 given as a number is `InvalidOption`). The sealed parameters (SIM-DET-02) hold the seed as a decimal string, whatever its input form. A seed that is negative, fractional, not a number, or ≥ 2⁶⁴ is `InvalidOption` naming the seed. Seed 0 is a valid seed.
- Status: `assumed`
- Sources: `AGENTS.md` rule 10; design choice; D (a double holds integers exactly up to 2⁵³).

### SIM-RNG-03 The streams: what the counter holds
There is one stream for each (seed, replicate r, purpose p, subject label λ). Its counter and key are:

- **key** (k0, k1) from the seed (SIM-RNG-02);
- **c0** = the block index b (0 ≤ b < 2³²), counted from 0;
- **c1** = p·2²⁴ + r, with the purpose p in the top 8 bits (**p = 1 for the random effects η, p = 2 for the residual errors ε**; p = 0 is never used, and p = 3 to 255 are reserved for later draws such as parameter uncertainty or dose variability) and the replicate r in the low 24 bits (**r < 2²⁴ = 16 777 216**; a request for more replicates is refused, SIM-REF-06);
- **(c2, c3)** = the first 8 bytes of the **SHA-256** (FIPS 180-4; S-61) of the label's UTF-8 bytes, read as a big-endian 64-bit integer: c2 is the high 32 bits and c3 the low 32 bits. The label is the text of the subject cell **as imported, before any numeric conversion** (so "01" is not "1"), hashed exactly as it is held, with no trimming, case folding or Unicode normalisation ("1", "01" and "１" are three subjects).

Each block gives four 32-bit output words (r0, r1, r2, r3), used as **two 64-bit words**: **word n of the stream** is block ⌊n/2⌋, half n mod 2: x_n = r_{2h} + 2³²·r_{2h+1} with h = n mod 2 (so word 0 is r0 + 2³²·r1, word 1 is r2 + 2³²·r3, word 2 is the first half of block 1, …). The stream has 2³³ words (blocks are counted in 32 bits; a design does not approach that: SIM-REF-06).

**Properties (D).** Philox is a bijection of the counter for a fixed key, so two streams that differ in any field of (b, p, r, hash of the label) the same output block cannot occur twice for the same key. They are not *proven* independent; the statistical independence rests on the quality of the generator (SIM-RNG-01) and is tested (SIM-ORA-04). **A subject's draws depend only on (s, r, λ) and on the position of the draw**: adding, removing or reordering other subjects changes nothing in this subject's draws; the number of threads and the order of evaluation do not enter; changing R does not change replicate r's draws (replicates 0..R−1 of a run of R′ > R replicates are the same as those of the shorter run). The replicate index enters both streams, so each replicate has a new η and a new ε for every subject. The only coupling between subjects is the label: **two subjects of one run must have different 64-bit keys**; equal labels are refused as one subject (SIM-MOD-04) and two different labels with the same 64-bit hash are refused (`label_key_collision`, SIM-REF-06). The probability that m labels contain a collision is about m²/2⁶⁵ (D, the birthday bound): 2.71e-8 for m = 10⁶ and 2.71e-10 for m = 10⁵ (H40).
- Status: `assumed` for the layout (Caladrius's rule, with its reason above); `documented, untested` for the SHA-256 function itself
- Sources: S-59 (a counter and a key as the address of a draw); S-61 (SHA-256); H40; D.

### SIM-RNG-04 From a word to a uniform
u_n = (⌊x_n / 2¹²⌋ + 0.5) · 2⁻⁵², that is: take the top 52 bits of the 64-bit word as an integer m (0 ≤ m < 2⁵²), that is the 32 bits of r_{2h+1} (high part) and the top 20 bits of r_{2h} (low part), m = r_{2h+1}·2²⁰ + ⌊r_{2h} / 2¹²⌋, and set u = (m + 0.5) / 2⁵². **This is exact in double precision** (m + 0.5 has 53 significant bits), it lies in [2⁻⁵³, 1 − 2⁻⁵³], it is never 0 or 1 (so the inverse of SIM-RNG-05 is always finite), and 1 − u is exact too (a multiple of 2⁻⁵³ in (0, 0.5]). The low 12 bits of r_{2h} are discarded. Only integer operations, one conversion of a 52-bit integer to double (exact) and one multiplication by a power of two are used.
- Status: `assumed`
- Sources: design choice; D (the representability argument); H40.

### SIM-RNG-05 From a uniform to a normal: inversion, with Wichura's AS 241
**z_n = Φ⁻¹(u_n)**, the inverse of the standard normal distribution function, by the algorithm **PPND16 of Wichura, *Algorithm AS 241: the percentage points of the normal distribution*, Applied Statistics 37 (1988) 477-484** (S-63), the algorithm that R uses for `qnorm` and for its default `normal.kind = "Inversion"` (S-62, S-64). **One normal is made from one uniform**, so that the number of words consumed does not depend on the values drawn (SIM-RNG-06), and the transform is monotone. The Box-Muller transform (two uniforms to two normals) and the rejection methods (a variable number of uniforms) were set aside for exactly this reason: they make the position of a draw depend on earlier values.

The computation, with q = u − 0.5 (exact, for u ≥ 0.25), in double precision, every product and sum rounded separately (no fused multiply-add), polynomials by Horner's rule with the coefficients taken from the highest degree down (poly(x; c) = (…((c7·x + c6)·x + c5)·x + … + c1)·x + c0):

1. If |q| ≤ 0.425: ρ = 0.180625 − q·q and **z = q · poly(ρ; A) / poly(ρ; B)**.
2. Otherwise: p̃ = u if q < 0, else 1 − u (exact); t = √(−ln p̃) (`ln` from the `libm` crate, `sqrt` the exact IEEE square root); if t ≤ 5: t′ = t − 1.6 and v = poly(t′; C) / poly(t′; D); else t′ = t − 5 and v = poly(t′; E) / poly(t′; F). **z = −v if q < 0, else v.**

Coefficients (degree 0 first for each list; c7 is the last entry; as published and as used in H40):

| list | c0 | c1 | c2 | c3 | c4 | c5 | c6 | c7 |
|---|---|---|---|---|---|---|---|---|
| A | 3.387132872796366608 | 133.14166789178437745 | 1971.5909503065514427 | 13731.693765509461125 | 45921.953931549871457 | 67265.770927008700853 | 33430.575583588128105 | 2509.0809287301226727 |
| B | 1 | 42.313330701600911252 | 687.1870074920579083 | 5394.1960214247511077 | 21213.794301586595867 | 39307.89580009271061 | 28729.085735721942674 | 5226.495278852854561 |
| C | 1.42343711074968357734 | 4.6303378461565452959 | 5.7694972214606914055 | 3.64784832476320460504 | 1.27045825245236838258 | 0.24178072517745061177 | 0.0227238449892691845833 | 7.7454501427834140764e-4 |
| D | 1 | 2.05319162663775882187 | 1.6763848301838038494 | 0.68976733498510000455 | 0.14810397642748007459 | 0.0151986665636164571966 | 5.475938084995344946e-4 | 1.05075007164441684324e-9 |
| E | 6.6579046435011037772 | 5.4637849111641143699 | 1.7848265399172913358 | 0.29656057182850489123 | 0.026532189526576123093 | 0.0012426609473880784386 | 2.71155556874348757815e-5 | 2.01033439929228813265e-7 |
| F | 1 | 0.59983220655588793769 | 0.13692988092273580531 | 0.0148753612908506148525 | 7.868691311456132591e-4 | 1.8463183175100546818e-5 | 1.4215117583164458887e-7 | 2.04426310338993978564e-15 |

A literal is read to the nearest double **by a correctly rounded parser (Rust, C, Python); R's decimal parser is not one** (R 4.5.2 reads `"2.58859887947042"` as 0x1.4b573544e4d36p+1, one ulp from the nearest double 0x1.4b573544e4d35p+1), so the coefficients are exchanged as hexadecimal floats or bit patterns when a tool other than the engine is involved (SIM-ORA-01 item 3). **Accuracy (measured, H40):** against R's `qnorm` the implementation is identical on 10⁵ random probabilities and on 17 special ones; against a 256-bit inverse (Newton iteration on `erfc`, `Rmpfr`) the maximum relative error over 318 points spread over the three regions is 4.7e-16 (central region 4.7e-16, first tail 3.6e-16, second tail 2.0e-16). The largest value the generator can produce is at u = 2⁻⁵³: z = −8.2095361516013856 (256-bit reference −8.20953615160138685563076877867), so |z| ≤ 8.2096 always and a normal tail beyond that is not sampled (a consequence of 52 bits of uniform, noted for the user in the documentation of the output).
- Status: `documented, untested`
- Sources: S-63 (the algorithm, its three regions and constants: **transcribed via the C source of R's GPL `src/nmath/qnorm.c`, which implements AS 241; no code was taken, only the numerical constants; Wichura's paper itself was not read**. Decision taken meanwhile (2026-11-08), reversible by Abdou: the constants are cited to Wichura (1988) as numerical constants of a published algorithm; see `specs/sources.md` S-63); S-62, S-64 (R's use of it, for comparison); H40 (the checks above).

### SIM-RNG-06 What is drawn, in what position
For subject λ in replicate r:
1. **The η stream** (p = 1): the k-th random effect (k = 1..q, in the order of `random_effects`) uses the normal of word k − 1: z^η_k = z_{k−1}. Words q, q+1, … of the η stream are not used in v1 (reserved).
2. **The ε stream** (p = 2): observation row j of the subject (j = 1..n_i, every observation row of the design for that subject, in table order, SIM-MOD-04) uses the normal of word j − 1: z^ε_j = z_{j−1}.

**Consumption is fixed**: the position of a draw is a function of the model and the design alone and **never of a value**, so a draw is the same whatever happens to the other draws. In particular an ε is drawn for every row, also when σ = 0, when the row will be BLQ, and when the row is before the dose; and a random effect is drawn for every name of `random_effects` whatever its Ω entry. A different `error_model`, a different θ or a different LLOQ never changes a z.
- Status: `assumed`
- Sources: design choice with the reason above; S-59 (random access to a draw by its counter); D.

### SIM-RNG-07 Correlated random effects: the Cholesky factor and its convention
η = L·z^η, with **L the lower-triangular Cholesky factor of Ω**, Ω = L·Lᵀ, L_kk > 0, and z^η the vector of the q standard normals of SIM-RNG-06. Then η has mean 0 and covariance L·Lᵀ = Ω (D). The convention is fixed to the bit, because the order of operations changes the last digit:
1. Ω is given as variances (diagonal) or as the lower triangle by rows (SIM-MOD-03); the matrix used is Ω_ij for i ≥ j and Ω_ji above the diagonal.
2. **Cholesky-Banachiewicz, row by row**: for i = 1..q, for j = 1..i: s = Ω_ij − T where T = Σ_{l<j} L_il·L_jl is accumulated from 0, **left to right in l**, and then subtracted once (a sum of no terms is 0); if i = j then L_ii = √s, after checking that s is finite and **s > 0** (else `omega_not_positive_definite` naming the random effect i, SIM-REF-02); otherwise L_ij = s / L_jj.
3. **η_i = Σ_{j ≤ i} L_ij·z_j accumulated from 0, left to right in j.**
4. For the diagonal shape L = diag(√Ω_kk) and η_k = L_kk·z_k, with Ω_kk > 0 checked in the same way.
The convention is *the lower factor applied to a column of normals*, not the upper factor: with the other convention the second effect would be built from the first normal in a different way and the draws would not be the same. Because the 256-bit worked example (SX2) uses the same factor, the convention is testable in the exact part (SIM-ORA-03).
- Status: `assumed`
- Sources: D (covariance of L·z); H41 (the factor of Ω = [0.09 0.03; 0.03 0.04] and the draws that use it, at 256 bits); the convention is Caladrius's own (the lower factor, Banachiewicz order); no source is relied on for it.

### SIM-RNG-08 What the unused words mean
The streams are infinite for this purpose (2³³ words each), and v1 uses only the first q words of an η stream and the first n_i words of an ε stream. A later extension (for instance inter-occasion variability or a residual error with an autocorrelation) draws from other purposes p ≥ 3 or from later words of these streams and must not move the draws of v1: that is why the purpose is in the counter and why the positions are fixed by SIM-RNG-06.
- Status: `assumed`
- Sources: design choice.

## 3. What is simulated, and in what order (card item 3)

### SIM-ORD-01 The order of the computation
For each replicate r = 0..R−1 and each subject (in design order), **in this order**:
1. the standard normals z^η (SIM-RNG-06, -05, -04, -03) and η = L·z^η (SIM-RNG-07);
2. the individual parameters P_k = θ_k·exp(η_k) for the parameters with a random effect, P_k = θ_k for the others (`exp` from `libm`);
3. for each observation row j: PRED_j (the model at the typical values, every η = 0, which does not depend on r) and IPRED_j = f(P, dose, t_j) (SIM-ORD-02);
4. σ_j = g(IPRED_j; ξ) (SIM-ORD-03);
5. z^ε_j and **y_j = IPRED_j + σ_j·z^ε_j**, the raw observation;
6. the negative-value rule (SIM-ORD-04), then the BLQ rule (SIM-ORD-05).
Steps 1, 2, 3, 4, 5 are pure functions of their inputs (SIM-DET-01). The order matters in two places only: the negative rule comes before the BLQ rule, and the draws of step 1 and step 5 are positions of fixed streams, so their order of evaluation does not.
- Status: `assumed`
- Sources: card T-157 item 3; `specs/population.md` POP-MOD-01; D.

### SIM-ORD-02 The predictions
IPRED_j and PRED_j are values of the structural model of `specs/models.md` (the `pk1.*`, `pk2.*` and `pk3.*` functions, MOD-GEN-01 to -05) at the parameters of the subject and at the typical values respectively, with the dose, dose time and, for an infusion, duration of the subject. A time before the dose gives 0 (MOD-GEN-02). Values are doubles; no rounding is applied between the steps. The model may refuse the parameters (MOD-GEN-04, MOD-2C-22, MOD-3C-26): SIM-REF-05.
- Status: `documented, untested`
- Sources: `specs/models.md` MOD-GEN-01 to -05; `specs/population.md` POP-DIA-01 (PRED and IPRED as defined there).

### SIM-ORD-03 The error, and σ = 0
σ_j is the standard deviation of POP-ERR-01 on the individual prediction: `additive` a; `proportional` b·|IPRED_j|; `combined` √(a² + b²·IPRED_j²); `combined_sd` a + b·|IPRED_j|, with a ≥ 0 and b ≥ 0 given by the user (SIM-MOD-05). The simulated observation is y_j = IPRED_j + σ_j·z^ε_j. **σ_j = 0 is allowed** in a simulation (for instance a proportional error on an oral profile at time 0, or on a pre-dose time, where IPRED = 0): then y_j = IPRED_j exactly, z^ε_j is drawn all the same (SIM-RNG-06), and the number of such rows is returned (`n_zero_sigma`), because a fit of the table will refuse these rows (POP-INN-03 item 2, FIT-ML-13c, FIT-ML-14). All error parameters equal to 0 is allowed and gives y = IPRED with every row counted in `n_zero_sigma`.
- Status: `assumed`
- Sources: `specs/population.md` POP-ERR-01; FIT-ML-02, -13c, -14; design choice for σ = 0.

### SIM-ORD-04 Negative simulated values
Under `additive`, `combined` or `combined_sd` (and under `proportional` when b is large) the sum IPRED_j + σ_j·z^ε_j can be negative although a concentration cannot. The option `negative` has two values, **applied to y_j before the BLQ rule**:
- `keep` (the default): y_j is left as drawn, negative or not. This is the honest distribution of the model that the fit assumes (the fit treats the residual as Gaussian whatever its sign), and a fit of the simulated data sees exactly the model that generated them.
- `zero`: y_j = max(y_j, 0). The distribution is censored at 0 (a point mass), which the fit's model does not describe; the user who chooses it is told in the result (`negative = zero`, and the number of values changed, `n_negative`).
The count `n_negative` is returned under both values (the number of rows with raw y < 0). **No resampling and no truncation of the distribution** is offered: a rejection step would make the number of draws depend on the values, which SIM-RNG-06 forbids. When an LLOQ is given (SIM-ORD-05) the two values give the same `cens` and the same DV, because every y < 0 is below any LLOQ > 0 (the only difference is the `y` column, SX4).
- Status: `assumed`
- Sources: design choice with the reasons above; `specs/fit.md` FIT-ML-02 (the Gaussian residual); D.

### SIM-ORD-05 BLQ, and the limit of quantification
If `lloq` is given (a finite number **> 0**; one number for the whole run in v1, a limit per row or per subject is `later`, OS-07), then after the negative rule **a row is BLQ if and only if y_j < LLOQ** (strictly below; a value equal to the limit is quantified). A BLQ row is exported as a censored row of the dataset: `cens` = 1 and the value cell holds the **limit** (the convention of DAT-CEN-02 (b): DV carries the limit on a censored row), so that the dataset read back has status `blq`, value null and limit = LLOQ, and the fit leaves it out as M1 (POP-DAT-02, FIT-ML-19). The raw value is kept in the table of observations (`y`) and is **not** in the dataset. The counts `n_blq` (total and per subject) are returned. There is no upper limit in v1 (right censoring is refused by the dataset, DAT-CEN-02). Without `lloq` no row is flagged and `cens` is 0 for every row.
- Status: `assumed` for the strict inequality and the export; `documented, untested` for the `cens` and limit semantics of the dataset (DAT-CEN-02)
- Sources: `specs/data.md` DAT-CEN-02 (b); `specs/population.md` POP-DAT-02; FIT-ML-19; design choice for "strictly below".

### SIM-ORD-06 Replicates
A **replicate** is one complete virtual study of the same design with the same θ, Ω and ξ: new η and new ε for every subject (SIM-RNG-03), the same doses and times. R replicates cost R times the work; they are independent in the sense of SIM-RNG-03. Uncertainty in θ, Ω and ξ (a different parameter set per replicate, as `nStud` of the open simulation tools does, S-65) is **not** in v1; with it a replicate would draw its parameters from the covariance of a fit (purpose p = 3 is reserved), and a summary over replicates would then include the uncertainty of the estimates (OS-09).
- Status: `assumed`
- Sources: design choice; S-65 (the notion of a simulated study with and without parameter uncertainty, from a summary of the open tool's page).

## 4. The outputs (card item 4)

### SIM-OUT-01 The tables returned
The result holds, in this order of rows (replicate ascending, then subject in design order, then observation row in table order) and with the inputs repeated (SIM-MOD-05):
1. **The subject table**, one row per (replicate, subject): `rep`, `id` (the label), `eta_1..eta_q` (named after the parameter: `eta_cl`, `eta_v`, …) and the individual parameters of the model P_k (`cl`, `v`, …, all of them, also those without a random effect).
2. **The observation table**, one row per (replicate, subject, observation row): `rep`, `id`, `obs` (j, from 1), `time` (as in the design), `pred`, `ipred`, `sigma`, `eps` (z^ε_j), `y_raw` (before the negative rule), `y` (after it), `cens` (0 or 1), `dv` (the value written to the dataset: `y`, or LLOQ on a BLQ row).
3. **The counts**: `n_dv_ignored`, `n_zero_sigma`, `n_negative`, `n_blq` (total and per subject), the number of subjects, replicates and rows, and the seed, the options and the model id as given.
4. **The summary** of SIM-SUM-01.
`eta` and `eps` are in the tables so that anyone can replay the exact part (SIM-ORA-03) and so that the table is a complete record of the draws; they are the standard-normal `eps` and the η on the scale of Ω. The tables are produced for the whole run and returned in memory or written by the caller; the library never writes a file by itself.
- Status: `assumed`
- Sources: design choice; `specs/population.md` POP-DIA-01 (PRED, IPRED); `specs/data.md` DAT-CEN-02.

### SIM-OUT-02 The simulated dataset
For each replicate the result can be written as a **dataset in the event layout of `specs/data.md`** (DAT-LAY-01): per subject the dose row of the design, then one observation row per row of the design with `dv` and `cens` as in SIM-OUT-01, in the order of the design; **one dataset per replicate**. When several replicates are written in one table, the subject column holds `{rep}:{label}` (so that each replicate's subjects are distinct subjects of the dataset) and the original label is kept in a column `id`. A table written by the simulator and read by the dataset reader is a valid input of the population fit (POP-DAT-01) for the models the fit accepts (SIM-MOD-02), with the pre-dose rows left out by the fit (`before_dose`). A table with negative values (`negative = keep`) is also a valid input of the fit; NCA refuses it under its default policy (NCA-DAT-04). Numbers are written as **the shortest decimal string that reads back to the same double** (so that writing and reading are the identity); a sealed file instead fixes the number of significant digits once, as in POP-ORA-04 item 2, and then the sealed text, not the double, is the data.
- Status: `assumed`
- Sources: `specs/data.md` DAT-LAY-01, DAT-CEN-02; `specs/population.md` POP-ORA-04 item 2; design choice.

### SIM-SUM-01 The summary: quantiles over replicates, type 7
For each (subject, observation row), **for any R ≥ 1** (the shape of the result then does not depend on R; with R = 1 every quantile equals the value), the result gives the **quantiles over replicates** of `y` (after the negative rule, before the BLQ replacement, so BLQ rows enter with their simulated value) and of `ipred`, at each probability p of `quantiles` (default 0.05, 0.5, 0.95; each 0 ≤ p ≤ 1, else `InvalidOption`), and `blq_fraction`, the share of replicates in which the row is BLQ (0 without an LLOQ). **The quantile definition is the continuous sample quantile of type 7**, the default of R's `quantile` (Hyndman and Fan 1996, type 7; S-66, S-67): with the n = R values sorted ascending x_(1) ≤ … ≤ x_(n),

h = 1 + (n − 1)·p (computed in double),  lo = ⌊h⌋,  hi = ⌈h⌉,  g = h − lo,  **Q(p) = (1 − g)·x_(lo) + g·x_(hi)**

evaluated in exactly this form (the same expression as R's code, so the double result is identical; the form x_(lo) + g·(x_(hi) − x_(lo)) differs in the last digit, SX5). For p = 0 and p = 1 it is the minimum and the maximum, for R = 1 every quantile is the value. Reasons for type 7: it is the default of R and of the open tools used as second looks, it is linear in the order statistics (so it is a reduction of fixed form), and `specs/nca.md` and `specs/data.md` define no quantile (no NCA or DAT convention exists to follow). The sort is a total order on finite doubles (no NaN can occur: SIM-REF-05); there are no sums, so the result does not depend on the order of evaluation. This summary is the building block of a VPC and no more (SIM-LAT-01): it is across replicates for one design row, not across subjects.
- Status: `documented, untested`
- Sources: S-66 (the installed R help of `quantile`, read in full: the types, the formula of type 7); S-67 (Hyndman and Fan 1996, cited by that help; the paper itself was not read); H44.

## 5. Determinism (card item 2, determinism and streams)

### SIM-DET-01 Each (subject, replicate) is a pure function; threads sit outside the library
Layers L0 to L3, which hold the simulator, compile for WebAssembly and contain no threads (`AGENTS.md` golden rule 9). The contract that makes a parallel evaluation safe, as POP-DET-02 says for the fit: **everything about subject i in replicate r (z, η, P, PRED, IPRED, σ, ε, y, cens) is a pure function of (s, r, λ_i, θ, Ω, ξ, the design rows of subject i, the model id and the options)** and of nothing else: no global state, no shared random number, no clock, no value from another subject, no dependence on the order in which subjects or replicates finish. The library exposes the per-(replicate, subject) evaluation and the assembly separately: the caller (any number of threads, the order of its choice) fills an array indexed by (replicate, subject) and the library then assembles the tables and the summaries **in index order** (SIM-OUT-01). A parallel evaluation that sums per thread is forbidden; the simulator has no sums over subjects, but the counts (`n_blq`, …) are integer sums, which are associative, and the summary sorts values, so they cannot depend on the grouping either.
- Status: `assumed`
- Sources: `AGENTS.md` rules 9 and 10; `specs/population.md` POP-DET-02; D.

### SIM-DET-02 The same digest on 1 thread, on N threads and in WebAssembly
**Caladrius's rule, with its reason**, the one POP-DET-03 gives for the fit: the answer of a simulation is the same, bit for bit, whatever the number of threads and the target, and a test proves it. Reason: a user who reruns a project on a laptop, in the browser and in a cloud session must get the same simulated study from the same call, and a user who seals a simulated dataset (SIM-ORA-06) needs the same bytes later. The simulation workload is added to the fixed workload of `cargo xtask digest`: the design of SX2 (three subjects, times 0.5 to 12), `pk1.iv_bolus`, two error models, with and without LLOQ, 20 replicates, seed 20261108, plus one run with a seed above 2⁵³ given as a decimal string; the full result in canonical text (the tables of SIM-OUT-01 and the summary, numbers as the shortest round-trip decimal) is hashed with SHA-256, and CI compares the digest on Linux, Windows, macOS and in WebAssembly under Node; a threaded test compares 1 thread against N threads for one N that does not divide the number of (replicate, subject) cells and one N larger than that number. The seed, the replicate count and every option are part of the recorded call, so that `project.replay` and the seals (`AGENTS.md` rule 10) cover them: **a result is never stored without its seed**. The functions `exp`, `ln` and `pow` of the models and of the transform come from `libm` (rule 10); `sqrt` and the four arithmetic operations are exactly rounded by IEEE 754 and need nothing; the generator uses integers only.
- Status: `assumed`
- Sources: `AGENTS.md` rule 10 (the digest and the seals); `specs/population.md` POP-DET-03, -04 (this file is the specification that POP-DET-04 promises); design choice.

### SIM-DET-03 What "the same bits" does not promise
The simulator is deterministic for a given engine version. A change of the **model code** (a corrected closed form, a different order of operations in a sum) changes IPRED in the last digits and therefore y, and the digest, without any change of the draws. The draws themselves (the words, the uniforms and the normals) are fixed by this specification and are changed only by a new version of it: any change of the layout of SIM-RNG-03, of the uniform of SIM-RNG-04 or of the transform of SIM-RNG-05 is a breaking change that moves every simulated study, must be recorded as such, and re-seals the datasets that depend on it. The tables carry the version of the specification of the generator (`rng = "philox4x32-10/as241/v1"`) so that a result says which stream it used.
- Status: `assumed`
- Sources: design choice; D.

## 6. What can be tested, and how (card item 5)

There are five layers, in order of strength. The engine writes its own simulator, so none of them is "R against the engine at the same seed": R's stream is not ours (SIM-RNG-01). Instead the oracle script (the card T-158) carries **its own implementation of the generator and the transform in R**, as the scripts of this file do (H39 to H41), checked against the published test vectors; two implementations written independently agreed on every word (an R one in 16-bit limbs and a Python one with native 64-bit integers).

### SIM-ORA-01 (a) The generator, bit for bit
1. The three known-answer vectors of SIM-RNG-01 (the counter and key in, four 32-bit words out), exact equality.
2. The streams of SIM-RNG-03: for seeds, labels, replicates and purposes listed in a committed file (the examples of SX1 are candidates, including the seed above 2⁵³ and the replicate 2²⁴ − 1), the first words of the stream in hexadecimal, exact equality. This tests the layout (which field of the counter is which), the SHA-256 key, the 32/64-bit assembly of a word and the order of the halves.
3. The uniforms of SIM-RNG-04 for the same words, exact equality of the doubles. **Doubles are exchanged between tools as hexadecimal floats (`%a` in R, the bit pattern in Rust) or as 64-bit patterns, and compared as parsed values, never as strings** (R drops a trailing zero of a hexadecimal mantissa). **R never reads a decimal written by another tool as a bit-exact value**: R 4.5.2 reads `"2.58859887947042"` as 0x1.4b573544e4d36p+1, one ulp from the nearest double 0x1.4b573544e4d35p+1, which Rust's correctly rounded parser (and C, Python) returns. Rust may read decimals written by R; the converse is what fails. Bit-exact targets are the vectors, the words, the uniforms, the Cholesky factor and the z of the central region (|q| ≤ 0.425); everything after an `ln` or an `exp` is a tolerance target (SIM-ORA-02, -03).
- Status: `documented, untested` for the vectors; `assumed` for the stream layout
- Sources: S-60; H39, H40; SX1.

### SIM-ORA-02 (b) The normal transform, against 256 bits
For a list of uniforms (the probabilities of SX1 and the extremes 2⁻⁵³ and 1 − 2⁻⁵³, the region boundaries |q| = 0.425 and t = 5 on both sides, and a grid), the transform of SIM-RNG-05 against the value of the inverse at 256 bits (`Rmpfr`; the oracle script solves Φ(z) = u by Newton on `erfc`): relative error at most a bound **tol_z**. **The list must include these points exactly** (given as hexadecimal doubles in the case file): u = 0.075 and 0.925 (the edges |q| = 0.425; the double 0.925 falls in the tail branch, see section 9), u = exp(−25) = 0x1.e8a37a45fc32ep-37 and 1 − exp(−25) = 0x1.ffffffffe175dp-1 (the edges t = 5), u = 2⁻⁵³ and 1 − 2⁻⁵³, and the two worst points found by the review, u = 0.0048183637779074528 and 0.013215994403526499. Measured on the R double implementation of the algorithm: 4.7e-16 over 318 points (H40); **that is a sample maximum, not a bound**: the review measured 6.3e-16 on 2000 random points. The bound is open item OS-05 (proposal 1e-14, to be replaced by what the oracle measures on the engine's `libm`).
- Status: `documented, untested`
- Sources: S-63; H40.

### SIM-ORA-03 (c) The exact part: draws given, the rest is a function
With the standard normals z^η and z^ε **given** (written in the case file as hexadecimal floats or 64-bit patterns, SIM-ORA-01 item 3; a 17-digit decimal is for reading and is parsed only by a correctly rounded parser), η, P, PRED, IPRED, σ and y are a deterministic function of (θ, Ω, ξ, design). The oracle computes them at 256 bits (`Rmpfr`) from the exact doubles of the case file (the decimal inputs 0.09, 0.03, … are parsed by `Rmpfr` from strings, exact to 256 bits) and the engine, fed the same z through a test entry point that bypasses the generator, matches within **tol_exact**. This tests the Cholesky convention (SIM-RNG-07), the log-normal link, the error models, the negative rule and the BLQ rule independently of the generator, and the forward function through the model oracle. Measured on the double implementation of SX2 against 256 bits: the largest relative difference over η, IPRED and y for three subjects and six times each is 2.4e-16 (H41). The bound is open item OS-05 (proposal 1e-12, a small multiple of the model oracle's tolerance rather than of the rounding; the oracle card decides).
- Status: `assumed` (becomes `confirmed by oracle` when T-158 produces the files)
- Sources: H41, example SX2; `AGENTS.md` section 5 (independent computation, 256 bits).

### SIM-ORA-04 (d) The distribution of the draws, with stated bounds and a fixed seed
Statistical checks of the generator and the transform, **each with a bound derived from the null distribution at a stated false-alarm probability, evaluated at a fixed seed so that the test is deterministic**: a correct implementation passes with probability 1 − (the false-alarm level) per check if the seed were chosen at random, and with a fixed seed it either always passes or always fails, so a failure is a change of the stream and never bad luck. The false-alarm level is **about 1e-6 per check**: 2·Φ(−5) = 5.7e-7 for a 5-SD moment bound (R: 5.73303e-07) and 2e-6 in total for the two χ² bounds (R: 2e-06), and about 10⁻⁵ over the whole list. On N = 10⁶ words of one stream (SX3 gives the values the reference implementation obtains, which are the targets):
1. *Uniforms*: |mean − ½| ≤ 5·√(1/(12N)) (0.001443 at N = 10⁶); the χ² statistic of 100 equal bins (99 degrees of freedom) between qchisq(1e-6, 99) = 45.8344 and qchisq(1 − 1e-6, 99) = 180.7920.
2. *Normals*: |mean| ≤ 5/√N = 0.005; |variance − 1| ≤ 5·√(2/N) = 0.007071; |skewness| ≤ 5·√(6/N) = 0.012247; |excess kurtosis| ≤ 5·√(24/N) = 0.024495 (the large-sample standard errors of these moments of a normal sample).
3. *Quantiles*: for each p in a list (0.001, 0.025, 0.1587, 0.5, 0.84, 0.975, 0.999) the number of z below qnorm(p) is inside the exact binomial interval for N and p with probability 5e-7 in each tail (`qbinom`); also the counts of |z| > 3 and |z| > 4.
4. *Independence*: the sample correlation of two streams of N words is within 5/√N of 0 for: the η stream against the ε stream of one subject, the η streams of two subjects, the η streams of two replicates; and the lag-1 and lag-2 autocorrelations of one stream.
5. *Correlated η*: for m = 10⁵ subjects (labels "1" to "100000", replicate 0), Ω = [0.09 0.03; 0.03 0.04], the mean of each η is within 5 standard errors of 0 (√(Ω_kk/m)), and each element of the sample covariance S (with the sample mean removed) is within 5 standard deviations of Ω_kl, where, because (m − 1)·S is Wishart, Var(S_kl) = (Ω_kk·Ω_ll + Ω_kl²)/(m − 1) (D; 5 standard deviations are 0.002012, 0.001061 and 0.000894 for the three elements).
Item 5 also checks the structure of the streams across many labels. These tests need no R, only the formulas; the targets come from the reference implementation of this file.
- Status: `assumed`
- Sources: D (null distributions of the moments, the multinomial χ², the binomial counts, the Wishart variance); H42, example SX3; the bounds use R's `qchisq`, `qbinom` and `qnorm`, which are standard.

### SIM-ORA-05 (e) Invariance: threads, other subjects, replicate prefix
Tests that follow from SIM-DET-01 and SIM-RNG-03, each by comparing results bit for bit: (1) 1 thread against N threads, and the WebAssembly digest (SIM-DET-02); (2) a subject simulated alone equals the same subject inside a larger design, in any order of subjects; (3) a run of R replicates equals the first R replicates of a run of R′ > R; (4) changing `error_model`, `lloq` or `negative` leaves `eta`, `eps` and `ipred` unchanged; (5) appending a random effect at the end of `random_effects` (with Ω extended by a new last row) leaves the earlier η unchanged (SIM-MOD-03); (6) reversing `random_effects` changes the η (the test records that, as a guard against a silent reordering).
- Status: `assumed`
- Sources: SIM-DET-01, SIM-RNG-03, SIM-RNG-06; D.

### SIM-ORA-06 (f) Round trip with the fit: simulate, then fit (aggregates only)
This is the sealed design of POP-ORA-04 with the **simulator of this file** in the place of R. The rules of POP-ORA-04 apply (the dataset is a committed file with a fixed number of digits, its SHA-256 is committed, the commit that adds it precedes the engine commits of the fit it tests, a sealed case is never edited, a failing case stays); **in addition**: (i) the simulator is deterministic, so the committed file can be regenerated from the committed call (seed, options, design) and its hash compared, which tests SIM-DET-02 and shows that the seal is not an accident of one machine; (ii) K replicates of one design are the replicates 0..K−1 of one call, so the K datasets of an aggregate come from one seed and are independent (SIM-RNG-03); (iii) only the aggregates over the K replicates are pass/fail (bias of the mean estimate, coverage of ±c·SE) and the per-replicate |estimate − true|/SE is recorded, not asserted, exactly as POP-ORA-04 says; the design numbers are OP-13. **The honest limit of this test**: the simulator and the fit share the model functions, the error-model code and the dataset reader, so a bug in a shared function can cancel in the round trip (the fit would recover parameters of a wrong model that generated the data). The round trip therefore tests the estimator, the objective and the covariance, **not** the forward model; the forward model is tested by its own oracle (`specs/models.md`) and by SIM-ORA-03, and SIM-ORA-06 never replaces them. The agreement of the fit with the open reference tool on the same sealed data is POP-ORA-03.
- Status: `assumed`
- Sources: `specs/population.md` POP-ORA-04; SIM-DET-02; D.

## 7. Refusals (card item 6)

Each refusal stops the run **before any output is produced** (a run never returns a partial table), with an error that names what to change (`AGENTS.md` section 7). The texts below are the sentences the oracle may check.

### SIM-REF-01 Missing or invalid seed
`missing_seed`: "a simulation needs a seed: give an integer from 0 to 18446744073709551615 (a seed above 9007199254740991 must be written as a decimal string)". A seed that is negative, fractional, not a number, a number above 2⁵³ − 1, or ≥ 2⁶⁴ is `InvalidOption` naming `seed` and the reason.
- Status: `assumed`
- Sources: SIM-RNG-02; `AGENTS.md` rule 10.

### SIM-REF-02 Invalid Ω
`omega_not_positive_definite`: "omega is not positive definite: the Cholesky pivot of random effect {name} is {s} (it must be positive); remove the random effect or correct the variances and covariances". Raised by the check of SIM-RNG-07 (s not finite or s ≤ 0). A variance of 0 is a pivot of 0 and is refused, **unlike the fit** (POP-MOD-03 reads a zero variance as no random effect), with the extra advice that an effect without variance is the same as no random effect (POP-MOD-03). `InvalidOption` for: a number of Ω entries that does not fit the shape and q (named with both counts), a non-finite entry, a negative variance (pivot test would catch it, but the first message is clearer), and a `block` Ω given for q = 1 with more than one number. Ω is symmetric by construction (SIM-MOD-03). A matrix that is positive definite in exact arithmetic but whose double Cholesky pivot is ≤ 0 (a correlation too close to ±1) is refused by the same rule.
- Status: `assumed`
- Sources: SIM-RNG-07; `specs/population.md` POP-MOD-03, POP-INN-03 item 5.

### SIM-REF-03 Options
`InvalidOption` naming the option for: an unknown option, `error_model` or `negative`; a missing `error_model`; an error parameter that is negative or not finite; a missing parameter of the error model (`b` for `proportional`, both for the combined ones); an `lloq` that is not finite or not > 0; a `quantiles` entry outside [0, 1]; `replicates` < 1 or not an integer; a `random_effects` list that is empty, has a duplicate, names a parameter the model does not have (POP-MOD-02 rule 5), or names `tlag` or `dur` (SIM-MOD-03); a θ ≤ 0 for a parameter with a random effect; a parameter of the model missing in θ, or a θ for a parameter the model does not have; a model id that is not available (SIM-MOD-02).
- Status: `assumed`
- Sources: `specs/population.md` POP-INN-03 item 5, POP-MOD-02; FIT-ML-01; design choice.

### SIM-REF-04 The design
Refused with the dataset's own sentences where they exist and "in subject {S}" added: a subject with more than one dose row (POP-DAT-01), a dose row without an amount (DAT-ERR-02), time going backwards (`time_not_increasing`, DAT-ERR-01), two observations at the same time for one subject (`duplicate_times`, DAT-ORD-01), a subject with no observation row (DAT-ERR-04: nothing would be simulated for it), an observation row with an amount (DAT-ERR-03), a design with no subject, a non-finite time or dose (MOD-GEN-04), and a subject that carries any other refusal of the dataset reader (DAT-ERR-08). A covariate in the table is read and ignored (v1 has no covariate model, POP-DAT-03) and is listed as ignored.
- Status: `assumed`
- Sources: `specs/data.md` DAT-ERR-01 to -04, -08; `specs/population.md` POP-DAT-01, -03; MOD-GEN-04.

### SIM-REF-05 What the model refuses, at any draw
The model refuses a parameter set by its own rules (MOD-GEN-04: parameters out of domain; MOD-2C-22, MOD-3C-26: overflow, underflow, impossible exponents). In a simulation the parameters P_k = θ_k·exp(η_k) are random, so **a refusal can occur at a particular (subject, replicate)**: the run is refused as a whole with the model's sentence and "in subject {S}, replicate {r}, with {parameter} = {value}", with no partial output. A non-finite individual parameter (an overflow of `exp`, possible only for an absurd Ω since |z| ≤ 8.21), a non-finite IPRED, σ or y is refused in the same way (`non_finite_value`), so no NaN or infinity enters a table or a quantile.
- Status: `assumed`
- Sources: `specs/models.md` MOD-GEN-04, MOD-2C-22, MOD-3C-26; D.

### SIM-REF-06 Size limits
Two limits come from the layout and are rules: **R ≤ 2²⁴ = 16 777 216 replicates** (the replicate sits in 24 bits of c1) and **at most 2³² blocks per stream**, i.e. 2³³ words (the block index is 32 bits; no design approaches it, but the check is made: `size_limit`, "replicates {R} exceed 16777216, the replicate field holds 16777216 values"). A policy limit on the **total number of simulated observations** m·R·n and on the memory of the result protects the browser and the cloud session; its value is open item OS-04. Over the policy limit the run is refused before any work (`size_limit`: "the simulation would produce {N} observations; the limit is {L}; reduce subjects, replicates or times, or run in pieces: the replicates 0..K−1 and K..2K−1 of a seed can be run separately and are the same as in one run"). `label_key_collision` ("subjects {A} and {B} have the same 64-bit stream key; rename one") is the third case, with the probability of SIM-RNG-03.
- Status: `assumed`
- Sources: SIM-RNG-03; design choice for the policy limit.

## 8. The VPC (card item 7), `later`

### SIM-LAT-01 What a visual predictive check adds, and why it is not in v1
A VPC compares the observed data with the distribution of the same quantity under the fitted model. On top of the simulation of this file it needs: (1) **simulation from the fitted parameters of a population fit** on the design of the observed dataset (the observations' times, doses and subjects, SIM-MOD-04 item 1 already reads such a table), R replicates; (2) **binning in time** (or in another independent variable), by a rule that must be chosen and recorded (equal width, equal counts, or by nominal sampling time); (3) in each bin, the **same summary on the observed data and on each replicate** (the quantiles of the values of all subjects in the bin, type 7 as SIM-SUM-01), and then across replicates a **confidence band** for each of those quantiles (the quantiles over replicates of the replicate-wise quantile); (4) rules for BLQ rows (censored observations in the observed data against the BLQ rule of SIM-ORD-05 in the simulated), for covariate stratification, and optionally for **prediction correction** (a normalisation of values by the typical prediction in the bin, a method that changes the data and must be named). The summary of SIM-SUM-01 is the part that does not depend on these choices. The card T-160 may specify the VPC in a separate file once the population fit and this simulator exist; no rule of this section is a requirement of v1.
- Status: `later`
- Sources: D (the structure of a VPC from the definitions above); the nlmixr2 and `saemix` pages that describe a VPC were not read in this card.

## 9. Worked examples (all targets for the oracle card T-158)

**Common data.** `pk1.iv_bolus`, dose D = 100, θ = (`cl`, `v`) = (5, 50), `random_effects` = [`cl`, `v`], Ω = block, lower triangle by rows (0.09, 0.03, 0.04) (so Ω = [0.09 0.03; 0.03 0.04], the variances of PX2 with correlation 0.5), times 0.5, 1, 2, 4, 8, 12, subjects "1", "2", "3", seed 20261108 (hexadecimal 013528f4, so the key is (k0, k1) = (0x013528f4, 0)), replicate 0 unless stated. Error models for SX2: `proportional` with b = 0.1, and `additive` with a = 0.2, on the same ε. The scripts are `simlib.R` (generator, transform, key), `sx1.R` to `sx5.R` of the session scratchpad.

**What is a bit-exact target and what is a tolerance target.** The inputs are the doubles nearest to the decimals written above (0.09, 0.03, 0.04, 5, 50, 100, 0.1, 0.2, 0.8 give the same bits in R 4.5.2 and in a correctly rounded parser; checked). The decimals that follow are shown to 17 significant digits and are for reading; a comparison reads the hexadecimal form (SIM-ORA-01 item 3).
- *Bit-exact* (integer arithmetic, or exactly rounded operations only): the known-answer vectors, the counters, the words of a stream, the uniforms u, the Cholesky factor of Ω (L11 = 0x1.3333333333333p-2, L21 = 0x1.999999999999ap-4, L22 = 0x1.62b9586ad0a22p-3, the same bits from R and from Python), and the z of the central region of AS 241 (|q| ≤ 0.425, that is u in [0.075, 0.925] up to the rounding of the test in double: the double 0.075 is central, the double 0.925 has q = 0.42500000000000004 and is already in the tail branch), which uses only products, sums and one quotient.
- *Tolerance targets* (SIM-ORA-02, -03): every value after an `ln` or an `exp`, that is the z of the tails, exp(η), P, IPRED, PRED, σ and y. The numbers of this section come from R's `log` and `exp`, which are not the `libm` crate that the engine uses (golden rule 10), so the engine may differ from them in the last digits; the 256-bit values are the references.

### SX1 The streams and the transform (H39, H40)
**Known-answer vectors** of SIM-RNG-01: all three reproduced (R and Python agree on every word). Keys of labels (first 16 hexadecimal digits of SHA-256, which equals the output of Python's `hashlib` for the same bytes): "1" 6b86b273ff34fce1, "2" d4735e3a265e16ee, "3" 4e07408562bedb8b, "S1" 3696ad59777e09d5.

Subject "1", replicate 0, **η stream** (p = 1, so c1 = 01000000):

| block | counter (c0 c1 c2 c3) | output (r0 r1 r2 r3) |
|---|---|---|
| 0 | 00000000 01000000 6b86b273 ff34fce1 | bc2fc714 04bc16cb 0f6e941d b0671d12 |
| 1 | 00000001 01000000 6b86b273 ff34fce1 | e0c78126 c0b96feb dbcc1bce af56fc7b |

| n | word (hex) | u | z |
|---|---|---|---|
| 0 | 04bc16cbbc2fc714 | 0.018495011081710477 (0x1.2f05b2ef0bf2p-6) | −2.0858741749246219 |
| 1 | b0671d120f6e941d | 0.6890733880131229 (0x1.60ce3a241edd3p-1) | 0.49322555372575094 |
| 2 | c0b96febe0c78126 | 0.75282954701176952 (0x1.8172dfd7c18f1p-1) | 0.68342091773486213 |
| 3 | af56fc7bdbcc1bce | 0.68492105506793199 (0x1.5eadf8f7b7983p-1) | 0.48150463001583299 |

The 256-bit inverse of the same u: −2.08587417492462178278166974683, 0.493225553725750945300553494464, 0.683420917734862100864120238406, 0.481504630015832971334962323873 (relative errors of the transform 5.8e-17, 5.5e-18, 4.6e-17, 4.0e-17). Subject "1", replicate 0, **ε stream** (p = 2, c1 = 02000000), block 0: 77e3c473 5abb8cc4 c6d6da9d 702ddb8d, block 1: f336bfd1 65955cd3 9e9f9e92 960ccb3c; words 0 to 3 give z = −0.37340294508690741, −0.15553512657995675, −0.26161187860580409, 0.21760798641501161 (u = 0.35442428394880132, 0.4381997319349823, 0.39681034254609926, 0.58613271932574518).

In the hexadecimal values below the oracle compares parsed values, not strings, since R drops a trailing zero of a hexadecimal mantissa. First word of the η stream (hi lo, hexadecimal): replicate 0 subject "1" 04bc16cb bc2fc714, subject "2" 11c354cd b72ea848; replicate 1 subject "1" 0c6839e0 16c75d60, subject "2" e69c8696 0edcc929. **A seed above 2⁵³** (0x0123456789abcdef = 81985529216486895, given as a decimal string, key (0x89abcdef, 0x01234567)): word 0 of the η stream of "1", replicate 0, is f5623047 2f8d0529; word 5 of the ε stream of "S1", replicate 16777215 (the largest), is 15481cf8 2c2455f2 (R and Python agree).

**The transform's extremes and accuracy.** Smallest uniform u = 2⁻⁵³ = 1.110223e-16: z = −8.2095361516013856 (256-bit −8.20953615160138685563076877867), and z(1 − 2⁻⁵³) is its negative. Over 318 points, maximum relative error against 256 bits 4.708628e-16 (at u = 0.38704041927121591), 4.7e-16 in the central region, 3.6e-16 and 2.0e-16 in the two tails. Against R's `qnorm`: identical.

### SX2 One subject, three subjects: η, individual parameters, predictions, observations (H41)
Ω factor: L11 = 0.29999999999999999 (√0.09), L21 = 0.10000000000000001 (0.03/L11), L22 = 0.17320508075688773 (√(0.04 − L21²)). Typical-value prediction PRED at 0.5, 1, 2, 4, 8, 12: 1.90245885, 1.80967484, 1.63746151, 1.34064009, 0.898657928, 0.602388424 (9 digits).

| subject | z^η | η (cl, v) | CL_i, V_i |
|---|---|---|---|
| "1" | −2.0858741749246219, 0.49322555372575094 | −0.62576225247738659, −0.12315824562803285 | 2.6742678981535626, 44.206187138666579 |
| "2" | −1.480373728449994, −0.013535210711941937 | −0.44411211853499821, −0.15038174010942279 | 3.2069675234786281, 43.01897361867659 |
| "3" | 1.2061280786051529, 1.3303624331815078 | 0.36183842358154589, 0.35103834053564803 | 7.1798345241274584, 71.027089460061049 |

Observations (10 significant digits; z^ε from the ε stream; `y_prop` with b = 0.1, `y_add` with a = 0.2; the 17-digit values are in the script output):

| subject | t | z^ε | IPRED | y_prop | y_add |
|---|---|---|---|---|---|
| "1" | 0.5 | −0.3734029451 | 2.194727210 | 2.112775450 | 2.120046621 |
| "1" | 1 | −0.1555351266 | 2.129335791 | 2.096217140 | 2.098228765 |
| "1" | 2 | −0.2616118786 | 2.004339871 | 1.951903959 | 1.952017496 |
| "1" | 4 | 0.2176079864 | 1.775929778 | 1.814575428 | 1.819451375 |
| "1" | 8 | −1.835406745 | 1.394230685 | 1.138332645 | 1.027149336 |
| "1" | 12 | 0.0892712437 | 1.094569857 | 1.104341218 | 1.112424105 |
| "2" | 0.5 | 0.8446652458 | 2.239505422 | 2.428668661 | 2.408438471 |
| "2" | 1 | −2.416565893 | 2.157566950 | 1.636176679 | 1.674253771 |
| "2" | 2 | 1.389693162 | 2.002574151 | 2.280870511 | 2.280512783 |
| "2" | 4 | 1.169871183 | 1.725191288 | 1.927016446 | 1.959165525 |
| "2" | 8 | 2.039725011 | 1.280367251 | 1.541526962 | 1.688312253 |
| "2" | 12 | 1.671487504 | 0.9502368281 | 1.109067726 | 1.284534329 |
| "3" | 0.5 | −0.1030785394 | 1.338521852 | 1.324724564 | 1.317906144 |
| "3" | 1 | −2.165269643 | 1.272550276 | 0.9970088282 | 0.8394963479 |
| "3" | 2 | −1.136798377 | 1.150201469 | 1.019446752 | 0.9228417933 |
| "3" | 4 | −0.9596757551 | 0.9396624107 | 0.8494852874 | 0.7477272597 |
| "3" | 8 | 1.419698917 | 0.6271446573 | 0.7161803164 | 0.9110844407 |
| "3" | 12 | −1.253328174 | 0.4185656643 | 0.3661056503 | 0.1679000294 |

**256-bit values for subject "1"** (inputs parsed from the decimal strings 0.09, 0.03, 0.04, 5, 50, 100, 0.1, 0.2; the z are the doubles above): η = (−0.6257622524773865713143550, −0.1231582456280328309522156); IPRED at 0.5, 1, 2, 4, 8, 12 = 2.194727210157184274350938, 2.129335790734342907644516, 2.004339871342818624880866, 1.775929778143120224479069, 1.394230684798553405713860, 1.094569856509939153226976; `y_prop` = 2.112775449763677816107288, 2.096217139560033076543666, 1.951903959432167578775889, 1.814575428446738493601467, 1.138332644515781219552664, 1.104341217751047252699768; `y_add` = 2.120046621139802791986894, 2.098228765418351558631882, 1.952017495621657806185200, 1.819451375426122546810738, 1.027149335818279269873542, 1.112424105250653257509089. Maximum relative difference of the double computation from the 256-bit one, by subject: "1" η 1.3e-16 (v), IPRED 8.7e-17, y_prop 8.1e-17, y_add 1.1e-16; "2" IPRED 2.2e-16, y_prop 2.4e-16, y_add 2.3e-16; "3" IPRED 1.9e-16, y_prop 1.1e-16, y_add 2.4e-16. Part of these differences is the input Ω, which is the double nearest 0.09 and not 0.09.

### SX3 The distribution of the draws (H42)
Seed 20261108, replicate 0. **N = 10⁶ words of the η stream of subject "1"** (u in [1.986594e-06, 0.999999830]): uniform mean 0.5005576930 (bound 0.001443 around 0.5), variance 0.0832617157; χ² of 100 equal bins 99.6822 (bounds 45.8344 and 180.7920); normals: mean 0.001852 (bound 0.005), variance 0.999919 (bound 0.007071 around 1), skewness 0.000541 (bound 0.012247), excess kurtosis 0.011713 (bound 0.024495), largest |z| 5.099343. Counts below qnorm(p):

| p | qnorm(p) | count | expected | exact binomial interval (5e-7 per tail) |
|---|---|---|---|---|
| 0.001 | −3.090232 | 1010 | 1000.0 | [849, 1158] |
| 0.025 | −1.959964 | 24891 | 25000.0 | [24240, 25767] |
| 0.1587 | −0.999815 | 158075 | 158700.0 | [156915, 160490] |
| 0.5 | 0.000000 | 498631 | 500000.0 | [497554, 502446] |
| 0.84 | 0.994458 | 840291 | 840000.0 | [838204, 841791] |
| 0.975 | 1.959964 | 974834 | 975000.0 | [974233, 975760] |
| 0.999 | 3.090232 | 998928 | 999000.0 | [998842, 999151] |

|z| > 3: 2784 (expected 2699.80, interval [2450, 2957]); |z| > 4: 61 (expected 63.34, interval [29, 106]). Correlations of 10⁶ normals (bound 0.005): η against ε stream of subject "1" −0.001538; subject "1" against "2" −0.001217; replicate 0 against 1 0.000933; lag-1 autocorrelation −0.002105, lag-2 −0.000380. **Correlated η, m = 10⁵ subjects** (labels "1" to "100000", replicate 0, the first two words of each η stream): mean η = (0.000432, 0.000720) (standard errors 0.000949 and 0.000632); sample covariance S11 = 0.090762 (Ω 0.09, standard deviation 0.000402, distance 1.894 standard deviations), S21 = 0.030313 (0.03; 0.000212; 1.477), S22 = 0.040040 (0.04; 0.000179; 0.225); sample correlation 0.50284 (0.5).

### SX4 Negative values, BLQ, the combined error, invariance (H43)
**Negative values** (additive, a = 1.0, no LLOQ; the other values of SX2 unchanged): `keep` and `zero` for the rows that change, at t = 0.5, 1, 2, 4, 8, 12.

| subject | y (`keep`) | y (`zero`) |
|---|---|---|
| "1" | 1.821324, 1.973801, 1.742728, 1.993538, −0.441176, 1.183841 | 1.821324, 1.973801, 1.742728, 1.993538, 0.000000, 1.183841 |
| "2" | 3.084171, −0.258999, 3.392267, 2.895062, 3.320092, 2.621724 | 3.084171, 0.000000, 3.392267, 2.895062, 3.320092, 2.621724 |
| "3" | 1.235443, −0.892719, 0.013403, −0.020013, 2.046844, −0.834763 | 1.235443, 0.000000, 0.013403, 0.000000, 2.046844, 0.000000 |

`n_negative` = 1, 1 and 3 for the three subjects. **BLQ** (additive, a = 0.2, LLOQ = 0.8, y of SX2): the flags `cens` are 0 0 0 0 0 0 for subjects "1" (smallest value 1.027149) and "2" (smallest 1.284534), and **0 0 0 1 0 1 for subject "3"**, whose values at t = 4 and t = 12 are 0.747727 and 0.167900 (below 0.8) while 0.839496 at t = 1 and 0.922842 at t = 2 are above; the dataset then holds DV = 0.8 and `cens` = 1 on those two rows, `n_blq` = 2. **Combined error** (variance sum, a = 0.2, b = 0.1, subject "1", same ε): σ = 0.296931432, 0.292131322, 0.283149754, 0.267468252, 0.243800722, 0.227993052 and y = 2.083852139, 2.083899109, 1.930264532, 1.834133006, 0.946757195, 1.114923080. **Invariance**: subject "2" simulated in a run of one subject and in a run of three gives identical vectors (`identical()` TRUE). **Replicates** of subject "1": replicate 0 η = (−0.625762252, −0.123158246), y(t = 4) = 1.819451375; replicate 1 η = (−0.497975732, −0.202599477), y = 1.646151118; replicate 2 η = (0.366804870, 0.183387866), y = 0.880424710 (additive a = 0.2).

### SX5 The quantile of type 7 (H44)
Subject "1", additive a = 0.2, observation at t = 4, **replicates 0 to 18** (R = 19), y in replicate order: 1.819451, 1.646151, 0.880425, 1.483731, 1.517560, 1.430817, 0.793062, 1.839124, 1.378773, 1.428002, 1.220858, 0.862334, 1.135400, 1.386719, 1.270416, 1.675814, 1.237788, 1.145566, 1.022745; sorted: 0.793062, 0.862334, 0.880425, 1.022745, 1.135400, 1.145566, 1.220858, 1.237788, 1.270416, 1.378773, 1.386719, 1.428002, 1.430817, 1.483731, 1.517560, 1.646151, 1.675814, 1.819451, 1.839124. Type 7, n = 19:
- p = 0.05: h = 1 + 18·0.05 = 1.9, lo = 1, hi = 2, g = 0.9 (the double nearest), Q = 0.85540657059604297 (x_(1) = 0.793061827, x_(2) = 0.862333764);
- p = 0.5: h = 10, Q = x_(10) = 1.378773104187816;
- p = 0.95: h = 18.1, Q = 1.8214186412665074 (x_(18) = 1.819451375, x_(19) = 1.839124034).
The expression (1 − g)·x_(lo) + g·x_(hi) reproduces R's `quantile(type = 7)` to the bit in all three cases (`identical` TRUE); the form x_(lo) + g·(x_(hi) − x_(lo)) differs from it in the last digit for p = 0.05 (the equality test with tolerance 0 failed). For comparison, at p = 0.05 type 6 gives 0.7930618 and type 8 gives 0.8138434, so the definition matters at R = 19. Four values 1, 2, 3, 10: type 7 at p = 0.25, 0.5, 0.9 gives 1.75, 2.5, 7.9.

### SX6 Refusals (text of the checks, no run)
- `missing_seed`: the sentence of SIM-REF-01.
- `omega_not_positive_definite`: Ω lower triangle (0.09, 0.3, 0.04), that is a covariance 0.3 with variances 0.09 and 0.04 (correlation 5): after the first row L11 = 0.3, L21 = 0.3/0.3 = 1, and the pivot of the second effect is 0.04 − 1 = −0.96, so the sentence names the effect `v` and the pivot −0.96 (arithmetic, exact in decimals).
- `size_limit`: R = 16777217.
- `InvalidOption`: `negative = "clip"` ("unknown value of `negative`: use `keep` or `zero`"); `lloq` = 0.
- `non_finite_value`: not constructed here (it needs an absurd Ω); the oracle may use Ω = 10⁴ on one effect (a variance with ω = 100, so |η| up to 821 and exp(η) overflows).

## 10. Open items for Abdou (card item 8)

These are questions, never rules. The orchestrator turns them into entries of `board/QUESTIONS.md`.

- **OS-01 The generator.** Proposal (SIM-RNG-01): Philox4x32-10 with inversion by AS 241, as above. Alternative: the Threefry member of the same family, or a different transform. **Decision taken meanwhile (2026-11-08), reversible by Abdou: Philox4x32-10 and AS 241 inversion are kept for v1.** A later change is a breaking change (SIM-DET-03) that re-seals every simulated dataset.
- **OS-02 Stream addressing by label.** Proposal (SIM-RNG-03): the stream is keyed by the SHA-256 of the subject's label, so adding or reordering subjects never moves a draw. Alternative: key by the position of the subject in the design (simpler, no hash, but a subject's draws then change when another is inserted before it). The cost of the proposal: two different labels with the same 64-bit hash are refused (probability 2.71e-8 at 10⁶ subjects).
- **OS-03 Random effects on `tlag` and `dur` in a simulation.** **Decision taken meanwhile (2026-11-08), reversible by Abdou:** refused in v1, as in the fit (POP-MOD-02 rule 3; SIM-MOD-03), so that every simulated dataset can be refitted, and lifting this later moves no draw. Alternative if Abdou reverses it: allow them in the simulator only, with the note that such data cannot be refitted with that effect in v1.
- **OS-04 Size limits.** The structural limits (R ≤ 2²⁴, 2³³ words per stream) are rules. The policy limit on m·R·n and on memory (SIM-REF-06) is a number the orchestrator sets from a measurement of the digest workload on WebAssembly; no value is proposed here because none was measured.
- **OS-05 Tolerances of the exact part and of the transform.** Proposal: relative 1e-12 for η, P, IPRED and y given the z (measured on the reference double implementation: 2.4e-16 against 256 bits), and 1e-14 for the transform (measured 4.7e-16); to be replaced by what T-158 measures on the engine's `libm` (`AGENTS.md` section 5, set in `caladrius-testkit` and asked in `board/QUESTIONS.md` before any engine card uses them).
- **OS-06 Negative values.** Proposal: `keep` is the default and `zero` the only alternative (SIM-ORD-04). Alternatives not offered because they change the number of draws or the distribution: resampling until positive, truncation.
- **OS-07 Limits of quantification.** Proposal: one LLOQ for the whole run, strictly-below rule, no upper limit. Later: a limit per row or per subject (a `limit` column read from the design), and M3-style export. Whether the next card after the fit is the censored likelihood (OP-07) decides how much this matters.
- **OS-08 Several doses per subject** (multiple dosing, steady state: MOD-MD-*): refused as in the fit (POP-DAT-01, OP-09). Whether the simulator should accept them before the fit does (it only needs the forward function, which exists for `pk1.*`) is open; proposal: no, so that every simulated table can be fitted.
- **OS-09 Parameter uncertainty and simulation from a fit.** Proposal: not in v1. A later card draws θ, Ω and ξ per replicate from the covariance of a fit (the purpose p = 3 is reserved) and offers "simulate from this fit's estimates" as a convenience. Abdou to decide whether it comes before or with the VPC (SIM-LAT-01).
- **OS-10 Default quantiles and the summary's place.** Proposal: the summary of SIM-SUM-01 is in the result of every simulation (any R ≥ 1, as SIM-SUM-01 says), probabilities 0.05, 0.5, 0.95, type 7. Alternative: no summary in v1, only tables, and the summary belongs to the VPC card.
- **OS-11 The `rng` version string.** Proposal: `philox4x32-10/as241/v1` in every result (SIM-DET-03). Whether the number is bumped for a model-code change that moves IPRED (this file says no: the draws did not change) is Abdou's call.
- **Waiting on nothing external:** every check of section 6 needs only R, `Rmpfr` and `gmp` (installed) or the formulas; the Python cross-check of the generator is not needed by the oracle.
