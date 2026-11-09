// A synthetic table for the measures: 100,000 rows x 20 columns. Demo and tests only: no number
// of this table exists in the app. A small generator makes the values the same on every run.
import type { CellValue, ColumnSpec, RowSpec, TableSpec } from '../src/index.ts';

export function syntheticTable(rows: number, cols: number): TableSpec {
  let state = 12345;
  const next = (): number => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
  const columns: ColumnSpec[] = [
    { id: 'subject', label: 'Subject', kind: 'text', frozen: true, width: 110 },
    { id: 'group', label: 'Group', kind: 'text', width: 100 },
  ];
  for (let j = 2; j < cols; j += 1) {
    columns.push({ id: 'p' + j, label: 'Parameter ' + j, unit: j % 2 === 0 ? 'mg/L' : 'h', kind: 'number' });
  }
  const groups = ['placebo', 'low dose', 'high dose', 'reference'];
  const out: RowSpec[] = [];
  for (let i = 0; i < rows; i += 1) {
    const values: Record<string, CellValue> = {
      subject: 'S' + String(i + 1).padStart(6, '0'),
      group: groups[i % groups.length] ?? '',
    };
    for (let j = 2; j < cols; j += 1) {
      const r = next();
      let v: CellValue = Math.exp((next() - 0.5) * 20) * (r < 0.5 ? 1 : -1);
      if (r > 0.995) v = null;
      else if (r > 0.99) v = Number.NaN;
      else if (r > 0.985) v = Number.POSITIVE_INFINITY;
      values['p' + j] = v;
    }
    const row: RowSpec = { id: 'r' + i, values };
    if (i % 5000 === 17) row.flag = 'Demo flag: this row is flagged by the demo, not by the engine.';
    out.push(row);
  }
  return { columns, rows: out };
}
