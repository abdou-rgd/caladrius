# Questions for the human

Each entry: the question, the decision taken meanwhile, and the human's answer when given.

## Q-001 (2026-10-08, orchestrator): changes made to the brief when creating `AGENTS.md`

Decisions taken meanwhile, to confirm or revert:

1. Project name Caladrius, crates prefixed `caladrius-`, the bare `caladrius` crate reserved for the desktop app (free on crates.io and PyPI on 2026-10-08).
2. `apps/caladrius-mcp` added to the v1 scope and to step 4 of the marching order, so that Apothicaire can call the command registry.
3. Golden rule 9 (wasm): advisory until step 6, a gate from step 6 on; platform-only code gated behind `cfg` and kept in `apps/`.
4. A `janitor` role (Haiku) for mechanical work.
5. Repository at `C:\Users\abdou\apothicaire\caladrius`, its own git repository, separate from the Apothicaire agent; can be moved out later without changes.

Answer (2026-10-08, human): all five confirmed.

## Q-002 (2026-10-08, orchestrator): comparing results with a commercial program

Closed 2026-11-01 (T-129). The comparison this question was about is removed from the repository. No conclusion is drawn here about what an academic licence permits. The answer recorded on 2026-10-08 is not relied on.

## Q-003 (2026-10-08, orchestrator): R packages

T-003 needs `PKNCA` installed in R (`install.packages("PKNCA")`). `AGENTS.md` allows installs through package managers, so the oracle agent will do it unless told otherwise.

Answer (2026-10-08, human): ok, install PKNCA.

## Q-004 (2026-10-08, oracle): license of the public datasets and of PKNCA output

T-003 adds `Theoph` and `Indometh` (R `datasets` package, distributed with R under GPL-2 or GPL-3) as CSV files in `oracle/data/`, and expected values computed with PKNCA (AGPL-3). The numbers are experimental measurements and computed results, but the CSV files are derived from files distributed under a copyleft license, while Caladrius is MIT OR Apache-2.0.

Decision taken meanwhile: keep the data and expected values in `oracle/`, list them in `ATTRIBUTION.md` with their origin and license, copy no code or documentation text from R or PKNCA, and keep `oracle/` out of any published crate (it is not under `crates/`). To confirm before the first release: either keep as is, or replace the CSV files by a pointer plus a script that regenerates them from R at test time.

Orchestrator note (2026-10-08): decision stands. Measured concentrations are facts, not code; R's GPL covers the package, PKNCA's AGPL covers its code, neither covers computed output. `oracle/` is not part of any published crate. To confirm by the human before the first public release.

Answer (human): pending.

## Q-005 (2026-10-08, reader): terms of a commercial product's public documentation

Closed 2026-11-01 (T-129). That documentation is not a source. The notes of what its terms say, and of what a search summary paraphrased, are removed from `specs/sources.md`. No conclusion is drawn here about what those terms permit.

## Q-006 (2026-10-08, reader): vendor and product names in `specs/sources.md`

Closed 2026-11-01 (T-129). The terms-of-use record that contained a vendor name is removed. The names stay in the README trademark notice, in the contract's ban, and in the lint that enforces the ban and the vendor-export guard.

## Q-007 (2026-10-08, reader): a published comparison table

Closed 2026-11-01 (T-129). The statement that the rules reproduce a published comparison table is removed (hand check H3). No number from that paper is in a versioned file. The licence of the paper is not ruled on, because the comparison is not used.

## Q-008 (2026-10-08, reader): defaults to read off a screen

Closed 2026-11-01 (T-129). Defaults are the ones specified from PKNCA and the engine. Notes of what was seen on a screen are removed. No further observation is requested.

## Q-009 (2026-10-08, reader): fitting defaults to read off a screen

Closed 2026-11-01 (T-129). Fit behaviour is specified from papers, from R `nls`, and from the engine. Comparing another program's setup is not a task.

## Q-010 (2026-10-08, reader): replaced values and the extrapolation to infinity

Closed 2026-11-01 (T-129) as to any other program. The engine keeps the PKNCA-compatible tail and raises `area_past_tlast` (`specs/nca.md` NCA-EXT-01b). Whether another program counts the stretch twice is not asked.

