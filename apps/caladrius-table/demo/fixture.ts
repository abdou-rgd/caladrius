// The recorded fit answer (fit.run, pk3.iv_bolus, subject A) as a table of estimates.
import fixture from '../test/fixtures/fit_pk3_A.json';
import type { CellValue, RowSpec, TableSpec } from '../src/index.ts';

interface Ok {
  parameters: string[];
  values: Record<string, number>;
}

export function fitEstimatesTable(): TableSpec {
  const ok = (fixture.result.outcome as { ok: Ok }).ok;
  const rows: RowSpec[] = ok.parameters.map((p) => {
    const values: Record<string, CellValue> = {
      parameter: p,
      estimate: ok.values['estimate.' + p] ?? null,
      se: ok.values['se.' + p] ?? null,
      cv: ok.values['cv_percent.' + p] ?? null,
      lo: ok.values['ci_lo.' + p] ?? null,
      hi: ok.values['ci_hi.' + p] ?? null,
    };
    const row: RowSpec = { id: p, values };
    // Demo only: the engine's flag sentences come with the answer; this one is made up to show a mark.
    if ((ok.values['cv_percent.' + p] ?? 0) > 30) {
      row.cellFlags = { cv: 'Demo flag: a CV% above 30 means this parameter is poorly determined; check the data and the starting values.' };
    }
    return row;
  });
  return {
    columns: [
      { id: 'parameter', label: 'Parameter', kind: 'text', frozen: true, width: 120 },
      { id: 'estimate', label: 'Estimate', kind: 'number' },
      { id: 'se', label: 'SE', kind: 'number' },
      { id: 'cv', label: 'CV', unit: '%', kind: 'number' },
      { id: 'lo', label: 'CI low', kind: 'number' },
      { id: 'hi', label: 'CI high', kind: 'number' },
    ],
    rows,
  };
}
