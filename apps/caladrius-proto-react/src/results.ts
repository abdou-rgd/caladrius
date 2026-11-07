// The cells of the results table. Every number goes through the formatter, which is the
// `caladrius-format` crate in WebAssembly (the same code the egui page runs): nothing here rounds,
// pads or switches to scientific notation. What this file decides is which parameters are the
// headline, in which order, and which of them carry a flag.

import type { Candidate, Flag, NcaOk, SubjectResult } from './types.ts';

/** The part of the formatter this screen uses (the `Formatter` of the bridge client has all of it). */
export interface Fmt {
  number(x: number): string;
  fitQuality(x: number): string;
  exact(x: number): string;
  parameterLabel(name: string, extravascular: boolean): string;
  parameterUnit(name: string, time: string | undefined, conc: string | undefined, derived?: Record<string, string>): string | undefined;
  withLabels(sentence: string): string;
  words(code: string): string;
}

/** The headline parameters, in the order of the egui page. */
export const SUMMARY: readonly string[] = [
  'cmax',
  'tmax',
  'auclast',
  'aucinf.obs',
  'aucpext.obs',
  'lambda.z',
  'half.life',
  'adj.r.squared',
  'span.ratio',
  'cl.obs',
  'vz.obs',
  'mrt.obs',
];

export interface Units {
  time: string | undefined;
  conc: string | undefined;
  derived: Record<string, string>;
}

export interface Cell {
  key: string;
  label: string;
  /** What is shown in the value cell. */
  text: string;
  unit: string;
  flagged: boolean;
  calculated: boolean;
  /** For a parameter that is not calculated: the sentence of the engine. */
  sentence?: string;
}

/** The parameter of the summary a flag is about (the rule of the egui page). */
export function concerns(flag: Flag): string {
  switch (flag.code) {
    case 'low_adjusted_r_squared':
      return 'adj.r.squared';
    case 'short_span':
      return 'span.ratio';
    case 'few_points':
      return 'lambda.z.n.points';
    case 'high_extrapolation':
      return flag.parameter ?? 'aucpext.obs';
    case 'replaced_points_in_lambda_z':
      return 'lambda.z';
    case 'area_past_tlast':
      return 'auclast';
    default:
      return '';
  }
}

function cell(
  name: string,
  value: { value?: number; not_calculated?: string },
  subject: SubjectResult,
  flagged: Set<string>,
  fmt: Fmt,
  units: Units,
  extravascular: boolean,
): Cell {
  const label = fmt.parameterLabel(name, extravascular);
  if (value.value !== undefined) {
    const shown = name.includes('r.squared') ? fmt.fitQuality(value.value) : fmt.number(value.value);
    return {
      key: name,
      label,
      text: shown,
      unit: fmt.parameterUnit(name, units.time, units.conc, units.derived) ?? '',
      flagged: flagged.has(name),
      calculated: true,
    };
  }
  const reason = fmt.words(value.not_calculated ?? '');
  return {
    key: name,
    label,
    text: 'not calculated',
    unit: reason,
    flagged: false,
    calculated: false,
    sentence: subject.not_calculated_messages?.[name] ?? reason,
  };
}

function flaggedSet(ok: NcaOk): Set<string> {
  return new Set(ok.flags.map(concerns).filter((s) => s !== ''));
}

/** The summary: the headline parameters that the answer holds, each with its flag. */
export function headlineCells(subject: SubjectResult, ok: NcaOk, fmt: Fmt, units: Units, extravascular: boolean): Cell[] {
  const flagged = flaggedSet(ok);
  const byName = new Map(ok.parameters.map((p) => [p.name, p.value]));
  const out: Cell[] = [];
  for (const name of SUMMARY) {
    const value = byName.get(name);
    if (value) out.push(cell(name, value, subject, flagged, fmt, units, extravascular));
  }
  return out;
}

/** "All parameters": every parameter of the answer, under its engine name. */
export function allCells(subject: SubjectResult, ok: NcaOk, fmt: Fmt, units: Units, extravascular: boolean): Cell[] {
  const flagged = flaggedSet(ok);
  return ok.parameters.map((p) => {
    const c = cell(p.name, p.value, subject, flagged, fmt, units, extravascular);
    return { ...c, label: p.name };
  });
}

/** The terminal-phase sentence of the results (numbers from the formatter). */
export function terminalPhaseSentence(selected: Candidate | undefined, manual: boolean, timeUnit: string | undefined, fmt: Fmt): string {
  if (!selected) return 'No terminal phase could be chosen: λz and what depends on it are not calculated.';
  const unit = timeUnit ? ` ${timeUnit}` : '';
  return `Terminal phase: ${fmt.number(selected.n_points)} points from t = ${fmt.number(selected.time_first)} to ${fmt.number(selected.time_last)}${unit}, ${manual ? 'chosen by hand' : 'chosen automatically'}.`;
}

/** Tab-separated text with a header row; the digits are those on screen. */
export function resultsTsv(cells: Cell[]): string {
  const lines = ['Parameter\tValue\tUnit'];
  for (const c of cells) lines.push(`${c.label}\t${c.text}\t${c.unit}`);
  return lines.join('\n');
}
