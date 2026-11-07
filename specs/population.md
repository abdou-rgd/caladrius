# Population fit: FOCE with interaction, and FO as its special case (POP-*, card T-162)

Brick 3 of iteration 3 (T-139). Bricks 1 and 2 fit one subject at a time (`specs/fit.md`, sections 2 to 13). This file specifies the fit of many subjects at once, with random effects: one estimator, **FOCE with interaction (FOCEI)**, and **FO** as the same function evaluated with every random effect held at 0 (`AGENTS.md` section 1; any other estimator is out of scope until this one is green). Written by the `reader` agent on 2026-11-07, in its own words, from the open sources of `specs/sources.md` section 14 (keys S-48 to S-58) and from derivations (D). **When this file was written nothing in it was implemented or exercised by an oracle.** The numbers of the worked examples (section 10) come from throwaway R scripts kept outside the repository (hand checks H31 to H34): the 256-bit values of example PX1 are exact to the digits shown, the others are double-precision values good to about 1e-6, and all of them are targets for the oracle card T-152, not results. Where a number is a threshold, a step size or an iteration limit, the rule names a symbol and the value is an open item (OP-11, OP-12, OP-13): `AGENTS.md` section 5 asks that population tolerances come from what the oracle measures, through `board/QUESTIONS.md`.

**Reading guide.** The rules answer, in order, the ten items of the card: the model (1), the residual error (2), the objective and the inner problem (3), the estimation (4), the covariance (5), the diagnostics (6), the data (7), determinism (8), what the oracle can test (9), and the open items for Abdou (11). The names `ofv_convention`, the digest across 1 thread, N threads and WebAssembly, and the sealed simulate-then-fit tests are Caladrius's own rules, each with its reason (POP-OBJ-05, POP-DET-03, POP-ORA-04).

Notation added to sections 1 and 13 of `specs/fit.md`:

| symbol | meaning |
|---|---|
| m | number of subjects used; i = 1..m |
| n_i, N | observations of subject i that enter the fit; N = Σ n_i |
| y_i, t_ij | the observation vector (length n_i) and the times of subject i |
| θ | typical values (fixed effects), P_θ free entries |
| η_i | random effects of subject i, length q, η_i ~ N(0, Ω) |
| Ω | q × q covariance of the random effects; Ω = L·Lᵀ with L lower triangular (POP-EST-01); ω_k = √Ω_kk |
| ξ | error parameters, Q of them (FIT-ML-02) |
| P_i | the individual structural parameters, P_ik = θ_k·exp(η_ik) for a parameter with a random effect (POP-MOD-01) |
| ŷ_ij(η) | model prediction f(P_i(η), dose_i, t_ij) of the structural model (`specs/models.md`) |
| σ_ij(η) = g(ŷ_ij(η); ξ) | standard deviation of observation ij (FIT-ML-02), a function of η through the prediction (the interaction) |
| r_ij(η) = y_ij − ŷ_ij(η) | residual |
| G_i(η) | n_i × q matrix, G_ijk = ∂ŷ_ij/∂η_k |
| η̂_i | the conditional mode of subject i (POP-INN-01), also called the EBE |
| ŷ_ij = ŷ_ij(η̂_i), σ̂_ij, r̂_ij | the same quantities at η̂_i (IPRED, its standard deviation, its residual) |
| E_i, C_i | the linearised mean and covariance of y_i at η̂_i (POP-OBJ-02) |
| OFV | the objective function value, −2 times a log-likelihood, with its convention (POP-OBJ-05) |
| φ | the vector the outer optimiser moves (POP-EST-01) |

## 1. The model (card item 1)

### POP-MOD-01 The model
For subject i, observation j:

y_ij = ŷ_ij(η_i) + σ_ij(η_i)·ε_ij,  ε_ij independent standard normal, independent of η_i, η_i ~ N(0, Ω).

The structural model f is one of the single-dose models of `specs/models.md` (POP-MOD-04) with the dose, dose time and duration of subject i taken from the dataset (POP-DAT-01). The individual parameters are **log-normal on the typical value**: P_ik = θ_k·exp(η_ik) for every parameter k that carries a random effect, and P_ik = θ_k for the others. Consequences (D): θ_k is the median of the distribution of P_ik, not its mean (the mean is θ_k·exp(Ω_kk/2)); a parameter with a random effect must have θ_k > 0 and the domain of the structural parameter (MOD-GEN-04) is then respected by construction; the derivative of the prediction with respect to the random effect is G_ijk = (∂ŷ_ij/∂P_ik)·P_ik (the chain rule through the exponential, which makes the relative sensitivity the natural unit). The sources agree on the form of the model (a conditional Gaussian observation model and Gaussian random effects, S-56 and S-57 at abstract level, S-52 for the notation of the linearisation); the log-normal link is a choice of the card (`AGENTS.md` section 1).
- Status: `assumed`
- Sources: card T-162 item 1; S-56, S-57 (abstract level: the nonlinear mixed-effects model and its approximate likelihoods); S-52 (the notation y = f(θ, η) + h(θ, η, ε)); D (the median, the mean and the chain rule).

### POP-MOD-02 Which parameters may have no random effect
Every structural parameter may be declared with or without a random effect; a parameter without one has η = 0 for all subjects and is shared exactly. The user declares the set explicitly (`random_effects`: a list of parameter names); there is no silent default, because the choice changes the answer. Rules:
1. A parameter with a random effect must have a typical value θ_k > 0 (POP-MOD-01); an initial estimate or a bound that allows 0 is `InvalidOption`.
2. **The bioavailability F is never estimated and never has a random effect** (MOD-GEN-03: only F·D is observable; F stays fixed).
3. **The lag time `tlag` and the duration `dur` (the parameter T of the zero-order models) carry no random effect in v1.** Reason (D, and the structure of MOD-AB1-05 and MOD-AB0-02): the prediction is continuous but has a kink at t = tlag (and at the end of the input), so G_i has a jump there and an observation close to the kink makes the inner problem and the derivatives of POP-COV-02 unstable. The parameter itself may be estimated as a typical value shared by all subjects. A request for a random effect on `tlag` or `dur` is refused with `InvalidOption` naming the parameter (OP-08).
4. At least one parameter must carry a random effect (q ≥ 1): with q = 0 the fit is a pooled fit of all subjects and belongs to `specs/fit.md`.
5. A random effect on a parameter that the chosen model does not have is `InvalidOption`, naming the parameter and the model.
6. The derived parameter k = CL/V is not a parameter for this purpose: the user gives the primary set of MOD-GEN-03 (V with CL) or V with k, and the random effects go on those that are given (one of the two sets; both is an error, as in MOD-GEN-03).
- Status: `assumed`
- Sources: design choice of this card; MOD-GEN-03, MOD-GEN-04 (parameters and domains); D (the kink and its effect on the derivatives).

### POP-MOD-03 The shape of Ω: diagonal by default, one full block as an option
**Proposal.** v1 offers two shapes: `diagonal` (the default; q independent random effects) and `block` (one full q × q covariance over all the random effects). Block structures over subsets (some effects correlated, others not) are `later`. Reasons: (1) clearance and volume are often correlated in real data, and forcing a diagonal Ω when they are is a known source of bias and of misleading shrinkage, so a block is needed for real use; (2) a full block costs q(q + 1)/2 parameters and needs more subjects, so the diagonal must stay the default; (3) both shapes are offered by the open tools used as second looks (`nlme` through `pdDiag` and `pdLogChol`, S-48; the others at summary level), so both can be tested against the oracle; (4) with the log-Cholesky parameterisation of POP-EST-01 the block costs almost no extra code over the diagonal. Whether the block belongs to v1 or to a later card is open item OP-01. A random effect that the user fixes to a variance of 0 is the same as declaring no random effect for that parameter (POP-MOD-02).
- Status: `assumed`
- Sources: design choice with the reasons above; S-48 (`pdDiag`, `pdLogChol`, read in full); S-44 (`saemixModel` covariance structure, summary only).

### POP-MOD-04 Structural models in v1
The population fit accepts the single-dose models of `specs/models.md` that have closed forms and closed-form derivatives: the six `pk1.*` ids (`pk1.iv_bolus`, `pk1.iv_infusion`, `pk1.oral_1`, `pk1.oral_1_lag`, `pk1.oral_0`, `pk1.oral_0_lag`) and the six `pk2.*` ids (the same six routes with two compartments), the latter **in the clearance parameterisation only** (clearance, central volume, intercompartmental clearance, peripheral volume: MOD-2C-02). Reason for the restriction: the clearance set is the one with positive, independent parameters, so the log-normal link of POP-MOD-01 applies to each; the micro-constant and macro sets have ordering constraints (α > β) that a log-normal random effect does not respect. The three-compartment models (`pk3.*`, MOD-LT-01) and Michaelis-Menten elimination (MOD-LT-02) are `later` and are not named by this version. The derivatives ∂ŷ/∂P come from the closed forms of FIT-JAC-01 to 03 (the same functions as the individual fit).
- Status: `assumed`
- Sources: design choice; MOD-GEN-05 (ids), MOD-2C-02, MOD-LT-01 (status `later`); FIT-JAC-01 to 03.

### POP-MOD-05 A test-only model with the identity link
For the exact cases of POP-ORA-01 the core of the objective takes, besides the user-facing models, a **test-only structural model with the identity link and a prediction linear in η**: P_i = θ + η_i (no exponential), ŷ_ij = a_ij(θ) + Σ_k z_ijk(θ)·η_ik, with a and z given by the test (for the worked example PX1: a_ij = θ_1 + θ_2·t_ij, z_ij = 1). It is not offered to users, is not in the model catalogue of MOD-GEN-05, and exists so that the Laplace-type objective can be checked where it is exact without any population software. Where it lives (a test model in `caladrius-testkit` or a feature of the crate) is the engine card's choice; the requirement is that the same objective code runs it.
- Status: `assumed`
- Sources: design choice; D (POP-ORA-01 needs a model linear in η, and `P = θ·exp(η)` is not linear in η).

