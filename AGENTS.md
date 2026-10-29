# AGENTS.md: contract for agents

Project name: **Caladrius** (crates are prefixed `caladrius-`; the bare `caladrius` name is free on crates.io and PyPI and is reserved for the desktop application). The name, the code, the docs and the UI never contain "Phoenix", "WinNonlin" or "Certara", except in a neutral statement of compatibility and the trademark notice in the README.

Caladrius is an open-source pharmacokinetic analysis application written in **Rust only**: non-compartmental analysis (NCA), individual compartmental models, least-squares fitting, plots. Native UI in egui (eframe on wgpu); the same code compiles to WebAssembly for the demo. **No Tauri, Electron or webview.** License: MIT OR Apache-2.0.

Caladrius is also the verified calculation layer of **Apothicaire**, a local DMPK agent developed in the parent folder: every Caladrius command must be callable by a machine (CLI and MCP server) with the same semantics as from the UI.

**Everything in the repository is written in English**: code, identifiers, comments, docs, commit messages, the board, UI text, and `board/QUESTIONS.md`. The human may write to you in French; answer in English.

Read this whole file before doing anything. If a task conflicts with this file, this file wins.

**Motto: we should be more ambitious.** Ambitious in what we build and test, never in what we claim. Each role says what this means for it in `.claude/agents/`, and every report ends with one bolder next step, which the human decides on (section 10, rule 7).

## 1. Scope

In scope (v1): data worksheet (CSV import), NCA, XY plots, one-compartment models (bolus, infusion, zero- and first-order absorption, tlag), weighted fitting, export of result tables, a CLI and an MCP server exposing the command registry.

Only afterwards, in this order: two compartments, user-written models (ODEs), several subjects. Start an item only when `docs/conformance.md` is green for everything before it.

Out of scope: population modelling (NLME), bioequivalence, IVIVC. Do not start them, not even "to prepare".

## 2. What the human does (closed list)

The human (Abdou) only:

1. Installs the reference software on the machine, outside the project folder.
2. Drops once into `private/`: exported result tables (CSV), his coursework data, and about fifteen screenshots of the reference software.
3. Answers the questions in `board/QUESTIONS.md`.
4. Decides every publication: pushing to GitHub, making the repository public, publishing any numeric comparison with the reference software.

Agents do everything else. Ask the human only when none of the rules below settles the matter; then write the question in `board/QUESTIONS.md` with the decision you took meanwhile, move to another task, and do not block.

## 3. Golden rules

