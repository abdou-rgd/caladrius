// The plot scene -> a Vega-Lite specification. A pure function: no DOM, no React.
//
// The scene already holds every number a person reads: axis ranges, tick values, the label of each
// tick (written by the engine) and the points. This file only maps them to marks. It never rounds,
// formats, aggregates or fits anything. The single arithmetic it writes is `logScaleExpression`
// below, a scale mapping (drawing) for the overlay curves.

import type { TopLevelSpec } from 'vega-lite';
import type { PlotScene, SceneAxis, ScenePanel, ScenePoint, SceneRole, SceneSeries, SeriesStyle } from './scene.ts';

/** Theme colours, read by the component from the CSS variables of the page. */
export interface PlotColors {
  observed: string;
  predicted: string;
  fit: string;
  selected: string;
  start: string;
  replaced: string;
  other: string;
  subjects: string[];
  grid: string;
  background: string;
  text: string;
  muted: string;
}

/** A curve given in data units by the app (live preview of initial estimates, from `model.simulate`). */
export interface Overlay {
  name: string;
  points: { x: number; y: number }[];
}

export type TooltipText = (series: string, point: ScenePoint, panel: number) => string;

export interface SpecOptions {
  colors: PlotColors;
  /** Number of panel columns, 1 or 2. Default 1. */
  columns?: 1 | 2;
  /** Width of the plotting area of one panel, in pixels. Default 480. */
  width?: number;
  /** Height of the plotting area of one panel, in pixels. Default 280. */
  height?: number;
  overlay?: Overlay[];
  tooltip?: TooltipText;
}

/** Shown when the scene has nothing to draw. */
export const NOTHING_TO_PLOT = 'Nothing to plot yet: the scene has no panel.';

const DEFAULT_WIDTH = 480;
const DEFAULT_HEIGHT = 280;
const SPACING = 28;
const DASH_OVERLAY = [6, 4];
const DASH_GUIDE = [4, 4];

type Json = Record<string, unknown>;

/** How a series is drawn: its colour and whether its markers are hollow. */
export interface SeriesLook {
  color: string;
  hollow: boolean;
}

/** The colour of a series from its role and slot. */
export function seriesLook(role: SceneRole, slot: number | undefined, colors: PlotColors): SeriesLook {
  switch (role) {
    case 'subject': {
      const n = colors.subjects.length;
      const pick = n > 0 ? colors.subjects[(slot ?? 0) % n] : undefined;
      return { color: pick ?? colors.other, hollow: false };
    }
    case 'observed':
      return { color: colors.observed, hollow: false };
    case 'predicted':
    case 'lambda_z_fit':
      return { color: colors.fit, hollow: false };
    case 'lambda_z_used':
      return { color: colors.selected, hollow: false };
    case 'lambda_z_start':
      return { color: colors.start, hollow: false };
    case 'excluded':
      return { color: colors.muted, hollow: true };
    case 'replaced':
      return { color: colors.replaced, hollow: false };
    case 'profile':
      return { color: colors.other, hollow: false };
    default:
      return { color: colors.other, hollow: false };
  }
}

/** A Vega expression that maps a positive data value to its decade (log10). This is a scale
 *  mapping for drawing a curve on a logarithmic axis, the only arithmetic of the package;
 *  the engine's own points and ticks are already in decades. */
function logScaleExpression(field: 'x' | 'y'): string {
  return `log(datum.${field}) / LN10`;
}

function finite(n: unknown): n is number {
  return typeof n === 'number' && Number.isFinite(n);
}

/** `labelExpr`: the label of each tick is the engine's, chosen by exact value. */
function labelExpression(ticks: SceneAxis['ticks']): string {
  let expr = "''";
  for (let k = ticks.length - 1; k >= 0; k -= 1) {
    const t = ticks[k];
    if (t === undefined || !finite(t.value)) continue;
    expr = `datum.value === ${JSON.stringify(t.value)} ? ${JSON.stringify(String(t.label))} : ${expr}`;
  }
  return expr;
}