## 2. The residual error (card item 2)

### POP-ERR-01 Error models, evaluated on the individual prediction
The error models are those of FIT-ML-02 and FIT-ML-03, with the same ids and the same meaning: `additive` (σ = a), `proportional` (σ = b·|ŷ|), `combined` (σ = √(a² + b²·ŷ²), the variance sum, the default combined form) and `combined_sd` (σ = a + b·|ŷ|). The prediction in the formula is **the individual prediction ŷ_ij(η̂_i)**: this is the interaction, so the standard deviation of an observation moves with the random effect of its subject, and in the inner problem σ_ij is a function of η (POP-INN-01). The error parameters ξ are shared by all subjects, there is one error model per run and one endpoint (DAT-CMT-01), and they are moved on the variance scale of FIT-ML-05 (A = a², B = b², bound 0, active-set rule; `combined_sd` moves a and b themselves). The user must name `error_model`; there is no default (FIT-ML-01). An error model that is not `additive` on a prediction of 0 is the case FIT-ML-13c and FIT-ML-14b, handled in POP-INN-03.
- Status: `documented, untested`
- Sources: FIT-ML-02, -03, -05 (S-39, S-43); S-52 (the dependence of the variance on η̂ in the FOCE covariance); D.

### POP-ERR-02 The derivative of the error model with respect to η
Needed by the inner problem (POP-INN-02) and by POP-OBJ-04. With s = σ²: ∂σ/∂η_k = (∂σ/∂ŷ)·G_k, and ∂ ln σ/∂ŷ is the column of FIT-ML-06:

| error model | ∂ ln σ/∂ŷ | ∂σ²/∂ŷ |
|---|---|---|
| `additive` | 0 | 0 |
| `proportional` | 1/ŷ | 2b²·ŷ |
| `combined` | B·ŷ/σ² (B = b²) | 2b²·ŷ |
| `combined_sd` | b·sgn(ŷ)/σ | 2σ·b·sgn(ŷ) |

(∂σ²/∂ŷ = 2σ²·∂ ln σ/∂ŷ in every row.) With `additive` the variance does not depend on η, which is why the exact cases of POP-ORA-01 use it.
- Status: `documented, untested`
- Sources: FIT-ML-06 (the table of ∂ ln σ/∂ŷ, D); D (the chain rule and σ² = σ·σ).

## 3. The objective (card item 3)

### POP-OBJ-01 What is approximated
For subject i the marginal likelihood is

L_i = ∫ Π_j N(y_ij; ŷ_ij(η), σ_ij(η)²) · N(η; 0, Ω) dη,

an integral over η ∈ R^q that has no closed form for a model nonlinear in η. The population objective is OFV = −2·Σ_i ln L_i (subjects are independent given θ, Ω, ξ). FOCEI replaces each integral by an approximation built from the conditional mode η̂_i and first derivatives (POP-OBJ-02); FO replaces η̂_i by 0 (POP-OBJ-03). Neither is exact for a model nonlinear in η; the gap is not small (section 10, PX2: 0.09 to 0.3 on a total of about −14.2 for four subjects), which is why the exact cases of POP-ORA-01 are the first oracle and why the engine must never be tested against the exact marginal in the nonlinear case with a tight tolerance.
- Status: `documented, untested`
- Sources: S-56, S-57, S-55 (abstract level: the likelihood of a nonlinear mixed-effects model has no closed form and is approximated around the conditional modes); D (the integral); H32 (R: the integral computed by adaptive Gauss-Hermite quadrature, 60 and 40 nodes per dimension agree to 2e-9, example PX2).

### POP-OBJ-02 The FOCEI objective, per subject
At η̂_i (POP-INN-01), with ŷ_i = ŷ_i(η̂_i), G = G_i(η̂_i), σ̂_ij = g(ŷ_ij; ξ):

- E_i = ŷ_i − G·η̂_i (the first-order expansion of the prediction at η̂_i, evaluated at η = 0),
- C_i = G·Ω·Gᵀ + diag(σ̂_i1², ..., σ̂_in²) (the covariance of y_i under the same expansion; the variances are those of the individual prediction, which is the interaction),
- **OFV_i = n_i·ln(2π) + ln|C_i| + (y_i − E_i)ᵀ·C_i⁻¹·(y_i − E_i)**,
- OFV = Σ_i OFV_i, summed in subject order (POP-DET-01).

This is the objective of the FOCE method written from the linearisation of Lindstrom and Bates, read here as the −2 log-likelihood of the Gaussian model y_i ≈ E_i + G·η + e with e ~ N(0, diag σ̂²) and η ~ N(0, Ω): the two terms are the log-determinant and the squared Mahalanobis distance of the same Cov. The S-52 slides write it, for FOCE, as the sum of ln Cov and the squared standardised distance with exactly this E and this Cov, with the derivative of the prediction at η̂_i and the derivative of the error model at ε = 0. What this rule adds is the interaction (the error model derivative taken at the individual prediction) and the constant n_i·ln(2π) (POP-OBJ-05). C_i is positive definite whenever Ω is and every σ̂_ij > 0 (D), so its Cholesky factor exists; the engine computes ln|C_i| and the distance from that factor (n_i × n_i, small), never from an explicit inverse. When the structural model is linear in η and σ does not depend on η, E_i = ŷ_i(0) and C_i does not depend on η̂_i, and OFV_i is the exact −2 ln L_i (POP-ORA-01). The identity-link test model of POP-MOD-05 uses the same formulas with G = Z.

A Laplace expression with the Hessian of the joint log-density in place of the linearisation gives a different number (POP-OBJ-04). Which of them a reference tool computes is open item OP-02; this rule is the default because it is the one the cited slides write for the FOCE objective.
- Status: `documented, untested`
- Sources: S-52 (the FOCE expectation and covariance and the objective as the sum of ln Cov and the squared standardised distance; slides, read in full, equations garbled by the text extraction, so the symbols of the objective were read from the layout and checked by H32); S-55, S-58 (Lindstrom and Bates linearisation and the family of FO, FOCE, FOCEI methods, abstract level); D (the Gaussian density, the extension to the interaction); H32, H33 (R: the identity of the linearised form with the exact marginal for a model linear in η, example PX1, and its value for a nonlinear model, example PX2).

### POP-OBJ-03 The FO objective: the same function at η = 0
FO is the objective of POP-OBJ-02 evaluated **with η̂_i := 0 for every subject**, with no inner problem:

- E_i = ŷ_i(0) (the population prediction, PRED),
- G_i⁰ = G_i(0), σ⁰_ij = g(ŷ_ij(0); ξ), C_i = G_i⁰·Ω·G_i⁰ᵀ + diag(σ⁰_ij²),
- OFV_i^FO = n_i·ln(2π) + ln|C_i| + (y_i − ŷ_i(0))ᵀ·C_i⁻¹·(y_i − ŷ_i(0)).

There is no interaction in FO: the error variances are those of the population prediction. This is the case "η = 0" of the card. FO and FOCEI coincide when the model is linear in η and σ does not depend on η (POP-ORA-01), and differ otherwise (PX2). The FO objective is what the S-52 slides write for the FO method. FO is offered as the option `method = fo` (FOCEI is `foce_i`, the default); the FO fit has no η̂_i, so the EBE-based diagnostics (IPRED, IWRES, CWRES, shrinkage) are not defined after an FO fit in v1 and are NC with the reason `fo` (computing them after an FO fit by a post-hoc step is `later`, OP-14).
- Status: `documented, untested`
- Sources: S-52 (the FO expectation f(θ, 0), the FO covariance and the FO objective); D (the statement that POP-OBJ-02 at η̂ = 0 is the FO formula); H33 (R: the FO value of PX2 and of PX1).

### POP-OBJ-04 Other objective forms, named for the oracle
Two further forms are written here so that the oracle can recognise the one a reference tool computes (OP-02); neither is required of the engine in v1, and the choice between them is not a tolerance but a method difference (`AGENTS.md` section 5, `specs/differences.md`). Let h_i(η) = Σ_j [ln σ_ij(η)² + r_ij(η)²/σ_ij(η)²] + ηᵀΩ⁻¹η, so that η̂_i = argmin h_i (POP-INN-01). The Laplace approximation of the integral of POP-OBJ-01 is (D: expand h_i to second order at η̂_i; the q·ln(2π) of the prior cancels against the Gaussian integral)

OFV_i^Lap = n_i·ln(2π) + h_i(η̂_i) + ln|Ω| + ln|½·∇²h_i(η̂_i)| = n_i·ln(2π) + ℓ_i(η̂_i) + η̂_iᵀΩ⁻¹η̂_i + ln|I_q + Ω·A_i|,

with ℓ_i = Σ_j[ln σ² + r²/σ²] and A_i a q × q matrix that stands for ½·∇²ℓ_i(η̂_i): (a) `laplace_full`: A_i the exact half-Hessian (second derivatives of the model needed); (b) `laplace_expected`: A_i = Σ_j [G_j·G_jᵀ/σ_j² + ½·(∇σ_j²)(∇σ_j²)ᵀ/σ_j⁴] (the expected information of the Gaussian observation, using E r² = σ²; it is the matrix 𝒥ᵀ𝒥 of POP-INN-02 without its prior rows); (c) `laplace_no_variance`: the first term only. All three equal the exact value for a model linear in η with σ independent of η, and all three differ from POP-OBJ-02 otherwise (PX2). No source read in full gives the formula that a given open tool uses; the foceiControl page of S-49 lists two inner curvature choices (`focei`, `conditional`) without defining them, which shows that the choice exists in the reference tool.
- Status: `documented, untested` (the forms are derived; which one the reference computes is not known)
- Sources: D (second-order expansion; the expected information of a Gaussian with mean and variance depending on η); S-49 (`innerHessian`), S-55 (abstract level); H32 (R: the four forms and the quadrature value on PX2).

