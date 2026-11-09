# @caladrius/plot

The engine's plot scene (`plot.scene`) drawn by Vega-Lite on a canvas.

- `sceneToVegaLite(scene, options)` (`src/spec.ts`): a pure function, scene to Vega-Lite spec. Axis ranges, tick values and tick labels are the engine's (`labelExpr` by exact value); nothing is rounded, formatted or aggregated here. The one arithmetic is the log10 scale mapping of an app-supplied overlay curve on a logarithmic axis.
- `PlotView` (`src/PlotView.tsx`): the React component. Colours come from the `--c-*` CSS variables of the page (`theme.json`), the legend, notes and the canvas `aria-label` come from the scene.
- Tests: `npm test -w @caladrius/plot` from `apps/` (`tsc --noEmit`, then `node --test` on the recorded scenes of `test/fixtures/`).
- Demo: `npx vite caladrius-plot/demo --host 127.0.0.1` from `apps/`; screenshots in `demo/screens/`.