function axisEncoding(axis: SceneAxis, field: string, colors: PlotColors): Json {
  const values = axis.ticks.filter((t) => finite(t.value)).map((t) => t.value);
  return {
    field,
    type: 'quantitative',
    scale: { type: 'linear', domain: [axis.min, axis.max], nice: false, zero: false, clamp: false },
    axis: {
      title: axis.title,
      values,
      labelExpr: labelExpression(axis.ticks),
      grid: true,
      gridColor: colors.grid,
      domainColor: colors.muted,
      tickColor: colors.muted,
      labelColor: colors.text,
      titleColor: colors.text,
      labelOverlap: false,
    },
  };
}

function axisUsable(a: SceneAxis): boolean {
  return finite(a.min) && finite(a.max) && a.max > a.min;
}

interface Row {
  x: number;
  y: number;
  order: number;
  i: number;
  series: string;
  panel: number;
  blq: boolean;
  tip?: string;
}

function rowsOf(series: SceneSeries, panelIndex: number, tooltip: TooltipText | undefined): Row[] {
  const rows: Row[] = [];
  series.points.forEach((p, i) => {
    if (!finite(p.x) || !finite(p.y)) return; // a NaN in a point: the point is not drawn
    const row: Row = { x: p.x, y: p.y, order: i, i, series: series.name, panel: panelIndex, blq: p.blq === true };
    if (tooltip !== undefined) row.tip = tooltip(series.name, p, panelIndex);
    rows.push(row);
  });
  return rows;
}

function wantsLine(style: SeriesStyle): boolean {
  return style === 'line' || style === 'line_points';
}
function wantsPoints(style: SeriesStyle): boolean {
  return style === 'points' || style === 'line_points';
}

function panelSpec(panel: ScenePanel, index: number, o: Required<Pick<SpecOptions, 'width' | 'height'>> & SpecOptions): Json {
  const { colors, width, height } = o;
  const title: Json = { text: panel.title, color: colors.text, anchor: 'start' };

  if (panel.empty !== undefined) {
    return textSpec(panel.empty, title, width, height, colors);
  }
  if (!axisUsable(panel.x) || !axisUsable(panel.y)) {
    return textSpec('This panel has no usable axis range.', title, width, height, colors);
  }

  const xEnc = axisEncoding(panel.x, 'x', colors);
  const yEnc = axisEncoding(panel.y, 'y', colors);
  const hasTip = o.tooltip !== undefined;
  const tipEnc: Json = hasTip ? { tooltip: { field: 'tip', type: 'nominal' } } : {};

  // The frame layer carries the axes and scales even when nothing else is drawn.
  const layers: Json[] = [
    { data: { values: [] }, mark: { type: 'point', opacity: 0, aria: false }, encoding: { x: xEnc, y: yEnc } },
  ];

  if (panel.guide !== undefined) {
    const { from, to } = panel.guide;
    if (from.every(finite) && to.every(finite)) {
      layers.push({
        data: { values: [{ x: from[0], y: from[1], x2: to[0], y2: to[1] }] },
        mark: { type: 'rule', color: colors.muted, strokeDash: DASH_GUIDE, strokeWidth: 1, clip: true, aria: false },
        encoding: { x: xEnc, y: yEnc, x2: { field: 'x2' }, y2: { field: 'y2' } },
      });
    }
  }

  for (const s of panel.series) {
    const rows = rowsOf(s, index, o.tooltip);
    if (rows.length === 0) continue;
    const look = seriesLook(s.role, s.slot, colors);
    if (wantsLine(s.style)) {
      layers.push({
        data: { values: rows },
        mark: { type: 'line', color: look.color, strokeWidth: 1.75, clip: true, aria: false },
        encoding: { x: xEnc, y: yEnc, order: { field: 'order', type: 'quantitative' } },
      });
    }
    if (wantsPoints(s.style)) {
      const plain = rows.filter((r) => !r.blq);
      const below = rows.filter((r) => r.blq);
      if (plain.length > 0) {
        layers.push({
          data: { values: plain },
          mark: look.hollow
            ? { type: 'point', shape: 'circle', filled: false, size: 40, strokeWidth: 1.5, color: look.color, aria: false }
            : { type: 'point', shape: 'circle', filled: true, size: 40, opacity: 1, color: look.color, aria: false },
          encoding: { x: xEnc, y: yEnc, ...tipEnc },
        });
      }
      if (below.length > 0) {
        layers.push({
          data: { values: below },
          mark: { type: 'point', shape: 'triangle-up', filled: false, size: 56, strokeWidth: 1.5, color: look.color, aria: false },
          encoding: { x: xEnc, y: yEnc, ...tipEnc },
        });
      }
    }
  }

  for (const ov of o.overlay ?? []) {
    const layer = overlayLayer(ov, panel, xEnc, yEnc, colors);
    if (layer !== undefined) layers.push(layer);
  }

  return { title, width, height, layer: layers };
}

