// The session: the store, and every action of the page. Every engine call goes through `run`, which
// records the answer in the log. The state is plain data (types.ts); this class holds the functions.

import type { Answer, Display } from '@caladrius/bridge-client';
import type { PlotScene } from '@caladrius/plot';
import { COMPARTMENTS, ROUTES, SETS, modelId, offeredModels, parameterNames, routeInfo } from '../model/parameterSets.ts';
import type { Compartments, Route } from '../model/parameterSets.ts';
import { commandInfos, enumOf } from '../model/registry.ts';
import { LatestWins, debounce } from './latestWins.ts';
import type { Debounced, Timers } from './latestWins.ts';
import { fitPlotRequest, fitRunRequest, initialEstimatesRequest, inputKey, missingSentence, parseInitial, rawPlotRequest, sceneRequest, simulateRequest, evaluateRequest } from './requests.ts';
import type { FitInputs, Params } from './requests.ts';
import { Store } from './store.ts';
import type { Curve, FitAnalysis, PlotTab, SessionState, Subject, ThemeChoice, WorksheetInfo } from './types.ts';

/** The engine as the session sees it (the bridge client's `EngineClient` has this). */
export interface Backend {
  execute(command: string, params?: unknown): Promise<Answer>;
}

/** What the session needs of the bridge's `Formatter`. */
export interface Numbers {
  parseNumber(text: string): number | undefined;
  exact(x: number): string;
  setDisplay(display: Display): void;
  plain(text: string): string;
}

export const PREVIEW_DELAY_MS = 150;
const LOG_LIMIT = 300;

export const DEFAULT_DATASETS: { id: string; file: string; label: string; compartments: Compartments; route: Route }[] = [
  { id: 'pk3_iv_bolus', file: 'data/pk3_iv_bolus.csv', label: 'pk3: three-compartment IV bolus', compartments: 3, route: 'iv_bolus' },
  { id: 'theoph', file: 'data/theoph.csv', label: 'Theophylline (oral)', compartments: 1, route: 'oral_1' },
  { id: 'indometh', file: 'data/indometh.csv', label: 'Indomethacin (IV bolus)', compartments: 2, route: 'iv_bolus' },
];

export function initialState(display: Display, theme: ThemeChoice): SessionState {
  return {
    engine: { status: 'loading', message: null, way: null },
    commands: [],
    models: [],
    weightings: ['inv_y2'],
    data: { worksheet: null, sample: null, notes: [], rows: [], busy: false, error: null },
    subject: null,
    input: { compartments: 1, route: 'iv_bolus', set: 'cl', weighting: 'inv_y2', initial: {}, dose: '' },
    rawPlot: { analysis: null, scene: null, xMax: null, error: null },
    preview: { status: 'idle', wrss: null, dose: null, nObservations: null, curve: null, message: null },
    estimate: { busy: false, error: null },
    fit: null,
    fitBusy: false,
    fitError: null,
    plots: { tab: 'data', profile: { analysis: null, scene: null }, residuals: { analysis: null, scene: null }, error: null },
    display,
    theme,
    log: [],
  };
}

interface RunOptions {
  /** A call of the live preview: logged, but hidden unless asked for. */
  quiet?: boolean;
  /** Keep the answer as JSON text in the log. */
  keepJson?: boolean;
}

export class Session {
  readonly store: Store<SessionState>;
  private counter = 0;
  private readonly previewGuard = new LatestWins();
  private readonly rawGuard = new LatestWins();
  private readonly previewDebounce: Debounced;

  private readonly backend: Backend;
  private readonly numbers: Numbers;

  constructor(backend: Backend, numbers: Numbers, display: Display, theme: ThemeChoice, timers?: Timers) {
    this.backend = backend;
    this.numbers = numbers;
    this.store = new Store(initialState(display, theme));
    this.numbers.setDisplay(display);
    this.previewDebounce = debounce(() => void this.runPreview(), PREVIEW_DELAY_MS, timers);
  }

  get state(): SessionState {
    return this.store.getState();
  }

  // ---- the one engine call ------------------------------------------------------------------

