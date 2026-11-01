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
