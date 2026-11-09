// The table's row processing, owned here and not by the table library: the formatted-text
// cache, the sort permutation and the global filter.
//
// - Formatted text: one string array per column, keyed by (column id, rows, display key). It is
//   filled for the cells that are drawn, and completely the first time a filter or a copy needs
//   the column. It is dropped when the rows or the display key change, never otherwise.
// - Sorting works on raw values (`sort.ts`) and never calls the formatter.
// - The global filter scans the cache only: once a column is complete, no filter formats again.

import { displayText } from './cell.ts';
import { applyMask, filterMask, queryWords } from './filter.ts';
import { buildSortKeys, identityOrder, sortPermutation } from './sort.ts';
import type { ColumnSpec, NumberFormatter, RowSpec, SortKey, TableSpec } from './types.ts';

interface ColumnText {
  text: (string | undefined)[];
  complete: boolean;
  lower: string[] | null;
}

export class RowProcessor {
  private spec: TableSpec = { columns: [], rows: [] };
  private formatter: NumberFormatter | null = null;
  private displayKey = '';
  private texts = new Map<string, ColumnText>();
  private sortKeys = new Map<string, Float64Array>();
  private sortCache: { key: string; order: Uint32Array } | null = null;
  private maskCache: { key: string; mask: Uint8Array } | null = null;
  private viewCache: { key: string; order: Uint32Array } | null = null;
  private columnIndex = new Map<string, ColumnSpec>();

  /** Number of cell texts produced by the formatter since the creation (for the tests). */
  formatted = 0;

  /**
   * Gives the processor its input. Nothing is dropped unless the rows (by identity), the display
   * key or the formatter changed; the sort keys follow the rows and the columns only.
   */
  configure(spec: TableSpec, formatter: NumberFormatter, displayKey: string): void {
    const rowsChanged = spec.rows !== this.spec.rows;
    const columnsChanged = spec.columns !== this.spec.columns;
    const textChanged = rowsChanged || columnsChanged || displayKey !== this.displayKey || formatter !== this.formatter;
    this.spec = spec;
    this.formatter = formatter;
    this.displayKey = displayKey;
    if (columnsChanged) this.columnIndex = new Map(spec.columns.map((c) => [c.id, c] as const));
    if (textChanged) {
      this.texts = new Map();
      this.maskCache = null;
      this.viewCache = null;
    }
    if (rowsChanged || columnsChanged) {
      this.sortKeys = new Map();
      this.sortCache = null;
      this.viewCache = null;
    }
  }

  get rowCount(): number {
    return this.spec.rows.length;
  }

  private column(id: string): ColumnText {
    let col = this.texts.get(id);
    if (col === undefined) {
      col = { text: new Array<string | undefined>(this.spec.rows.length).fill(undefined), complete: false, lower: null };
      this.texts.set(id, col);
    }
    return col;
  }

  private format(row: RowSpec, spec: ColumnSpec): string {
    this.formatted += 1;
    const formatter = this.formatter;
    if (formatter === null) return '';
    return displayText(row.values[spec.id], spec, formatter);
  }

  /** The displayed text of a cell, from the cache (filled on first use). */
  cellText(rowPosition: number, columnId: string): string {
    const spec = this.columnIndex.get(columnId);
    const row = this.spec.rows[rowPosition];
    if (spec === undefined || row === undefined) return '';
    const col = this.column(columnId);
    const cached = col.text[rowPosition];
    if (cached !== undefined) return cached;
    const text = this.format(row, spec);
    col.text[rowPosition] = text;
    return text;
  }

  /** Fills a whole column of the cache; a column already complete costs nothing. */
  private complete(columnId: string): ColumnText {
    const col = this.column(columnId);
    if (col.complete) return col;
    const spec = this.columnIndex.get(columnId);
    if (spec !== undefined) {
      const rows = this.spec.rows;
      for (let i = 0; i < rows.length; i += 1) {
        if (col.text[i] !== undefined) continue;
        const row = rows[i];
        if (row !== undefined) col.text[i] = this.format(row, spec);
      }
    }
    col.complete = true;
    col.lower = null;
    return col;
  }

  private lowerOf(columnId: string): string[] {
    const col = this.complete(columnId);
    if (col.lower === null) col.lower = col.text.map((t) => (t ?? '').toLowerCase());
    return col.lower;
  }

  /**
   * Prepares one more column for the filter (its whole text, lower-cased) and tells whether
   * columns are still to prepare. The component calls it in idle time once the user shows
   * interest in filtering, so that the first filter does not pay for every column at once.
   */
  warmStep(columnIds: readonly string[]): boolean {
    const pending = columnIds.filter((id) => {
      const col = this.texts.get(id);
      return this.columnIndex.has(id) && (col === undefined || !col.complete || col.lower === null);
    });
    const next = pending[0];
    if (next !== undefined) this.lowerOf(next);
    return pending.length > 1;
  }

  private sortKeysOf(spec: ColumnSpec): Float64Array {
    let keys = this.sortKeys.get(spec.id);
    if (keys === undefined) {
      keys = buildSortKeys(this.spec.rows, spec);
      this.sortKeys.set(spec.id, keys);
    }
    return keys;
  }

  /** The row positions in the order of `sorting` (unknown columns are ignored). */
  sorted(sorting: readonly SortKey[]): Uint32Array {
    const usable = sorting.filter((s) => this.columnIndex.has(s.id));
    const key = usable.map((s) => (s.desc ? '-' : '+') + s.id).join('\u0000');
    if (this.sortCache !== null && this.sortCache.key === key) return this.sortCache.order;
    const columns = usable.flatMap((s) => {
      const spec = this.columnIndex.get(s.id);
      return spec === undefined ? [] : [{ keys: this.sortKeysOf(spec), desc: s.desc }];
    });
    const order = columns.length === 0 ? identityOrder(this.rowCount) : sortPermutation(this.rowCount, columns);
    this.sortCache = { key, order };
    return order;
  }

  /**
   * The rows of the view: sorted by `sorting`, then filtered by the words of `query` in the
   * displayed text of `columnIds`. A second query on the same columns formats nothing.
   */
  view(sorting: readonly SortKey[], query: string, columnIds: readonly string[]): Uint32Array {
    const order = this.sorted(sorting);
    const words = queryWords(query);
    if (words.length === 0) return order;
    const ids = columnIds.filter((id) => this.columnIndex.has(id));
    // The scan depends on the words and the columns only (not on the sort): a new sort reuses it.
    const maskKey = words.join('\u0000') + '\u0001' + ids.join('\u0000');
    if (this.maskCache === null || this.maskCache.key !== maskKey) {
      const mask = filterMask(this.rowCount, words, ids.map((id) => this.lowerOf(id)));
      this.maskCache = { key: maskKey, mask };
    }
    const key = maskKey + '\u0001' + (this.sortCache?.key ?? '');
    if (this.viewCache !== null && this.viewCache.key === key) return this.viewCache.order;
    const result = applyMask(order, this.maskCache.mask);
    this.viewCache = { key, order: result };
    return result;
  }

  /** The id of the row at a position of the table's own order. */
  rowId(position: number): string | undefined {
    return this.spec.rows[position]?.id;
  }
}
