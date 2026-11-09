// The plot scene as the engine's `plot.scene` command returns it (result_schema of plot.scene).
// No pixel and no colour: the front maps scene units to pixels and picks colours from the theme.
// A logarithmic axis is already in log10 (range, ticks and points).

export type SceneRole =
  | 'subject'
  | 'observed'
  | 'predicted'
  | 'profile'
  | 'replaced'
  | 'excluded'
  | 'lambda_z_used'
  | 'lambda_z_start'
  | 'lambda_z_fit';

export type SeriesStyle = 'points' | 'line' | 'line_points';

export interface SceneTick {
  value: number;
  label: string;
}

export interface SceneAxis {
  title: string;
  log: boolean;
  min: number;
  max: number;
  ticks: SceneTick[];
}

export interface ScenePoint {
  x: number;
  y: number;
  blq?: boolean;
}

export interface SceneSeries {
  name: string;
  role: SceneRole;
  slot?: number;
  style: SeriesStyle;
  points: ScenePoint[];
}

export interface SceneGuide {
  kind: 'identity' | 'zero';
  from: [number, number];
  to: [number, number];
}

export interface HiddenPoints {
  count: number;
  note: string;
  points: { series: string; x: number; y: number }[];
}

export interface ScenePanel {
  title: string;
  x: SceneAxis;
  y: SceneAxis;
  series: SceneSeries[];
  guide?: SceneGuide;
  hidden?: HiddenPoints;
  empty?: string;
}

export interface LegendEntry {
  name: string;
  role: SceneRole;
  slot?: number;
  style: SeriesStyle;
}

export interface PlotScene {
  title: string;
  panels: ScenePanel[];
  legend: { entries: LegendEntry[]; below_limit?: string };
  notes: string[];
}

/** The answer of `plot.scene`. */
export interface SceneAnswer {
  analysis: number;
  kind: 'plot' | 'nca';
  status: 'fresh' | 'stale' | 'no_result';
  subject?: string;
  scene: PlotScene;
}
