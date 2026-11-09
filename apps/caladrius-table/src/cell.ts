import type { CellValue, ColumnSpec, NumberFormatter } from './types.ts';

/** What a missing or non-finite value looks like: the same dash as the engine's formatter. */
export const DASH = '-';

/** The text of a cell as the user reads it. Every number goes through the formatter. */
export function displayText(
  value: CellValue | undefined,
  column: ColumnSpec,
  formatter: NumberFormatter,
): string {
  if (value === null || value === undefined) return DASH;
  if (typeof value === 'number') {
    return Number.isFinite(value) ? formatter.number(value) : DASH;
  }
  return value;
}

/** The text of a cell for a copy that must lose no digit; empty when there is no finite value. */
export function exactText(
  value: CellValue | undefined,
  column: ColumnSpec,
  formatter: NumberFormatter,
): string {
  if (value === null || value === undefined) return '';
  if (typeof value === 'number') {
    return Number.isFinite(value) ? formatter.exact(value) : '';
  }
  return value;
}
