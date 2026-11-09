import { exactText, displayText } from './cell.ts';
import type { ColumnSpec, NumberFormatter, TableSpec } from './types.ts';

// Text copied to the clipboard: one header row, tab-separated cells, rows ended by "\n" (no
// final line end), so that it pastes as is into a spreadsheet, R or a text editor. A tab or a
// line end inside a text becomes a space; a missing value is an empty cell.

/** A cell value that cannot break the table's shape. */
export function tsvField(text: string): string {
  return text.replace(/[\t\r\n]+/g, ' ');
}

export function tsvHeader(column: ColumnSpec): string {
  const unit = column.unit;
  return tsvField(unit === undefined || unit === '' ? column.label : column.label + ' (' + unit + ')');
}

/**
 * The table as TSV with a header row. `rowIds` are the rows to copy, in the order given (the
 * order of the view), or `'all'` for every row in the table's order; an id that is not in the
 * table is skipped. `exact` copies every number with `formatter.exact` instead of the displayed
 * text, so that pasting loses no digit.
 */
export function toTsv(
  spec: TableSpec,
  rowIds: string[] | 'all',
  formatter: NumberFormatter,
  exact: boolean,
): string {
  const lines: string[] = [spec.columns.map(tsvHeader).join('\t')];
  let rows = spec.rows;
  if (rowIds !== 'all') {
    const byId = new Map(spec.rows.map((r) => [r.id, r] as const));
    rows = [];
    for (const id of rowIds) {
      const row = byId.get(id);
      if (row !== undefined) rows.push(row);
    }
  }
  for (const row of rows) {
    const cells = spec.columns.map((col) => {
      const value = row.values[col.id];
      return tsvField(exact ? exactText(value, col, formatter) : tsvDisplay(value, col, formatter));
    });
    lines.push(cells.join('\t'));
  }
  return lines.join('\n');
}

function tsvDisplay(
  value: Parameters<typeof displayText>[0],
  col: ColumnSpec,
  formatter: NumberFormatter,
): string {
  // A displayed copy of a missing value is an empty cell, not the dash of the screen.
  if (value === null || value === undefined) return '';
  if (typeof value === 'number' && !Number.isFinite(value)) return '';
  return displayText(value, col, formatter);
}
