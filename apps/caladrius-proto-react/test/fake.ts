// A stand-in for the engine: it answers the commands the screen sends with fixed answers and
// keeps every call, so a test reads what the screen sent.

import type { EngineLike } from '../src/session.ts';
import type { Fmt } from '../src/results.ts';
import type { AnalysisView, Candidate, Scene, ScenePanel, WorksheetInfo } from '../src/types.ts';

export interface Call {
  command: string;
  params: Record<string, unknown>;
}

/** A formatter that marks every string it produces: a cell that does not carry the mark was not formatted by it. */
export const markFmt: Fmt = {
  number: (x) => `«${String(x)}»`,
  fitQuality: (x) => `«q${String(x)}»`,
  exact: (x) => `«e${String(x)}»`,
  parameterLabel: (name) => `«label ${name}»`,
  parameterUnit: (name) => `«unit ${name}»`,
  withLabels: (s) => `«${s}»`,
  words: (c) => `«words ${c}»`,
};

export const TIMES = [0, 0.5, 1, 2, 4, 6, 8, 12, 24];
export const CONC = [0, 2.1, 6.3, 5, 3.3, 2.2, 1.4, 0, 0];

export function info(): WorksheetInfo {
  return {
    id: 1,
    name: 'edge_blq',
    rows: TIMES.length,
    subjects: ['3'],
    columns: [
      { name: 'subject', role: 'subject', unit: null },
      { name: 'time', role: 'time', unit: 'h' },
      { name: 'conc', role: 'concentration', unit: 'mg/L' },
    ],
    derived_units: {},
    unit_warnings: [],
  };
}

/** A panel with a logarithmic y axis (already in log10): zeros are not on it, they are under `hidden`. */
export function logPanel(): ScenePanel {
  const shown = TIMES.map((x, i) => ({ x, c: CONC[i] ?? 0, y: Math.log10(CONC[i] || 1) })).filter((p) => p.c > 0);
  return {
    title: 'Concentration over time, subject 3',
    x: { title: 'Time', log: false, min: 0, max: 24, ticks: [{ value: 0, label: '0' }, { value: 12, label: '12' }, { value: 24, label: '24' }] },
    y: { title: 'Concentration', log: true, min: -1, max: 1, ticks: [{ value: 0, label: '1' }] },
    series: [
      { name: 'Profile', role: 'profile', style: 'line', points: shown.map((p) => ({ x: p.x, y: p.y })) },
      { name: 'Observed', role: 'observed', style: 'points', points: shown.map((p) => ({ x: p.x, y: p.y })) },
      { name: 'Used for λz', role: 'lambda_z_used', style: 'points', points: shown.filter((p) => p.x >= 4 && p.x <= 8).map((p) => ({ x: p.x, y: p.y })) },
    ],
    hidden: { count: 3, note: '3 point(s) at zero or below have no logarithm and are not shown on the logarithmic axis.', points: [] },
  };
}

export function sceneOf(panel: ScenePanel): Scene {
  return { title: panel.title, panels: [panel], legend: { entries: [] }, notes: [] };
}

export function candidate(first: number, last: number, n: number, selected: boolean): Candidate {
  return { n_points: n, time_first: first, time_last: last, lambda_z: 0.2, intercept: 1, adj_r_squared: 0.99, r_squared: 0.99, selected, valid: true, replaced_points: 0 };
}

export function view(id: number, state: 'fresh' | 'stale', manual: unknown, reasons: string[] = []): AnalysisView {
  return {
    id,
    kind: 'nca',
    label: 'NCA of edge_blq, subject 3',
    status: state === 'fresh' ? { state: 'fresh' } : { state: 'stale', reasons: reasons.map((code) => ({ code })) },
    spec: { options: { lambda_z_selection: { manual } } as never },
    result: {
      kind: 'nca',
      subjects: [
        {
          subject: '3',
          dose: 100,
          outcome: {
            ok: {
              flags: [],
              lambda_z_candidates: [candidate(4, 8, 3, true), candidate(2, 8, 4, false)],
              parameters: [
                { name: 'cmax', value: { value: 6.3 } },
                { name: 'tmax', value: { value: 1 } },
                { name: 'adj.r.squared', value: { value: 0.99917 } },
                { name: 'aucinf.obs', value: { not_calculated: 'no_lambda_z' } },
              ],
              profile: [],
              removed: [],
            },
          },
        },
      ],
    },
  };
}

export interface Fake {
  client: EngineLike;
  calls: Call[];
  /** The scene the next `plot.scene` returns. */
  scene: Scene;
}

export function fake(): Fake {
  const calls: Call[] = [];
  let stale = false;
  const self: Fake = {
    calls,
    scene: sceneOf(logPanel()),
    client: {
      async execute<R>(command: string, params?: unknown) {
        const p = (params ?? {}) as Record<string, unknown>;
        calls.push({ command, params: p });
        const ok = (result: unknown) => ({
          ok: true,
          command: `cmd-${calls.length}`,
          result: result as R,
          error: null,
          seal: { format: 1, sha256: 'ab'.repeat(32), engine: '0.3.2' },
        });
        switch (command) {
          case 'data.import':
            return ok({ worksheet: info() });
          case 'export.table':
            return ok({ columns: ['subject', 'time', 'conc'], rows: TIMES.map((t, i) => ['3', t, CONC[i]]) });
          case 'nca.run': {
            const options = p['options'] as { lambda_z_selection?: { manual?: unknown } } | undefined;
            return ok(view(7, stale ? 'stale' : 'fresh', options?.lambda_z_selection?.manual ?? null));
          }
          case 'plot.scene':
            return ok({ analysis: 7, kind: 'nca', status: 'fresh', subject: '3', scene: self.scene });
          case 'data.set_cell':
            stale = true;
            return ok({ understood: Number(p['value']), stale_analyses: [7], worksheet: info() });
          case 'analysis.get':
            return ok(view(7, 'stale', null, ['input_changed']));
          default:
            return { ok: false, command: null, result: null, error: { code: 'unknown_command', message: `no ${command}` }, seal: null };
        }
      },
    },
  };
  return self;
}