## Q-011 (2026-10-08, oracle): how to drop vendor exports

Closed 2026-11-01 (T-129). There is no private export oracle. Do not add vendor exports, project files or screenshots.

## Note on Q-008 (2026-10-08, reader): screenshots

Closed with Q-008 (T-129). The screenshot notes are removed.

## Q-011b (2026-10-08, reader): a sample file for the import heuristic

Closed 2026-11-01 (T-129). The import heuristic stays as written in `specs/ux.md`. No sample taken from a vendor file is requested.

## Q-012 (2026-10-08, reader): a private oracle for open NCA conventions

Closed 2026-11-01 (T-129). The campaign to settle conventions by running a profile through another program is withdrawn. The later note that an export had been compared is withdrawn with it: that comparison is not a result the project stands on.

## Q-013 (2026-10-08, orchestrator): publication

The human decided on 2026-10-08: publish the repository on GitHub, public, under a new organisation `officina-dmpk` (repository `caladrius`). The organisation is created by the human on github.com; the orchestrator then adds the remote and pushes `main`. Every later push is covered by this decision for this repository. A numeric comparison with a commercial program is not in this repository (T-129).

Answer (human): confirmed (chat, 2026-10-08).

## Q-014 (2026-10-09, orchestrator): default fit settings: the looser preset, or the exact minimum?

T-029 and T-030 measured, on all 185 reference fits (490 estimates), how far the engine's DEFAULT settings land from the exact minimum reached by R (`nlsLM`/`nls`, analytic derivatives, tight stop):

| setting | estimates beyond 1e-4 | worst gap |
|---|---|---|
| forward differences 1e-3, reference stopping rule | 40 | 6.9e-4 |
| forward differences 1e-5, reference stopping rule | 0 | 1.1e-5 |
| forward differences 1e-3, default stopping rule (relative decrease 1e-4) | 168 | 5.8e-3 |
| analytic derivatives, default stopping rule | 149 | 5.1e-3 |

The defaults (forward differences with increment 0.001, convergence 0.0001, 50 iterations) are the optional preset `reference_conventions`. Keeping them means a user who clicks "Fit" gets estimates accurate to about 5e-3 relative; the oracle tests, which run with analytic derivatives and the tight stop, are unaffected either way. Options:

1. Keep the defaults; state in D-03, `specs/fit.md` and the README that default fits are accurate to about 5e-3, and show the "derivatives" and "convergence" options in the UI with a sentence.
2. Default to analytic derivatives (available for every v1 model in the (v, k) parameterisation since T-030) and a tighter stop (relative decrease 1e-8), and document the difference as D-04. Estimates then match R within 1e-4 by default.

Decision taken meanwhile: option 1 for v0.1.0 (no behaviour change before the tag); the texts say so. Your answer decides v0.2.

Answer (2026-10-09, human): option 2. The default fit uses analytic derivatives and the tight stopping rule; the other iteration settings stay available as the named preset `reference_conventions`. Card T-040.

## Q-015 (2026-10-29, orchestrator): remove the user-level `/onizuka` command?

Since T-115, Onizuka has one place: `.claude/agents/onizuka.md` in this repository, the master copy. The French command `C:\Users\abdou\.claude\commands\onizuka.md` (235 lines, an earlier version) is still offered in every Claude Code session of the PC, other projects included. It lies outside every repository, so section 11 needs written agreement before it is deleted. Options: (1) delete it; (2) keep it as the human's own shortcut, knowing that it is no longer updated.

