# Usage reference

How to build Caladrius and use its command line, its MCP server and its desktop application, in more detail than the README. Figures about validation live in `docs/validation.md` and `docs/conformance.md`.

## Build, usage, MCP and desktop

### Build

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask layers   # prints the layer table, fails if a crate depends on a higher layer
cargo xtask lint     # contract checks: forbidden names, private/ leaks, deny-lints headers, tolerances once, board hygiene, clippy::indexing_slicing denied in the four numerical crates (exceptions in xtask/lint_allow.toml)
cargo xtask review   # the figures this README and the specs state, confronted with the files that own them (read-only; --strict also fails on a note)
cargo xtask wasm     # checks that layers L0 to L3 compile for wasm32-unknown-unknown
cargo xtask web      # builds the browser page (apps/caladrius-web) into target/web-page, or into the folder given by --out DIR; needs the wasm-bindgen of the version in Cargo.lock
cargo xtask digest   # runs a fixed workload through the engine and prints one SHA-256 of the answers: the same on Linux, Windows, macOS and WebAssembly (--wasm runs it under Node, --dump FILE keeps the text, --diff A B compares two dumps)
```

`cargo xtask` is an alias for the `xtask` crate (see `.cargo/config.toml`). The layers and their rules are in `AGENTS.md`, section 4.

### Usage

```sh
# NCA of the Theoph dataset, one table (subject, parameter, value, reason) on standard output
cargo run -q -p caladrius-cli -- nca.run --csv oracle/data/theoph.csv --param route=extravascular --format csv

# the same as JSON, linear trapezoids, one subject
cargo run -q -p caladrius-cli -- nca.run --csv oracle/data/theoph.csv --param route=extravascular --param options.auc_method=linear --param subject=1

# a model fit; the project file keeps the work between calls
cargo run -q -p caladrius-cli -- fit.run --csv oracle/data/indometh.csv --project indo.caladrius.json --param subject=1 --param model=pk1.iv_bolus --param weighting=inv_y2 --format csv
cargo run -q -p caladrius-cli -- project.describe --project indo.caladrius.json

# run the history of the saved project again and check every step against its seal (exit code 1 when something differs)
cargo run -q -p caladrius-cli -- project.replay --project indo.caladrius.json
```

`--param KEY=VALUE` sets one parameter (JSON when the value parses, text otherwise; `a.b=1` nests; `csv=@file` reads a file), `--json FILE` (or `-` for standard input) gives them all as an object, `--csv FILE` imports a worksheet first and is used by commands that take a `worksheet`, `--project FILE` loads and saves the project (worksheets, analyses with their options and results, and the history of the commands), `--format csv` prints the main table of an analysis (`--table NAME` picks another). What an import guessed (roles, units) and unit warnings go to standard error. A failure prints `error: <code>: <message>` and exits with code 1. By default an answer carries the seal of its command, the SHA-256 of the command, its parameters and its answer (a `seal` member in JSON, a first line `# caladrius engine=... seal=...` above a CSV table, which `data.import` skips; `--no-seal` prints the answer as it is), and `project.replay --project FILE` runs the history of a saved project again and checks every step against its seal, with exit code 1 when something differs. `docs/reproducibility.md` says what a seal proves and what it does not.

`caladrius-cli compare A B --project FILE` (MCP tool `analysis_compare`) compares two analyses of one subject, NCA or fit: for each parameter they share, both values with units, the difference, the relative difference in percent and the ratio, computed by the engine (never convert or subtract by hand), or `not_comparable` with the reason (unit mismatch, missing value, a = 0); `--format csv` prints the table.

A saved project contains the data it was built from, twice: in its worksheets, and in the history, which keeps the parameters of each command up to 64 KiB (an import keeps its CSV text; from the second invocation on, a `project.load` entry can hold the previous project text while that is under 64 KiB). This is provenance, and it makes the file larger and de-identification harder: before sharing a project, remove the `history` array from the JSON file (the project loads without it; `history_restored` is then false), or start from `project.new` and import only what you want to keep. Each entry also holds the seal of its answer and the time it ran, and the history is what `project.replay` runs again, so a project without it cannot be verified.

### MCP

`caladrius-mcp` is a Model Context Protocol server on standard input and output (JSON-RPC 2.0, one message per line; no network). It exposes every command of the registry as a tool: `data_import`, `nca_run`, `fit_run`, `model_simulate`, `export_table`... (the command id with `.` written `_`; the original id is accepted too). Each tool's `inputSchema` and `outputSchema` are the JSON schemas of the command, a result is JSON text content plus `structuredContent`, and a failing command is a tool error (`isError: true`) whose text is `<code>: <message>`. The seal of the command and its time are in the `_meta` of the result (`caladrius/seal`, `caladrius/at`), never in the text or in `structuredContent`: what an assistant reads, and what a number gate checks against, holds no hash. The server keeps one project for the session: worksheets and analyses persist between calls, `project_save` returns the whole project (the seals and times of its history are in `_meta` under `caladrius/result`), `project_load` restores it and `project_replay` runs a saved history again, or the work of the session when it is given none, and checks every step against its seal.