1. **Clean room.** We reproduce behaviour, never code. Allowed sources: textbooks (Gabrielsson & Weiner, Gibaldi & Perrier), papers, the `PKNCA` documentation, the public user documentation of the reference software (section 6), observed results. Forbidden: opening the reference software's install folder, decompiling, copying code, icons, help text, wording of dialogs, or screenshots. Every third-party asset has its license and a row in `ATTRIBUTION.md` in the same commit.
2. **Tests are the gate.** No change without a test. A number outside tolerance fails the test. Conformance floors only go up.
3. **Engine first.** Everything is drivable without a window, from tests and the CLI. The UI is one client.
4. **Everything is a command.** Each user-visible action has a stable id (`nca.run`, `fit.run`, `plot.xy`, `data.import`…) and serializable parameters (serde). Menus, shortcuts, CLI, the MCP server and agents call the same command.
5. **Nothing baked into the core.** Units, route of administration, number of compartments, weighting, number of subjects, AUC rule: these are data, not assumptions, even if v1 exposes only part of them.
6. **Never crash.** Outside tests: no `unwrap()`, `expect()`, `panic!`, `todo!`, `unreachable!`, no `[i]` indexing on input-derived data (denied by `cargo xtask lint` through `clippy::indexing_slicing` in the four numerical crates (nca, models, fit, project), library targets; checked in review elsewhere), no `unsafe`. Every crate starts with `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unreachable)]`. NaN, zero or negative concentrations, values below the limit of quantification, duplicated or unsorted times, non-estimable λz, singular matrix, non-convergence: each gives a readable error and a regression test.
7. **Thin, data-driven UI.** UI state is serializable. Colours, radii and spacing come from a theme token file (light and dark), never hard-coded.
8. **Look at what you draw.** Every UI change is rendered offscreen to PNG (`cargo run -p caladrius-ui --example snapshot`) and looked at before delivery.
9. **Never break wasm.** Layers L0 to L3 compile for `wasm32-unknown-unknown`. Platform-only code (files, threads, processes) is gated behind `cfg` and lives in `apps/`. `cargo xtask wasm` is a gate.
10. **Same numbers on every machine.** The desktop application, the command line, the MCP server and the page in the browser are one engine, and what `Engine::execute` returns must not depend on where it runs. The standard library's `exp`, `ln`, `powf`, `powi`, `sin`... call the platform's maths library and are not rounded alike on Windows, Linux, macOS and WebAssembly. In the numerical crates, the engine and `xtask`, `clippy.toml` bans them (`disallowed-methods`) and the code calls the pure Rust `libm` (`libm::exp(x)`), pinned to one version in the manifests; `sqrt`, `abs`, `floor`, sums, products and quotients are rounded the same everywhere and stay. `caladrius-ui` and `caladrius-testkit` opt out in their crate root, with the reason. `cargo xtask digest` runs a fixed workload through the engine and prints one SHA-256 of the answers; CI runs it on Linux, Windows, macOS and in WebAssembly under Node, and fails on any difference. A new `libm` version changes the digest: upgrade it on purpose, with `docs/conformance.md` regenerated. Known gaps, listed rather than fixed: `min`, `max` and `clamp` may return either zero when they meet +0 and -0 (only the sign of a zero can differ, and the digest tells them apart), and `usize` is 32 bits on wasm32 (an option above 4 294 967 295 is refused with another message there). The answer of every command is sealed (`caladrius-engine`, `seal`: the SHA-256 of the command, its parameters and its answer) and `project.replay` runs a saved history again against its seals (`docs/reproducibility.md`), so an answer is a function of the parameters and of the project alone: no clock, no random number, no path and no fact about the machine in it (a time or a version goes beside the seal, never in it). Changing what is hashed changes every seal: bump `seal::FORMAT` (a seal of another format is not recomputed) and say so in the card.

## 4. Repository map and layers

```text
crates/
  caladrius-nca      L0  AUC (linear, log-linear, mixed), λz, t½, extrapolation, AUMC, MRT, CL/F, Vz/F, Vss
  caladrius-models   L0  closed-form solutions: 1 then 2 compartments, bolus, infusion, zero- and first-order absorption, tlag
  caladrius-fit      L0  weighted least squares (1, 1/y, 1/y², 1/ŷ, 1/ŷ²): estimates with SE, CV% and intervals, secondary
                         parameters, diagnostics (sums of squares, S, DF, AIC, SBC), matrices, eigenvalues, condition numbers,
                         partial derivatives, predicted data, minimization trace
  caladrius-project  L1  project model: worksheets, workflow objects, results (pure data, serde)
  caladrius-engine   L2  command registry, history, seals, replay, export
  caladrius-ui       L3  egui interface
  caladrius-testkit      oracles, tolerances, table comparison
  caladrius-digest       a fixed workload through the engine, canonical text and SHA-256 of the answers (cargo xtask digest)
apps/
  caladrius          desktop application
  caladrius-cli      the same without a window
  caladrius-mcp      MCP server exposing the command registry to agents (Apothicaire)
  caladrius-web      the same interface in a browser page (WebAssembly)
xtask/               layers | lint | review | wasm | web | conformance | digest
specs/               behaviour and UX specifications, written by us
oracle/              public data and expected results (versioned)
private/             reference exports, coursework data, screenshots (git-ignored)
board/               shared board for agents (section 9)
```

A crate depends only on lower layers. Nothing below `caladrius-ui` knows egui, eframe or winit. `cargo xtask layers` checks this; every new crate is registered there.

