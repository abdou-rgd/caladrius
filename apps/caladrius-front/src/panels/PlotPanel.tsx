import { Suspense, lazy, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import type { Overlay } from '@caladrius/plot';
import { useApp, useSessionState } from '../session/hooks.ts';
import type { PlotTab } from '../session/types.ts';

// Vega is the largest part of the page: it loads when the first plot is drawn.
const PlotView = lazy(() => import('@caladrius/plot').then((m) => ({ default: m.PlotView })));

const TABS: { id: PlotTab; label: string }[] = [
  { id: 'fit', label: 'Fit' },
  { id: 'residuals', label: 'Residuals' },
  { id: 'data', label: 'Data' },
];

/** The plotting height that fits the panel: its own height less the tabs, the legend, the axis and the notes. */
function useFitHeight(ref: React.RefObject<HTMLDivElement | null>): number {
  const [height, setHeight] = useState(280);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el === null) return;
    const update = (h: number): void => setHeight(Math.max(140, Math.min(640, Math.floor(h - 112)))); // layout: pixels
    update(el.clientHeight);
    const observer = new ResizeObserver((entries) => {
      const h = entries[0]?.contentRect.height;
      if (h !== undefined) update(h);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);
  return height;
}

export function PlotPanel(): ReactNode {
  const { session } = useApp();
  const state = useSessionState();
  const { plots, rawPlot, preview, fit } = state;
  const area = useRef<HTMLDivElement>(null);
  const height = useFitHeight(area);

  const overlay = useMemo<Overlay[] | undefined>(
    () => (preview.curve !== null && preview.curve.length > 0 ? [{ name: 'Initial estimates', points: preview.curve }] : undefined),
    [preview.curve],
  );

  const tab = plots.tab;
  let content: ReactNode;
  if (tab === 'data') {
    content =
      rawPlot.scene !== null ? (
        <PlotView scene={rawPlot.scene} columns={2} height={height} {...(overlay !== undefined ? { overlay } : {})} />
      ) : (
        <p className="muted">{rawPlot.error ?? 'Load data to see the profile of the subject.'}</p>
      );
  } else if (tab === 'fit') {
    content =
      plots.profile.scene !== null ? <PlotView scene={plots.profile.scene} columns={2} height={height} /> : <p className="muted">{plots.error ?? (fit === null ? 'Fit the model to see the observed and fitted curves.' : 'The plot of the fit is not available.')}</p>;
  } else {
    content =
      plots.residuals.scene !== null ? <PlotView scene={plots.residuals.scene} columns={1} height={height} /> : <p className="muted">{plots.error ?? 'Fit the model to see the weighted residuals.'}</p>;
  }

  return (
    <div className="panel fill">
      <div className="tabs" role="tablist" aria-label="Plots">
        {TABS.map((t) => (
          <button key={t.id} role="tab" className="tab" aria-selected={tab === t.id} onClick={() => session.setPlotTab(t.id)}>
            {t.label}
          </button>
        ))}
        <span className="spacer" />
        {tab === 'data' && preview.status === 'pending' ? <span className="muted small">updating…</span> : null}
      </div>
      <div className="plot-area" ref={area} data-testid="plot-area">
        <Suspense fallback={<p className="muted">Loading the plot…</p>}>{content}</Suspense>
      </div>
    </div>
  );
}