Limits: the server handles one request at a time, so a long fit blocks the loop until it ends (no progress notifications, and a `ping` sent meanwhile is answered afterwards); a line longer than 64 MiB is answered with an error and skipped, a batch is limited to 100 requests, a message without an `id` is a notification and is never answered, and a byte order mark on the first line is ignored.

Build the binary once, then register it in the agent host.

```sh
cargo build --release -p caladrius-mcp      # target/release/caladrius-mcp
```

Claude Code, in `.mcp.json` at the root of a project (or `claude mcp add caladrius -- /path/to/caladrius-mcp`):

```json
{
  "mcpServers": {
    "caladrius": {
      "command": "/path/to/caladrius/target/release/caladrius-mcp",
      "args": []
    }
  }
}
```

Any other host that starts MCP servers over stdio (Pi Durable, dsh, an Apothicaire agent): run `/path/to/caladrius-mcp` with no arguments as a child process and speak the protocol on its standard input and output; a quick check by hand:

```sh
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"hand","version":"0"}}}' '{"jsonrpc":"2.0","method":"notifications/initialized"}' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | caladrius-mcp
```

### Desktop

`caladrius` is the desktop application (egui on wgpu): a project tree, the worksheet, and three analysis pages, each read from top to bottom with its plot always visible beside it: NCA (data, route and dose, options, results), model fit (data, model and dose, starting values, weighting and options, results) and model simulation. A plot is a project object of its own, with its own page (see Plots below), and the same interface runs in a browser page (see In the browser below). Every action goes through the same engine commands as the CLI and the MCP server; the interface holds no numerical code and reads no file itself.

```sh
cargo run -p caladrius                       # open the window
cargo run -p caladrius -- oracle/data/theoph.csv   # open it with a file to import
cargo run -p caladrius -- study.caladrius.json     # open a saved project
cargo run -p caladrius -- --help             # usage, without a window
```

