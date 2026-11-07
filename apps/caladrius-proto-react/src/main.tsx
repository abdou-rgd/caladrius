// Boot: connect to the engine (Worker or Tauri), read the data, draw the screen, and (measure mode)
// run the measure once the first frame is on screen.

import 'virtual:theme.css';
import './app.css';
import { createRoot } from 'react-dom/client';
import { App } from './App.tsx';
import { SessionContext } from './hooks.ts';
import { asset, connect, inTauri } from './engine.ts';
import { runMeasure } from './measure.ts';
import { Session } from './session.ts';

declare const __VERSION__: string;

interface MeasureAsk {
  requested: boolean;
  since_start_ms: number;
}

async function text(path: string): Promise<string> {
  const response = await fetch(asset(path));
  if (!response.ok) throw new Error(`${path}: ${response.status}`);
  return response.text();
}

function show(message: string): void {
  const root = document.getElementById('root');
  if (root) root.innerHTML = `<p class="fatal" role="alert"></p>`;
  const p = document.querySelector('.fatal');
  if (p) p.textContent = message;
}

/** In the Tauri shell the process start is known to Rust: ask for it, early, to date the first frame. */
async function askShell(): Promise<{ ask: MeasureAsk; at: number } | null> {
  if (!inTauri()) return null;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const ask = await invoke<MeasureAsk>('measure_requested');
    return { ask, at: performance.now() };
  } catch {
    return null;
  }
}

async function main(): Promise<void> {
  const params = new URLSearchParams(location.search);
  const forced = params.get('theme');
  if (forced === 'light' || forced === 'dark') document.documentElement.dataset['theme'] = forced;
  const shell = await askShell();
  const measureWanted = params.get('measure') === '1' || shell?.ask.requested === true;

  const [connection, edge, nca50] = await Promise.all([connect(), text('data/edge_blq.csv'), text('data/nca50.csv')]);
  const session = new Session(connection.client, connection.fmt);
  const datasets: Record<string, string> = { edge_blq: edge, nca50 };

  let firstFrame = 0;
  const mark = () => {
    // After the first paint that shows the loaded screen.
    requestAnimationFrame(() => {
      const channel = new MessageChannel();
      channel.port1.onmessage = () => {
        firstFrame = performance.now();
        framed();
      };
      channel.port2.postMessage(0);
    });
  };

  let framedResolve: () => void = () => undefined;
  const framedPromise = new Promise<void>((resolve) => {
    framedResolve = resolve;
  });
  const framed = () => framedResolve();

  const container = document.getElementById('root');
  if (!container) throw new Error('no #root');
  createRoot(container).render(
    <SessionContext.Provider value={session}>
      <App datasets={datasets} onFirstFrame={mark} />
    </SessionContext.Provider>,
  );
  const initial = params.get('dataset') === 'nca50' ? 'nca50' : 'edge_blq';
  const loading = session.loadDataset(initial, datasets[initial] ?? edge, initial === 'edge_blq' ? '3' : undefined);
  // `?stale=1` edits one concentration after the first run, so the stale state can be photographed.
  if (params.get('stale') === '1') {
    void loading.then(async () => {
      const row = session.getState().rows.find((r) => r.subject === session.getState().subject && r.conc !== null && r.conc > 0);
      if (row) await session.editCell(row, 'concentration', String((row.conc ?? 0) + 0.1));
    });
  } else {
    void loading;
  }

  if (!measureWanted) return;
  await framedPromise;
  const firstFrameMs = shell ? shell.ask.since_start_ms + (firstFrame - shell.at) : firstFrame;
  const report = await runMeasure(session, container, { stack: 'react', version: __VERSION__, firstFrameMs: Math.round(firstFrameMs * 10) / 10, csv: nca50 });
  const json = JSON.stringify(report);
  if (shell) {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('measure_write', { json });
  } else {
    console.log(`CALADRIUS_MEASURE ${json}`);
    (window as unknown as { __CALADRIUS_MEASURE__?: unknown }).__CALADRIUS_MEASURE__ = report;
  }
}

main().catch((error: unknown) => show(`The screen could not start: ${error instanceof Error ? error.message : String(error)}`));
