// The map between the plot scene and the pixels, and back. The scene holds a logarithmic axis already
// in log10 (its points, its range and its ticks), so one affine map is all a front applies: no
// logarithm is taken here, and a click is turned into a sample by the same map in reverse.

import type { SceneAxis, ScenePanel, ScenePoint, SceneSeries } from './types.ts';

/** The drawing area inside an SVG of `width` by `height` pixels. */
export interface Frame {
  width: number;
  height: number;
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export interface Rect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** Where `t` (scene units) falls between 0 (`min`) and 1 (`max`); 0.5 for an empty range, as the engine's own `fraction`. */
export function fraction(axis: SceneAxis, t: number): number {
  return axis.max > axis.min ? (t - axis.min) / (axis.max - axis.min) : 0.5;
}

/** The inverse of `fraction`. */
export function unfraction(axis: SceneAxis, f: number): number {
  return axis.min + f * (axis.max - axis.min);
}

export function plotRect(frame: Frame): Rect {
  return {
    x0: frame.left,
    y0: frame.top,
    x1: Math.max(frame.left + 1, frame.width - frame.right),
    y1: Math.max(frame.top + 1, frame.height - frame.bottom),
  };
}

export function pxOfX(frame: Frame, panel: ScenePanel, x: number): number {
  const r = plotRect(frame);
  return r.x0 + fraction(panel.x, x) * (r.x1 - r.x0);
}

export function pyOfY(frame: Frame, panel: ScenePanel, y: number): number {
  const r = plotRect(frame);
  return r.y1 - fraction(panel.y, y) * (r.y1 - r.y0);
}

export function xOfPx(frame: Frame, panel: ScenePanel, px: number): number {
  const r = plotRect(frame);
  return unfraction(panel.x, (px - r.x0) / (r.x1 - r.x0));
}

export function yOfPy(frame: Frame, panel: ScenePanel, py: number): number {
  const r = plotRect(frame);
  return unfraction(panel.y, (r.y1 - py) / (r.y1 - r.y0));
}

export function find(panel: ScenePanel, role: SceneSeries['role']): SceneSeries | undefined {
  return panel.series.find((s) => s.role === role);
}

/**
 * The samples a person can pick: the points of the profile line that the axes can show and that
 * hold a positive concentration. On a logarithmic axis the scene has already left out the points it
 * cannot show (zero or below); on a linear axis a zero is a point of the profile but not a sample
 * the terminal phase can use. A marker below the limit (`blq`) is never one.
 */
export function selectable(panel: ScenePanel): ScenePoint[] {
  const profile = find(panel, 'profile');
  if (!profile) return [];
  return profile.points.filter((p) => !p.blq && (panel.y.log || p.y > 0));
}

/** The sample nearest to a pixel, within `radius` pixels (Euclidean), or null. */
export function pickPoint(frame: Frame, panel: ScenePanel, px: number, py: number, radius: number): ScenePoint | null {
  let best: ScenePoint | null = null;
  let bestD = radius * radius;
  for (const p of selectable(panel)) {
    const dx = pxOfX(frame, panel, p.x) - px;
    const dy = pyOfY(frame, panel, p.y) - py;
    const d = dx * dx + dy * dy;
    if (d <= bestD) {
      bestD = d;
      best = p;
    }
  }
  return best;
}

/** The sample whose column is nearest to a pixel: where a handle lands while it is dragged. */
export function snapSample(frame: Frame, panel: ScenePanel, px: number): ScenePoint | null {
  let best: ScenePoint | null = null;
  let bestD = Number.POSITIVE_INFINITY;
  for (const p of selectable(panel)) {
    const d = Math.abs(pxOfX(frame, panel, p.x) - px);
    if (d < bestD) {
      bestD = d;
      best = p;
    }
  }
  return best;
}

/** The time of the sample `steps` places after `time` (before, if negative), kept inside the list. */
export function stepSample(panel: ScenePanel, time: number, steps: number): number | null {
  const samples = selectable(panel);
  const at = samples.findIndex((p) => p.x === time);
  if (at < 0) return null;
  const next = samples[Math.min(samples.length - 1, Math.max(0, at + steps))];
  return next ? next.x : null;
}

/** A coordinate as text for an SVG attribute: two decimals at most. Not a displayed number. */
export function coord(n: number): string {
  return String(Math.round(n * 100) / 100);
}

/** The points of a line as an SVG `points` attribute. */
export function polyline(frame: Frame, panel: ScenePanel, series: SceneSeries): string {
  return series.points
    .map((p) => `${coord(pxOfX(frame, panel, p.x))},${coord(pyOfY(frame, panel, p.y))}`)
    .join(' ');
}
