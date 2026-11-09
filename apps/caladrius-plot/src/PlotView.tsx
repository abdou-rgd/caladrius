// The plot component: draws a plot scene with Vega-Lite on a canvas, and the legend and notes in HTML.
import { Component, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { CSSProperties, ErrorInfo, ReactNode } from 'react';
import { parse, View } from 'vega';
import { compile } from 'vega-lite';
import { sceneAriaLabel } from './aria.ts';
import { colorsFromVariables } from './colors.ts';
import type { LegendEntry, PlotScene } from './scene.ts';
import { sceneToVegaLite, seriesLook } from './spec.ts';
import type { Overlay, PlotColors, TooltipText } from './spec.ts';

export interface PlotViewProps {
  scene: PlotScene;
  overlay?: Overlay[];
  columns?: 1 | 2;
  /** Height of the plotting area of one panel, in pixels. */
  height?: number;
  tooltip?: TooltipText;
  onPointClick?: (series: string, index: number, panel: number) => void;
}

// Space taken around the plotting area of a panel (y axis, its title, right margin), and between panels.
const SIDE = 84;
const GAP = 28;

function readColors(): PlotColors {
  const style = getComputedStyle(document.documentElement);
  return colorsFromVariables((name) => style.getPropertyValue(name));
}

/** Re-reads the colours when the theme changes: `data-theme` on the root, or the system preference. */
function useThemeColors(): PlotColors {
  const [colors, setColors] = useState<PlotColors>(() => readColors());
  useEffect(() => {
    const update = (): void => setColors(readColors());
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    media.addEventListener('change', update);
    update();
    return () => {
      observer.disconnect();
      media.removeEventListener('change', update);
    };
  }, []);
  return colors;
}

function useWidth(ref: React.RefObject<HTMLDivElement | null>): number {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el === null) return;
    setWidth(el.clientWidth);
    const observer = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width;
      if (w !== undefined) setWidth(Math.floor(w));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}

interface Tooltip {
  show: (x: number, y: number, text: string) => void;
  hide: () => void;
  element: HTMLDivElement;
}

function makeTooltip(): Tooltip {
  const element = document.createElement('div');
  element.setAttribute('role', 'tooltip');
  Object.assign(element.style, {
    position: 'fixed',
    zIndex: '1000',
    pointerEvents: 'none',
    display: 'none',
    padding: '4px 8px',
    borderRadius: '4px',
    background: 'var(--c-panel, #fff)',
    color: 'var(--c-text, #000)',
    border: '1px solid var(--c-border, #888)',
    font: '12px system-ui, sans-serif',
    whiteSpace: 'pre',
  });
  document.body.appendChild(element);
  return {
    element,
    show(x, y, text) {
      element.textContent = text; // text given by the app, shown as is
      element.style.left = `${x + 12}px`;
      element.style.top = `${y + 12}px`;
      element.style.display = 'block';
    },
    hide() {
      element.style.display = 'none';
    },
  };
}

interface Datum {
  series?: unknown;
  i?: unknown;
  panel?: unknown;
  tip?: unknown;
}

function Canvas(props: PlotViewProps & { colors: PlotColors; width: number }): ReactNode {
  const { scene, overlay, columns, height, tooltip, onPointClick, colors, width } = props;
  const host = useRef<HTMLDivElement>(null);
  const cols: 1 | 2 = columns === 2 ? 2 : 1;
  const plotWidth = Math.max(120, Math.floor((width - (cols - 1) * GAP) / cols) - SIDE);
  const [problem, setProblem] = useState<string | undefined>(undefined);
  const onClick = useRef(onPointClick);
  onClick.current = onPointClick;

  const spec = useMemo(() => {
    const options = { colors, columns: cols, width: plotWidth, ...(height !== undefined ? { height } : {}) };
    return sceneToVegaLite(scene, {
      ...options,
      ...(overlay !== undefined ? { overlay } : {}),
      ...(tooltip !== undefined ? { tooltip } : {}),
    });
  }, [scene, overlay, tooltip, colors, cols, plotWidth, height]);

  useEffect(() => {
    const el = host.current;
    if (el === null || width <= 0) return;
    let view: View | undefined;
    let tip: Tooltip | undefined;
    let cancelled = false;
    try {
      const vega = compile(spec).spec;
      view = new View(parse(vega), { renderer: 'canvas', container: el, hover: true });
      if (tooltip !== undefined) {
        const t = makeTooltip();
        tip = t;
        view.tooltip((_handler, event, item) => {
          const text = (item?.datum as Datum | undefined)?.tip;
          if (typeof text === 'string' && text !== '' && event instanceof MouseEvent) t.show(event.clientX, event.clientY, text);
          else t.hide();
        });
      }
      view.addEventListener('click', (_event, item) => {
        const d = item?.datum as Datum | undefined;
        if (d === undefined || typeof d.series !== 'string' || typeof d.i !== 'number' || typeof d.panel !== 'number') return;
        onClick.current?.(d.series, d.i, d.panel);
      });
      const label = sceneAriaLabel(scene);
      view
        .runAsync()
        .then(() => {
          if (cancelled) return;
          for (const c of el.querySelectorAll('canvas')) {
            c.setAttribute('role', 'img');
            c.setAttribute('aria-label', label);
          }
          setProblem(undefined);
        })
        .catch(() => {
          if (!cancelled) setProblem('The plot could not be drawn from this scene.');
        });
    } catch {
      setProblem('The plot could not be drawn from this scene.');
    }
    return () => {
      cancelled = true;
      tip?.element.remove();
      view?.finalize();
      el.replaceChildren();
    };
  }, [spec, scene, tooltip, width]);

  return (
    <>
      <div ref={host} style={{ minHeight: 1, background: 'var(--c-plot-background)', borderRadius: 4 }} />
      {problem !== undefined ? <p role="alert" style={noteStyle}>{problem}</p> : null}
    </>
  );
}

