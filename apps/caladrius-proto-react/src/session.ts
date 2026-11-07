// The state of the screen and everything that changes it. No React and no DOM here: a component
// reads `getState()` and calls a method; the methods call the engine through the client of the bridge
// (`execute(command, params)`), so a test drives the screen with a stand-in client, and the page
// drives it with the Worker transport or the Tauri one.
//
// Every user action is a command of the registry. A drag of the λz range, a click on a point, a
// candidate row and "Back to automatic" all send `nca.run` with the parameters the egui page sends
// (`crates/caladrius-ui/src/nca.rs`, `NcaPage::params`): worksheet, subject, route, dose, the
// analysis once it exists, and the options as the engine echoed them, with
// `lambda_z_selection.manual` set to `{ "range": { "start", "end" } }` (a range, a candidate row),
// `{ "times": [...] }` (a click) or null (automatic).

import { selectable } from './plotmap.ts';
import type { Fmt } from './results.ts';
import type {
  AnalysisView,
  Candidate,
  DataRow,
  NcaOk,
  NcaOptions,
  Scene,
  SceneAnswer,
  SubjectResult,
  WorksheetInfo,
} from './types.ts';

/** What the session needs of the bridge client (`EngineClient.execute`). */
export interface EngineLike {
  execute<Result = unknown>(
    command: string,
    params?: unknown,
  ): Promise<{ ok: boolean; command: string | null; result: Result | null; error: { code: string; message: string } | null; seal: SealLike | null }>;
}

export interface SealLike {
  format: number;
  sha256: string;
  engine?: string;
  commit?: string;
}

export interface State {
  /** The worksheet is loaded and the first run is done (or failed). */
  ready: boolean;
  datasetName: string;
  info: WorksheetInfo | null;
  rows: DataRow[];
  subject: string;
  analysis: number | null;
  options: NcaOptions | null;
  view: AnalysisView | null;
  /** Seal, command id of the last `nca.run`. */
  seal: SealLike | null;
  command: string | null;
  scene: Scene | null;
  log: boolean;
  /** The sentence of an engine error or of a refusal, shown in place. */
  error: string | null;
  /** Calls to the engine in flight. */
  busy: number;
  /** Why the result is stale, as codes of the engine. */
  staleReasons: string[];
}

export function initialState(): State {
  return {
    ready: false,
    datasetName: '',
    info: null,
    rows: [],
    subject: '',
    analysis: null,
    options: null,
    view: null,
    seal: null,
    command: null,
    scene: null,
    log: true,
    error: null,
    busy: 0,
    staleReasons: [],
  };
}

export function statusOf(s: State): 'none' | 'fresh' | 'stale' {
  const st = s.view?.status.state;
  return st === 'fresh' ? 'fresh' : st === 'stale' ? 'stale' : 'none';
}

export function subjectResult(s: State): SubjectResult | undefined {
  return s.view?.result?.subjects.find((x) => x.subject === s.subject) ?? s.view?.result?.subjects[0];
}

export function okOf(s: State): NcaOk | null {
  const r = subjectResult(s);
  return r && 'ok' in r.outcome ? r.outcome.ok : null;
}

export function selectedCandidate(s: State): Candidate | undefined {
  return okOf(s)?.lambda_z_candidates.find((c) => c.selected);
}

export function isManual(s: State): boolean {
  const manual = s.options?.lambda_z_selection?.manual;
  return manual !== undefined && manual !== null;
}

/** The sentence that says what changed, for a stale result (the wording of the egui page). */
export function staleSentence(reasons: string[]): string {
  const parts = reasons.map((code) => {
    switch (code) {
      case 'input_changed':
        return 'the data changed';
      case 'options_changed':
        return 'the options changed';
      case 'worksheet_missing':
        return 'the worksheet is gone';
      default:
        return 'an input changed';
    }
  });
  const what = parts.length === 0 ? 'an input changed' : parts.join(' and ');
  return `Out of date: ${what} since this result was computed. Run the analysis again to refresh it.`;
}

const TWO_POINTS = 'The terminal phase needs at least 2 points: keep one more selected before removing this one.';

export class Session {
  private state: State = initialState();
  private readonly listeners = new Set<() => void>();
  private readonly client: EngineLike;
  readonly fmt: Fmt;
  private running: Promise<void> | null = null;
  private again = false;

  constructor(client: EngineLike, fmt: Fmt) {
    this.client = client;
    this.fmt = fmt;
  }

  getState = (): State => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  private set(patch: Partial<State>): void {
    this.state = { ...this.state, ...patch };
    for (const l of this.listeners) l();
  }

