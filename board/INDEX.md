# Board index

One line per task: id, title, role, state. Only the orchestrator writes this file. Cards live in `tasks/`.

| id | title | role | state | depends on |
|---|---|---|---|---|
| T-000 | Step 0: contract, roles, board, repository | orchestrator | done | |
| T-001 | Cargo workspace skeleton, xtask layers/wasm, lints | interface | done | T-000 |
| T-002 | specs/nca.md from allowed sources (λz, AUC rules, BLQ, extrapolation) | reader | done | T-000 |
| T-003 | Public NCA oracle: theophylline and indomethacin with PKNCA, testkit tolerances, failing tests | oracle | done | T-001 |
| T-004 | caladrius-nca (umbrella, split into T-004a/b/c) | engine | split | T-002, T-003 |
| T-004a | caladrius-nca: input types, validation errors, AUC rules (linear, lin-up/log-down), Cmax/Tmax/Tlast/Clast | engine | done | T-002, T-003 |
| T-004b | caladrius-nca: λz selection, t½, AUC/AUMC extrapolation, %extrapolated | engine | done | T-004a |
| T-004c | caladrius-nca: C0 back-extrapolation, AUMC, MRT, CL/F, Vz/F, Vss; 100% public oracle; conformance table | engine | done | T-004b |
| T-005 | Synthetic oracle profiles to settle λz open items O-01 and O-02 (PKNCA runs) | oracle | done | T-003 |
| T-006 | xtask conformance hardening: per-parameter floors, parse errors, computed-presence check, atomic write | interface | done | T-004c |
| T-007 | specs/nca.md sync: oracle-confirmed tags (O-01, O-02, LZ-05 strict >, LZ-07), dose NC instead of error (DAT-11, 2.4, section 9) | reader | done | T-004c, T-005 |
| T-008 | specs/models.md and specs/fit.md | reader | done | T-007 |
| T-009 | Oracle for models (closed-form grids) and fitting (R nls / nlsLM references) | oracle | done | T-008 |
| T-010 | caladrius-models: one-compartment closed-form models, derivatives, secondary parameters | engine | done | T-008, T-009 |
| T-011 | caladrius-fit (umbrella, split into T-011a/b) | engine | split | T-010 |
| T-011a | caladrius-fit core: WLS, Gauss-Newton Levenberg-Hartley, estimates, SE, CV%, WRSS, AIC/SBC | engine | done | T-010 |
| T-011b | caladrius-fit outputs: CI, matrices, condition numbers, trace, initial estimates, bounds | engine | done | T-011a |
| T-012 | Oracle edge cases without public coverage (interior zeros, missing values, IV infusion, Tlag, dose-normalised, AUMC %%extrap) via synthetic PKNCA runs | oracle | done | T-007 |
| T-013 | caladrius-nca: edge parameters (tlag, vss, auciv*, aumcall.dn), PKNCA profile for replaced values | engine | done | T-012 |
| T-014 | Documented-difference marker in oracle/testkit/conformance; D-01 negative concentrations | oracle | done | T-012 |
| T-015 | caladrius-nca quality flags (NCA-LZ-12b) and T-013 review follow-ups | engine | done | T-013 |
| T-016 | specs/nca.md: PKNCA facts from T-012/T-013 (replaced values, IV first segments, AUMC tail, Tlag) | reader | done | T-013 |
| T-017 | specs/models.md and specs/fit.md sync after the oracle (M5 fix, precision note, fit facts, tags) | reader | done | T-009, T-010, T-011 |
| T-018 | Arbitration of the 1/ŷ fit oracle cases (fixed points) and the F1 trace tolerance | oracle | done | T-009, T-011a |
| T-019 | Private oracle infrastructure (feature private-oracle, export loader, private conformance counts) and exercise 1 NCA | oracle | done | T-013 |
| T-020 | caladrius-project (data model) and caladrius-engine (command registry, schemas, history) | interface | done | T-011b |
| T-021 | caladrius-cli and conformance extended to models and fit | interface | done | T-020 |
| T-022 | caladrius-mcp (MCP server over stdio) | interface | done | T-020 |
| T-023 | specs/nca.md: facts observed on the reference screen (default AUC method, λz weighting, empty acceptance criteria) | reader | done | T-016 |
| T-024 | caladrius-ui first slice: theme tokens, worksheet, NCA page with λz point selection, snapshots | interface | done | T-022, T-017 |
| T-025 | Units in NCA results (time/conc/dose units as data, derived units per parameter) | engine | done | T-015 |
| T-026 | caladrius-ui second slice: model fit page with live curve, diagnostics plots, simulation page | interface | done | T-024 |
| T-027 | caladrius-ui third slice: project save/load, command palette, settings page, T-026 follow-ups | interface | done | T-026 |
| T-028 | Release scaffolding for v0.1.0: GitHub Actions CI, CITATION.cff, conformance wording fix | interface | done | T-027 |
| T-029 | Honest validation claims: coverage table per model, tolerance note, missing fit oracles (infusion, zero-order, lag) | oracle | done | T-021 |
| T-030 | Closed-form derivatives for infusion, zero-order and lag models; xtask fit conformance with fixed parameters | engine | done | T-029 |
| T-031 | specs/models.md: two-compartment models (parameterisations, closed forms, derivatives, ids, oracle cases) | reader | done | T-017 |
| T-032 | Oracle for the two-compartment models (grids, 256-bit, deSolve, derivatives, failing tests) | oracle | done | T-031 |
| T-033 | caladrius-models pk2.* (two compartments), derivatives, xtask conformance for pk2 | engine | done | T-032 |
| T-034 | pk2 reachable from the engine, CLI and MCP (a, now), then the UI (b, after the benchmark) | interface | done | T-033 |
| T-035 | specs sync after T-032/T-033: output names, AUMC(0,t), dose 0 conventions, OF-08 (no automatic initial estimates for pk2), confirmed-by-oracle tags for MOD-2C | reader | done | T-033 |
| T-036 | cargo xtask lint: forbidden names, private/ leaks, deny-lints header, tolerances once, board hygiene; CI step | interface | done | T-028 |
| T-037 | README: sync the verified section with main (PR #1, external contribution) | orchestrator | done | T-034 |
| T-038 | docs/screens: the application illustrated from the snapshot example (PR #2, external contribution) | orchestrator | done | T-034 |
| T-040 | Default fit settings reach the exact minimum (Q-014 option 2); reference_conventions preset; D-04 | engine | done | T-030 |
| T-039 | cargo xtask review: the stated numbers confront the files that own them (PR #3, external contribution) | interface | done | T-036 |
| T-041 | cargo xtask review follow-ups (strict can pass, pk2 drift rule, missing failure tests, .get indexing, line numbers) | interface | done | T-039 |
| T-043 | Release workflow: binaries for Linux and Windows attached to every version tag; checkout@v5 | contributor-dsh | done | T-028 |
| T-042 | analysis.compare command (differences, ratios between two analyses, computed by the engine), CLI and MCP | interface | done | T-034 |
| T-045 | docs/validation.md: validation report (versions, methods, coverage, differences, limits, reproduction) | contributor-dsh | done | T-040 |
| T-044 | Fit page: preset choice (Caladrius default / reference conventions) in More options | interface | done | T-040 |
| T-046 | Second source for the compartmental specs (Bertrand and Mentré 2008): page citations, multiple dosing and steady state, later models | reader | done | T-035 |
| T-047 | Oracle for multiple dosing and steady state (OM-16): superposition and steady-state grids, ODE cross-check, failing tests | oracle | done | T-046 |
| T-048 | Release packaging: SHA256SUMS with LF only on Windows (follow-up of T-043) | contributor-dsh | done | T-043 |
| T-049 | caladrius-models dosing regimens (schedules, steady state) for pk1 and pk2; xtask conformance for md/ | engine | done | T-047 |
| T-050 | specs sync after the multiple-dosing oracle and engine (decisions, D-05, confirmed tags, OM-16/17) | reader | done | T-049 |
| T-051 | Superposition cost: early exit for exhausted doses (O(doses x times) today) | engine | done | T-049 |
| T-052 | Plain-words README introduction and self-contained explainer page (docs/explainer.html) | orchestrator | done | T-049 |
| T-052b | Superposition follow-ups from the T-051 review (early refusal, count-based timing test, cheaper exhaustion check, spec lines) | engine, reader | done | T-051 |
| T-053 | Indexing lint on the numerical crates: clippy::indexing_slicing denied in nca, models, fit, project through cargo xtask lint | interface | done | T-052b |
| T-054 | Gate v2: closed typed renderer that prints the parameter name with every value (RT-A1) | apothicaire | todo | |
| T-055 | Gate v2: model-written text fields never render as a quantity (RT-A9) | apothicaire | todo | T-054 |
| T-056 | Gate v2: no raw JSON path or unitless scalar in the number channel (RT-A6) | apothicaire | todo | T-054 |
| T-057 | Gate v2: a contradictory unit anywhere in the sentence is a finding (RT-A2) | apothicaire | todo | |
| T-058 | Gate v2: close the blind number channels (sign outside adjacency, numbers in words, Unicode digits) (RT-A10) | apothicaire | todo | |
| T-059 | Apothicaire: send the units option to nca_run so CL and V come out in L/h and L (RT-E4) | apothicaire | to review | |
| T-060 | Apothicaire: expose λz point selection (exclude, manual) to the model (RT-E6) | apothicaire | to review | |
| T-061 | Apothicaire: run only the exposed tools and bound the data_import path (RT-E8) | apothicaire | to review | |
| T-062 | Apothicaire: break tool-call loops and never show a raw tool call as the answer (RT-E1) | apothicaire | to review | |
| T-063 | Gate v2: a threshold cannot be rendered where a result is expected (RT-A3) | apothicaire | todo | T-054 |
| T-064 | Gate v2: user numbers keep their role, unit and sign (RT-A4) | apothicaire | todo | |
| T-065 | Gate v2: drop the v1 exemptions outside a recognised PK label (RT-A5) | apothicaire | todo | |
| T-066 | Benchmark scorer: score precision, sign, negation, contradiction and the acceptability verdict (RT-B3) | apothicaire | in progress | |
| T-067 | Apothicaire: flag or refuse a reference to a superseded analysis (RT-C3) | apothicaire | todo | |
| T-068 | Apothicaire: invalidate stored references when the engine restarts (RT-C9) | apothicaire | todo | |
| T-069 | Apothicaire: fixed seed and an exportable trace per answer (RT-E5) | apothicaire | todo | |
| T-070 | Frozen set: write the aggregation, the success definition and the precision target; align 25 and 60 (RT-B5) | apothicaire | to review | |
| T-071 | Benchmark: a turn that fails with an error counts as wrong (RT-B9) | apothicaire | in progress | |
| T-072 | Frozen set: choose the demo gate outside the frozen set (RT-B10) | apothicaire | to review | |
| T-073 | Benchmark generator: include profiles where the half-life cannot be computed (RT-B7) | apothicaire | todo | |
| T-074 | OptChat: a recall bench on long NCA results with confusable values (RT-C1) | optchat | todo | |
| T-075 | OptChat: make long messages fully readable through the memory tools (RT-C2) | optchat | todo | |
| T-076 | OptChat: the summary cleanup must not cut a leading number (RT-C10) | optchat | todo | |
| T-077 | Engine: a full output schema for analysis.compare (RT-A8) | interface | todo | |
| T-078 | Officina README: state the OptChat result as measured (8/8, 7/8, 8/8) (RT-C7) | orchestrator | todo | |
| T-079 | NCA: show the area_past_tlast flag in answers and decide O-18 (RT-D7) | reader, apothicaire | todo | |
| T-080 | CI: test on the demo platform, test before release, pin the Rust toolchain (RT-E9) | interface | todo | |
| T-081 | docs/validation.md: write the real rule for interval bounds (RT-D6) | oracle | todo | |
| T-082 | Freeze the scorer and labels by tag before the frozen run (RT-B1) | apothicaire | todo | T-066, T-070, T-071, T-072 |
| T-083 | Benchmark statistics at the exercise level; retire the per-number 0.32 % bound (RT-B4) | apothicaire | in progress | |
| T-084 | Say 'never computes' in its narrow sense, and check tool-call arguments (RT-D1) | orchestrator, apothicaire | todo | |
| T-085 | validation.md: say what is internal, tighten the independent cross-checks, require nls convergence (RT-D2) | oracle | todo | |
| T-086 | Show the validated grid and its gaps under the conformance figure (RT-D3) | orchestrator | todo | |
| T-087 | Retire 'clean room' as an argument; describe the process and the recorded exposure (RT-D4) | orchestrator | todo | |
| T-088 | Show the fit tolerance in the output when the reference_conventions preset is active (RT-D5) | interface | todo | |
| T-089 | Prepare the answer to 'does it reproduce our reference software?' (RT-E2) | orchestrator | todo | |
| T-090 | Demo slide: the assistant's scope (RT-E3) | orchestrator | todo | |
| T-091 | Trademark check for 'Caladrius'; label in-sample figures as such (RT-E7) | orchestrator | todo | |
| T-092 | Gate v2: remove the false positives (unit after the reference, words, Markdown) (RT-A7) | apothicaire | todo | |
| T-093 | Gate v1: membership is not provenance for summarised values (RT-C6) | apothicaire | todo | |