Decision taken meanwhile: kept, untouched. `~/.claude/commands/plume.md` stays where it is either way (one place already; it holds the owner's biography and personal texts, which stay out of public repositories).

Answer (2026-10-29, human, in chat): "puis tu supprimes Q-015", read by the orchestrator as option 1: delete the user-level command. Not done by the orchestrator: the session's permission check refused the deletion of a file outside every repository, and it was not worked around. Abdou deletes it by hand (`rm C:/Users/abdou/.claude/commands/onizuka.md`, or `Remove-Item` in PowerShell); until then it stays, no longer updated.

## Q-016 (2026-10-29, orchestrator): merge and push the branches of T-113 to T-118

`contract/iterations` (this repository), `onizuka/one-place` (apothicaire) and `method/iterations` (officina umbrella) were committed and not pushed, as asked. Q-013 covers pushes of this repository only.

Answer (2026-10-29, human, in chat): "on fusionne et push d'abord". The three branches are merged into their `main` and pushed. A push of the umbrella's `main` deploys the site only when `site/**` changes; `METHOD.md` does not.

## Q-017 (2026-10-29, orchestrator): hosted-model runs and rule 8 ("No paid service. No billed API")

The audit of T-119 to T-122 found that `../agent/bench/FRONTIER_ARM.md` and `../agent/bench/frozen/LEDGER.md` (2026-10-22) record three runs of the agent on hosted models, in a cloud session: 12.17, 20.96 and 10.30 USD, 43.43 USD in all. `AGENTS.md` section 10 rule 8 forbids any billed API, and no entry of this file authorises them. Options: (1) record that Abdou authorised these runs, as an exception for the frontier arm of the frozen set, with a budget; (2) amend rule 8 so that a billed API is allowed for a measurement Abdou approves in writing, never in the product; (3) stop the frontier arm. Meanwhile the READMEs say "nothing leaves the machine" only of the assistant at run time (T-119, T-122).

Answer (2026-10-29, human, in chat): the runs used free credits offered by Anthropic, so no money was spent; Abdou knew the rule was set aside. Read by the orchestrator as option 1: an exception recorded here for the frontier arm, on free credits only. Rule 8 is unchanged: any later billed run needs the same written agreement in this file first.

## Q-018 (2026-10-29, orchestrator): the family name on the GitHub profile

T-104 (Abdou's request of 2026-10-22) removed the family name from the four repositories, and `.claude/agents/onizuka.md` says it never appears, anywhere. The public profile `abdou-rgd/abdou-rgd` starts with the full name (`### Abdallah ...`), as does its copy `../notes/github-profile-README.md`. Options: (1) keep it on the profile only, which is Abdou's own page, and say so in the rule; (2) replace it with « Abdallah » or « Abdou ». Untouched until the answer.

Answer (2026-10-29, human, in chat): "il faut retirer mon nom de famille", option 2. The profile now says « Abdallah » (abdou-rgd/abdou-rgd ff2ecf1, pushed), and its copy in `../notes/` too. Still there, outside the text: the author e-mail of past commits in every public repository, which only a history rewrite would remove; future commits can use the GitHub no-reply address if Abdou changes the git setting.

## Q-019 (2026-11-06, orchestrator): push the prototype branches of iteration 2

Iteration 2 (T-130) builds three prototypes of the NCA screen on branches (`iter2/base`, then one branch per stack). Q-013 covers pushes of this repository after a closed task; these branches are pushed before any card closes, so that CI can render them on Linux. Nothing of them merges into `main` before 2026-11-18.

Answer (2026-11-06, human, in chat): "oui pour pousser les branches". The branches of T-133 to T-137 may be pushed. Merging into `main` still waits for the decision of T-138.

## Q-020 (2026-11-07, orchestrator): amend `AGENTS.md` for iteration 3 (T-139): the interface target and the population scope

Iteration 3 brings three compartments, population fitting and population simulation into the engine, and records the stack verdict of T-130 (`board/messages/2026-11-07-fable-stack-investigation.md`). Without points 3 and 4 the contract forbids bricks 1, 3 and 4: line 21 puts NLME out of scope "not even to prepare", and line 11 says the contract wins over any task. Each point is today's text, then the proposed text; answer per point (yes, no, or your wording).

1. **Line 5, what Caladrius is and its interface.**
   Today: "Caladrius is an open-source pharmacokinetic analysis application written in **Rust only**: non-compartmental analysis (NCA), individual compartmental models, least-squares fitting, plots. Native UI in egui (eframe on wgpu); the same code compiles to WebAssembly for the demo. **No Tauri, Electron or webview.** License: MIT OR Apache-2.0."
   Proposed: "Caladrius is an open-source pharmacokinetic analysis application written in **Rust only** (a web front adds HTML, CSS and the JavaScript glue that wasm-bindgen generates, no JavaScript framework): non-compartmental analysis (NCA), compartmental models, individual and population fitting, simulation, plots. The reference interface is egui (eframe on wgpu), native and in WebAssembly, until its successor does everything it does. The successor is Leptos 0.8 in Tauri 2, under the three conditions of `board/messages/2026-11-07-fable-stack-investigation.md`: the engine never runs on the display thread; long commands report progress and can be cancelled (`docs/long-running-commands.md`); Leptos's maintenance is re-read at each of its releases, with Dioxus as the named fallback and nothing Leptos-specific outside the view layer. The browser without Tauri stays a target: the engine in WebAssembly in a Web Worker. **No Electron.** License: MIT OR Apache-2.0."
   Consequence: the TypeScript client of T-134 (branch `iter2/base`) does not reach `main` under this wording; the Leptos front gets a Rust client.

2. **Line 71, the layers.**
   Today: "A crate depends only on lower layers. Nothing below `caladrius-ui` knows egui, eframe or winit. `cargo xtask layers` checks this; every new crate is registered there."
   Proposed: "A crate depends only on lower layers. Nothing below the interface layer (L3: `caladrius-ui` today, the Leptos front and the Tauri shell later) knows egui, eframe, winit, Leptos, Tauri, `wasm-bindgen`, `web-sys`, `js-sys` or a webview; the fronts, the shell and the bridge live in `apps/`. `cargo xtask layers` checks this; every new crate is registered there."
   Consequence: `cargo xtask layers` learns the new names, on an `iter3/*` branch (code, so not on `main` before 2026-11-18).

3. **Section 1, lines 19 and 21, the scope.**
   Today, line 19: "Only afterwards, in this order: two compartments, user-written models (ODEs), several subjects. Start an item only when `docs/conformance.md` is green for everything before it."
   Proposed: "Then, in this order: two compartments (done), three compartments, several subjects with their own doses, population modelling (NLME: FOCE with interaction and its diagnostics), population simulation, user-written models (ODEs). Start an item only when `docs/conformance.md` is green for everything before it."
   Today, line 21: "Out of scope: population modelling (NLME), bioequivalence, IVIVC. Do not start them, not even "to prepare"."
   Proposed: "Out of scope: bioequivalence, IVIVC, and any population estimator other than the one named above until it is green. Do not start them, not even "to prepare"."
   If brick 2 keeps Michaelis-Menten elimination, it joins line 19 before population modelling as "one built-in ODE model (Michaelis-Menten elimination)".

4. **Rule 10 (line 43), a seeded stream for simulation.**
   Today: "...so an answer is a function of the parameters and of the project alone: no clock, no random number, no path and no fact about the machine in it..."
   Proposed: "...so an answer is a function of the parameters and of the project alone: no clock, no random number except a stream drawn from a seed that is one of the parameters, by a generator written in the engine and specified in `specs/`, no path and no fact about the machine in it..."

5. **Sections 4 and 5, the map and the tolerances.**
   Line 50, today "closed-form solutions: 1 then 2 compartments, bolus, ...", proposed "closed-form solutions: 1, 2 and 3 compartments, bolus, ...".
   Line 53, after "minimization trace", add "; population fitting (FOCE with interaction), its diagnostics, population simulation".
   Line 77, after the PKNCA sentence, add "population fits by nlmixr2, with saemix and `nlme` as second looks, from versioned R scripts; never a commercial program."
   After line 83, add one line: "- Population fitting: the tolerances are set in `caladrius-testkit` by the oracle card of iteration 3 from what it measures, asked in this file before any engine card uses them, then written here."

6. **`README.md` line 3, the interface sentence.**
   Today: "...with a native desktop UI (egui) and the same interface in a browser page (WebAssembly)."
   Proposed: "...with a native desktop UI (egui) and the same interface in a browser page (WebAssembly); egui stays the interface until its planned successor, Leptos in Tauri 2, does everything it does."
   The scope words of the same sentence ("individual compartmental models, weighted least-squares fitting") change only when population fitting reaches `main`, after 2026-11-18.

Decision taken meanwhile: nothing applied. On a yes, the orchestrator applies the agreed points in one commit on `main` (contract and README text, no code: the demo of 2026-11-17 is unaffected), with `cargo xtask lint` and `review` green and `AGENTS.md` under 200 lines (179 today; the points add about two). The roles table (section 8) is unchanged: the model of each card is chosen on the card of T-139.

Answer (2026-11-07, human, in chat): "oui lance, je vais m'en aller donc je te laisse gérer tout ça. pour l'interface, on refera des tests quand on aura intégré la popPK qui nous permettra de vraiment trancher sur quelle stack on part". Read by the orchestrator as a yes to the iteration and to the engine points 3, 4 and 5, applied on `main` the same day. Points 1, 2 and 6 (the interface) are held: Abdou decides the stack on the tests that follow population PK (T-130, wave 4), so line 5, line 71 and the README keep today's text until then. Line 5 still says "individual compartmental models" and "No Tauri, Electron or webview"; iteration 3 touches no interface, so nothing it does conflicts with it.

## Q-021 (2026-11-07, orchestrator): install Rtools 4.5 for the nlmixr2 oracle

nlmixr2 is the open oracle of the population fit (T-139, brick 3). It compiles each model to C through rxode2, which on Windows needs Rtools 4.5 (free, distributed by CRAN, with its own installer). Section 11 forbids installing software outside the package managers without written agreement here; the R packages themselves install through `install.packages`, which is allowed. On this PC (2026-11-07): R 4.5.2 with `nlme` 3.1.168; no nlmixr2, rxode2, saemix or Rtools. Options:
1. Install Rtools 4.5 on this PC; the oracle scripts run here like the PKNCA ones.
2. Run the nlmixr2 scripts in a cloud session (Linux has a compiler) and commit only their versioned outputs.
3. No nlmixr2: saemix and `nlme` only. The OFV at fixed parameters would then have no external oracle, only our naive implementation and the cases where the Laplace approximation is exact.

Decision taken meanwhile: option 1 is recommended; until the answer, the oracle card installs saemix with `install.packages` and writes the nlmixr2 scripts without running them.

Answer (2026-11-07): not answered in words. "je te laisse gérer tout ça" is not read as a yes to download and run an installer outside the package managers; the decision taken meanwhile stands. Rtools is needed only from wave 6 of T-139.

## Q-022 (2026-11-07, orchestrator): push the branches of iteration 3

Iteration 3 (T-139) keeps its code on branches `iter3/*` until 2026-11-18. CI checks them on Linux, Windows, macOS and wasm32 only once they are pushed, as Q-019 allowed for `iter2/*`. Q-013 covers pushes after a closed task only.

Decision taken meanwhile: the `iter3/*` branches stay local; `main` receives only `docs/` and `board/`, pushed under Q-013.

Answer (2026-11-07): not answered in words; the decision taken meanwhile stands.

## Q-023 (2026-11-07, orchestrator): the tolerances of the maximum-likelihood fit in `AGENTS.md` section 5

T-148 (the oracle of brick 2) needed tolerances that section 5 does not have: −2LL crosses zero, so a relative rule on it means nothing. It defined them once in `caladrius-testkit` (`AbsoluteTolerance`, `crates/caladrius-testkit/src/ml.rs`), and only the new tests use them. They agree with what `specs/fit.md` OF-12 proposed from the reader's prototype (−2LL to 1.3e-9 against `optim`).

Proposed line, after "Fitting: ..." in section 5 (181 lines today, one line added):

"- Maximum-likelihood fitting (`objective = ml`): −2LL, AIC and BIC within 1e-7 absolute (a log-likelihood can be negative, so a relative rule means nothing), IWRES within 1e-5 absolute, a value that is zero by structure within 1e-8 absolute; estimates and standard errors as for fitting."

Decision taken meanwhile: the constants stand in the testkit for the new tests and T-149 uses them as they are, reporting the worst error it reaches for each. Nothing is written in `AGENTS.md` without a yes here.
