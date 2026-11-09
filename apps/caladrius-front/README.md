# @caladrius/front

The web front of Caladrius: React 19, dockview panels, the engine in a Web Worker (browser) or in the Tauri 2
process (desktop), the same code in both. Everything the page shows as a number comes from the engine
(`caladrius-format` through the bridge's `Formatter`); ticks and their labels come from `plot.scene`.

```sh
cd apps
npm run dev -w @caladrius/front        # http://127.0.0.1:5173 (builds public/pkg/ first when it is missing)
npm test -w @caladrius/front           # tsc --noEmit, then node tests of the pure modules and of the session
npm run build -w @caladrius/front      # type check and production build (dist/)
npm run test:e2e -w @caladrius/front   # Chromium (Playwright from the npx cache) against `vite preview` of dist/
npm run wasm -w @caladrius/front       # rebuilds the engine module and copies the sample CSVs
```

`CARGO_TARGET_DIR` is honoured by `npm run wasm`. The end-to-end check compares the page's numbers with
`caladrius-cli` (default `target/agent-interface/debug/caladrius-cli`, or `CLI=`), every digit.

## Layout of `src/`

| Path | What |
|---|---|
| `session/` | the store (`useSyncExternalStore`, plain data), `Session` (every action; `run(command, params)` is the one engine call and logs the answer), pure request builders, the debounce and latest-wins tools |
| `model/` | `parameterSets.ts` (routes, parameter sets, labels, units: UI knowledge in one file), `registry.ts` (commands and enums read from `describe()`) |
| `results/fitTable.ts` | the `fit.run` answer as `TableSpec`s: raw numbers, flags from the engine's flags only |
| `panels/` | Data, Model, Plot, Results, History |
| `shell/` | top bar, command palette (cmdk, Ctrl/Cmd+K via tinykeys), dockview layout saved in `localStorage`, error boundary per panel |
| `engine.ts`, `engine.worker.ts` | Worker transport in a browser, Tauri transport when `__TAURI_INTERNALS__` is present |

Dependencies and licences: see `package.json` (exact versions) and `ATTRIBUTION.md`.