const noteStyle: CSSProperties = { margin: '4px 0', fontSize: 12, color: 'var(--c-text-muted)' };

function Swatch(props: { entry: LegendEntry; colors: PlotColors }): ReactNode {
  const { entry, colors } = props;
  const look = seriesLook(entry.role, entry.slot, colors);
  const line = entry.style === 'line' || entry.style === 'line_points';
  const dot = entry.style === 'points' || entry.style === 'line_points';
  return (
    <svg width="26" height="12" aria-hidden="true" focusable="false">
      {line ? <line x1="1" y1="6" x2="25" y2="6" stroke={look.color} strokeWidth="2" /> : null}
      {dot ? (
        <circle cx="13" cy="6" r="3.5" fill={look.hollow ? 'none' : look.color} stroke={look.color} strokeWidth="1.5" />
      ) : null}
    </svg>
  );
}

function Legend(props: { scene: PlotScene; overlay: Overlay[] | undefined; colors: PlotColors }): ReactNode {
  const { scene, overlay, colors } = props;
  const entries = scene.legend?.entries ?? [];
  const belowLimit = scene.legend?.below_limit;
  if (entries.length === 0 && belowLimit === undefined && (overlay?.length ?? 0) === 0) return null;
  const item: CSSProperties = { display: 'inline-flex', alignItems: 'center', gap: 6, marginRight: 16 };
  return (
    <ul aria-label="Legend" style={{ listStyle: 'none', margin: '6px 0 0', padding: 0, fontSize: 12, color: 'var(--c-text)' }}>
      {entries.map((e) => (
        <li key={`${e.role}:${e.name}`} style={{ ...item, display: 'inline-flex' }}>
          <Swatch entry={e} colors={colors} />
          {e.name}
        </li>
      ))}
      {(overlay ?? []).map((o) => (
        <li key={`overlay:${o.name}`} style={item}>
          <svg width="26" height="12" aria-hidden="true" focusable="false">
            <line x1="1" y1="6" x2="25" y2="6" stroke={colors.fit} strokeWidth="2" strokeDasharray="6 4" />
          </svg>
          {o.name}
        </li>
      ))}
      {belowLimit !== undefined ? (
        <li style={item}>
          <svg width="26" height="12" aria-hidden="true" focusable="false">
            <path d="M13 2 L17 10 L9 10 Z" fill="none" stroke={colors.observed} strokeWidth="1.5" />
          </svg>
          {belowLimit}
        </li>
      ) : null}
    </ul>
  );
}

function Notes(props: { scene: PlotScene }): ReactNode {
  const panels = Array.isArray(props.scene.panels) ? props.scene.panels : [];
  const hidden = panels.flatMap((p) => (p.hidden !== undefined && p.hidden.note !== '' ? [{ title: p.title, note: p.hidden.note }] : []));
  const notes = Array.isArray(props.scene.notes) ? props.scene.notes : [];
  if (hidden.length === 0 && notes.length === 0) return null;
  return (
    <div style={{ marginTop: 4 }}>
      {hidden.map((h) => (
        <p key={`h:${h.title}`} style={noteStyle}>
          {panels.length > 1 ? `${h.title}: ` : ''}
          {h.note}
        </p>
      ))}
      {notes.map((n, i) => (
        <p key={`n:${i}`} style={noteStyle}>
          {n}
        </p>
      ))}
    </div>
  );
}

class Boundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  override state = { failed: false };
  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }
  override componentDidCatch(_error: Error, _info: ErrorInfo): void {}
  override render(): ReactNode {
    return this.state.failed ? (
      <p role="alert" style={noteStyle}>
        The plot could not be drawn from this scene.
      </p>
    ) : (
      this.props.children
    );
  }
}

export function PlotView(props: PlotViewProps): ReactNode {
  const wrapper = useRef<HTMLDivElement>(null);
  const width = useWidth(wrapper);
  const colors = useThemeColors();
  return (
    <div ref={wrapper} style={{ width: '100%' }}>
      <Boundary>
        <Canvas {...props} colors={colors} width={width} />
        <Legend scene={props.scene} overlay={props.overlay} colors={colors} />
        <Notes scene={props.scene} />
      </Boundary>
    </div>
  );
}

