// The requests of the page, as pure functions: what is sent to the engine for a given input.
// Nothing here computes a result; a typed number is parsed by the engine's `parseNumber`, given in.

import type { Subject } from './types.ts';

export interface FitInputs {
  worksheet: number;
  subject: Subject;
  model: string;
  initial: Record<string, number>;
  weighting: string;
  /** Typed dose; absent: the dose of the data. */
  dose?: number | undefined;
}

export type Params = Record<string, unknown>;

export function evaluateRequest(i: FitInputs): Params {
  const p: Params = { worksheet: i.worksheet, subject: i.subject, model: i.model, initial: i.initial, weighting: i.weighting };
  if (i.dose !== undefined) p['dose'] = i.dose;
  return p;
}

/** `fit.run`; with `analysis` the same analysis object is updated instead of a new one made. */
export function fitRunRequest(i: FitInputs, analysis?: number | null): Params {
  const p = evaluateRequest(i);
  if (analysis !== undefined && analysis !== null) p['analysis'] = analysis;
  return p;
}

export function initialEstimatesRequest(i: Pick<FitInputs, 'worksheet' | 'subject' | 'model' | 'dose'>): Params {
  const p: Params = { worksheet: i.worksheet, subject: i.subject, model: i.model };
  if (i.dose !== undefined) p['dose'] = i.dose;
  return p;
}

export function simulateRequest(model: string, dose: number, params: Record<string, number>, end: number, points = 200): Params {
  return { model, dose, params, grid: { start: 0, end, points } };
}

/** The raw plot of one subject, both panels; with `analysis` the plot is updated, not made again. */
export function rawPlotRequest(worksheet: number, subject: Subject, analysis?: number | null): Params {
  const p: Params = { worksheet, layout: 'overlay', scale: 'both', subjects: [subject] };
  if (analysis !== undefined && analysis !== null) p['analysis'] = analysis;
  return p;
}

export type FitView = 'profile' | 'weighted_residuals_time';

export function fitPlotRequest(fit: number, view: FitView, analysis?: number | null): Params {
  const p: Params = { fit, view, scale: view === 'profile' ? 'both' : 'linear' };
  if (analysis !== undefined && analysis !== null) p['analysis'] = analysis;
  return p;
}

export function sceneRequest(analysis: number): Params {
  return { analysis };
}

export type ParsedInitial =
  | { ok: true; values: Record<string, number> }
  | { ok: false; missing: string[]; invalid: string[] };

/** Reads what was typed for each parameter. `parse` is the engine's `Formatter.parseNumber`. */
export function parseInitial(typed: Record<string, string>, names: readonly string[], parse: (text: string) => number | undefined): ParsedInitial {
  const values: Record<string, number> = {};
  const missing: string[] = [];
  const invalid: string[] = [];
  for (const name of names) {
    const text = (typed[name] ?? '').trim();
    if (text === '') {
      missing.push(name);
      continue;
    }
    const x = parse(text);
    if (x === undefined || !Number.isFinite(x)) invalid.push(name);
    else values[name] = x;
  }
  return missing.length === 0 && invalid.length === 0 ? { ok: true, values } : { ok: false, missing, invalid };
}

/** A key of everything a fit depends on: two equal keys, the same fit. Used to tell a stale result. */
export function inputKey(i: FitInputs): string {
  const initial = Object.keys(i.initial)
    .sort()
    .map((k) => [k, i.initial[k]]);
  return JSON.stringify([i.worksheet, i.subject, i.model, i.weighting, i.dose ?? null, initial]);
}

/** The sentence that says what to type when the initial estimates are not all numbers yet. */
export function missingSentence(parsed: Extract<ParsedInitial, { ok: false }>): string {
  const parts: string[] = [];
  if (parsed.missing.length > 0) parts.push(`enter a value for ${parsed.missing.join(', ')}`);
  if (parsed.invalid.length > 0) parts.push(`${parsed.invalid.join(', ')} ${parsed.invalid.length === 1 ? 'is' : 'are'} not a number`);
  const text = parts.join('; ');
  return text.charAt(0).toUpperCase() + text.slice(1);
}