- Open a CSV with the button or drop it on the window. A preview shows the table as it will be read (separator and decimal mark detected, with the other readings listed and the checks on each: a decimal comma read as a separator, a time that goes back or repeats) before anything is imported; you can change the separator and the decimal mark there.
- **Type or paste data.** Without a file, `Type or paste data` opens a table of time, concentration, dose and an optional subject column, with units in the headers; Ctrl+V of spreadsheet cells fills it (header line skipped, decimal comma accepted). With `Several subjects`, times go in rows and subjects in columns, the times typed once for all; a wide table pasted from a spreadsheet is read the same way. A menu next to each unit cell lists the units the engine recognises (`units.check`). The table goes through the same `data.preview` and `data.import` commands as a CSV file.
- The worksheet is editable: click a cell, type a number with a point or a comma, press Enter. Columns take a role (time, concentration, subject, dose) and a unit.
- `New analysis` opens the NCA page and runs it at once; any change of route, dose or option runs it again. The plot has a linear and a semi-log view (a zero or negative concentration is left out of the log view only, and a note says how many); click a point to add it to the terminal phase or take it out, or click a candidate row; `Back to automatic` undoes the choice. The summary comes first, with a marker and a sentence for each quality flag, then all parameters, the candidate fits and the profile on demand.
- `New analysis` is one menu: NCA, model fit, model simulation, XY plot of the data. `Model fit` is picked by route (IV bolus, IV infusion, oral first-order or zero-order, with or without a lag time) and by one or two compartments, and shows the compartment diagram and the equation of the model it picked; for two compartments it also offers the parameter set (clearances and volumes, micro-constants, or macro-constants) and the palette has `New two-compartment fit` and `New two-compartment simulation`. The starting values are generated from the data (`fit.initial_estimates`) and can be edited (for two compartments the engine cannot generate them: the page says so in one sentence and asks for every value, shows placeholders muted until you set them, and the `Fit` button waits): the curve on the plot (`model.simulate`) and the weighted sum of squares (`fit.evaluate`) follow every change. Weighting and the options of the fit (iterations, convergence, derivatives, confidence level, flag thresholds) are on the same page. `Fit the model` runs `fit.run`; the summary comes first: convergence, estimates with standard error, CV% and both confidence intervals (univariate and planar), goodness of fit, and a sentence for each flag (high CV%, an interval that contains zero, strongly correlated estimates, non-convergence) saying what to check. The plot shows the data and the fitted curve (linear or semi-log), with the residual plots (against time, against predicted, weighted) under it. The variance-covariance and correlation matrices, eigenvalues and condition numbers, the minimization trace, the partial derivatives and the predicted values are one click away. Every curve is computed by the engine, never by the interface.
- `Model simulation` needs no data: pick a model, give a dose, the parameters and a time range, and the engine's curve and secondary parameters (half-life, clearance, AUC to infinity, predicted peak) update as you type. Opened with a worksheet selected, it takes that worksheet's units (otherwise it says there is no worksheet and the values carry no unit); `Save as analysis` stores it in the project and saving again updates that analysis instead of adding another. Durations are fields with their unit written in them.
- After an edit of the data, the analysis is marked out of date in the tree (amber name, a sentence on hover, a `Run again` button, a count under the project name), on the page and on the plot until it is run again. Analyses are named from their content (`NCA of oral, subject 1`), not by a running number.
- **Projects.** The `File` menu has New project, Open project, Open CSV, Save (Ctrl+S), Save as (Ctrl+Shift+S), Verify a project and Quit; they go through the commands `project.new`, `project.load`, `project.save` and `project.replay`. The window title is the project name with a `*` while there are unsaved changes (the project differs from the file last saved or opened). Starting a new project, opening another, dropping a project file on the window or closing the window with unsaved changes asks first: save, don't save, or cancel. A file that cannot be read says why and changes nothing. Files are written whole (a neighbouring temporary file replaces the target), so a failed save does not damage the previous one. `Verify a project` runs the history of a saved file again from an empty project and lists every step with its stored and its recomputed seal; the project in the window is not read or changed, and the file is not opened (`docs/reproducibility.md`).
- **Command palette.** Ctrl+K (or the `Commands` button) opens a searchable list: the actions of the interface (new NCA or model fit from the selected worksheet, new simulation, theme, settings), every analysis and worksheet by name, and every command of the engine registry with its id, title and one-line description. Enter runs the highlighted entry with the current selection as context, or opens the page that asks for its parameters. From a worksheet, Ctrl+K, `nca`, Enter creates an NCA.
- **Settings.** `Commands`, then `settings` (or the palette entry `Open settings`) opens one searchable page: interface language (a placeholder, English only), theme (light, dark, or follow the system), decimal mark (point or comma) and significant digits, and the defaults a new NCA (AUC method, terminal-phase options) and a new fit (weighting, iterations, convergence) start from. Each row says what it affects and shows its default, with a way back to it. They are stored in `settings.json` in the application's configuration folder (`%APPDATA%\Caladrius` on Windows, `~/Library/Application Support/Caladrius` on a Mac, `$XDG_CONFIG_HOME/caladrius` or `~/.config/caladrius` elsewhere). The operating-system locale is read at start, shown on the page, and used only as the first suggestion for the decimal mark on a first start (the page says so); changing the decimal mark never changes stored data or what a CSV import understands.
- **Plots.** The worksheet screen plots the concentrations as entered against time, linear and semi-log side by side, before any analysis (one line, colour and marker per subject; points below the limit of quantification as hollow triangles). `Keep as a plot`, `New plot`, or `Keep a plot of this fit` under a fit result make a plot a project object, like an NCA or a fit: its source (a worksheet, or a fit already run), the subjects, the scale (linear, semi-log or both), the layout (overlaid, or one panel per subject) and a title. It is saved with the project, redrawn by `Run again` and marked out of date when its source changes. From a fit, a plot shows the fitted curve over the observations, observed against predicted, the residuals or the weighted residuals. `Export SVG` saves the drawing (light colours); the commands `plot.xy` (create) and `export.plot` (export) are also on the CLI and the MCP server. A plot only draws values already in the worksheet or in a stored fit result; it computes nothing.
- **In the browser.** `apps/caladrius-web` is the same interface compiled to WebAssembly and drawn on a WebGL canvas, with the same engine and commands, at <https://officina-dmpk.com/app/>. Files are read inside the page and saved as downloads, settings stay in the browser's local storage and nothing is sent anywhere. `cargo xtask web` builds it, and each release attaches it as `caladrius-web-<tag>.tar.gz`, a folder any static server can serve. The oracle suite itself is not run inside WebAssembly; the digest of `cargo xtask digest` is compared under Node (`docs/reproducibility.md`).
- Colours, radii, spacing and font sizes are the tokens of `crates/caladrius-ui/src/theme.json` (light and dark). `cargo run -p caladrius-ui --example snapshot` renders each screen to `target/snapshots/*.png` without a window.

## The application

<table>
<tr>
<td width="33%"><img src="screens/import-french-csv.png" alt="Import preview of a French CSV"><br><sub><b>Import.</b> A French export (semicolons, decimal commas) read before anything is stored: roles, units and the reading of each value are shown, and the other possible reading is named.</sub></td>
<td width="33%"><img src="screens/nca-linear.png" alt="NCA page with the linear plot"><br><sub><b>NCA.</b> Parameters, quality flags and the terminal-phase candidates on one page; the range is chosen automatically and every point can be added to or taken out of the regression.</sub></td>
<td width="33%"><img src="screens/fit-two-compartments-results.png" alt="Two-compartment fit results"><br><sub><b>Fit.</b> Two compartments, three parameter sets, the fitted curve and the residuals; estimates with standard error, CV% and both intervals, one sentence per flag.</sub></td>
</tr>
</table>

[More screenshots of the application](screens/) — import, NCA, fitting, simulation and the command
palette. Every scene is drawn offscreen by `crates/caladrius-ui/examples/snapshot.rs`
(`cargo run -p caladrius-ui --example snapshot`), through the application's own actions and engine
commands; the interface holds no numerical code and reads no file itself. In "More options" of the fit page, a `Preset` choice (the default settings or `reference_conventions`) fills the iteration settings with the values the engine holds; the fields stay editable and an edit reads "Custom".