### POP-OBJ-05 The OFV convention is named in every answer
The constant is the whole difference between two conventions, and both are valid; a value without its convention cannot be compared with another (the value can be negative, FIT-ML-04). Caladrius's rule, with its reason: **every command that returns an OFV, an AIC or a BIC returns, in the same answer, the field `ofv_convention`, and a value is never shown without it.** Two conventions exist:

- `full`: OFV as defined in POP-OBJ-02 and POP-OBJ-03, constant kept, i.e. −2 times the log of the (approximate) marginal density of the observations. **Default.** Reason: it is the one that keeps the meaning of FIT-ML-04 (the true Gaussian −2LL) and it is what the exact cases of POP-ORA-01 compute without a population tool.
- `no_2pi`: OFV_no_2pi = OFV_full − N·ln(2π), N the number of observations used (all subjects). The open reference tool omits "the 2π constant" from its objective by default and has an option to add it (S-49, `adjLik`); whether that constant is exactly N·ln(2π) in every model, and whether it also contains a per-subject term, is not stated by the page and is open item OP-03, to be settled by the oracle by computing both values for one fit.

Properties: the minimiser is the same in both; differences of OFV between fits of the same observations are the same; AIC and BIC are built from the OFV of the convention stated and carry it (POP-DIA-04); comparing OFV, AIC or BIC across two answers with different `ofv_convention`, or with different N, is refused by any command that compares (the engine does not subtract values of two conventions silently). The convention is part of the answer and therefore part of its seal (`AGENTS.md` rule 10). It is chosen by the option `ofv_convention` (`full` or `no_2pi`) and recorded in the result and in the trace.
- Status: `assumed`
- Sources: design choice with the reasons above; FIT-ML-04 (the constant and its consequences), FIT-ML-16 and OF-20 (a value of another program cannot be compared without checking the constant); S-49 (`adjLik`: the open tool omits a 2π constant by default); D (the difference N·ln(2π) and its cancellation in differences).

## 3b. The inner problem: the empirical Bayes estimate

### POP-INN-01 The inner problem
For each subject, at the current (θ, Ω, ξ):

η̂_i = argmin_η h_i(η),  h_i(η) = Σ_j [ ln σ_ij(η)² + r_ij(η)²/σ_ij(η)² ] + ηᵀ·Ω⁻¹·η.

