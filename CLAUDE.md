# Caladrius: notes for Claude Code

Read `AGENTS.md` in full before any code change. It is this repository's contract (scope, golden rules,
tolerances, layers, board) and wins over any task that conflicts with it.

- **One agent.** In a cloud session you are the only agent: the roles of `AGENTS.md` section 8 and
  `.claude/agents/*.md` say how work is split, they are not subagents to start. Keep the separations
  that matter: whoever writes an oracle test does not write the code under test, and a change in
  `caladrius-nca`, `caladrius-models` or `caladrius-fit` gets an independent review before it is done.
- **Board.** `board/RESUME.md`, `board/INDEX.md`, `board/tasks/T-xxx.md`, `board/LOG.md`,
  `board/QUESTIONS.md`. One card per task; the commit message starts with the task id. Q-013 covers
  pushes of this repository (`AGENTS.md` section 11).
- **Paths.** The board and `RESUME.md` use the PC layout: `..\agent` is the apothicaire repository,
  `..\optchat` is optchat, `..\` is the officina umbrella. In a cloud session the repositories sit
  side by side under their own names.
- **`private/`** is git-ignored and empty outside the PC; tests behind the `private-oracle` feature
  skip. Nothing from it (number, file name, subject) goes into a versioned file or a commit message.
- **Toolchain** pinned by `rust-toolchain.toml` (1.99.1, clippy, rustfmt, wasm32); `rustup toolchain
  install` installs it.

Before finishing, run what CI runs (`.github/workflows/ci.yml`) for what you touched:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # or -p <crate>
cargo xtask layers && cargo xtask lint && cargo xtask review && cargo xtask wasm
cargo xtask conformance && git diff --exit-code docs/conformance.md   # generated, never edited by hand
```

The binary Apothicaire starts: `cargo build --release -p caladrius-mcp --target-dir target/agent-apothicaire`.
