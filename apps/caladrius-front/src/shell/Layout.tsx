import { DockviewReact, type DockviewApi, type DockviewReadyEvent, type IDockviewPanelProps } from 'dockview-react';
import 'dockview-react/dist/styles/dockview.css';
import type { ReactNode } from 'react';
import { DataPanel } from '../panels/DataPanel.tsx';
import { HistoryPanel } from '../panels/HistoryPanel.tsx';
import { ModelPanel } from '../panels/ModelPanel.tsx';
import { PlotPanel } from '../panels/PlotPanel.tsx';
import { ResultsPanel } from '../panels/ResultsPanel.tsx';
import { PanelBoundary } from './PanelBoundary.tsx';
import { forgetLayout, loadLayout, saveLayout } from './persist.ts';

function wrap(name: string, Panel: () => ReactNode): (props: IDockviewPanelProps) => ReactNode {
  return function Wrapped(): ReactNode {
    return (
      <PanelBoundary name={name}>
        <Panel />
      </PanelBoundary>
    );
  };
}

const components = {
  data: wrap('Data', DataPanel),
  model: wrap('Model', ModelPanel),
  plot: wrap('Plot', PlotPanel),
  results: wrap('Results', ResultsPanel),
  history: wrap('History', HistoryPanel),
};

/** The layout when nothing is saved: data and model on the left, the plot on top, results and history below it. */
export function defaultLayout(api: DockviewApi): void {
  api.addPanel({ id: 'plot', component: 'plot', title: 'Plot' });
  api.addPanel({ id: 'data', component: 'data', title: 'Data', position: { referencePanel: 'plot', direction: 'left' } });
  api.addPanel({ id: 'model', component: 'model', title: 'Model', position: { referencePanel: 'data', direction: 'below' } });
  api.addPanel({ id: 'results', component: 'results', title: 'Results', position: { referencePanel: 'plot', direction: 'below' } });
  api.addPanel({ id: 'history', component: 'history', title: 'History', position: { referencePanel: 'results', direction: 'within' }, inactive: true });
  try {
    api.getPanel('data')?.api.setSize({ width: 500, height: 330 });
    api.getPanel('results')?.api.setSize({ height: 380 });
    api.getPanel('results')?.api.setActive();
  } catch {
    // Sizes are a nicety: the layout works without them.
  }
}

/** Puts the panels back where they start, and forgets the saved layout. */
export function resetLayout(api: DockviewApi): void {
  forgetLayout();
  api.clear();
  defaultLayout(api);
}

export function Layout(props: { onApi: (api: DockviewApi) => void }): ReactNode {
  const onReady = (event: DockviewReadyEvent): void => {
    const api = event.api;
    let restored = false;
    const saved = loadLayout();
    if (saved !== null) {
      try {
        api.fromJSON(saved as Parameters<DockviewApi['fromJSON']>[0]);
        restored = api.panels.length >= Object.keys(components).length;
      } catch {
        restored = false;
      }
      if (!restored) api.clear();
    }
    if (!restored) defaultLayout(api);
    api.onDidLayoutChange(() => saveLayout(api.toJSON()));
    props.onApi(api);
  };
  return <DockviewReact className="dockview-theme-light caladrius-dock" components={components} onReady={onReady} />;
}
