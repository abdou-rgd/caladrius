# How to resume (state of 2026-11-07, afternoon)

1. Open Claude Code in `C:\Users\abdou\apothicaire\caladrius`: the agent files of `.claude/agents/` load only when a session starts here. Say: "Reprends le rôle d'orchestrateur : lis AGENTS.md, board/RESUME.md, board/INDEX.md et la carte d'itération ouverte, puis continue."
2. One session = one iteration (`AGENTS.md` section 9). The open iteration card holds the brief, the plan and the state of each card; if none is open, propose the next one to Abdou from the bolder next step of the last outcome.
3. Subagents do not survive a session: start one per card, with its role file, its allowed files and an absolute `CARGO_TARGET_DIR` under `target/agent-<role>`. At most two at a time, one on Fable.
4. A subagent's report is a claim: re-run the card's completion command before marking it done.

## State

- **Iteration 3 is open: T-139**, the missing PK bricks of the engine (three compartments, what individual fitting lacks, population PK with FOCEI, population simulation, the long-running command protocol). Abdou gave his go on 2026-11-07 ("oui lance") and left the running to the orchestrator. Q-020 points 3 to 5 (engine scope) are applied in `AGENTS.md` (8773e52); points 1, 2 and 6 (interface stack) are held until the stack is retested after population PK. Q-021 (Rtools) and Q-022 (pushing `iter3/*`) stand as decided: no installer, branches local. Brick 2: the orchestrator's pick is recorded in the T-139 thread. **Wave 1 is done:** T-140 (section 14 of `specs/models.md`, on `iter3/pk3`) and T-141 (`docs/long-running-commands.md`, merged into `main`). **Wave 2:** T-142 (`oracle`, worktree `C:\Users\abdou\caladrius-iter3\pk3`, branch `iter3/pk3`) and T-143 (`engine`, worktree `C:\Users\abdou\caladrius-iter3\progress`, branch `iter3/progress`; done after two Fable rounds, at fa306d4). **Wave 4, started early:** T-146 (`interface`, the same worktree and branch; done after two Sonnet rounds, at 611e0fe), because it needs only T-143. T-142's oracle is at 30e379c (`to review`: the Sonnet reviewer reads it); T-144 (`pk3.*` in `caladrius-models`, `engine`) is written and waits for that review and a free slot. **Wave 5 (brick 2: ML fit with an estimated error model, IWRES, population dataset):** T-147 (specs, done at 7304519) and T-148 (`oracle`, in progress), both on branch `iter3/ml`, worktree `C:\Users\abdou\caladrius-iter3\ml`; T-149 (engine, ML fit) and T-150 (interface, dataset loading) follow. The Pumas investigation is a private note, `..\notes\2026-11-07-pumas-investigation.md`; its three ideas for the population cards are in the T-139 thread. Use one `CARGO_TARGET_DIR` per worktree: `cargo xtask` finds its workspace from a path compiled into it. Subagents do not survive a session: if this one ended, look at the commits on those branches and at the card threads before starting a card again.
- **Iteration 2 is paused: T-130** (`blocked`, the board has no `paused` state) until population PK with an OFV exists. Waves 1 to 3 are done: T-131 (inventory), T-132 (three mockups, local files in `../notes/iteration-2-mockups/`), T-133 (the plot scene), T-134 (the engine bridge), T-135 to T-137 (the prototypes). Left: wave 4, T-138, the audit and the outcome.
- **Code of the iteration** is on branches, pushed under Q-019: `iter2/base` (T-133 and T-134 merged), `iter2/scene` and `iter2/bridge`. Their worktrees are `C:\Users\abdou\caladrius-iter2\scene` (now on `iter2/base`) and `C:\Users\abdou\caladrius-iter2\bridge`. Only `docs/` and `board/` reach `main` before 2026-11-18.
- **CI on the branches:** `iter2/base` adds a push trigger for `iter2/**` to `.github/workflows/ci.yml`. Remove it before the iteration is merged into `main`.
- **Wave 3 is done** without a visual direction (Abdou turned down the three mockups on 2026-11-07): T-135 (egui 0.37, `iter2/egui`), T-136 (React 19 + Tauri 2, `iter2/react`) and T-137 (Leptos 0.8 + Tauri 2, `iter2/leptos`), worktrees under `C:\Users\abdou\caladrius-iter2`, not pushed. Shared brief: `board/messages/2026-11-07-orchestrator-interface-wave3-brief.md`. Leptos is Abdou's favourite so far. Wave 4 (aligned measures, heavy scenario, T-138) waits until population PK with OFV is in the engine (Abdou, 2026-11-07).
- Iteration 1 (T-124, done 2026-10-29): READMEs that do not drift. The site redesign (T-123) comes after iteration 2.
- **First deadline:** the demo of 2026-11-17 stays on the current egui interface. If the quota runs short, demo prep comes before waves 3 to 5.

## This PC

- Programs under `target/` are refused file writes (os error 5). `cargo test --workspace --no-fail-fast` gives 25 failures for that reason alone, the same on `main` and on `iter2/base` (2026-11-07). The linker started by `cargo xtask` can hit LNK1104 on a cold target. Build first with plain `cargo`: for `cargo xtask bridge`, run `cargo test --no-run --manifest-path apps/caladrius-tauri/Cargo.toml` with `CARGO_TARGET_DIR=<target>/bridge/tauri-target`.
- `caladrius-mcp` was rebuilt from `main` into `target/agent-apothicaire` on 2026-11-07. The code is that of f8d1c1a, after T-111 and T-129; sha256 b72beca797749eb3f5abf78c0f5ec10d7188e2fbebfeeb0da25610695cd0e678. The frozen-set freeze hashes this binary.
- Frozen set, case freeze: it waits for `../agent/bench/frozen/demo/wordings.json`, written by Abdou (2 per turn kind) and dsh (1 per kind) from `wordings.template.json`; Claude never opens it. Then follow the "Commands" section of `../agent/bench/frozen/README.md`, with C1 = v1 and C2 = v2.

## Waiting for Abdou

- **Iteration 3:** Q-020 points 1, 2 and 6 (the interface lines of `AGENTS.md` and the README), after the stack is retested on population PK; Rtools 4.5 (Q-021), needed from wave 6; pushing `iter3/*` (Q-022).
- **Iteration 2 (paused):** a new visual direction (the three mockups were turned down on 2026-11-07), applied to the chosen stack later.
- Q-015, answered (delete): Abdou deletes `C:\Users\abdou\.claude\commands\onizuka.md` by hand.
- Ideas that wait for Abdou:
  - the Fisher-information design, which needs an amendment of `AGENTS.md` section 1;
  - `qc.nca`;
  - the plan-first assistant;
  - the PAGE 2027 abstract.
- Q-004 (public datasets) is still open.
- Todo cards from the red team (T-054 to T-093, `INDEX.md`): most touch `../agent`, `../optchat` or the umbrella.

## Places

- Publication: https://github.com/abdou-rgd/caladrius. Q-013 authorises pushes of this repository; push after each closed task.
- Apothicaire is `..\agent\` (github.com/abdou-rgd/apothicaire), OptChat `..\optchat\` (github.com/abdou-rgd/optchat). The umbrella is `..\` (`README.md`, `notes/`); its repository github.com/abdou-rgd/officina is checked out in `C:\Users\abdou\officina-umbrella`.
