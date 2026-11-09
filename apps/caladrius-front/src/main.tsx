// Boot: connect to the engine (Worker or Tauri), make the session, draw the screen.
// While the module loads the page says so; if it cannot load, the screen is drawn all the same, with the
// engine status in the top bar saying why.

import 'virtual:theme.css';
import '@caladrius/table/table.css';
import './app.css';
import { createRoot } from 'react-dom/client';
import { Formatter, type EngineClient, type FormatGlue } from '@caladrius/bridge-client';
import { connect, type Connection } from './engine.ts';
import { AppContext } from './session/hooks.ts';
import { Session } from './session/session.ts';
import { App } from './shell/App.tsx';
import { loadSettings } from './shell/persist.ts';

/** What formats numbers when the engine's module did not load: plain JavaScript, so the screen can still be drawn. */
const FALLBACK: FormatGlue = {
  formatNumber: (x) => String(x),
  formatFitQuality: (x) => String(x),
  formatExact: (x) => String(x),
  formatExactPoint: (x) => String(x),
  parseNumber: (t) => {
    const x = Number(t.trim().replace(',', '.'));
    return t.trim() !== '' && Number.isFinite(x) ? x : undefined;
  },
  parameterLabel: (n) => n,
  parameterUnit: () => undefined,
  withLabels: (s) => s,
  plain: (s) => s,
  words: (c) => c,
};

function failing(message: string): EngineClient {
  const fail = async (): Promise<never> => {
    throw new Error(message);
  };
  return { execute: fail, send: fail, close: () => undefined };
}

async function main(): Promise<void> {
  const container = document.getElementById('root');
  if (!container) throw new Error('no #root');
  container.innerHTML = '<p class="fatal muted" role="status">Loading the engine…</p>';
  const params = new URLSearchParams(location.search);
  const forced = params.get('theme');
  const settings = loadSettings();
  if (forced === 'light' || forced === 'dark') settings.theme = forced;

  let connection: Connection | null = null;
  let failure = '';
  try {
    connection = await connect();
  } catch (error) {
    failure = `The engine could not start: ${error instanceof Error ? error.message : String(error)}`;
  }
  const fmt = connection?.fmt ?? new Formatter(FALLBACK, settings.display);
  const session = new Session(connection?.client ?? failing(failure), fmt, settings.display, settings.theme);
  container.replaceChildren();
  createRoot(container).render(
    <AppContext.Provider value={{ session, fmt }}>
      <App />
    </AppContext.Provider>,
  );
  if (connection !== null) session.engineReady(connection.describe, connection.way);
  else session.engineFailed(failure);
  // The checks in a browser (test/e2e.mjs) read the state through this; nothing in the page depends on it.
  (window as unknown as { __caladrius?: unknown }).__caladrius = { session };
}

main().catch((error: unknown) => {
  const root = document.getElementById('root');
  if (root) root.innerHTML = '<p class="fatal" role="alert"></p>';
  const p = document.querySelector('.fatal');
  if (p) p.textContent = `The screen could not start: ${error instanceof Error ? error.message : String(error)}`;
});