  private async call<R>(command: string, params: unknown) {
    this.set({ busy: this.state.busy + 1 });
    try {
      return await this.client.execute<R>(command, params);
    } finally {
      this.state = { ...this.state, busy: this.state.busy - 1 };
    }
  }

  // ---- data -------------------------------------------------------------------------------

  /** Imports a CSV (columns subject, time, conc, dose), reads the worksheet and runs the first subject. */
  async loadDataset(name: string, csv: string, subject?: string): Promise<void> {
    const imported = await this.call<{ worksheet: WorksheetInfo }>('data.import', { name, csv });
    if (!imported.ok || !imported.result) {
      this.set({ error: imported.error?.message ?? 'The engine could not read the data.', ready: true });
      return;
    }
    const info = imported.result.worksheet;
    const table = await this.call<{ columns: string[]; rows: unknown[][] }>('export.table', { table: 'worksheet', worksheet: info.id });
    if (!table.ok || !table.result) {
      this.set({ error: table.error?.message ?? 'The engine could not list the data.', ready: true });
      return;
    }
    const rows = readRows(info, table.result.columns, table.result.rows);
    this.set({
      datasetName: name,
      info,
      rows,
      subject: subject ?? info.subjects[0] ?? '',
      analysis: null,
      options: null,
      view: null,
      scene: null,
      seal: null,
      command: null,
      staleReasons: [],
      error: null,
    });
    await this.run();
    this.set({ ready: true });
  }

  /** A cell edit: the worksheet changes, and the analysis that read it is stale until it runs again. */
  async editCell(row: DataRow, role: 'time' | 'concentration', text: string): Promise<void> {
    const info = this.state.info;
    const column = info?.columns.find((c) => c.role === role)?.name;
    if (!info || !column) return;
    const answer = await this.call<{ understood: number | null; stale_analyses: number[]; worksheet: WorksheetInfo }>('data.set_cell', {
      worksheet: info.id,
      column,
      row: row.index,
      value: text,
    });
    if (!answer.ok || !answer.result) {
      this.set({ error: answer.error?.message ?? 'The engine refused the value.', rows: [...this.state.rows] });
      return;
    }
    const understood = answer.result.understood;
    const rows = this.state.rows.map((r) =>
      r.index !== row.index ? r : role === 'time' ? { ...r, time: understood ?? r.time } : { ...r, conc: understood },
    );
    this.set({ rows, info: answer.result.worksheet, error: null });
    const analysis = this.state.analysis;
    if (analysis !== null && answer.result.stale_analyses.includes(analysis)) {
      const got = await this.call<AnalysisView>('analysis.get', { analysis });
      if (got.ok && got.result) {
        this.set({ view: got.result, staleReasons: reasonsOf(got.result) });
      }
    }
  }

  // ---- the analysis -----------------------------------------------------------------------

  /** The parameters of `nca.run` for what the screen holds (the egui page's `NcaPage::params`). */
  runParams(): Record<string, unknown> {
    const s = this.state;
    const params: Record<string, unknown> = { worksheet: s.info?.id ?? 0, subject: s.subject, route: 'extravascular', dose: null };
    if (s.analysis !== null) params['analysis'] = s.analysis;
    if (s.options !== null) params['options'] = s.options;
    return params;
  }

  /** Runs `nca.run` and draws its scene. Calls that arrive while one is in flight run once more, with the latest choice. */
  run(): Promise<void> {
    if (this.running) {
      this.again = true;
      return this.running;
    }
    const loop = async () => {
      do {
        this.again = false;
        await this.runOnce();
      } while (this.again);
    };
    this.running = loop().finally(() => {
      this.running = null;
    });
    return this.running;
  }

  private async runOnce(): Promise<void> {
    const answer = await this.call<AnalysisView>('nca.run', this.runParams());
    if (!answer.ok || !answer.result) {
      this.set({ error: answer.error?.message ?? 'The engine gave no result.' });
      return;
    }
    const view = answer.result;
    const options = view.spec.options ?? this.state.options;
    const sceneOf = await this.fetchScene(view.id, view);
    this.set({
      analysis: view.id,
      options,
      view,
      seal: answer.seal,
      command: answer.command,
      scene: sceneOf.scene,
      error: sceneOf.error,
      staleReasons: reasonsOf(view),
    });
  }

  private async fetchScene(analysis: number, view: AnalysisView): Promise<{ scene: Scene | null; error: string | null }> {
    const subject = view.result?.subjects.find((x) => x.subject === this.state.subject) ?? view.result?.subjects[0];
    if (!subject) return { scene: null, error: null };
    if ('error' in subject.outcome) return { scene: null, error: subject.outcome.error };
    const answer = await this.call<SceneAnswer>('plot.scene', { analysis, subject: this.state.subject, log: this.state.log });
    if (!answer.ok || !answer.result) return { scene: null, error: answer.error?.message ?? 'The engine could not draw the plot.' };
    return { scene: answer.result.scene, error: null };
  }