## 5. Oracle and tolerances

Three sources, in priority order for versioned tests:

1. **Public, in `oracle/`**: free datasets (theophylline, indomethacin) with `PKNCA` results produced by a versioned R script; worked examples from textbooks, with the page reference; published validation tables of other open or free NCA tools when their license allows it.
2. **Private, in `private/`**: coursework results and exports from the reference software. The matching tests sit behind the Cargo feature `private-oracle` and are skipped when the folder is empty. No file, number or subject name from `private/` may appear in a versioned file, a commit message or the board.
3. **Independent computation**: for a case with no reference, a second, naive implementation in `caladrius-testkit` (never the same function tested against itself).

Tolerances (defined once in `caladrius-testkit`, never loosened inside a test):

- NCA against PKNCA: relative error ≤ 1e-6.
- Against a reference export: equality at the precision displayed in the export.
- Fitting: parameters within 1e-4 relative, weighted sum of squares within 1e-6 relative. Two optimizers do not reach the same minimum bit for bit; a gap beyond tolerance is a method difference to document in `specs/differences.md`, not a tolerance to widen.

"100% reproduction" means: 100% of in-scope parameters, within these tolerances, on every oracle case. `cargo xtask conformance` generates `docs/conformance.md` (validated parameters per case); never edit that file by hand. The README states no measured figure (`review-readme-figures`): each figure lives in its owner file (`docs/conformance.md`, `docs/validation.md`, a release note) and the README links to it.

## 6. Reference documentation: how to use it

Goal: learn the conventions and defaults that change a number (point selection for λz, trapezoid rule, handling of values below the limit of quantification, of zero times, of extrapolation, definition of the weights, stopping criterion of the fit).

Two separate roles, to keep the clean room credible:

- The **reader** agent is the only one who reads the public documentation of the reference software. It writes in `specs/` a description of the behaviour **in its own words and in formulas**, with no copied sentence, citing for each rule its source (page title) and, where possible, a second independent source (textbook, paper, PKNCA).
- **Implementer** agents read only `specs/` and the oracle. They never open the reference documentation.

Every rule in `specs/` carries a status: `confirmed by oracle`, `documented, untested` or `assumed`. An `assumed` rule is never presented as settled; it becomes a test task.

Fitting defaults seen on the human's screen (status `assumed` until the reader confirms them and the oracle tests them): Gauss-Newton minimization with the Levenberg and Hartley modification, increment for partial derivatives 0.001, convergence criterion 0.0001, 50 iterations maximum, 1000 predicted values for the curve, uniform weighting on observed values, parameter bounds generated by the software unless the user supplies them. The same method must be available in `caladrius-fit` as the named preset `reference_conventions`; since Q-014 (2026-10-09, option 2) the default is the exact minimum (closed-form derivatives, relative decrease 1e-10), documented as D-04 in `specs/differences.md`. Another optimizer may exist as an option, never as a silent replacement.

If the reference software's license or the documentation site's terms forbid this use, the reader stops and records it in `board/QUESTIONS.md`.

## 7. UI and screenshots

Design goal: a user of the reference software finds the same way of working (project tree, worksheet, analysis object with setup and results, plots) and recognizes nothing else. Caladrius must be visibly more modern, faster and easier: fewer clicks, sensible defaults, live preview, readable errors.

