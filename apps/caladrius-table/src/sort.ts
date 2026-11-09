// Sorting on raw values. A column becomes a `Float64Array` of keys (a number, or the rank of a
// text among the sorted distinct texts), and the order is a `Uint32Array` permutation of the row
// positions: nothing is formatted, nothing is moved. A value that is not a finite number (null,
// NaN, ±Infinity, a text in a number column) is "missing": it goes last whatever the direction.

import type { ColumnSpec, RowSpec } from './types.ts';

const MISSING = Number.NaN;

// One fixed locale and option set: the same order on every machine and in every webview.
const TEXT_COLLATOR = new Intl.Collator('en', { numeric: true, sensitivity: 'variant' });

/** The sort keys of one column. */
export function buildSortKeys(rows: readonly RowSpec[], column: ColumnSpec): Float64Array {
  const keys = new Float64Array(rows.length);
  if (column.kind === 'number') {
    for (let i = 0; i < rows.length; i += 1) {
      const v = rows[i]?.values[column.id];
      keys[i] = typeof v === 'number' && Number.isFinite(v) ? v : MISSING;
    }
    return keys;
  }
  // Text: the rank of the text among the distinct texts, so a comparison is one number.
  const distinct = new Set<string>();
  for (const row of rows) {
    const v = row.values[column.id];
    if (typeof v === 'string') distinct.add(v);
  }
  const sorted = Array.from(distinct).sort(TEXT_COLLATOR.compare);
  const rank = new Map<string, number>();
  sorted.forEach((s, r) => rank.set(s, r));
  for (let i = 0; i < rows.length; i += 1) {
    const v = rows[i]?.values[column.id];
    const r = typeof v === 'string' ? rank.get(v) : undefined;
    keys[i] = r === undefined ? MISSING : r;
  }
  return keys;
}

export interface SortColumn {
  keys: Float64Array;
  desc: boolean;
}

/** The identity order. */
export function identityOrder(n: number): Uint32Array {
  const order = new Uint32Array(n);
  for (let i = 0; i < n; i += 1) order[i] = i;
  return order;
}

/**
 * The row positions in sorted order: by the first column, ties by the next, remaining ties in
 * the original order (so the result is deterministic and a re-sort never shuffles equal rows).
 */
export function sortPermutation(n: number, columns: readonly SortColumn[]): Uint32Array {
  const order = identityOrder(n);
  if (columns.length === 0) return order;
  const count = columns.length;
  const compare = (a: number, b: number): number => {
    for (let c = 0; c < count; c += 1) {
      const col = columns[c];
      if (col === undefined) continue;
      const x = col.keys[a] ?? MISSING;
      const y = col.keys[b] ?? MISSING;
      const xMissing = x !== x;
      const yMissing = y !== y;
      if (xMissing || yMissing) {
        if (xMissing && yMissing) continue;
        return xMissing ? 1 : -1;
      }
      if (x !== y) {
        const asc = x < y ? -1 : 1;
        return col.desc ? -asc : asc;
      }
    }
    return a - b;
  };
  order.sort(compare);
  return order;
}
