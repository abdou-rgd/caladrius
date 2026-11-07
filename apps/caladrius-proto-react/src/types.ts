// The shapes of the answers this screen reads. They follow the result schemas of the commands
// (`describe()`): only the keys the screen uses are named.

export interface Candidate {
  n_points: number;
  time_first: number;
  time_last: number;
  lambda_z: number;
  intercept: number;
  adj_r_squared: number | null;
  r_squared: number | null;
  selected: boolean;
  valid: boolean;
  replaced_points: number;
}

export interface ParamValue {
  name: string;
  value: { value?: number; not_calculated?: string };
}

export interface Flag {
  code: string;
  parameter?: string;
  first?: string;
  [key: string]: unknown;
}

export interface NcaOk {
  flags: Flag[];
  lambda_z_candidates: Candidate[];
  parameters: ParamValue[];
  profile: { conc: number; index: number; origin: string; time: number }[];
  removed: { index: number; time: number; reason: string }[];
}

export interface SubjectResult {
  subject: string;
  dose: number | null;
  flag_messages?: string[];
  not_calculated_messages?: Record<string, string>;
  outcome: { ok: NcaOk } | { error: string };
}

export interface StaleReason {
  code: string;
}

export type AnalysisStatus = { state: 'fresh' } | { state: 'no_result' } | { state: 'stale'; reasons?: StaleReason[] };

/** The result of `nca.run` and `analysis.get`. */
export interface AnalysisView {
  id: number;
  kind: string;
  label: string;
  status: AnalysisStatus;
  spec: { options?: NcaOptions; [key: string]: unknown };
  result: { kind: string; subjects: SubjectResult[] } | null;
}

export interface NcaOptions {
  lambda_z_selection?: {
    manual?: { times: number[] } | { range: { start: number; end: number } } | null;
    [key: string]: unknown;
  };
  [key: string]: unknown;
}

export interface Column {
  name: string;
  role: string;
  unit: string | null;
}

export interface WorksheetInfo {
  id: number;
  name: string;
  rows: number;
  subjects: string[];
  columns: Column[];
  derived_units: Record<string, string>;
  unit_warnings: { code: string; message: string }[];
}

/** One row of the worksheet, with the row number the engine counts from 0. */
export interface DataRow {
  index: number;
  subject: string;
  time: number;
  conc: number | null;
}

/** The plot scene (`plot.scene`): no pixel and no colour, a role per series. */
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
export type SceneRoleName =
  | 'subject'
  | 'observed'
  | 'predicted'
  | 'profile'
  | 'replaced'
  | 'excluded'
  | 'lambda_z_used'
  | 'lambda_z_start'
  | 'lambda_z_fit';
export interface SceneSeries {
  name: string;
  role: SceneRoleName;
  style: 'line' | 'points';
  points: ScenePoint[];
}
export interface ScenePanel {
  title: string;
  x: SceneAxis;
  y: SceneAxis;
  series: SceneSeries[];
  hidden?: { count: number; note: string; points: { series: string; x: number; y: number }[] };
  empty?: string;
}
export interface Scene {
  title: string;
  panels: ScenePanel[];
  legend: { entries: { name: string; role: SceneRoleName; style: string }[]; below_limit?: string };
  notes: string[];
}
export interface SceneAnswer {
  analysis: number;
  kind: string;
  status: string;
  subject?: string;
  scene: Scene;
}