  /** Runs a command and records its answer. A transport failure is an answer too (`ok: false`, code `transport`). */
  async run<R = unknown>(command: string, params?: Params, options: RunOptions = {}): Promise<Answer<R>> {
    let answer: Answer<R>;
    try {
      answer = (await this.backend.execute(command, params)) as Answer<R>;
    } catch (cause) {
      const message = `The engine could not be reached: ${cause instanceof Error ? cause.message : String(cause)}`;
      this.store.set((s) => ({ ...s, engine: { ...s.engine, status: 'error', message } }));
      answer = { at: null, command, error: { code: 'transport', message }, ok: false, result: null, seal: null };
    }
    const entry = {
      n: ++this.counter,
      command,
      ok: answer.ok,
      code: answer.error?.code ?? null,
      message: answer.error?.message ?? null,
      seal: answer.seal?.sha256 ?? null,
      quiet: options.quiet === true && answer.ok,
      json: options.keepJson === true ? limit(JSON.stringify(answer, null, 1)) : null,
    };
    this.store.set((s) => ({ ...s, log: [entry, ...s.log].slice(0, LOG_LIMIT) }));
    return answer;
  }

  /** The sentence of a refused command, as it is shown to a person. */
  sentence(answer: Answer): string {
    return this.numbers.plain(answer.error?.message ?? 'The command failed.');
  }

  // ---- engine and registry ------------------------------------------------------------------

  engineReady(describe: unknown, way: string): void {
    const models = enumOf(describe, 'fit.run', 'ModelId');
    const weightings = enumOf(describe, 'fit.run', 'Weighting');
    this.store.set((s) => ({
      ...s,
      engine: { status: 'ready', message: null, way },
      commands: commandInfos(describe),
      models,
      weightings: weightings.length > 0 ? weightings : s.weightings,
      input: this.coherent({ ...s.input, weighting: weightings.includes(s.input.weighting) ? s.input.weighting : (weightings[0] ?? s.input.weighting) }, models),
    }));
  }

  engineFailed(message: string): void {
    this.store.set((s) => ({ ...s, engine: { ...s.engine, status: 'error', message } }));
  }

  // ---- display ------------------------------------------------------------------------------

  setDisplay(display: Display): void {
    this.numbers.setDisplay(display);
    this.store.patch({ display: { ...display } });
  }

  setTheme(theme: ThemeChoice): void {
    this.store.patch({ theme });
  }

  setPlotTab(tab: PlotTab): void {
    this.store.set((s) => ({ ...s, plots: { ...s.plots, tab } }));
  }

  // ---- data ---------------------------------------------------------------------------------

  async importCsv(name: string, csv: string, defaults?: { compartments: Compartments; route: Route; sample: string }): Promise<boolean> {
    this.store.set((s) => ({ ...s, data: { ...s.data, busy: true, error: null } }));
    const answer = await this.run<{ delimiter: string; notes: string[]; worksheet: WorksheetInfo }>('data.import', { name, csv });
    const result = answer.result;
    if (!answer.ok || result === null) {
      this.store.set((s) => ({ ...s, data: { ...s.data, busy: false, error: this.sentence(answer) } }));
      return false;
    }
    const preview = await this.run<{ readings: { delimiter: string; preview: (string | number | null)[][] }[]; recommended: number }>('data.preview', { csv, rows: 200 }, { quiet: true });
    const readings = preview.result?.readings ?? [];
    const reading = readings.find((r) => r.delimiter === result.delimiter) ?? readings[preview.result?.recommended ?? 0] ?? readings[0];
    const worksheet = result.worksheet;
    this.previewGuard.invalidate();
    this.rawGuard.invalidate();
    this.store.set((s) => ({
      ...s,
      data: { worksheet, sample: defaults?.sample ?? null, notes: result.notes, rows: reading?.preview ?? [], busy: false, error: null },
      subject: worksheet.subjects[0] ?? null,
      input: this.coherent(
        { ...s.input, ...(defaults !== undefined ? { compartments: defaults.compartments, route: defaults.route, set: 'cl' } : {}), initial: {}, dose: '' },
        s.models,
      ),
      rawPlot: { analysis: null, scene: null, xMax: null, error: null },
      preview: { status: 'idle', wrss: null, dose: null, nObservations: null, curve: null, message: null },
      estimate: { busy: false, error: null },
      fit: null,
      fitError: null,
      plots: { tab: 'data', profile: { analysis: null, scene: null }, residuals: { analysis: null, scene: null }, error: null },
    }));
    await this.refreshRawPlot();
    return true;
  }