- Screenshots in `private/screenshots/` are seen by the **reader** only. It writes `specs/ux.md`: the workflow step by step, what each screen lets the user decide, and a list of friction points to remove. It describes; it does not transcribe labels or layouts.
- The **interface** agent works from `specs/ux.md` and the theme tokens. It never opens the screenshots.
- Do not reproduce the reference software's layout pixel for pixel, its icons, its colours or the wording of its dialogs. Standard domain vocabulary (NCA, AUC, λz, Cmax…) is free to use.
- Required improvements for v1: click points on the curve to choose the λz range; the predicted curve updates live while initial estimates change; one screen from data to results for NCA; every error says what to fix.
- Friction observed in the reference workflow, each to be removed:
  1. Creating a model takes a four-level nested menu from a worksheet. Caladrius: one "New analysis" button and a command palette.
  2. Settings for one analysis are spread over a left-hand list, five bottom tabs and a separate setup/results switch. Caladrius: one page, read top to bottom (data, model, dose, initial estimates, options), with the plot always visible.
  3. Results are a flat tree of about twenty tables and six plots opened one at a time. Caladrius: a summary first (parameters with precision, fit plot, residuals, goodness of fit), details on demand.
  4. A poor fit is not flagged: a CV% in the hundreds or a confidence interval crossing zero looks like any other number. Caladrius: flag them and say what to check.
  5. Units are free text and inconsistent combinations pass silently into derived units. Caladrius: check dose, concentration and time units together and warn before running.
  6. Models are picked from a numbered table. Caladrius: pick by route and number of compartments, with the diagram and equation shown.
  7. The project tree fills with objects named "PK 1", "ASCII 2". Caladrius: name objects from their content and show stale results clearly after an input change.
  8. User models need a fixed-format block language. Caladrius (later step): readable equations with named parameters and immediate error messages.
- No screenshot of the reference software ever enters the repository, the README or an issue.

## 8. Agent organization

The main session is the **orchestrator**: it plans the iteration (section 9), writes the cards, delegates, integrates, runs the verification itself and talks to the human in French. Every other role is a file `.claude/agents/<role>.md`, with its model and effort in the front matter; `cargo xtask lint` checks that they agree with this table.

| Role | Model, effort | Does | Does not | Why this model |
|---|---|---|---|---|
| `orchestrator` | Opus, max | plans, writes cards, delegates, integrates, verifies, keeps the board | write code, read large files | it holds the longest context, so not the dearest model |
| `engine` | Opus, high | `caladrius-nca`, `caladrius-models`, `caladrius-fit`: everything that produces a number | UI | long agentic coding where a wrong digit matters |
| `reader` | Sonnet, high | `specs/` from the allowed sources and the screenshots | code | careful reading against a clear card |
| `oracle` | Sonnet, high | `oracle/`, `caladrius-testkit`, R scripts, tests that fail first | fix the engine to make a test pass | a clear card, and another model than the engine it checks |
| `interface` | Sonnet, high | the workspace, `xtask`, `caladrius-project`, `caladrius-engine` commands, `caladrius-ui`, `caladrius-cli`, `caladrius-mcp`, `caladrius-web` | numerical code | routine coding against a clear card |
| `reviewer` | Fable, high | reviews a diff it did not see being written; the iteration audit | modify the reviewed code | it reasons hardest and wrote none of the code |
| `janitor` | Haiku, low | formatting, renames, file moves, board housekeeping asked by the orchestrator | any decision | mechanical work |
| `onizuka` | Opus, high | explains a part of Officina to the human, in French; the learning page of each iteration | edit a repository | teaching from the real code |

Rules:

- A subagent receives **one** card, with its allowed files and its completion criterion. It returns a report of at most ten lines (an audit: one line per finding): what was done, the numbers, what is still open, and one bolder next step.
- A subagent's report is a claim, not a proof: the orchestrator runs the card's completion command itself before marking it done.
- Whoever writes an oracle test does not write the code under test.
- Every change in `caladrius-nca`, `caladrius-models` or `caladrius-fit` goes through `reviewer` before being marked done. `reviewer` gets a diff and its card, never a whole crate; a card outside these three crates may be reviewed with `model: sonnet`.
- Parallel work: two agents never touch the same crate. Each uses `CARGO_TARGET_DIR=target/agent-<role>`. A new module per feature instead of growing a shared file. If someone else's work breaks the build, wait and retry; do not fix their files.
- Agent files load when a session starts in this repository. After adding or renaming one, start a new session before delegating to it.

## 9. The shared board

