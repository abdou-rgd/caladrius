// The answer of `fit.run` as tables for `@caladrius/table`. Every value cell is the raw number of the
// engine's `outcome.ok.values` (keys `estimate.<name>`, `se.<name>`, `cv_percent.<name>`,
// `ci_lo.<name>`, `ci_hi.<name>`); nothing is computed or rounded here, and which cell is flagged is
// the engine's flags alone.

import type { CellValue, ColumnSpec, RowSpec, TableSpec } from '@caladrius/table';
import type { FitFlag, FitOk } from '../session/types.ts';

export interface ParameterText {
  label: string;
  unit?: string | undefined;
}

/** The engine's flag sentences by cell: `<column id>` of the parameter it concerns. */
export function flaggedCells(
  flags: readonly FitFlag[],
  messages: readonly string[],
  words: (code: string) => string,
): Map<string, Record<string, string>> {
  const out = new Map<string, Record<string, string>>();
  // The sentences are in the order of the flags; when the lists do not pair up, the code in words is the sentence.
  const paired = messages.length === flags.length;
  flags.forEach((flag, index) => {
    const sentence = paired ? (messages[index] ?? words(flag.code)) : words(flag.code);
    const add = (parameter: string | undefined, columns: string[]): void => {
      if (parameter === undefined) return;
      const cells = out.get(parameter) ?? {};
      for (const column of columns) cells[column] = sentence;
      out.set(parameter, cells);
    };
    switch (flag.code) {
      case 'high_cv':
        add(flag.parameter, ['cv']);
        break;
      case 'interval_contains_zero':
        add(flag.parameter, ['ci_lo', 'ci_hi']);
        break;
      case 'at_bound':
        add(flag.parameter, ['estimate']);
        break;
      case 'high_correlation':
        add(flag.first, ['estimate']);
        add(flag.second, ['estimate']);
        break;
      default:
        break;
    }
  });
  return out;
}

const ESTIMATE_COLUMNS: ColumnSpec[] = [
  { id: 'parameter', label: 'Parameter', kind: 'text', width: 92, frozen: true },
  { id: 'unit', label: 'Unit', kind: 'text', width: 56 },
  { id: 'estimate', label: 'Estimate', kind: 'number', width: 84 },
  { id: 'se', label: 'SE', kind: 'number', width: 84 },
  { id: 'cv', label: 'CV%', kind: 'number', width: 74 },
  { id: 'ci_lo', label: 'CI low', kind: 'number', width: 84 },
  { id: 'ci_hi', label: 'CI high', kind: 'number', width: 84 },
];

function value(ok: FitOk, key: string): CellValue {
  const x = ok.values[key];
  return typeof x === 'number' ? x : null;
}

/**
 * The estimates table: one row per fitted parameter, in `order` (the model's parameter order) first, then
 * the rest in the engine's order. `labels` gives the label and unit of a name.
 */
export function estimatesSpec(
  ok: FitOk,
  labels: (name: string) => ParameterText,
  flagged: Map<string, Record<string, string>> = new Map(),
  order: readonly string[] = [],
): TableSpec {
  const known = ok.parameters.filter((p) => order.includes(p)).sort((a, b) => order.indexOf(a) - order.indexOf(b));
  const names = [...known, ...ok.parameters.filter((p) => !order.includes(p))];
  const rows: RowSpec[] = names.map((name) => {
    const text = labels(name);
    const row: RowSpec = {
      id: name,
      values: {
        parameter: text.label,
        unit: text.unit ?? null,
        estimate: value(ok, `estimate.${name}`),
        se: value(ok, `se.${name}`),
        cv: value(ok, `cv_percent.${name}`),
        ci_lo: value(ok, `ci_lo.${name}`),
        ci_hi: value(ok, `ci_hi.${name}`),
      },
    };
    const cells = flagged.get(name);
    if (cells !== undefined) row.cellFlags = cells;
    return row;
  });
  return { columns: ESTIMATE_COLUMNS, rows };
}

/** The goodness-of-fit quantities in the order shown, with their names for a person. */
const DIAGNOSTICS: [string, string][] = [
  ['wrss', 'Weighted sum of squares'],
  ['s', 'Residual standard deviation S'],
  ['df', 'Degrees of freedom'],
  ['n', 'Observations'],
  ['p', 'Parameters'],
  ['corr_obs_pred', 'Correlation observed/predicted'],
  ['aic', 'AIC'],
  ['sbc', 'SBC'],
  ['condition_number', 'Condition number'],
  ['kappa_jacobian', 'Condition number of the Jacobian'],
  ['ss_weighted', 'Weighted sum of squares of the data'],
  ['ss_corrected', 'Corrected sum of squares'],
];

/** The scalar values of the fit (the keys with no `.`): the known ones in order, then any other. */
export function diagnosticsSpec(ok: FitOk, words: (code: string) => string = (c) => c): TableSpec {
  const scalars = Object.keys(ok.values).filter((k) => !k.includes('.'));
  const known = DIAGNOSTICS.filter(([key]) => scalars.includes(key));
  const rest = scalars.filter((k) => !DIAGNOSTICS.some(([key]) => key === k)).sort();
  const rows: RowSpec[] = [
    ...known.map(([key, label]) => ({ id: key, values: { quantity: label, value: value(ok, key) } })),
    ...rest.map((key) => ({ id: key, values: { quantity: words(key), value: value(ok, key) } })),
  ];
  return {
    columns: [
      { id: 'quantity', label: 'Quantity', kind: 'text', width: 215, frozen: true },
      { id: 'value', label: 'Value', kind: 'number', width: 84 },
    ],
    rows,
  };
}
