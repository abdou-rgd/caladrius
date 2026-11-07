# NCA screen in React 19 (T-136)

The NCA screen of the wave 3 brief, as a web front: React 19, TypeScript, Vite, panels in `dockview`.
The plot is SVG in the DOM, drawn from `plot.scene` only. The engine is reached through the
TypeScript client of `apps/caladrius-bridge/ts/`. One build, two ways in:

- in a browser: `WorkerTransport` (the engine is WebAssembly in a Web Worker);
- in the Tauri 2 shell `apps/caladrius-tauri`: `TauriTransport` (the engine is in the Rust process).

Numbers are formatted by `caladrius-format` as WebAssembly in the page (the client's `createFormatter`),
the same code the egui page runs. Colours come from `crates/caladrius-ui/src/theme.json`, turned into CSS
variables at build time (`build/theme-css.mjs`). No chart library, CSS framework, web font or icon set.

Needs: Node 22, Rust with the `wasm32-unknown-unknown` target, `wasm-bindgen` (same version as the
lock file), and for the window the WebView2 runtime (Windows).

## One command: build and open the window

```
cd apps/caladrius-proto-react
npm ci
npm run desktop
```

`npm run desktop` builds the engine module (`npm run wasm`), builds the page (`npm run build`), then
`cargo run --release` of the Tauri shell, which embeds `apps/caladrius-proto-react/dist`.

## One command: serve the browser build

```
cd apps/caladrius-proto-react
npm ci
npm run serve
```

Opens nothing by itself: go to <http://127.0.0.1:4173/>. Query parameters: `?theme=light|dark`,
`?dataset=nca50`, `?stale=1` (edits one cell after the first run, for the screenshots), `?measure=1`.

## Tests and build

```
npm ci && npm test      # tsc, then node:test (16 tests)
npm run build           # tsc --noEmit, then vite build
cargo test --manifest-path apps/caladrius-tauri/Cargo.toml
```

`cargo test` of the shell needs the built page: `generate_context!()` embeds `frontendDist`
(`apps/caladrius-proto-react/dist`, which git ignores), so run `npm run wasm && npm run build` first.

## What the screen does

- Top bar: dataset, analysis label, subject, Run, fresh or stale badge.
- Data table (left): time, concentration and role chips (from the scene roles). A cell is edited in place
  (Enter or leaving the cell commits, Escape undoes); the analysis becomes stale.
- Plot (centre): semi-log or linear, the note about points a log axis cannot show, the λz fit.
  The λz range is set by two handles (drag, or Left/Right keys when a handle is focused), a click on a
  point, or a row of the candidate list. Each sends `nca.run` with the parameters of the egui page.
  "Back to automatic" clears the hand-picked range.
- Results (below): terminal-phase sentence, flags, headline parameters with units, "All parameters".
  Copy gives tab-separated text with a header row. A seal line shows the short seal, the engine version
  and the command id. An engine error is shown as its own sentence, in place.
- Keyboard: Tab reaches every control; rows of the data table take Up/Down and Space/Enter (point in or
  out of the terminal phase); handles take the arrow keys; candidate rows take Enter/Space.

## Measure mode

The contract of the brief (100 drags over `prototypes/nca50/nca50.csv`, one result cell check).

- Browser: open `?measure=1`; the console gets one line starting `CALADRIUS_MEASURE ` followed by the JSON,
  also kept in `window.__CALADRIUS_MEASURE__`.
- Tauri: `CALADRIUS_MEASURE=<path>` makes the shell write the JSON to that file (command `measure_write`),
  then exit three seconds later. Windows keeps the executable under `target/` from writing anywhere, so
  copy it out before running it:

```
copy target\release\caladrius-tauri.exe C:\somewhere\
set CALADRIUS_MEASURE=C:\somewhere\measure.json
C:\somewhere\caladrius-tauri.exe
```

`first_frame_ms` in Tauri counts from the start of the process (the shell tells the page how long it
has run); in a browser it counts from the start of the page. `input_to_paint_ms` runs from the handler of
the start handle (`setRangeStart`) to the first frame that shows the new result.
`prototypes/react/measure-local.json` is from the Tauri run on this machine.

## Files

- `screens/`: four PNGs at 1440x900 (light and dark, fresh and stale), headless Edge on the built page.
- `measure-local.json`: the Tauri measure.