```text
board/
  INDEX.md          overview: one line per task (id, title, role, state). Only the orchestrator writes it.
  tasks/T-042.md    one card per task, iterations included. Only the assignee writes it, plus the orchestrator.
  LOG.md            append-only: one entry per finished task.
  QUESTIONS.md      questions for the human, with the decision taken meanwhile.
  messages/         one file per message between agents: <date>-<from>-<to>-<subject>.md
```

Task card: goal, allowed files, verifiable completion criterion (a command and its expected result), dependencies, state (`todo`, `in progress`, `to review`, `done`, `blocked`), then a dated thread of notes. A card that belongs to an iteration says so in its header: `- **Iteration:** T-xxx`.

One file per task and per message: this is what stops two agents from editing the same file. Subagents do not talk to each other directly; they write in `messages/` and the orchestrator relays. The board is versioned: it is also the project's memory from one session to the next.

**One session = one iteration.** An iteration is a card titled `Iteration <n>: <name>`, with `- **Kind:** iteration` in its header; no other file holds the plan. It is `done` only when none of its cards is `todo`, `in progress` or `to review`, and `cargo xtask lint` checks its sections.
1. **Brief**, a `## Brief` section, before any work: `**Goal.**`, `**Deliverables.**`, `**Claims touched.**` (the published figures it may change), `**Done when.**` and `**Plan.**` (wave, card, role). Each card is written before it is assigned, so that a cut session resumes from it.
2. **Delegate**: the independent cards of a wave run in parallel, within section 10, rule 2.
3. **Verify**: the orchestrator integrates, then runs section 11 on the whole tree itself.
4. **Audit**: `reviewer` reads the iteration's whole diff; every finding is fixed or logged on its card.
5. **Outcome**, a `## Outcome` section: the results with their denominators, the lessons, and `**Bolder next step.**`, proposed to the human and not started.
6. **Teach**: `onizuka` writes the iteration's learning page in French; the orchestrator checks it, publishes it as an Artifact and links it as `**Learning page.**`; when the human waives it for an iteration, the line says `none` and why.

## 10. Budget: subscription constraints

The project runs on a Max subscription, with no pay-as-you-go billing. The quota refills in windows; when it runs out, the session stops dead, with no warning. Consequences:

1. **Always resumable.** After each task: tree builds, tests green, commit, entry in `LOG.md`. A session can be cut at any moment; the next one restarts from the board, not from conversation memory.
2. **Two at a time.** At most 2 subagents in parallel, only on independent cards that share no crate and no file, and at most one of them on Fable. Never 8.
3. **The most expensive model reads the least.** Fable reads a diff and its card. The orchestrator reads the board, the reports and the output of the commands it re-runs, not the code. File search, renames, formatting: Sonnet or lighter.
4. **Small tasks.** A task fits in one subagent's context: one function and its tests, not a whole crate.
5. **No loops.** Three failed attempts on the same test: the task becomes `blocked` with the diagnosis, and you switch tasks.
6. **Do not re-read needlessly.** Read the parts of a file you need, not the whole file; do not rerun the whole test suite when one crate is enough.
7. **No speculative work.** Nothing outside section 1, no rewrite "while we're at it". A bolder next step is a proposal to the human, never work started on its own.
8. **No paid service.** No billed API, no paid CI, no dependency under a commercial license.

## 11. Before you finish a task

```sh
cargo test -p <crates you touched>
cargo clippy -p <crates you touched> --all-targets -- -D warnings
cargo xtask layers
cargo xtask lint && cargo xtask review   # the mechanical rules, the board, the agent files, the validation figures, a README without figures
cargo xtask wasm          # if L0 to L3 changed
cargo xtask conformance   # if a number changed; commit docs/conformance.md
cargo xtask digest        # if the arithmetic of a result changed: the digest is the same on every platform (CI compares them)
```

Then: task card updated, entry in `LOG.md`, commit whose message starts with the task id.

Forbidden without written agreement in `QUESTIONS.md`: `git push`, changing the repository's visibility, deleting files outside the repository, installing software outside package managers (cargo, rustup, R), touching `private/` other than reading it.
