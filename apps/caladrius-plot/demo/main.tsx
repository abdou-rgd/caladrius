import { StrictMode, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import theme from '../../../crates/caladrius-ui/src/theme.json';
import dataOverlayLinear from '../test/fixtures/data_overlay_linear.json';
import dataPerSubjectLog from '../test/fixtures/data_per_subject_log.json';
import fitObsPred from '../test/fixtures/fit_obs_pred.json';
import fitProfileBoth from '../test/fixtures/fit_profile_both.json';
import fitWres from '../test/fixtures/fit_wres.json';
import { PlotView } from '../src/index.ts';
import type { Overlay, PlotScene } from '../src/index.ts';

type Mode = 'light' | 'dark';

// The app provides these variables from its own build of theme.json: --c-<key with _ as ->,
// arrays as --c-<key>-<i>. This is the demo's inline loader.
function applyTheme(mode: Mode): void {
  const root = document.documentElement;
  root.setAttribute('data-theme', mode);
  const colors = theme[mode].colors as Record<string, string | string[]>;
  for (const [key, value] of Object.entries(colors)) {
    const name = key.replaceAll('_', '-');
    if (Array.isArray(value)) value.forEach((v, i) => root.style.setProperty(`--c-${name}-${i}`, v));
    else root.style.setProperty(`--c-${name}`, value);
  }
  document.body.style.background = colors['background'] as string;
  document.body.style.color = colors['text'] as string;
  document.body.style.font = '14px system-ui, sans-serif';
  document.body.style.margin = '0';
}

const initial: Mode = new URLSearchParams(location.search).get('theme') === 'dark' ? 'dark' : 'light';
applyTheme(initial);

// A stand-in for the app's `model.simulate` answer: the demo only needs a curve in data units.
function previewCurve(scale: number): Overlay {
  const points: { x: number; y: number }[] = [];
  for (let k = 0; k <= 60; k += 1) {
    const x = k * 1.6;
    points.push({ x, y: scale * (0.6 * Math.exp(-x / 6) + 0.4 * Math.exp(-x / 60)) });
  }
  points.push({ x: 100, y: 0 }, { x: 101, y: -1 }); // a non-positive end, dropped on a log axis
  return { name: 'initial estimates', points };
}

interface Entry {
  id: string;
  scene: PlotScene;
  columns: 1 | 2;
  overlay?: Overlay[];
}

const fixtures: Entry[] = [
  { id: 'fit_profile_both', scene: fitProfileBoth.scene as unknown as PlotScene, columns: 2, overlay: [previewCurve(8)] },
  { id: 'fit_wres', scene: fitWres.scene as unknown as PlotScene, columns: 1 },
  { id: 'fit_obs_pred', scene: fitObsPred.scene as unknown as PlotScene, columns: 2 },
  { id: 'data_per_subject_log', scene: dataPerSubjectLog.scene as unknown as PlotScene, columns: 2 },
  { id: 'data_overlay_linear', scene: dataOverlayLinear.scene as unknown as PlotScene, columns: 1, overlay: [previewCurve(8)] },
];

function Demo() {
  const [mode, setMode] = useState<Mode>(initial);
  const [clicked, setClicked] = useState('(click a point)');
  const tooltip = useMemo(
    () => (series: string, p: { x: number; y: number }, panel: number) => `${series} (panel ${panel})\nx = ${p.x}\ny = ${p.y}`,
    [],
  );
  return (
    <main style={{ maxWidth: 1100, margin: '0 auto', padding: 16 }}>
      <header style={{ display: 'flex', gap: 16, alignItems: 'baseline' }}>
        <h1 style={{ fontSize: 18, margin: 0 }}>@caladrius/plot</h1>
        <button
          onClick={() => {
            const next: Mode = mode === 'light' ? 'dark' : 'light';
            applyTheme(next);
            setMode(next);
          }}
        >
          theme: {mode}
        </button>
        <span style={{ color: 'var(--c-text-muted)' }}>{clicked}</span>
      </header>
      {fixtures.map((f) => (
        <section key={f.id} style={{ margin: '20px 0', padding: 12, background: 'var(--c-panel)', border: '1px solid var(--c-border)', borderRadius: 8 }}>
          <h2 style={{ fontSize: 13, margin: '0 0 8px', color: 'var(--c-text-muted)' }}>{f.id}</h2>
          <PlotView
            scene={f.scene}
            columns={f.columns}
            height={f.columns === 2 ? 240 : 260}
            tooltip={tooltip}
            onPointClick={(s, i, p) => setClicked(`clicked ${s}[${i}] in panel ${p}`)}
            {...(f.overlay !== undefined ? { overlay: f.overlay } : {})}
          />
        </section>
      ))}
    </main>
  );
}

const root = document.getElementById('root');
if (root !== null) createRoot(root).render(<StrictMode><Demo /></StrictMode>);
