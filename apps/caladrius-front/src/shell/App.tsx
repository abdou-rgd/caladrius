import type { DockviewApi } from 'dockview-react';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { tinykeys } from 'tinykeys';
import { DEFAULT_DATASETS } from '../session/session.ts';
import { useApp, useSessionState } from '../session/hooks.ts';
import { Layout, resetLayout } from './Layout.tsx';
import { Palette, type Action } from './Palette.tsx';
import { saveSettings } from './persist.ts';
import { loadSample } from './samples.ts';
import { TopBar } from './TopBar.tsx';

export function App(): ReactNode {
  const { session } = useApp();
  const { theme, display, engine } = useSessionState();
  const api = useRef<DockviewApi | null>(null);
  const [palette, setPalette] = useState(false);

  // The theme: `data-theme` on <html> forces light or dark, none follows the system.
  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'system') delete root.dataset['theme'];
    else root.dataset['theme'] = theme;
  }, [theme]);

  useEffect(() => saveSettings({ display, theme }), [display, theme]);

  useEffect(
    () =>
      tinykeys(window, {
        '$mod+KeyK': (event) => {
          event.preventDefault();
          setPalette((open) => !open);
        },
      }),
    [],
  );

  const reset = useCallback(() => {
    if (api.current !== null) resetLayout(api.current);
  }, []);

  const actions = useMemo<Action[]>(
    () => [
      { id: 'app.layout.reset', title: 'Reset layout', run: reset },
      { id: 'app.theme.light', title: 'Theme: light', run: () => session.setTheme('light') },
      { id: 'app.theme.dark', title: 'Theme: dark', run: () => session.setTheme('dark') },
      { id: 'app.theme.system', title: 'Theme: follow the system', run: () => session.setTheme('system') },
      { id: 'app.fit.run', title: 'Fit the model', run: () => void session.runFit() },
      ...DEFAULT_DATASETS.map((d) => ({ id: `app.sample.${d.id}`, title: `Import sample: ${d.label}`, run: () => void loadSample(session, d.id) })),
    ],
    [session, reset],
  );

  return (
    <div className="app" data-engine={engine.status}>
      <TopBar onPalette={() => setPalette(true)} onResetLayout={reset} />
      <div className="dock">
        <Layout onApi={(a) => (api.current = a)} />
      </div>
      <Palette open={palette} onOpenChange={setPalette} actions={actions} />
    </div>
  );
}