function overlayLayer(ov: Overlay, panel: ScenePanel, xEnc: Json, yEnc: Json, colors: PlotColors): Json | undefined {
  const values = ov.points.filter((p) => finite(p?.x) && finite(p?.y)).map((p, i) => ({ x: p.x, y: p.y, order: i }));
  const transform: Json[] = [];
  let xField = 'x';
  let yField = 'y';
  // A curve on a logarithmic axis: drop non-positive values first, then map to decades.
  for (const [axis, field] of [[panel.x, 'x'], [panel.y, 'y']] as const) {
    if (!axis.log) continue;
    transform.push({ filter: `datum.${field} > 0` });
    transform.push({ calculate: logScaleExpression(field), as: `${field}p` });
    if (field === 'x') xField = 'xp';
    else yField = 'yp';
  }
  if (values.length === 0) return undefined;
  return {
    data: { values },
    transform,
    mark: { type: 'line', color: colors.fit, strokeWidth: 1.75, strokeDash: DASH_OVERLAY, clip: true, aria: false },
    encoding: {
      x: { ...xEnc, field: xField },
      y: { ...yEnc, field: yField },
      order: { field: 'order', type: 'quantitative' },
    },
  };
}

function textSpec(sentence: string, title: Json, width: number, height: number, colors: PlotColors): Json {
  return {
    title,
    width,
    height,
    data: { values: [{ t: sentence }] },
    mark: { type: 'text', align: 'center', baseline: 'middle', color: colors.muted, limit: width, aria: false },
    encoding: { text: { field: 't', type: 'nominal' }, x: { value: width / 2 }, y: { value: height / 2 } },
  };
}

/** The Vega-Lite specification that draws `scene`: one view per panel, laid out by `concat`. */
export function sceneToVegaLite(scene: PlotScene, options: SpecOptions): TopLevelSpec {
  const colors = options.colors;
  const width = options.width !== undefined && options.width > 0 ? options.width : DEFAULT_WIDTH;
  const height = options.height !== undefined && options.height > 0 ? options.height : DEFAULT_HEIGHT;
  const opts = { ...options, width, height };
  const panels = Array.isArray(scene.panels) ? scene.panels : [];
  const config: Json = {
    background: colors.background,
    view: { stroke: colors.grid },
    title: { color: colors.text, fontWeight: 'normal' },
  };
  const common: Json = { $schema: 'https://vega.github.io/schema/vega-lite/v6.json', background: colors.background, config };

  if (panels.length === 0) {
    const spec: Json = { ...common, ...textSpec(NOTHING_TO_PLOT, { text: scene.title ?? '', color: colors.text }, width, height, colors) };
    return spec as unknown as TopLevelSpec;
  }
  const spec: Json = {
    ...common,
    spacing: SPACING,
    columns: options.columns === 2 ? 2 : 1,
    concat: panels.map((p, i) => panelSpec(p, i, opts)),
  };
  return spec as unknown as TopLevelSpec;
}
