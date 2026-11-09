import type { NumberFormatter, RowSpec, TableSpec } from '../src/types.ts';

/** A formatter that marks every number it formats, and counts its calls. */
export function markFormatter(comma = false): NumberFormatter & { calls: number } {
  const f = {
    calls: 0,
    number(x: number): string {
      f.calls += 1;
      const t = '⟨' + x + '⟩';
      return comma ? t.replace('.', ',') : t;
    },
    exact(x: number): string {
      f.calls += 1;
      return '⟪' + x + '⟫';
    },
  };
  return f;
}

export function table(
  rows: Array<Record<string, number | string | null>>,
  kinds: Record<string, 'number' | 'text' | 'flag'> = {},
): TableSpec {
  const ids = Object.keys(rows[0] ?? { x: 0 });
  return {
    columns: ids.map((id) => ({ id, label: id, kind: kinds[id] ?? 'number' })),
    rows: rows.map((values, i): RowSpec => ({ id: 'r' + i, values })),
  };
}

/** The ids of the rows of a permutation, in order. */
export function ids(spec: TableSpec, order: ArrayLike<number>): string[] {
  return Array.from(order, (i) => spec.rows[i]?.id ?? '?');
}