  async selectSubject(subject: Subject): Promise<void> {
    this.store.patch({ subject });
    this.previewGuard.invalidate();
    await this.refreshRawPlot();
  }

  /** The raw plot of the selected subject; the same plot object is updated once it exists. */
  async refreshRawPlot(): Promise<void> {
    const s = this.state;
    const worksheet = s.data.worksheet;
    if (worksheet === null || s.subject === null) return;
    const current = this.rawGuard.begin();
    const made = await this.run<{ id: number }>('plot.xy', rawPlotRequest(worksheet.id, s.subject, s.rawPlot.analysis));
    if (!current()) return;
    if (!made.ok || made.result === null) {
      this.store.set((x) => ({ ...x, rawPlot: { ...x.rawPlot, error: this.sentence(made) } }));
      return;
    }
    const analysis = made.result.id;
    const scene = await this.scene(analysis);
    if (!current()) return;
    if (scene.error !== null || scene.scene === null) {
      this.store.set((x) => ({ ...x, rawPlot: { analysis, scene: null, xMax: null, error: scene.error } }));
      return;
    }
    const panel = scene.scene.panels.find((p) => !p.y.log) ?? scene.scene.panels[0];
    this.store.set((x) => ({ ...x, rawPlot: { analysis, scene: scene.scene, xMax: panel?.x.max ?? null, error: null } }));
    this.schedulePreview();
  }

  private async scene(analysis: number): Promise<{ scene: PlotScene | null; status: string | null; error: string | null }> {
    const answer = await this.run<{ scene: PlotScene; status: string }>('plot.scene', sceneRequest(analysis), { quiet: true });
    if (!answer.ok || answer.result === null) return { scene: null, status: null, error: this.sentence(answer) };
    return { scene: answer.result.scene, status: answer.result.status, error: null };
  }

  // ---- model --------------------------------------------------------------------------------

  /** Brings a model choice back to one the engine lists (a model the engine does not list is not offered). */
  private coherent(input: SessionState['input'], models: readonly string[]): SessionState['input'] {
    if (models.length === 0 || models.includes(modelId(input.compartments, input.route))) return input;
    const offered = offeredModels(models);
    const same = offered.find((o) => o.compartments === input.compartments);
    const pick = same ?? offered[0];
    return pick === undefined ? input : { ...input, compartments: pick.compartments, route: pick.route };
  }

  private changeInput(change: Partial<SessionState['input']>): void {
    this.store.set((s) => ({ ...s, input: this.coherent({ ...s.input, ...change }, s.models) }));
    this.schedulePreview();
  }

  setCompartments(compartments: Compartments): void {
    if (COMPARTMENTS.includes(compartments)) this.changeInput({ compartments });
  }

  setRoute(route: Route): void {
    if (ROUTES.some((r) => r.id === route)) this.changeInput({ route });
  }

  setParameterSet(set: string): void {
    this.changeInput({ set });
  }

  setWeighting(weighting: string): void {
    this.changeInput({ weighting });
  }

  setInitial(name: string, text: string): void {
    this.changeInput({ initial: { ...this.state.input.initial, [name]: text } });
  }

  setDose(dose: string): void {
    this.changeInput({ dose });
  }

  /** The names the current model takes values for. */
  names(): string[] {
    const { compartments, route, set } = this.state.input;
    return parameterNames(compartments, route, set);
  }

