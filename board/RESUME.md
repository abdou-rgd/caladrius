# How to resume (state of 2026-10-29)

1. Open Claude Code in `C:\Users\abdou\apothicaire\caladrius`: the agent files of `.claude/agents/` load only when a session starts here. Say: "Reprends le rôle d'orchestrateur : lis AGENTS.md, board/RESUME.md, board/INDEX.md et la carte d'itération ouverte, puis continue."
2. One session = one iteration (`AGENTS.md` section 9). The open iteration card holds the brief, the plan and the state of each card; if none is open, propose the next one to Abdou from the bolder next step of the last outcome.
3. Subagents do not survive a session: start one per card, with its role file, its allowed files and an absolute `CARGO_TARGET_DIR` under `target/agent-<role>`. At most two at a time, one on Fable.
4. A subagent's report is a claim: re-run the card's completion command before marking it done.

## State

- **Iterations.** None has run yet. T-113 to T-118 (merged and pushed on 2026-10-29: 2e3b2b0 here, apothicaire 278929c, umbrella 32b6953) brought in the motto, the roles with their model and effort, the iteration loop and its lint, and one place for Onizuka. Iteration 1 (T-124, done 2026-10-29, pushed in all five repositories): READMEs that do not drift; its outcome holds the figures and the bolder next step. Iteration 2, chosen by Abdou, next session: review the interface stack (prompt in `../notes/prompt-iteration-2-interface-2026-10-29.md`); the demo of 2026-11-17 stays on the current interface. The site redesign (T-123) comes after it.
- **Last pieces merged** (Sceau, 2026-10-23): T-111 same numbers on every machine (#19, 634ad58), T-112 a seal on every result and `project.replay` (#20, cb327a0); the site serves cb327a0 under `/app/`; apothicaire#19 re-golded the bench after T-111; gate v3 was killed by its pre-registered rule (apothicaire#18, 557/600 turns against 562), so v1 stays the demo gate. Details: the cards, `LOG.md` and git history.
- **First deadline:** the demo at Sanofi Montpellier on 2026-11-17 (`../notes/insights-audit-2026-10-16.md` section 4).

## Open on the PC

- Rebuild `caladrius-mcp` from main into `target/agent-apothicaire` before the next benchmark: the binary there is still the build of 2026-10-21, before T-111, and the frozen-set freeze must hash the new one (`cargo build --release -p caladrius-mcp --target-dir target/agent-apothicaire`).
- Frozen set, case freeze: it waits for `../agent/bench/frozen/demo/wordings.json`, written by Abdou (2 per turn kind) and dsh (1 per kind) from `wordings.template.json`; Claude never opens it. Then the case freeze and the frozen runs, as in the "Commands" section of `../agent/bench/frozen/README.md`, with C1 = v1 and C2 = v2.

## Waiting for Abdou

- Q-015, answered (delete): Abdou deletes `C:\Users\abdou\.claude\commands\onizuka.md` by hand; the session's permission check refused it.
- The Fisher-information design (it needs an amendment of `AGENTS.md` section 1 and the written agreement of the Sanofi tutor); `qc.nca` (differences between NCA tools as named conventions); the plan-first assistant (after the frozen run); the PAGE 2027 abstract (one per participant).
- Q-012 (reference mini-campaign on Theoph subject 1), the exports of exercises 2 to 4 into `private/exports/td2..4/`, the Windows decimal symbol of the reference software.
- Todo cards from the red team (T-054 to T-093, `INDEX.md`): most touch `../agent`, `../optchat` or the umbrella.

## Places

- Publication: https://github.com/abdou-rgd/caladrius. Q-013 authorises pushes of this repository; push after each closed task.
- Apothicaire is `..\agent\` (github.com/abdou-rgd/apothicaire), OptChat `..\optchat\` (github.com/abdou-rgd/optchat). The umbrella is `..\` (`README.md`, `notes/`); its repository github.com/abdou-rgd/officina is checked out in `C:\Users\abdou\officina-umbrella`.