  /** Semi-log or linear: the scene is asked again; the analysis does not run. */
  async setLog(log: boolean): Promise<void> {
    this.set({ log });
    const { analysis, view } = this.state;
    if (analysis === null || !view) return;
    const sceneOf = await this.fetchScene(analysis, view);
    this.set({ scene: sceneOf.scene, error: sceneOf.error });
  }

  /** A new subject: the hand-picked terminal phase belongs to one subject, so it is forgotten. */
  async selectSubject(subject: string): Promise<void> {
    this.set({ subject, options: withManual(this.state.options, null) });
    await this.run();
  }

  // ---- the terminal phase -----------------------------------------------------------------

  /** The samples of the profile a person can pick, from the scene. */
  samples(): number[] {
    const panel = this.state.scene?.panels[0];
    return panel ? selectable(panel).map((p) => p.x) : [];
  }

  /** The times of the points used for λz, from the scene (the series `lambda_z_used`). */
  usedTimes(): number[] {
    const panel = this.state.scene?.panels[0];
    const used = panel?.series.find((s) => s.role === 'lambda_z_used');
    return used ? used.points.map((p) => p.x) : [];
  }

  /** The two ends of the terminal phase on the plot. */
  range(): { start: number; end: number } | null {
    const used = this.usedTimes();
    if (used.length === 0) return null;
    return { start: Math.min(...used), end: Math.max(...used) };
  }

  /** A range of the terminal phase, chosen by hand. Refused with a sentence when it holds fewer than 2 samples. */
  setRange(start: number, end: number): Promise<void> {
    const [a, b] = start <= end ? [start, end] : [end, start];
    if (this.samples().filter((t) => t >= a && t <= b).length < 2) {
      this.set({ error: TWO_POINTS });
      return Promise.resolve();
    }
    this.set({ options: withManual(this.state.options, { range: { start: a, end: b } }) });
    return this.run();
  }

  /** The handler of the start handle of the plot (a pointer drag, a key, the measure). */
  setRangeStart(time: number): Promise<void> {
    const range = this.range();
    return this.setRange(time, range?.end ?? time);
  }

  /** The handler of the end handle. */
  setRangeEnd(time: number): Promise<void> {
    const range = this.range();
    return this.setRange(range?.start ?? time, time);
  }

  /** A click on the sample at `time`: it joins the hand-picked terminal phase or leaves it. */
  togglePoint(time: number): Promise<void> {
    const used = this.usedTimes();
    const next = used.includes(time) ? used.filter((t) => t !== time) : [...used, time].sort((a, b) => a - b);
    if (next.length < 2) {
      this.set({ error: TWO_POINTS });
      return Promise.resolve();
    }
    this.set({ options: withManual(this.state.options, { times: next }) });
    return this.run();
  }

  /** A row of the candidate list. */
  pickCandidate(candidate: Candidate): Promise<void> {
    this.set({ options: withManual(this.state.options, { range: { start: candidate.time_first, end: candidate.time_last } }) });
    return this.run();
  }

  backToAutomatic(): Promise<void> {
    this.set({ options: withManual(this.state.options, null) });
    return this.run();
  }
}

function reasonsOf(view: AnalysisView): string[] {
  return view.status.state === 'stale' ? (view.status.reasons ?? []).map((r) => r.code) : [];
}

/** The options with `lambda_z_selection.manual` replaced; the other options are kept as the engine echoed them. */
export type Manual = { times: number[] } | { range: { start: number; end: number } } | null;

export function withManual(options: NcaOptions | null, manual: Manual): NcaOptions {
  const base = options ?? {};
  return { ...base, lambda_z_selection: { ...(base.lambda_z_selection ?? {}), manual } };
}

/** Reads the rows of `export.table` into the rows the table shows. */
export function readRows(info: WorksheetInfo, columns: string[], rows: unknown[][]): DataRow[] {
  const at = (role: string) => columns.indexOf(info.columns.find((c) => c.role === role)?.name ?? '');
  const subject = at('subject');
  const time = at('time');
  const conc = at('concentration');
  const out: DataRow[] = [];
  rows.forEach((row, index) => {
    const t = row[time];
    const c = row[conc];
    out.push({
      index,
      subject: subject >= 0 ? String(row[subject]) : (info.subjects[0] ?? ''),
      time: typeof t === 'number' ? t : Number.NaN,
      conc: typeof c === 'number' ? c : null,
    });
  });
  return out;
}