This is the extended-least-squares objective of the individual profile (FIT-ML-04 without its constant) plus the prior penalty ηᵀΩ⁻¹η, and η̂_i is the mode of the density of η given y_i (D: the negative log of the joint density up to constants). With the interaction the variance depends on η and the first sum includes ln σ²; without it (σ held at ŷ(0)) the problem would be a weighted least squares plus the penalty, which is not what FOCEI means. The problem has q unknowns and no bounds. It is solved separately for each subject, from data of that subject only (POP-DET-02).
- Status: `documented, untested`
- Sources: D (Bayes' rule for the density of η; FIT-ML-04 for the individual part); S-52 (the conditional estimates η̂_i as the point of expansion); H32 (R: the mode found by BFGS then Newton polish, gradient about 1e-5 at the printed digits on PX2).

### POP-INN-02 How the inner problem reuses `caladrius-fit`
The individual maximum-likelihood fit of FIT-ML-06 to FIT-ML-08 is a Gauss-Newton (Fisher-scoring) iteration on a stacked system of residual rows. The inner problem is the same system with **ζ replaced by η and one block of prior rows added**. Write Ω = L·Lᵀ (Cholesky, from the current Ω) and, for each observation, u_jk = G_jk/σ_j, l_jk = (∂ ln σ_j/∂ŷ_j)·G_jk (POP-ERR-02), z_j = r_j/σ_j. The stacked system has 2n_i + q rows:

- residual vector R: z_j; then (z_j² − 1)/√2; then **−L⁻¹η** (the prior rows),
- matrix 𝒥: rows u_j; rows √2·l_j; rows **L⁻¹** (q × q).

Then 𝒥ᵀR is half the negative gradient of h_i exactly (D: the first two blocks are g of FIT-ML-06, and −Ω⁻¹η is half the negative gradient of ηᵀΩ⁻¹η), and 𝒥ᵀ𝒥 = Σ_j (u_j·u_jᵀ + 2·l_j·l_jᵀ) + Ω⁻¹, the expected information of the subject given η plus the prior precision. The fixed point of the scoring step is therefore the exact mode, whatever approximation 𝒥ᵀ𝒥 makes of the curvature. All of FIT-ALG-02 to FIT-ALG-04 applies unchanged: column scaling, QR (Householder) of the stacked matrix, step halving down to the same floor (FIT-ML-07), Levenberg damping on failure, and the stopping test of FIT-ML-08, the **Newton decrement** Δ_i = gᵀ(𝒥ᵀ𝒥)⁻¹g ≤ ε_in, absolute. ε_in is the default ε of FIT-ML-08 (the value of that rule; a tighter value is used while the covariance step differences the objective, POP-COV-02). Differences from the individual fit: nothing is bounded, no error parameter moves, the derivatives are with respect to η (the chain rule of POP-MOD-01), and the output of the last iteration includes ln|𝒥ᵀ𝒥| (twice the sum of the logarithms of the diagonal of the R factor of the unscaled stacked matrix) for the forms of POP-OBJ-04 that use it. The Newton polish with the observed information of FIT-ML-08 is applied to the inner problem when the covariance step runs (POP-COV-02) and may be switched on elsewhere.

**Start.** The first evaluation of a subject starts at η = 0, the prior mean. Later evaluations may start at the η̂_i of the nearest earlier evaluation of the same subject (a warm start) when the engine keeps it; if the solve from a warm start fails the engine restarts at 0 before giving up. The warm start is part of the algorithm and must be a function of the subject's own history only (POP-DET-02). A subject whose h_i has several minima may be found at either; this is not detected, as FIT-OBJ-01 for the individual fit.
- Status: `documented, untested`
- Sources: D (the stacked system, the prior rows and their sign; 𝒥ᵀ𝒥 as information plus precision; the fixed point); FIT-ML-06 to -08, FIT-ALG-02 to -04 (S-23 to S-26, the machinery reused); H33 (R: the scoring system on the 4 subjects of PX2 reaches the same modes as BFGS plus Newton polish).

### POP-INN-03 Refusals, reusing the individual rules
The run stops before the first outer iteration, with an error that names what to change (`AGENTS.md` section 7), in these cases. Each reuses the individual rule it names, with "subject" in the place of "profile".
1. **`unbounded_likelihood`** (FIT-ML-13c): an observation y = 0 that the model predicts as 0 for every θ and every η, under `combined` and `combined_sd` at the dose time on a route that is not a bolus, and (under `combined`, `combined_sd` or `proportional`) the leading zeros of a lagged route when the domain of `tlag` meets [t0, t_nz) of that subject (`tlag` carries no random effect in v1, POP-MOD-02, so its domain is that of the typical value). The check is made for every subject before any fitting, the error names the subject and the rows, and it comes first, before `NonPositiveSigma`. The sentences are those of FIT-ML-13c with "in subject {S}" added.
2. **`NonPositiveSigma`** (FIT-ML-14b): σ not strictly positive or not finite at the start point (η = 0), for `proportional`. During the iteration a trial with the same condition is a rejected trial (FIT-ML-07), also in the inner problem.
3. **Too few data** (the analogue of FIT-ML-14a): refused if m < 2 (`too_few_subjects`: the covariance of the random effects cannot be estimated from one subject), or if N ≤ P_θ,free + Q (`TooFewObservations`: the fixed effects and the error parameters alone would use up the data). A model with N barely above the parameter count (including the elements of Ω) is allowed and is flagged in the covariance (POP-COV-03).
4. **`inner_failed`**: the inner problem of a subject does not converge, or does not produce a finite h_i, at the **starting** parameters. The error names the subject. At a later outer iteration the same condition is not an error: the trial is rejected and the outer step shortened (POP-EST-03).
5. Options: an unknown `method`, `error_model`, `ofv_convention`, `omega` shape or `covariance` kind, a missing `error_model`, a missing `random_effects`, an initial value outside its domain, or a non-positive-definite starting Ω, is `InvalidOption` naming the option.
6. Data: a subject that carries a refusal of the dataset (DAT-ERR-08) makes the run refuse, naming the subject and the dataset's sentence, unless the user excludes it with the option `exclude_subjects` (POP-DAT-01).
- Status: `assumed` for 3 to 6; `confirmed by oracle` for the reused rule 1 in the individual fit, not yet for the population use
- Sources: FIT-ML-13c, -14, -07 (the reused rules, their oracle in `oracle/expected/fit/ml/`); DAT-ERR-08; design choice for 3 to 6.

## 4. The estimation (card item 4)

### POP-EST-01 What the outer optimiser moves
The outer vector φ holds, in this order:

1. **Typical values.** For every θ_k that carries a random effect: ln θ_k. For a θ_k without a random effect: θ_k itself on its natural scale with its own bounds (FIT-BND-01, 03), because it may legitimately be 0 (`tlag`) or sit on a bound.
2. **Ω** (the free entries of Ω after the user's fixed ones): the **log-Cholesky** parameters. For `diagonal`: λ_k = ln ω_k (the logarithm of the standard deviation). For `block`: Ω = L·Lᵀ with L lower triangular, L_kk = exp(λ_kk) and the below-diagonal entries of L free, which is the same parameterisation as the one `nlme` documents for a general positive-definite matrix (S-48: the logarithms of the diagonal of the Cholesky factor, followed by the off-diagonal part of the triangle; its diagonal form is the logarithm of the square roots of the variances). Positive definiteness holds for every φ, with no constraint.
3. **Error parameters ξ.** On the variance scale of FIT-ML-05 (A = a², B = b², bound 0), not on a log scale. Reason (the argument of FIT-ML-05, unchanged): the boundary solution of a combined model must be reachable exactly; a log scale stalls before it.

Why log for θ and for Ω but not for ξ: a typical value with a random effect cannot be 0 or negative, and the objective is smooth and scale-free in its logarithm (the parameters of the models range over decades); a random-effect standard deviation tends to 0 only when the data cannot identify it, which is a diagnosis (POP-EST-04), not a solution to reach. The price is that ω_k = 0 is not reachable: a variance that wants to collapse makes the log scale drift (the information in ln ω_k tends to 0) and the outer stop test fires before 0; the result carries the flag `omega_collapse` (POP-EST-04). The variance scale with bound 0 (as for ξ) was not chosen for Ω because a variance at 0 removes a dimension from the inner problem, so the dimension of η would change during the fit.
- Status: `assumed`
- Sources: S-48 (`pdLogChol` and `pdDiag`, read in full); FIT-ML-05 (error parameters); D (smoothness and positivity); design choice.

### POP-EST-02 Bounds and fixed parameters
The bounds of FIT-BND-01 and FIT-BND-03 apply to every typical value, converted to the log scale for those with a random effect (a lower bound ≤ 0 on such a θ is replaced by the open domain θ > 0). A **fixed** parameter (the user gives `fix`) leaves φ: θ_k, an element of Ω (a standard deviation or a covariance) or an error parameter can be fixed; a fixed ω_k = 0 is the same as no random effect for that parameter (POP-MOD-02). A parameter held on a bound at the end is reported with the status `at_bound` as in FIT-ML-15, its standard error is NC with that reason, and the others are conditional on it (FIT-BND-02). The error parameters have the lower bound 0 of FIT-ML-05. Ω has no bound: positive definiteness is built in.
- Status: `assumed`
- Sources: FIT-BND-01, -02, -03; FIT-ML-05, -15; design choice.

### POP-EST-03 The outer optimiser and its stopping rules
The outer problem is the minimisation of OFV(φ) = Σ_i OFV_i(φ) (POP-OBJ-02 or -03) over φ, each evaluation running the m inner problems of POP-INN-02. FO has no inner problem and is the same outer problem with cheaper evaluations.

**Algorithm.** A quasi-Newton (BFGS) iteration on φ, written in the engine (pure Rust, no external optimiser, `AGENTS.md` rule 10), with the active-set treatment of FIT-BND-01 for the variables on a bound, a backtracking step control like FIT-ML-07 (ν = 1 halved while the trial does not lower the OFV; a trial is rejected if any inner problem fails, if any OFV_i is not finite, or if a bound is violated; Levenberg-style damping does not apply here), and the gradient of the OFV by **central differences** of the OFV with a step proportional to max(|φ_j|, 1) (h, OP-11), each difference re-solving the inner problems. The reference tool uses L-BFGS-B for the outer problem and a different inner optimiser (S-49); the method is not required to follow its path, only to reach the same optimum within the method tolerance (POP-EST-06). Exact gradients by sensitivity equations (S-58, abstract level) are a later optimisation, not a change of the objective.

**Stopping.** (1) Candidate: the predicted decrease of the BFGS step is at most ε_out. (2) Confirmation, once per candidate: the finite-difference Hessian R of the OFV at the candidate (POP-COV-02), and the Newton decrement of the OFV Δ_out = ½·gᵀR⁻¹g over the free variables; the fit has converged when Δ_out ≤ ε_out, **absolute** (the OFV can be negative, FIT-ML-08). If R is not positive definite at a candidate, the candidate is a saddle or a flat direction: the iteration continues with a steepest-descent step, and if it comes back to the same point the status is `singular`. (3) A limit of K_max accepted iterations. ε_out, K_max and h are OP-11.

**Status** (the names of FIT-ML-08): `converged`, `max_iterations`, `no_decrease`, `singular`, `non_finite`, `at_bound`, and the new `inner_failed` (every trial point in the line search had an inner problem that failed). The trace has one row per accepted iteration, including iteration 0: iteration, OFV, `ofv_convention`, φ in user units, ν, number of halvings, the decrement; the last row carries the status.
- Status: `assumed`
- Sources: design choice; S-49 (the reference tool uses an L-BFGS-B outer iteration and an n1qn1 inner iteration, at the level of the argument descriptions); FIT-ML-07, -08, FIT-BND-01 (the rules reused); S-58 (exact gradients exist, abstract level); D (the Newton decrement of a quadratic model, ½·gᵀR⁻¹g, in OFV units).

### POP-EST-04 Result, statuses of the estimates and flags
The result holds: the method, `ofv_convention`, OFV at the estimate, θ̂ (typical values), Ω̂ (variances and covariances, and the standard deviations and correlations derived from them), ξ̂ in the user's units (FIT-ML-17), the η̂_i of every subject used (NC for a subject with no observation), the counts of POP-DAT-01, the status, the trace, and the diagnostics of section 6. Flags (informational unless said): *at bound* for a parameter; *omega collapse* when a fitted ω_k is below a small fraction of its two-stage start (OP-11) at convergence, which tells the user that the data do not identify that random effect; *high η-shrinkage* (POP-DIA-03) per random effect; *few subjects* when m is not larger than the number of elements of Ω plus P_θ,free (the covariance is flagged, POP-COV-03); *slow convergence* above a number of iterations (OP-11). A fit that did not converge is shown with the status and the flag "not converged" on every statistic (FIT-CNV-03).
- Status: `assumed`
- Sources: design choice; FIT-ML-08, -15, -17, -20 (the layout and statuses carried over).

### POP-EST-05 Initial estimates: the two-stage method (card T-154)
The user's initial values, where given, are used. For the others the engine computes a **two-stage** start:

1. Stage 1: one ML fit per subject with the chosen structural model and error model, using `subject = all` (DAT-USE-02) and the initial estimates of FIT-INI-01 to 04 per subject, with the objective `ml` of FIT-ML-01. A subject whose individual fit is refused (FIT-ML-14, -15, an `unbounded_likelihood`), does not converge, or ends with a parameter on a bound is **left out of the means** and listed; it still takes part in the population fit.
2. Stage 2, over the subjects that survived (m₂ of them): θ_k⁰ = exp(mean of ln P_ik) for a parameter with a random effect (the geometric mean, the median of a log-normal); the natural-scale mean for a parameter without one; Ω⁰ = the sample covariance of the vectors (ln P_ik)_k with divisor m₂ − 1, its diagonal for `diagonal`, the full matrix for `block`; ξ⁰ = the mean over the surviving subjects of the individual error parameters, on the variance scale of FIT-ML-05.
3. A start that cannot be built is an error that says what to give: fewer than 2 surviving subjects, a diagonal element of Ω⁰ equal to 0 (all subjects returned the same value of that parameter), or, for `block`, a sample covariance that is not positive definite (then the diagonal is used for the start and a note says so).

The two-stage values are only a start: they are not a statistical estimate of Ω (the variance of individual estimates includes their own estimation error and is too large when subjects have few observations), so the fit moves away from them. The result records the start used (`start`: user, two_stage, or mixed per parameter) as FIT-ML-20 does.
- Status: `assumed`
- Sources: card T-154 (two-stage start); `specs/fit.md` DAT-USE-02 and FIT-ML-10, -01, FIT-INI-01 to 04 (the machinery); S-56, S-57 (abstract level: the nonlinear mixed-effects model whose two-stage start is the standard one); D (the geometric mean as the median of a log-normal; the inflation of the between-subject variance by the individual estimation error).

### POP-EST-06 What the method tolerance is meant to cover
Tests of estimates compare the engine with the open oracle within a **method tolerance** (`AGENTS.md` section 5 asks for the numbers through `board/QUESTIONS.md`; none is written here). What the tolerance is meant to cover, and not to cover:

- It covers: (1) two correct optimisers stopping at slightly different points of a flat objective (the stopping test measures a gain in OFV, not a distance, so the parameters can differ by far more than the OFV); (2) the accuracy of the inner solves and of the finite-difference derivatives, which differ between implementations; (3) the numerical accuracy of the reference itself (its ODE solver tolerance when it does not use a closed form: with `sigdig = 3` the page of S-49 gives atol 1e-6 and rtol 1e-3, so a reference at that setting is much less accurate than the engine, and the oracle must record the reference at two settings, POP-ORA-02); (4) the rounding of data and times between tools.
- It does not cover: a different objective formula (POP-OBJ-04, OP-02: a method difference to document), a different constant in the OFV (an explicit shift, POP-OBJ-05, never a tolerance), a different parameterisation of the same model, a different local minimum (the oracle checks the OFV at the engine's estimate under the reference and at the reference's estimate under the engine; a lower OFV on either side is a finding, not a tolerance), a different error-model form (FIT-ML-03) or a different treatment of the data (POP-DAT-02).

Separate tolerances are proposed, with the quantity each covers, in the report of this card: OFV at fixed parameters (the tightest, POP-ORA-02), OFV at the estimate, typical values, the elements of Ω, the error parameters, the standard errors, and η̂_i. A tolerance is a statement about a gap in a quantity of the output; it is never widened to make a test pass (`AGENTS.md` section 5).
- Status: `assumed`
- Sources: `AGENTS.md` section 5; FIT-OBJ-01 and the line of `AGENTS.md` section 5 on optimisers (two optimisers do not reach the same minimum bit for bit); S-49 (`sigdig` and the ODE tolerances); design choice.

## 5. The covariance of the estimates (card item 5)

### POP-COV-01 Which matrix, and how it is built
Let φ̂ be the converged vector, **R** = ∇²_φ OFV(φ̂) (the Hessian of the full OFV) and **S** = Σ_i g_i·g_iᵀ with g_i = ∇_φ OFV_i(φ̂) (the sum over subjects of the outer products of the gradients of the per-subject OFV; by independence, the empirical covariance of the scores). Three estimators of Cov(φ̂) are offered:

| `covariance` | formula | reading |
|---|---|---|
| `sandwich` (**default**) | R⁻¹·S·R⁻¹ | robust to a misspecified error structure; needs R only to be invertible, and many subjects for S to be reliable |
| `r` | 2·R⁻¹ | the inverse observed information; right if the model is correct |
| `s` | 4·S⁻¹ | the inverse of the outer-product information; needs m larger than the number of parameters |

(D: OFV = −2 ln L, so the information is R/2 and the score of a subject is −g_i/2; the sandwich H⁻¹·(Σ s·sᵀ)·H⁻¹ with H = R/2 and s = g/2 is R⁻¹·S·R⁻¹, the inverse information is 2R⁻¹, and the outer-product form is 4S⁻¹.) The reference tool offers the same three, with the sandwich as its default and the same factors (S-49, `covMethod`: "r,s", "r", "s"). **Why the sandwich is the default and not the expected information of FIT-ML-17:** the FOCEI objective is an approximation of the likelihood, so the model-based inverse information is not exactly the covariance of its minimiser, which is the case the sandwich is for; no closed-form expected information exists for a population model in general, so FIT-ML-17's matrix is not available (the working assumption written there for brick 3 is replaced by this rule); and the sandwich is the reference tool's default, which the oracle compares with. The price: with few subjects S is noisy or rank-deficient (the flag *few subjects*, POP-COV-03). The default is open item OP-04.
- Status: `assumed`
- Sources: S-49 (`covMethod`, version 2.2.2, the three kinds and the sandwich default, read through a fetch excerpt); D (the factors 2, 4 and the sandwich from OFV = −2 ln L); FIT-ML-17 (the working assumption this replaces).

### POP-COV-02 How the derivatives are taken
R and the g_i are **central finite differences of the OFV**, in the unconstrained vector φ, with each evaluation re-solving the inner problems (the OFV as a function of φ alone, with η̂_i = η̂_i(φ)). The shortcut that the gradient of the sum of the h_i at the mode only needs the partial derivatives at fixed η̂_i (the envelope argument) does not extend to the second term of the OFV, which depends on η̂_i through G and σ̂; it is not used. Requirements: (1) the inner problems are solved to a tighter Newton decrement than during the outer iteration, and the polish of FIT-ML-08 is on, so that the inner error is far below the difference quotient (the value is OP-11); (2) g_i is the difference of the single-subject OFV_i, so S needs 2·p extra evaluations of each subject (p the length of φ) and R the p(p + 1) or 2p² evaluations of the whole OFV by second differences, or p gradients; the cost is bounded by the fit's own; (3) the step is a fraction of max(|φ_j|, 1) (OP-11); (4) the sums over subjects are in subject order (POP-DET-01). The matrix is then carried from φ to the user's scale by the **delta method**: SE of θ_k = θ_k·SE(ln θ_k) for a log-scale one; for Ω, the Jacobian of the map from the log-Cholesky vector to (Ω_kl) (closed form: Ω = L·Lᵀ, ∂Ω/∂λ by the product rule); for ξ, FIT-ML-17 (SE_a = SE_A/(2a)). A parameter on a bound, or a fixed one, is removed before the inversion and the covariance is that of the free block, conditional on the bound (FIT-BND-02). Exact derivatives of the OFV by sensitivity equations are `later` (S-58).
- Status: `assumed`
- Sources: design choice; D (the envelope argument and why the log-determinant term needs the full derivative; the delta method); FIT-ML-08 (polish), FIT-ML-17 (delta method for ξ), FIT-BND-02.

### POP-COV-03 When the covariance is "not computable" (NC)
As in FIT-ML-17, a standard error that cannot be computed is shown as NC with its reason, never as 0 or as a blank:

- `not_converged`: the status of POP-EST-03 is not `converged` (the statistics are shown with the "not converged" mark, FIT-CNV-03).
- `singular`: R is not positive definite at φ̂ (its Cholesky factorisation fails, or an eigenvalue is not positive). Then no kind of covariance is shown (R is needed by all but `s`); for `s` alone, S is not invertible (its rank is below p).
- `at_bound`: the parameter is held on a bound or fixed; its SE is NC with that reason and the others are conditional (POP-EST-02).
- `non_finite`: a difference quotient needed an OFV that was not finite or an inner problem that failed (`inner_failed` at a differenced point).
- `few_subjects` (a flag on the sandwich, not an NC): m is not larger than p; S then has rank at most m and the sandwich is a poor estimate in the directions S does not span. The numbers are shown with the flag; `s` is NC.
There is no automatic fallback from one kind to another: a covariance that cannot be computed for the kind the user asked for is NC with its reason.
- Status: `assumed`
- Sources: FIT-ML-17 (reasons), FIT-CNV-03, FIT-BND-02; design choice.

### POP-COV-04 SE, RSE and correlations
SE_j = √Cov_jj in the user's units (POP-COV-02); RSE_j = 100·SE_j/|estimate_j|, NC at an estimate of 0; the correlation matrix is that of the covariance in the user's units over all free estimates (θ, the elements of Ω, ξ), with its eigenvalues and condition number as in FIT-OUT-05. For Ω the estimates reported are the variances and covariances; the standard deviations and correlations of the random effects are derived values with their own delta-method standard errors; the standard error of a variance is not a symmetric interval for it (a variance is positive), and no confidence interval is defined in v1 (the choice between a normal and a Student quantile is open, as OF-21 for the individual fit).
- Status: `assumed`
- Sources: FIT-ML-17, FIT-OUT-05 (carried over); D (the delta method for the variance and the correlation).

## 6. The diagnostics (card item 6)

### POP-DIA-01 PRED, IPRED, IWRES
For every observation used, at the estimate: **PRED_ij = ŷ_ij(0)**, the population prediction, which is the prediction of the typical individual, the **median** of the individual predictions for a monotone dependence on a single random effect (POP-MOD-01), not their mean; **IPRED_ij = ŷ_ij(η̂_i)**; **IWRES_ij = (y_ij − IPRED_ij)/σ̂_ij** with σ̂_ij = g(IPRED_ij; ξ̂), which is FIT-ML-18 applied to the individual prediction. The summaries of FIT-ML-18 (mean, standard deviation with divisor N − 1, counts of |IWRES| > 2 and > 3) are computed over all subjects. The fit does not impose Σ IWRES² = N (FIT-ML-13 is a property of the individual fit: the population fit's optimum includes the prior terms), so the standard deviation of IWRES is an honest diagnostic here. Rows left out (missing, BLQ) have none, and a subject with no used observation has none.
- Status: `assumed` for the names and the summaries; `documented, untested` for IWRES (S-53)
- Sources: FIT-ML-18; S-53 (IWRES as the individual weighted residual, abstract and Methods prose); D (the median).

### POP-DIA-02 CWRES
The conditional weighted residual of observation j of subject i is

CWRES_ij = (y_ij − E_ij) / √(C_i,jj),

with E_i and C_i the linearised mean and covariance of POP-OBJ-02 at η̂_i (the S-52 definition: the expectation f(θ, η̂) − (∂f/∂η)·η̂ and the covariance (∂f/∂η)Ω(∂f/∂η)ᵀ + diag of the error derivative term). Two points differ from the slides' text and are decisions of this rule. (1) The slides evaluate the error derivative at ε = 0 without saying at which η; here it is at η̂_i, consistent with the interaction of POP-ERR-01. (2) The slides divide a vector by the square root of the covariance matrix of that vector without saying which square root; the **default is the diagonal form** above (each residual by its own marginal standard deviation, which does not depend on the order of the observations); the alternative, the decorrelated form w_i = L_i⁻¹·(y_i − E_i) with C_i = L_i·L_iᵀ (Cholesky), has the property that Σ_j w_ij² equals the squared-distance term of OFV_i (PX3), depends on the observation order, and is `later`. Which one a reference tool reports is open item OP-06. CWRES is not defined after an FO fit (POP-OBJ-03). A bias in the CWRES with respect to the fitted FOCE is a known behaviour that the cited slides discuss; the erratum of S-54 corrects the printed residual of that paper to y − E(y), consistent with the definition here.
- Status: `documented, untested` for the form; `assumed` for the interaction extension and the diagonal default
- Sources: S-52 (the CWRES definition; slides, equations garbled in the text extraction and read together with the layout); S-54 (the erratum, residual y − E(y), at the level of a note); D (the interaction extension; the identity Σ w² = squared-distance term); H34 (R: PX3).

### POP-DIA-03 η-shrinkage and ε-shrinkage
For random effect k, over the subjects used with a defined η̂ (m₁ of them): **η-shrinkage_k = 1 − SD(η̂_k)/ω̂_k**, SD the standard deviation over subjects of η̂_ik (divisor m₁ − 1, OP-10) and ω̂_k = √Ω̂_kk the estimate. **ε-shrinkage = 1 − SD(IWRES)**, SD over all observations used (divisor N − 1, FIT-ML-18). Both are the definitions of the cited paper at the level of its abstract and Methods prose: the typeset equations (Eqs. 3 to 5) were not readable, so the SD's divisor and the exact set of observations are the reader's reading. 0 means no shrinkage and 1 means complete; a negative value is possible and is shown as it is, because the paper says it can signal misspecification. NC when m₁ < 2, when ω̂_k = 0, or after an FO fit. A high η-shrinkage makes the EBE-based diagnostics (η̂ plots, IPRED, individual covariate relations) uninformative; the flag *high η-shrinkage* is raised at a threshold that is OP-11 (the paper discusses ranges, not a rule read in full).
- Status: `documented, untested`
- Sources: S-53 (Savic and Karlsson 2009: the abstract's 1 − SD(η_EBE)/ω and 1 − SD(IWRES), the Methods prose on direction and sign; equations 3 to 5 are images, not read); FIT-ML-18 (SD of IWRES); H34 (R: PX3).

### POP-DIA-04 AIC and BIC, and the parameter count
AIC = OFV + 2·k and BIC = OFV + k·ln N, with OFV of the convention stated (`ofv_convention` is returned with them, POP-OBJ-05) and **k the number of free parameters estimated**: the free typical values P_θ,free, plus the free elements of Ω (q for `diagonal`, q(q + 1)/2 for `block`, less those fixed), plus Q (the error parameters, **including a component that ended on its bound**, FIT-ML-16). Fixed parameters are not counted. N is the number of observations used; using the number of subjects instead is OP-05. The rule of FIT-ML-16 on comparisons holds: only for the same observations, the same conventions, and models fitted by the same method (an FO fit and a FOCEI fit have different objectives; comparing their OFV is meaningless and is refused by any comparing command). AIC and BIC are NC when the OFV is not finite or the fit did not converge (shown with the mark).
- Status: `assumed`
- Sources: FIT-ML-16 (S-27, S-29: the form −2 log-likelihood plus a penalty); design choice for the count and for N; D.

## 7. The data (card item 7)

### POP-DAT-01 The input: the population dataset
The input is the dataset of `specs/data.md` (the event layout, DAT-LAY-01, with several subjects, or a profile-layout file with several subjects read as in DAT-LEG-01), with the same reading of a subject as the individual fit of DAT-USE-02: **one dose row per subject in v1**, giving its dose, dose time and, for an infusion, duration or rate; a subject with more than one dose row is refused (multiple dosing in the population fit is `later`, OP-09), with the sentence of DAT-USE-02. The time axis of a subject is measured from its dose (DAT-USE-01); an observation before the dose is left out with the reason `before_dose` (FIT-ERR-01). The subjects are taken in the **order of the dataset** (first appearance of the label after DAT-ORD-01); this order fixes the order of every sum over subjects (POP-DET-01). The counts returned: m (subjects used), N (observations used), n_i per subject, `n_missing` and `n_blq` in total and per subject (DAT-CEN-01, -02), and the list of subjects left out with the reason. A subject whose observations are all missing or BLQ has n_i = 0: it contributes nothing to the OFV (L_i = 1), its η̂_i is NC, it is not counted in m, and it is listed. A subject that carries a refusal of the dataset (DAT-ERR-08) makes the run refuse, naming it; the user may exclude subjects explicitly with `exclude_subjects` (a list of labels, listed in the result, never silently). The table is never re-sorted or corrected silently (DAT-ORD-01, golden rule 6).
- Status: `assumed`
- Sources: `specs/data.md` DAT-LAY-01, DAT-ORD-01, DAT-CEN-01, -02, DAT-ERR-04, DAT-ERR-08, DAT-LEG-01, DAT-USE-01, -02; FIT-ERR-01; design choice for the refusal and the exclusion.

### POP-DAT-02 BLQ: exclusion only in v1, the censored likelihood is an open item
Observations below the limit of quantification are **left out** of the likelihood and counted (`n_blq`), exactly as FIT-ML-19 and DAT-CEN-02 say: this is the method called M1 in the censoring literature. When many observations are BLQ the estimates, in particular of the error and of the terminal phase, can be biased; the user is told the count and the share per subject. The method that keeps them, a likelihood term for the probability of being below the limit given η (M3, Beal 2001, S-46), changes the objective and the inner problem (the term is not Gaussian) and is **not specified**: it is open item OP-07. No formula for it is given here. The data columns that would carry the censoring flag and the limit are already in `specs/data.md` (DAT-CEN-02), so the dataset does not change when M3 arrives.
- Status: `assumed`
- Sources: FIT-ML-19, DAT-CEN-02 (the same decision); S-46 (Beal 2001, existence and naming of the methods, abstract-level summary; paper not read).

### POP-DAT-03 Covariates: read, carried, not modelled
The constant covariates declared in the dataset (DAT-COV-01) are read and carried unchanged into the subject table of the result, as in DAT-USE-03. **They enter no model in v1**: no covariate relation on a typical value or on a random effect. Covariate models are a later card (after T-155). A declared covariate that varies within a subject is refused by the dataset (DAT-ERR-05), time-varying covariates are `later` (DAT-COV-02). A user who asks for a covariate relation in v1 gets `InvalidOption` that says so.
- Status: `assumed`
- Sources: DAT-COV-01, -02, DAT-USE-03; card T-162 item 7.

## 8. Determinism (card item 8)

### POP-DET-01 The order of every reduction
A population fit adds numbers from many subjects, and floating-point addition is not associative: a different order is a different last digit, and a different last digit changes a convergence test and so the path of the optimiser. The rule: **every reduction over subjects, observations or parameters is a plain left-to-right sum in a fixed order that does not depend on the number of threads or on the target.** Explicitly:
1. OFV = Σ_i OFV_i is a left-to-right sum over subjects in dataset order (POP-DAT-01), as is Σ over subjects of any statistic (S = Σ g_i·g_iᵀ entry by entry, the counts, the sums for the shrinkage).
2. Within a subject, sums over observations (the stacked system, ℓ_i, the distance term) are left to right in time order of the dataset after DAT-ORD-01.
3. The standard deviation of a sample (shrinkage, IWRES summaries) is computed in two passes, in the order of the data: first the mean by a left-to-right sum, then the sum of squared deviations.
4. The linear algebra (Householder QR, Cholesky, triangular solves) is the crate's own code with a fixed loop order; no external numerical library whose summation order depends on the machine.
5. The finite-difference derivatives of POP-COV-02 sum per-subject differences in subject order too.
No pairwise or blocked summation whose block size depends on the thread count is allowed; a fixed tree that is the same for any thread count (for instance by subject index) would also satisfy the rule but costs nothing to avoid, so the left-to-right sum is the rule. Reason: the digest of POP-DET-03 must be identical, bit for bit, for 1 thread, N threads and WebAssembly.
- Status: `assumed`
- Sources: `AGENTS.md` rule 10 (same numbers on every machine; sums, products and quotients are rounded alike everywhere); D (non-associativity of floating-point addition).

### POP-DET-02 Each subject is a pure function; threads sit outside the library
Layers L0 to L3, which hold the population fit, compile for WebAssembly and contain no threads (`AGENTS.md` golden rule 9); a parallel evaluation over subjects, if any, is arranged by the caller in `apps/`. The contract that makes this safe: **the inner problem and the per-subject quantities (η̂_i, OFV_i, g_i, the diagnostics) are a pure function of the data of subject i, of (θ, Ω, ξ) and of the warm start of subject i** (POP-INN-02), where the warm start is itself a function of the same subject's earlier evaluations, never of another subject's or of the order in which threads finished. The library exposes the per-subject evaluation and the reduction separately: the caller (any number of threads) fills an array indexed by subject, and the library then reduces it in index order (POP-DET-01). A partial sum per thread added at the end is forbidden, because its grouping depends on the thread count. No global state, no shared random number, no clock.
- Status: `assumed`
- Sources: `AGENTS.md` rules 9 and 10; design choice.

### POP-DET-03 The same digest on 1 thread, on N threads and in WebAssembly
**Caladrius's rule, with its reason:** the answer of a population fit is the same, bit for bit, whatever the number of threads and the target; and a test proves it. Reason: a user who reruns the same project on a laptop, in the browser and in a cloud session must be able to rely on the seal (`AGENTS.md` rule 10, `project.replay`), and a population fit is the first command whose result could depend on how the work was split. The population workload is added to the fixed workload of `cargo xtask digest`: a small dataset (the examples PX1 and PX2 are candidates), FOCEI and FO, the full result in canonical text (OFV and its convention, φ̂, Ω̂, ξ̂, every η̂_i, SE, the diagnostics, the number of iterations and the trace), hashed with SHA-256. CI compares the digest on Linux, Windows, macOS and in WebAssembly under Node, and a threaded test compares 1 thread against N for at least one N that does not divide the number of subjects and one N larger than the number of subjects. Because the trace is in the digest, the **path** of the optimiser must be identical, not only its end point. The `libm` functions replace the platform's `exp`, `ln`, `powf` (`AGENTS.md` rule 10).
- Status: `assumed`
- Sources: `AGENTS.md` rule 10 (the digest and the seals); design choice with the reason above.

### POP-DET-04 No randomness in the fit
The population fit uses no random number: the start is deterministic (POP-EST-05) and the optimiser is deterministic. Any later command that simulates (population simulation is the next item of `AGENTS.md` section 1) draws from a stream seeded by a parameter, with a generator written in the engine and specified in `specs/` (`AGENTS.md` rule 10); it is not specified here. The sealed datasets of POP-ORA-04 are simulated in R, with R's generator, and committed as data; the engine never has to reproduce R's stream.
- Status: `assumed`
- Sources: `AGENTS.md` rule 10 (no random number except a stream drawn from a seed that is a parameter).

## 9. What the oracle can test, in order (card item 9)

### POP-ORA-01 (a) Cases where the objective is exact, at 256 bits, with no population software
**Cases.** A model linear in η with an additive error: ŷ_ij(η) = a_ij(θ) + z_ij(θ)ᵀ·η (the identity-link test model of POP-MOD-05), σ = a constant, η ~ N(0, Ω). Then y_i is exactly Gaussian, y_i ~ N(a_i, Z_i·Ω·Z_iᵀ + σ²·I), and

OFV_i = n_i·ln(2π) + ln|C_i| + (y_i − a_i)ᵀ·C_i⁻¹·(y_i − a_i),  C_i = Z_i·Ω·Z_iᵀ + σ²·I,

is the exact −2 ln L_i. Both POP-OBJ-02 and POP-OBJ-03 and the three Laplace forms of POP-OBJ-04 equal it (D: the linearisation is exact, the Hessian of ℓ_i is constant and equal to 2ZᵀZ/σ², and the mode is the posterior mean). **This is the first oracle because it needs no population software, only the formula and 256-bit arithmetic.**

**How it is computed.** In R with `Rmpfr` at 256 bits, **with every input parsed from its decimal string** (`mpfr("2.1", 256)`, not a double converted to 256 bits: the data are decimal numbers and a double is not 2.1), π from `Const("pi", 256)`, the determinant and the quadratic form written out with the explicit Cholesky factor or closed forms for the balanced designs (in the reader's try, `solve` and `det` on `Rmpfr` matrices did not return in a reasonable time for a 4 × 4 case, so the script should carry its own small Cholesky). Rounded once to double for the file. The oracle also provides: the exact maximum-likelihood estimates of (θ, Ω, σ²) by maximising this closed form at high precision (the engine's estimate at the ML optimum is an estimate that needs no reference tool), the exact observed information at that point (the Hessian of the closed form), and the η̂_i in closed form (posterior means, example PX1). Cases to include: one random effect (an intercept) and two (an intercept and a slope) with a diagonal Ω and with a full block; balanced and unbalanced designs (different n_i and times per subject); a subject with a single observation; m = 2; Ω with a small variance (near the collapse of POP-EST-04) and with a large one; σ small and large relative to the random effects.

**What it tests.** The value of OFV at fixed parameters to rounding (the tolerance here covers only rounding and the order of the sums, not a method), the η̂_i, the covariance (R of POP-COV-01 against the closed-form Hessian), and the estimates (ML optimum of the closed form against the engine's, within the stopping tolerance of the engine). Because FO = FOCEI = the exact value in these cases, a mismatch here is a bug and cannot be a method difference.
- Status: `assumed` (becomes `confirmed by oracle` when T-152 produces the files); example PX1 gives the values for one case
- Sources: D (the Gaussian marginal of a linear model; the Hessian and the mode); H31 (R: `Rmpfr`, 256 bits: the closed form and the Laplace form agree to the last bit, example PX1); `AGENTS.md` section 5 (independent computation).

### POP-ORA-02 (b) OFV at fixed parameters against the open reference tool
**How the reference evaluates the objective without estimating** (from its own documentation, S-50 and S-49): in the examples of the help of `foceiFit` of the older `nlmixr` package the objective at the given initial values is obtained with `control = foceiControl(maxOuterIterations = 0, covMethod = "")` (zero outer iterations, no covariance step); the page of `foceiControl` of `nlmixr2est` 2.2.2 describes `maxOuterIterations` as "maximum number of L-BFGS-B optimization for outer problem" without saying what 0 does, so that zero iterations returns the objective at the initial values is **an inference from the examples, `documented, untested`** until the tool is installed. FO is requested with the flag `fo = TRUE`, and the interaction is the flag `interaction` (TRUE is FOCEI, FALSE is FOCE; S-49). The random effects at their conditional modes are found by the inner problem; a way to evaluate at given η is the argument `etaMat` ("initial (or final) ETA estimates, can also be a prior fit") together with `maxInnerIterations = 0` (an inference as well).

**What to record per case.** The reference's OFV in both of its conventions (default, and with `adjLik = TRUE`), the version of `nlmixr2`, `nlmixr2est` and `rxode2`, the value of `sigdig` or `atol`/`rtol`, and, to bound the reference's own numerical noise, the OFV at two values of those settings. The reference solves ODEs by default; a **closed-form linear compartment model** in the reference (the `linCmt()` facility of `rxode2`, not read) removes the ODE error and should be used where available; if only an ODE solution is possible, the case is tolerant only to the reference's accuracy (POP-EST-06). The OFV is compared with `ofv_convention` made explicit: the oracle adds N·ln(2π) to the reference's default value and checks that the result equals the reference with `adjLik = TRUE` (this settles OP-03 for each case; if the two disagree by a per-subject term, the case is a method difference, not a tolerance).

**What the OFV at fixed parameters decides.** Because it involves no optimiser, it identifies the objective formula: the oracle computes the reference's number and the four candidates of POP-OBJ-02 and POP-OBJ-04 (`linearised`, `laplace_expected`, `laplace_no_variance`, `laplace_full`) for the same parameters, using additive, proportional and combined error (the variance-derivative term only matters for the latter two, and the candidates coincide for additive error only when the model is also linear in η). The case set should span: a mild and a strong nonlinearity in η, small and large Ω, few and many observations, a lag model (the kink), and FO. The agreement of the engine with the reference at fixed parameters is a far tighter test than agreement of estimates: it is the first to run once nlmixr2 is installed (Q-021). If none of the four candidates matches, the formula is a method difference to document in `specs/differences.md` and the card is reopened.
- Status: `documented, untested` (the mechanism is an inference from examples; the tool is not installed: Q-021)
- Sources: S-50 (examples of `foceiFit`, summary), S-49 (`foceiControl`: `maxOuterIterations`, `maxInnerIterations`, `etaMat`, `interaction`, `fo`, `adjLik`, `sigdig`, read through excerpts); S-51 (the `nlmixr2` page: the objective is the one of the FOCEI family, other routines have none of their own, summary); D (the candidates).

### POP-ORA-03 (c) Estimates against the reference, with two second looks
The estimates of θ, Ω and ξ (and the OFV at the estimate, the standard errors and the η̂_i) are compared with the reference's FOCEI fit of the same data, started from the same initial values, within the method tolerance of POP-EST-06. Two second looks, which are not expected to reproduce the FOCEI estimates exactly and are used to catch gross errors and to show the spread between methods: `saemix` (stochastic approximation EM, a different estimator; its estimates differ from FOCEI by the difference of the two approximations of the likelihood and by the random stream, so its tolerance is the widest and its use is to confirm the region of the optimum, not to fix a digit), and `nlme` (the Lindstrom-Bates algorithm with `method = "ML"`: its random effects are additive on the parameter, so the log-normal structure is obtained by writing the model on the log scale, `exp(lCL)`, and the fixed effects it reports are ln θ; S-48; the variance structure of the error is `varPower`/`varConstProp` with the scale fixed or estimated as the help pages of S-39 describe). All three are open-source tools allowed by `AGENTS.md` rule 1 and section 5. `nlme` is installed (3.1.168); `nlmixr2` and `saemix` are not (Q-021). A discrepancy between the engine and the reference that is explained by the OFV-at-fixed-parameters (POP-ORA-02) is a method difference; a difference that is not is a bug or a different local minimum, and the OFV of each estimate under the other's objective separates them.
- Status: `documented, untested`
- Sources: `AGENTS.md` section 5 (nlmixr2 with saemix and `nlme` as second looks); S-48, S-39 (`nlme`, read in full); S-44 (`saemix`, summary); S-49, S-51 (nlmixr2).

### POP-ORA-04 (d) Sealed simulate-then-fit tests
**Caladrius's rule, with its reason.** Data are simulated in R from known parameters with a recorded seed, **hashed, and committed before any engine code for the population fit exists**; a test then fits them and checks that the parameters are recovered. Reason: a test whose data are generated after the engine's output has been seen can be adjusted, even without intent, until it passes; fixing the data first, with a hash that anyone can recompute, makes the test a prediction that the engine can fail, and the order of commits is the proof. Contents of a sealed case, in `oracle/expected/pop/`:
1. The R script that simulates (model, true θ, Ω, ξ, the design, the number of subjects), the seed, and the generator settings recorded in the case file: `RNGkind()` (in R 4.5.2 it returns `"Mersenne-Twister"`, `"Inversion"`, `"Rejection"`, H34) and `R.version.string`.
2. The dataset as a file with a fixed format: the numbers written with a fixed number of significant digits chosen once (so every tool reads the same numbers, which are the data), LF line endings, UTF-8 without a byte order mark.
3. The SHA-256 of the dataset's bytes, in its own committed file, and the true parameters in the options file.
4. The ordering rule: the commit that adds items 1 to 3 precedes, in the history of the branch, every commit that adds or changes the engine code of the population fit (a reviewer checks it with `git log`); once committed, a sealed case is **never edited or re-rolled**: a new seed is a new case, and a case on which the engine fails stays in the suite with its failure recorded.
5. The test recomputes the hash of the file before using it and fails if it differs.

**What is checked.** Two checks, which answer different questions. (i) *Agreement with the reference on the same sealed data* (POP-ORA-03), within the method tolerance. (ii) *Recovery of the truth*: the fit on a finite sample differs from the true parameters by sampling error, which a fixed tolerance on the parameters cannot describe; the check is that |estimate − true| ≤ c·SE for each parameter (SE from POP-COV-01 for the kind stated), and, over a set of K sealed replicates of the same design, that the mean of the estimates is within a small multiple of its standard error of the truth (no systematic bias) and the interval estimate ± c·SE covers the truth at about the expected rate. The design numbers (K, the number of subjects and observations, c) are open item OP-13; none is a rule here. Both designs of the model class are covered: a case where FOCEI is accurate (many observations per subject, small Ω) and a case where it is known to be biased or to struggle (sparse data, large Ω, strong nonlinearity), so that the suite also records the limits of the method.
- Status: `assumed`
- Sources: design choice with the reason above; S-52 (the discussion, in the cited slides, of simulate-and-estimate with a known truth, at the level of the slide titles); D (the sampling-error argument); `AGENTS.md` section 5.

## 10. Worked examples (all targets for the oracle card T-152)

### PX1 An exact case, at 256 bits (H31)
**Model** (POP-MOD-05): ŷ_ij = θ₁ + θ₂·t_j + η_i, one random effect, additive error. m = 3 subjects, n_i = 4, times t = 1, 2, 4, 8.

| subject | y (times 1, 2, 4, 8) |
|---|---|
| 1 | 2.1, 2.9, 5.2, 9.1 |
| 2 | 1.4, 2.2, 4.1, 7.6 |
| 3 | 3.0, 4.1, 5.9, 10.4 |

**Parameters:** θ₁ = 1, θ₂ = 1, Ω = 0.5, σ² = 0.25. Here C_i = 0.5·11ᵀ + 0.25·I (4 × 4) for every subject, ln|C_i| = 3·ln 0.25 + ln(0.25 + 4·0.5) and (y − a)ᵀC⁻¹(y − a) = Σr²/σ² − Ω·(Σr)²/(σ²·(σ² + 4Ω)) with r = y − a. The exact OFV_i (`full`) and the conditional modes, computed at 256 bits from the decimal strings, printed here to 20 digits:

| subject | η̂_i | OFV_i (`full`) |
|---|---|---|
| 1 | 1/15 = 0.066666666666666667 | 4.2035553984940388417 |
| 2 | −37/45 = −0.82222222222222222 | 6.9146665096051499528 |
| 3 | 44/45 = 0.97777777777777778 | 6.7146665096051499528 |

- OFV (`full`, N = 12) = 17.8328884177043387473080364024959621729687017
- N·ln(2π) = 22.0545247969121458027279136737348233566735394
- OFV (`no_2pi`) = −4.22163637920780705541987727123886118370483766
- The Laplace form of POP-OBJ-04 (`laplace_*`, all three), evaluated at the mode η̂_i with Ω·A_i = Ω·n_i/σ², equals the closed form: the difference computed at 256 bits is exactly 0 (H31). The linearised form of POP-OBJ-02 and the FO form of POP-OBJ-03 are the closed form by construction (E_i = a_i, C_i as above). The value in double precision is 17.832888417704339.

The `no_2pi` value is negative: this is why the convention must be named (POP-OBJ-05).

### PX2 A nonlinear case: one compartment, intravenous bolus, proportional error (H32, H33)
**Model:** `pk1.iv_bolus` with D = 100 (dose units), CL and V with log-normal random effects, diagonal Ω, `proportional` error. Times 0.5, 1, 2, 4, 8, 12. m = 4 subjects. Parameters (the values used to simulate the data, then held fixed for the evaluation): θ = (CL, V) = (5, 50), Ω = diag(0.3², 0.2²) = diag(0.09, 0.04), b = 0.1 (σ = b·|ŷ|).

| subject | y at 0.5, 1, 2, 4, 8, 12 |
|---|---|
| 1 | 2.44, 2.39, 1.98, 1.38, 0.600, 0.416 |
| 2 | 1.59, 1.72, 1.62, 1.33, 0.789, 0.417 |
| 3 | 1.40, 1.68, 1.14, 1.15, 0.732, 0.453 |
| 4 | 2.23, 2.00, 1.57, 1.47, 0.869, 0.559 |

(Simulated once in R 4.5.2 with `set.seed(20261107)`, η from `rnorm`, residual from `rnorm`, rounded to 3 significant digits. This is an example, not a sealed case: it is not hashed.) The conditional modes η̂_i = (η̂_CL, η̂_V): subject 1 (0.20380, −0.26150), subject 2 (0.21388, 0.02097), subject 3 (0.24425, 0.18824), subject 4 (0.04647, −0.09498). Per-subject OFV (`full`), double precision with finite-difference derivatives (good to about 1e-6), against the integral of POP-OBJ-01 computed by adaptive Gauss-Hermite quadrature in two dimensions (60 nodes per dimension; 40 nodes agree to 2e-9):

| subject | `linearised` (POP-OBJ-02) | `laplace_expected` | `laplace_no_variance` | `laplace_full` | FO | exact (quadrature) |
|---|---|---|---|---|---|---|
| 1 | −0.70398 | −0.64952 | −0.68705 | −0.53679 | −0.76677 | −0.56144 |
| 2 | −4.36642 | −4.32467 | −4.36199 | −4.53638 | −4.36425 | −4.57008 |
| 3 | −3.61581 | −3.55578 | −3.59289 | −3.68442 | −3.25241 | −3.71867 |
| 4 | −5.45569 | −5.39038 | −5.42763 | −5.35642 | −5.71680 | −5.38562 |
| **OFV (`full`)** | **−14.14190** | −13.92035 | −14.06957 | −14.11401 | −14.10023 | −14.23581 |
| OFV (`no_2pi`), N = 24 | −58.25095 | −58.02940 | −58.17862 | −58.22306 | −58.20928 | −58.34486 |

N·ln(2π) = 44.10905 (N = 24). The gaps from the exact value (`full`): `linearised` 0.0939, `laplace_expected` 0.3155, `laplace_no_variance` 0.1662, `laplace_full` 0.1218, FO 0.1356. Reading: (1) all approximations differ from the exact marginal by 0.1 to 0.3 on a total of about −14.2 with four subjects, far above any numerical tolerance, so the exact marginal is a check of the approximation, never of the engine, outside the linear case; (2) the four forms differ from each other by up to 0.2 in total and by up to 0.2 in a single subject, so the OFV at fixed parameters (POP-ORA-02) separates them easily; (3) the FO value is not the worst on this total, but it differs from `linearised` by 0.3 to 0.4 in individual subjects (subjects 3 and 4): the totals cancel. The default of the engine is the `linearised` column (POP-OBJ-02), and the row to compare with the open tool is, per its default convention, the `no_2pi` row after the explicit shift of POP-OBJ-05.

### PX3 Diagnostics for subject 1 of PX2 (H34)
At the parameters of PX2 (not an estimate), for the six observations of subject 1 (times 0.5, 1, 2, 4, 8, 12):

| quantity | values |
|---|---|
| PRED = ŷ(0) | 1.90246, 1.80967, 1.63746, 1.34064, 0.89866, 0.60239 |
| IPRED = ŷ(η̂_1) | 2.39892, 2.21532, 1.88919, 1.37390, 0.72663, 0.38430 |
| IWRES | 0.17123, 0.78852, 0.48069, 0.04443, −1.74265, 0.82494 |
| E_1 (POP-OBJ-02) | 1.86049, 1.80017, 1.67514, 1.42184, 0.96735, 0.62551 |
| diag C_1 | 0.255827, 0.199039, 0.134579, 0.097757, 0.083990, 0.054918 |
| CWRES, diagonal form | 1.14575, 1.32209, 0.83102, −0.13381, −1.26754, −0.89402 |
| CWRES, decorrelated form (Cholesky) | 1.14575, 0.69657, −0.21341, −0.86618, −1.28908, 1.58117 |

The two CWRES forms agree on the first observation and differ afterwards by up to 0.63, so the choice of OP-06 matters. For the decorrelated form, Σ w² = 6.755572 = (y − E)ᵀC⁻¹(y − E), and ln|C_1| = −18.48681, so OFV_1 = 6·ln(2π) + ln|C_1| + Σw² = −0.70398 (the `linearised` value of subject 1). Shrinkage over the four subjects (an illustration: four subjects are far too few for the statistic to mean anything): SD(η̂_CL) = 0.088768 and SD(η̂_V) = 0.189610 (divisor m − 1), so η-shrinkage = 0.70411 for CL (ω = 0.3) and 0.05195 for V (ω = 0.2); with divisor m the SDs are 0.076875 and 0.164207 (shrinkage 0.74375 and 0.17896). SD(IWRES) over the 24 observations = 0.848793, so ε-shrinkage = 0.15121.

### PX4 Refusals (text of the checks, no fit)
- `too_few_subjects`: a dataset with one subject: "the population fit needs at least 2 subjects to estimate the variance of the random effects; this dataset has 1".
- `InvalidOption`: a random effect on `tlag`: "random effect on `tlag`: not available in this version (the prediction has a kink at the lag time); give `tlag` as a shared typical value".
- `unbounded_likelihood`: subject S2, `pk1.oral_1`, `combined`, sample 0 at time 0: the sentence of FIT-ML-13c with "in subject S2" added.
- `inner_failed`: "the inner problem of subject S3 did not converge at the starting values; check the starting values, the model and the data of that subject".

## 11. Open items for Abdou (card item 10)

These are questions, never rules. The orchestrator turns them into entries of `board/QUESTIONS.md`. The first five are choices; the rest are numbers the oracle measures.

- **OP-01 Ω block in v1.** Proposal (POP-MOD-03): `diagonal` by default and one full `block` as an option. Alternative: diagonal only in v1, block in a later card. Needs a decision before T-153.
- **OP-02 Which OFV formula is the reference's.** POP-OBJ-02 (`linearised`) is the default, and POP-OBJ-04 names three Laplace forms. The OFV at fixed parameters against the reference (POP-ORA-02) decides it once the tool is installed (Q-021). If none matches, it is a method difference and Abdou decides whether the engine follows the reference or the written definition.
- **OP-03 The OFV constant.** Default `full` (constant kept). The open tool omits a "2π constant" by default; whether it is N·ln(2π) exactly is to be measured (POP-OBJ-05, POP-ORA-02). Should the engine's default follow the tool's (`no_2pi`) so that printed numbers are comparable at a glance? The current proposal prefers meaning (`full`) over resemblance and names the convention in every answer either way.
- **OP-04 The default covariance.** Proposal: `sandwich` (the reference's default, robust, needs many subjects). Alternative: `r` (2R⁻¹, stable with few subjects, right only if the model is correct).
- **OP-05 BIC's n.** Observations (proposal, as the individual fit) or subjects (a common alternative for mixed models).
- **OP-06 CWRES form.** Diagonal (proposal, order-independent) or decorrelated by the Cholesky factor (its squared sum equals the distance term of the objective, but it depends on the order of the observations). Which one the reference reports is read when the tool is installed.
- **OP-07 BLQ.** M1 (exclusion) in v1; M3 (censored likelihood) later. Whether M3 is the next card after T-155.
- **OP-08 Random effects on `tlag` and `dur`.** Refused in v1 (the kink). Alternative: allowed with a warning.
- **OP-09 Several doses per subject** (multiple dosing and steady state, MOD-MD-*) in the population fit: refused in v1.
- **OP-10 The divisor of the shrinkage SD** (m − 1 proposed; the cited paper does not state it in the text read).
- **OP-11 Algorithm constants** (numbers the orchestrator sets from measurement, none a rule here): ε_in (the default of FIT-ML-08 is the starting point), the tighter value used while differencing, ε_out, K_max, the finite-difference step h, the ratio under which `omega_collapse` is raised, the iteration count of *slow convergence*, the η-shrinkage level of the flag.
- **OP-12 The method tolerances** (POP-EST-06): OFV at fixed parameters, OFV at the estimate, typical values, Ω, ξ, SE, η̂_i, and the second looks. To be measured by T-152.
- **OP-13 The sealed design** (POP-ORA-04): K, the number of subjects and observations, c.
- **OP-14 Diagnostics after an FO fit** (the post-hoc step) and **plain FOCE without the interaction** (the reference offers `interaction = FALSE`): `later`.
- **Waiting on Q-021:** POP-ORA-02 and -03 need `nlmixr2` (Rtools); `saemix` is not installed either. POP-ORA-01 and the data of POP-ORA-04 need only R, `Rmpfr` and `nlme`, which are installed.
