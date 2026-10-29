# Caladrius

Open-source pharmacokinetic analysis in Rust: non-compartmental analysis, individual compartmental models, weighted least-squares fitting and plots, with a native desktop UI (egui) and the same interface in a browser page (WebAssembly). Every analysis is a command with a stable id, so the same calculations are available from the UI, the command line and an MCP server for agents. Caladrius is the calculation layer of Apothicaire, a local DMPK assistant. In one sentence, the project is an open, deterministic and verified pharmacokinetic engine, with a local assistant that never computes a number itself: every number is re-derivable from a pinned version, and the validation record is published with the code. What the project promises about that, and how anyone can check a result or a saved project, is in `docs/reproducibility.md`.

## In plain words

**What this is.** Caladrius is a small program that does the standard calculations of pharmacokinetics: how fast a drug is absorbed, how far it spreads in the body, how fast it leaves. Pharmacists and pharmaceutical scientists do these calculations every day with commercial software. Caladrius does a subset of them, in the open, with every number checked against an independent calculation.

**What this is not.** It is a prototype and a learning project. It is not a replacement for the commercial software used in industry, and it does not claim to be: the section "What is verified, and what is not" and the validation report it links to say exactly which calculations are checked, against what, and which are not. Population modelling, bioequivalence and regulatory submissions are out of scope.

**No AI inside.** Caladrius itself contains no AI: it is a deterministic engine, and what a command returns depends on its parameters and the project alone. It is the calculation layer of Apothicaire, a local DMPK assistant developed in its own repository. The assistant's language model decides which command to run and explains the result, but never computes a number itself, and a deterministic check, not learned, flags any number that does not come from a tool result or from the user. The earlier account of how the two fit together, with the benchmark figures it carried, is kept in `docs/history.md`.

## What it does

- **Non-compartmental analysis**: AUC by the linear, lin-up/log-down and mixed rules, the terminal-phase rate constant, half-life, extrapolation to infinity, AUMC, MRT, clearance and volumes, with units carried as data.
- **Individual compartmental models**: one and two compartments, IV bolus, IV infusion, first-order and zero-order absorption with or without a lag time, with multiple dosing and steady state; closed-form curves, secondary parameters and partial derivatives.
- **Weighted least-squares fitting**: the weightings `uniform`, `1/y`, `1/y²`, `1/ŷ` and `1/ŷ²`; estimates with standard error, CV% and confidence intervals; diagnostics, correlation and covariance matrices, and the minimization trace.
- **Data, plots and projects**: CSV import with a preview, typed or pasted data, XY plots, export of result tables, and project files whose history can be replayed against the seal of each result.
- **The same engine everywhere**: every analysis is a command with a stable id, so the desktop application, the command line, the MCP server and the browser page run the same calculations and return the same numbers on Linux, Windows, macOS and in the browser (CI compares one digest of a fixed workload, `docs/reproducibility.md`).

## Build

Requires Rust 1.85 or newer, installed with rustup (`rust-toolchain.toml` pins 1.99.1, the compiler the guarantees of `docs/reproducibility.md` are checked with; rustup installs it by itself). The WebAssembly check also needs the `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`. Prebuilt binaries for Linux and Windows are attached to each release: <https://github.com/abdou-rgd/caladrius/releases>.

```sh
cargo build --workspace
cargo test --workspace
cargo xtask web      # builds the browser page (apps/caladrius-web) into target/web-page, or into the folder given by --out DIR; needs the wasm-bindgen of the version in Cargo.lock
```

## Usage

The command-line application runs the same commands as the UI and the MCP server (`caladrius-cli commands` lists them with their titles; `caladrius-cli commands --format json` adds the JSON schema of the parameters and results of each).

```sh
# NCA of the Theoph dataset, one table (subject, parameter, value, reason) on standard output
cargo run -q -p caladrius-cli -- nca.run --csv oracle/data/theoph.csv --param route=extravascular --format csv
# a model fit; the project file keeps the work between calls
cargo run -q -p caladrius-cli -- fit.run --csv oracle/data/indometh.csv --project indo.caladrius.json --param subject=1 --param model=pk1.iv_bolus --param weighting=inv_y2 --format csv
# run the history of the saved project again and check every step against its seal (exit code 1 when something differs)
cargo run -q -p caladrius-cli -- project.replay --project indo.caladrius.json
```