  /** The inputs of a fit from what is typed, or the sentence that says what is missing. */
  inputs(): { ok: true; inputs: FitInputs } | { ok: false; message: string | null } {
    const s = this.state;
    if (s.data.worksheet === null || s.subject === null) return { ok: false, message: null };
    const parsed = parseInitial(s.input.initial, this.names(), (t) => this.numbers.parseNumber(t));
    if (!parsed.ok) return { ok: false, message: missingSentence(parsed) };
    let dose: number | undefined;
    if (s.input.dose.trim() !== '') {
      dose = this.numbers.parseNumber(s.input.dose.trim());
      if (dose === undefined) return { ok: false, message: 'The dose is not a number.' };
    }
    return {
      ok: true,
      inputs: { worksheet: s.data.worksheet.id, subject: s.subject, model: modelId(s.input.compartments, s.input.route), initial: parsed.values, weighting: s.input.weighting, dose },
    };
  }

  /** The key of the current inputs, to tell whether the fit on screen is of these. Null when they are not complete. */
  currentKey(): string | null {
    const i = this.inputs();
    return i.ok ? inputKey(i.inputs) : null;
  }

  // ---- live preview -------------------------------------------------------------------------

  schedulePreview(): void {
    this.previewGuard.invalidate();
    this.previewDebounce.call();
  }

  /** Starting WRSS and the starting curve for the typed values; the latest request wins. */
  async runPreview(): Promise<void> {
    const got = this.inputs();
    const idle = (message: string | null): void =>
      this.store.set((s) => ({ ...s, preview: { status: 'idle', wrss: null, dose: null, nObservations: null, curve: null, message } }));
    if (!got.ok) {
      idle(got.message);
      return;
    }
    const { inputs } = got;
    const current = this.previewGuard.begin();
    this.store.set((s) => ({ ...s, preview: { ...s.preview, status: 'pending', message: null } }));
    const evaluated = await this.run<{ dose: number; n_observations: number; wrss: number }>('fit.evaluate', evaluateRequest(inputs), { quiet: true });
    if (!current()) return;
    if (!evaluated.ok || evaluated.result === null) {
      this.store.set((s) => ({ ...s, preview: { status: 'error', wrss: null, dose: null, nObservations: null, curve: null, message: this.sentence(evaluated) } }));
      return;
    }
    const { wrss, dose: dataDose, n_observations: nObservations } = evaluated.result;
    const dose = inputs.dose ?? dataDose;
    const end = this.state.rawPlot.xMax;
    if (end === null) {
      this.store.set((s) => ({ ...s, preview: { status: 'ok', wrss, dose, nObservations, curve: null, message: null } }));
      return;
    }
    const simulated = await this.run<{ times: number[]; conc: number[] }>('model.simulate', simulateRequest(inputs.model, dose, inputs.initial, end), { quiet: true });
    if (!current()) return;
    if (!simulated.ok || simulated.result === null) {
      this.store.set((s) => ({ ...s, preview: { status: 'error', wrss, dose, nObservations, curve: null, message: this.sentence(simulated) } }));
      return;
    }
    const { times, conc } = simulated.result;
    const curve: Curve[] = [];
    times.forEach((x, i) => {
      const y = conc[i];
      if (y !== undefined) curve.push({ x, y });
    });
    this.store.set((s) => ({ ...s, preview: { status: 'ok', wrss, dose, nObservations, curve, message: null } }));
  }

