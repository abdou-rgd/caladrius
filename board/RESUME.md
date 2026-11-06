# How to resume (state of 2026-11-07)

1. Open Claude Code in `C:\Users\abdou\apothicaire\caladrius`: the agent files of `.claude/agents/` load only when a session starts here. Say: "Reprends le rôle d'orchestrateur : lis AGENTS.md, board/RESUME.md, board/INDEX.md et la carte d'itération ouverte, puis continue."
2. One session = one iteration (`AGENTS.md` section 9). The open iteration card holds the brief, the plan and the state of each card; if none is open, propose the next one to Abdou from the bolder next step of the last outcome.
3. Subagents do not survive a session: start one per card, with its role file, its allowed files and an absolute `CARGO_TARGET_DIR` under `target/agent-<role>`. At most two at a time, one on Fable.
4. A subagent's report is a claim: re-run the card's completion command before marking it done.

## State

- **Iteration 2 is open: T-130**, the interface stack chosen on measures. Waves 1 and 2 are done: T-131 (inventory), T-132 (three mockups, local files in `../notes/iteration-2-mockups/`), T-133 (the plot scene) and T-134 (the engine bridge).
- **Code of the iteration** is on branches, pushed under Q-019: `iter2/base` (T-133 and T-134 merged), `iter2/scene` and `iter2/bridge`. Their worktrees are `C:\Users\abdou\caladrius-iter2\scene` (now on `iter2/base`) and `C:\Users\abdou\caladrius-iter2\bridge`. Only `docs/` and `board/` reach `main` before 2026-11-18.
- **CI on the branches:** `iter2/base` adds a push trigger for `iter2/**` to `.github/workflows/ci.yml`. Remove it before the iteration is merged into `main`.
- **Next:** wave 3 (T-135 egui 0.37 and T-136 Tauri + React 19, then T-137 Leptos) starts when Abdou has picked a mockup. Its brief adds what wave 2 found (T-130, thread of 2026-11-07).
- Iteration 1 (T-124, done 2026-10-29): READMEs that do not drift. The site redesign (T-123) comes after iteration 2.
- **First deadline:** the demo of 2026-11-17 stays on the current egui interface. If the quota runs short, demo prep comes before waves 3 to 5.

## This PC

- Programs under `target/` are refused file writes (os error 5). `cargo test --workspace --no-fail-fast` gives 25 failures for that reason alone, the same on `main` and on `iter2/base` (2026-11-07). The linker started by `cargo xtask` can hit LNK1104 on a cold target. Build first with plain `cargo`: for `cargo xtask bridge`, run `cargo test --no-run --manifest-path apps/caladrius-tauri/Cargo.toml` with `CARGO_TARGET_DIR=<target>/bridge/tauri-target`.
- `caladrius-mcp` was rebuilt from `main` into `target/agent-apothicaire` on 2026-11-07. The code is that of f8d1c1a, after T-111 and T-129; sha256 b72beca797749eb3f5abf78c0f5ec10d7188e2fbebfeeb0da25610695cd0e678. The frozen-set freeze hashes this binary.
- Frozen set, case freeze: it waits for `../agent/bench/frozen/demo/wordings.json`, written by Abdou (2 per turn kind) and dsh (1 per kind) from `wordings.template.json`; Claude never opens it. Then follow the "Commands" section of `../agent/bench/frozen/README.md`, with C1 = v1 and C2 = v2.

## Waiting for Abdou

- **Iteration 2:** pick a mockup direction (A Workbench, B Paper, C Instrument). Later: decide the stack and the `AGENTS.md` amendment (T-138).
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