`--param KEY=VALUE` sets a parameter, `--csv FILE` imports a worksheet, `--project FILE` loads and saves the project and `--format csv` prints the main table; a failure prints `error: <code>: <message>` and exits with code 1. By default an answer carries the seal of its command (`docs/reproducibility.md` says what a seal proves and what it does not). A saved project holds the data it was built from twice, in its worksheets and in its history: remove the `history` array from the JSON file before sharing one (it still loads, but can no longer be verified; `docs/usage.md` has the details).

## MCP

`caladrius-mcp` is a Model Context Protocol server on standard input and output (JSON-RPC 2.0, one message per line; no network). It exposes every command of the registry as a tool: `data_import`, `nca_run`, `fit_run`, `model_simulate`, `export_table`... The seal of each result is in the `_meta` of the answer, never in the text an assistant reads.

Build the binary once with `cargo build --release -p caladrius-mcp` (it lands in `target/release/caladrius-mcp`), then register it in the agent host. In Claude Code, run `claude mcp add caladrius -- /path/to/caladrius-mcp`, or add this to `.mcp.json` at the root of a project; any other host that starts MCP servers over stdio runs the binary with no arguments as a child process.

```json
{ "mcpServers": { "caladrius": { "command": "/path/to/caladrius/target/release/caladrius-mcp", "args": [] } } }
```

## Desktop

`caladrius` is the desktop application (egui on wgpu): a project tree, the worksheet, and analysis pages for NCA, model fit and model simulation, each read from top to bottom with its plot always visible beside it, plus a command palette, settings and project files. Every action goes through the same engine commands as the CLI and the MCP server; the interface holds no numerical code and reads no file itself. Screenshots are in `docs/screens/`.

```sh
cargo run -p caladrius                       # open the window
cargo run -p caladrius -- oracle/data/theoph.csv   # open it with a file to import
cargo run -p caladrius -- study.caladrius.json     # open a saved project
```

**In the browser.** The same interface runs as a browser page, compiled to WebAssembly, with the same engine and commands, at <https://officina-dmpk.com/app/>. Files are read inside the page and saved as downloads, and nothing is sent anywhere. `cargo xtask web` builds it, and each release attaches it as `caladrius-web-<tag>.tar.gz`.

## What is verified, and what is not

"Validated" here has a narrow meaning: a number produced by Caladrius equals a number produced by an independent computation, within a stated tolerance, on a stated case. It does not mean that Caladrius gives the same answer as any commercial program, and the table says where the independent computation comes from. Everything is reproducible: the R scripts that write the expected values are in `oracle/scripts/` (versions recorded in each `*.options.json`), `cargo xtask conformance` regenerates `docs/conformance.md` (the per-case counts; never edited by hand), and the tests are in `crates/*/tests/`. The full validation report — versions, methods, tolerances, coverage, documented differences, open items and how to reproduce — is `docs/validation.md`.

**Where the numbers come from.** None of the values in the conformance table comes from the reference commercial software: every expected value is produced by PKNCA, by 256-bit evaluations of closed forms cross-checked with expm and deSolve, or by R nls and nlsLM, from public or synthetic data, with a versioned script. The reference software was compared once, privately, on counts only.

## Where to read more

- [`docs/validation.md`](docs/validation.md): what is verified, against what, at which tolerance, and what is not, with the tables of figures.
- [`docs/conformance.md`](docs/conformance.md): the per-case record, generated by `cargo xtask conformance`, never edited by hand.
- [`docs/reproducibility.md`](docs/reproducibility.md): what a result and a saved project can prove, and how to check them.
- [`docs/usage.md`](docs/usage.md): the full reference of the build, the command line, the MCP server and the desktop application, with screenshots.
- [`docs/history.md`](docs/history.md): how the project was built, what went wrong, and the status on each date.
- [`AGENTS.md`](AGENTS.md) is the contract the code is written under, [`ATTRIBUTION.md`](ATTRIBUTION.md) lists the sources and licenses, and [`CITATION.cff`](CITATION.cff) is how to cite the project (GitHub shows a "Cite this repository" button from it).

## Trademark notice

Caladrius is an independent project and is not affiliated with, endorsed by or derived from Certara. Phoenix and WinNonlin are trademarks of Certara. Caladrius aims at compatible conventions for standard pharmacokinetic calculations, reimplemented from public textbooks and documentation.

## License

MIT OR Apache-2.0. Third-party data and assets are listed in `ATTRIBUTION.md`.
