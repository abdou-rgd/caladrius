// The shapes of what the engine answers, as far as this page reads them, and the session state.
// Everything in `SessionState` is plain data (rule 7): it can be written as JSON and read back.

import type { PlotScene } from '@caladrius/plot';
import type { Answer, Display, Seal } from '@caladrius/bridge-client';
import type { Compartments, Route } from '../model/parameterSets.ts';
import type { CommandInfo } from '../model/registry.ts';

export type Subject = string | number;

export interface WorksheetColumn {
  name: string;
  role: string;
  type: string;
  unit: string | null;
  blq?: number;
  missing?: number;
  min?: number;
  max?: number;
}

export interface WorksheetInfo {
  id: number;
  name: string;
  revision: number;
  rows: number;
  subjects: Subject[];
  columns: WorksheetColumn[];
  derived_units: Record<string, string>;
  unit_warnings: { code: string; message: string }[];
}

export interface FitFlag {
  code: string;
  parameter?: string;
  first?: string;
  second?: string;
}

export interface FitOk {
  status: string;
  parameters: string[];
  values: Record<string, number>;
  flags: FitFlag[];
  trace?: unknown[];
}

export interface FitResultBody {
  dose: number;
  flag_messages: string[];
  status_message: string;
  n_observations: number;
  n_blq: number;
  n_missing: number;
  subject: Subject;
  outcome: { ok?: FitOk; error?: string };
}

/** The result of `fit.run` (an analysis object). */
export interface FitAnalysis {
  id: number;
  kind: string;
  label: string;
  result: FitResultBody;
  spec: { confidence_level?: number; options?: { confidence_level?: number } } & Record<string, unknown>;
  status: { state: string };
}

export interface LogEntry {
  n: number;
  command: string;
  ok: boolean;
  /** The error code when the command failed. */
  code: string | null;
  /** The error sentence when the command failed. */
  message: string | null;
  /** The seal's sha256, in full; the panel shortens it. */
  seal: string | null;
  /** A call of the live preview: shown only on request. */
  quiet: boolean;
  /** The answer as JSON text, kept for the commands run from the palette. */
  json: string | null;
}

export interface Curve {
  x: number;
  y: number;
}

export type PlotTab = 'data' | 'fit' | 'residuals';
export type ThemeChoice = 'system' | 'light' | 'dark';

export interface ModelInput {
  compartments: Compartments;
  route: Route;
  set: string;
  weighting: string;
  /** What was typed, by parameter name. */
  initial: Record<string, string>;
  /** What was typed; empty: the dose of the data. */
  dose: string;
}

export interface FitState {
  analysis: number;
  body: FitResultBody;
  /** The inputs the fit was run with, as the key `inputKey` makes. */
  key: string;
  /** The status the engine gave: `fresh`, `stale` or `no_result`. */
  engineStatus: string;
  seal: Seal | null;
  /** The model's parameter names in the order the person saw them, and whether its route is extravascular. */
  order: string[];
  extravascular: boolean;
}

export interface SessionState {
  engine: { status: 'loading' | 'ready' | 'error'; message: string | null; way: string | null };
  commands: CommandInfo[];
  models: string[];
  weightings: string[];
  data: {
    worksheet: WorksheetInfo | null;
    sample: string | null;
    notes: string[];
    /** The first rows of the worksheet, as `data.preview` gives them. */
    rows: (string | number | null)[][];
    busy: boolean;
    error: string | null;
  };
  subject: Subject | null;
  input: ModelInput;
  rawPlot: { analysis: number | null; scene: PlotScene | null; xMax: number | null; error: string | null };
  preview: {
    status: 'idle' | 'pending' | 'ok' | 'error';
    wrss: number | null;
    dose: number | null;
    nObservations: number | null;
    curve: Curve[] | null;
    message: string | null;
  };
  estimate: { busy: boolean; error: string | null };
  fit: FitState | null;
  fitBusy: boolean;
  fitError: string | null;
  plots: {
    tab: PlotTab;
    profile: { analysis: number | null; scene: PlotScene | null };
    residuals: { analysis: number | null; scene: PlotScene | null };
    error: string | null;
  };
  display: Display;
  theme: ThemeChoice;
  log: LogEntry[];
}

export type EngineAnswer<R = unknown> = Answer<R>;