  /** pk1 only: the engine's initial estimates, put in the inputs as exact text. */
  async estimateInitial(): Promise<void> {
    const s = this.state;
    if (s.data.worksheet === null || s.subject === null) return;
    this.store.set((x) => ({ ...x, estimate: { busy: true, error: null } }));
    const typedDose = s.input.dose.trim() === '' ? undefined : this.numbers.parseNumber(s.input.dose.trim());
    const answer = await this.run<{ initial: Record<string, number> }>(
      'fit.initial_estimates',
      initialEstimatesRequest({ worksheet: s.data.worksheet.id, subject: s.subject, model: modelId(s.input.compartments, s.input.route), dose: typedDose }),
    );
    if (!answer.ok || answer.result === null) {
      this.store.set((x) => ({ ...x, estimate: { busy: false, error: this.sentence(answer) } }));
      return;
    }
    // The engine estimates the rate-constant form (v, k, ka): switch to the parameter set it can fill, if the current one cannot be.
    const given = answer.result.initial;
    const { compartments, route } = this.state.input;
    const current = this.state.input.set;
    const fills = (set: string): boolean => parameterNames(compartments, route, set).every((n) => given[n] !== undefined);
    const set = fills(current) ? current : (SETS[compartments].find((p) => fills(p.id))?.id ?? current);
    const initial = { ...this.state.input.initial };
    for (const name of parameterNames(compartments, route, set)) {
      const value = given[name];
      if (value !== undefined) initial[name] = this.numbers.exact(value);
    }
    this.store.set((x) => ({ ...x, estimate: { busy: false, error: null }, input: { ...x.input, set, initial } }));
    this.schedulePreview();
  }

  // ---- fit ----------------------------------------------------------------------------------

  async runFit(): Promise<void> {
    const got = this.inputs();
    if (!got.ok) {
      this.store.patch({ fitError: got.message ?? 'Import data and choose a subject first.' });
      return;
    }
    const { inputs } = got;
    this.previewDebounce.cancel();
    this.store.patch({ fitBusy: true, fitError: null });
    const answer = await this.run<FitAnalysis>('fit.run', fitRunRequest(inputs, this.state.fit?.analysis ?? null));
    if (!answer.ok || answer.result === null) {
      this.store.patch({ fitBusy: false, fitError: this.sentence(answer) });
      return;
    }
    const analysis = answer.result;
    this.store.set((s) => ({
      ...s,
      fitBusy: false,
      fitError: null,
      fit: { analysis: analysis.id, body: analysis.result, key: inputKey(inputs), engineStatus: analysis.status.state, seal: answer.seal, order: this.names(), extravascular: routeInfo(s.input.route).extravascular },
      plots: { ...s.plots, tab: 'fit', error: null },
    }));
    if (analysis.result.outcome.ok === undefined) return;
    await this.fitPlots(analysis.id);
    await this.refreshStatus();
  }

  private async fitPlots(fit: number): Promise<void> {
    const draw = async (view: 'profile' | 'weighted_residuals_time', slot: 'profile' | 'residuals'): Promise<void> => {
      const made = await this.run<{ id: number }>('plot.xy', fitPlotRequest(fit, view, this.state.plots[slot].analysis));
      if (!made.ok || made.result === null) {
        this.store.set((s) => ({ ...s, plots: { ...s.plots, error: this.sentence(made) } }));
        return;
      }
      const analysis = made.result.id;
      const scene = await this.scene(analysis);
      this.store.set((s) => ({ ...s, plots: { ...s.plots, [slot]: { analysis, scene: scene.scene }, error: scene.error ?? s.plots.error } }));
    };
    await draw('profile', 'profile');
    await draw('weighted_residuals_time', 'residuals');
  }

  /** Reads the status of the fit from the project (fresh or stale), as the engine says it. */
  async refreshStatus(): Promise<void> {
    const fit = this.state.fit;
    if (fit === null) return;
    const answer = await this.run<{ analyses: { id: number; status: { state: string } }[] }>('project.describe', {}, { quiet: true });
    const entry = answer.result?.analyses.find((a) => a.id === fit.analysis);
    if (entry === undefined) return;
    this.store.set((s) => (s.fit === null || s.fit.analysis !== fit.analysis ? s : { ...s, fit: { ...s.fit, engineStatus: entry.status.state } }));
  }

  // ---- palette ------------------------------------------------------------------------------

  /** Runs an engine command with no required parameter; its answer is kept in the log. */
  async runCommand(id: string): Promise<void> {
    const answer = await this.run(id, {}, { keepJson: true });
    const info = this.state.commands.find((c) => c.id === id);
    if (answer.ok && info?.mutates === true) await this.refreshStatus();
  }
}

function limit(text: string): string {
  const max = 40_000;
  return text.length > max ? `${text.slice(0, max)}\n... (${text.length - max} more characters)` : text;
}
