# Caladrius NCA screen in Leptos 0.8 (T-137)

The same screen as the React prototype (`prototypes/react`), written in Rust: Leptos 0.8, client side rendered, compiled to
WebAssembly. The engine is `caladrius_bridge::Bridge`, linked into the same module and called in process: a click calls a
Rust function that calls the engine and gets the answer back before the next line runs. No worker, no message, no JSON
through JavaScript. The plot is SVG drawn from `plot.scene`; every number goes through `caladrius-format`; the colours are
CSS variables made from `crates/caladrius-ui/src/theme.json` (embedded at build time). No CSS framework, chart crate,
web font or icon set.

Code: `apps/caladrius-proto-leptos/` (not a workspace member, as `apps/caladrius-tauri`: its own `Cargo.lock`, so the
workspace clippy, `cargo xtask` and CI do not compile Leptos).

## Prerequisites

Rust with the `wasm32-unknown-unknown` target, and `trunk` (`cargo install trunk`, MIT OR Apache-2.0). Trunk downloads the
`wasm-bindgen` CLI that matches the lock file. `wasm-opt` is not needed (the release profile is `opt-level = "s"` with LTO).

## One command: the window

```
node apps/caladrius-proto-leptos/scripts/desktop.mjs
```

It runs `trunk build --release`, then builds and opens the Tauri shell (`apps/caladrius-tauri`) with the Leptos build as its
front.

**The switch.** The shell embeds the folder named by `build.frontendDist` of `tauri.conf.json`: the React build
(`../caladrius-proto-react/dist`). The environment variable `TAURI_CONFIG` (JSON that Tauri merges over `tauri.conf.json`)
changes it; the script sets it to `{"build":{"frontendDist":"../caladrius-proto-leptos/dist"}}`. Without the variable the
shell is the React one, exactly as before (`npm run desktop` in `apps/caladrius-proto-react`). The script builds the shell
into `target/shell-leptos` so the two shells never overwrite each other (or into `CARGO_TARGET_DIR` if it is set).

## One command: the browser

```
trunk serve --config apps/caladrius-proto-leptos/Trunk.toml --open
```

Serves on `http://127.0.0.1:5174/`. Query parameters: `?theme=light|dark`, `?dataset=edge_blq|nca50` (both are embedded in the
wasm; the React build fetches them), `?stale=1` (edits one cell after the first run, for the screenshots), `?measure=1`.

## Keys

- `r` run, `n` / `p` next and previous subject, `l` toggle semi-log and linear, `b` back to automatic (ignored while typing in
  an input, a select or a text area).
- Data table: Up and Down move between rows, Space or Enter take the point in or out of the terminal phase.
- The two handles of the plot (Tab to reach them): Left and Down, Right and Up move the end by one sample.
- Candidate rows: Enter or Space pick the range.
- A cell: Enter or leaving it commits the edit, Escape undoes it.

## Measure mode

Same contract as the other prototypes (`board/messages/2026-11-07-orchestrator-interface-wave3-brief.md`), same definitions as
T-136: load `nca50.csv`, 100 drags (drag k: subject `1 + k % 50`, start handle on the n-th sample from the end with n from
`[3,4,5,6]`), `input_to_paint_ms` from the handler (`set_range_start`) to the first frame after the render (a
`requestAnimationFrame` callback, then a message), `frame_ms` the interval between that frame and the one before, then every
`[data-cell]` is compared with the answer formatted by `caladrius-format`, wait 3 s, exit.

- Browser: open `?measure=1`; the console gets one line `CALADRIUS_MEASURE <json>` (also in `window.__CALADRIUS_MEASURE__`).
  Keep the tab in front: a hidden tab throttles `requestAnimationFrame`.
- Tauri: set `CALADRIUS_MEASURE=<path>`; the shell writes the JSON there and exits after 3 s. The shell must not run from
  `target/` on a machine where programs there cannot write files (copy the exe out first).

`measure-local.json` is the Tauri run on this machine. `screens/` holds the four PNGs at 1440x900 (light and dark, fresh and
stale), taken by `node prototypes/leptos/screenshots.mjs` (headless Edge driven through the DevTools protocol, which waits for
the badge; run `trunk build --release` first).

## Tests

```
cargo test   --manifest-path apps/caladrius-proto-leptos/Cargo.toml
cargo clippy --manifest-path apps/caladrius-proto-leptos/Cargo.toml --all-targets -- -D warnings
cargo fmt    --manifest-path apps/caladrius-proto-leptos/Cargo.toml --check
```

Plain Rust tests of the logic (the session is DOM-free and generic over the engine), against the real `Bridge` and a
recording wrapper: the semi-log map with hidden points, a drag calling `nca.run` with the expected parameters, an edit that
makes the analysis stale, every result cell against `caladrius-format`, and a scan that bans number formatting outside it.
No `wasm-bindgen-test`: the DOM code is thin and was checked in the browser and in the shell.
