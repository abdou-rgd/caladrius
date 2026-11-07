import { DockviewReact, type DockviewReadyEvent, type IDockviewPanelProps } from 'dockview-react';
import 'dockview-react/dist/styles/dockview.css';
import { useEffect, useRef } from 'react';
import { DataPanel } from './DataPanel.tsx';
import { PlotPanel } from './PlotPanel.tsx';
import { ResultsPanel } from './ResultsPanel.tsx';
import { TopBar } from './TopBar.tsx';
import { useScreen, useSession } from './hooks.ts';

const components: Record<string, React.FunctionComponent<IDockviewPanelProps>> = {
  data: () => <DataPanel />,
  plot: () => <PlotPanel />,
  results: () => <ResultsPanel />,
};

export interface AppProps {
  datasets: Record<string, string>;
  onFirstFrame?: () => void;
}

export function App({ datasets, onFirstFrame }: AppProps) {
  const session = useSession();
  const state = useScreen();
  const names = Object.keys(datasets);
  const done = useRef(false);

  useEffect(() => {
    if (!done.current && state.ready) {
      done.current = true;
      onFirstFrame?.();
    }
  }, [state.ready, onFirstFrame]);

  const ready = (event: DockviewReadyEvent) => {
    const plot = event.api.addPanel({ id: 'plot', component: 'plot', title: 'Plot' });
    event.api.addPanel({ id: 'data', component: 'data', title: 'Data', position: { referencePanel: plot, direction: 'left' }, initialWidth: 380 });
    event.api.addPanel({ id: 'results', component: 'results', title: 'Results', position: { referencePanel: plot, direction: 'below' }, initialHeight: 360 });
  };

  return (
    <div className="app">
      <TopBar
        datasets={names}
        onDataset={(name) => {
          const csv = datasets[name];
          if (csv !== undefined) void session.loadDataset(name, csv, name === 'edge_blq' ? '3' : undefined);
        }}
      />
      <div className="dock">
        <DockviewReact className={`dockview-theme-caladrius`} components={components} onReady={ready} />
      </div>
    </div>
  );
}
