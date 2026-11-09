// The public description of a table. The values are the engine's answers as they are (numbers
// as f64, `null` for absent): nothing in this package computes, rounds or formats a number.

export type CellValue = number | string | null;

export interface ColumnSpec {
  id: string;
  label: string;
  unit?: string;
  kind: 'number' | 'text' | 'flag';
  /** Width in pixels; the theme's cell width when absent. */
  width?: number;
  /** Pinned on the left, whatever the scroll. */
  frozen?: boolean;
}

export interface RowSpec {
  id: string;
  values: Record<string, CellValue>;
  /** The engine's sentence for a flagged row (shown as a tooltip on the mark). */
  flag?: string;
  /** The engine's sentence for a flagged cell, by column id. */
  cellFlags?: Record<string, string>;
}

export interface TableSpec {
  columns: ColumnSpec[];
  rows: RowSpec[];
}

/** What the table needs from the bridge's `Formatter`; the package never imports the bridge. */
export interface NumberFormatter {
  /** The text shown for a finite number (digits and decimal mark chosen by the user). */
  number(x: number): string;
  /** The shortest text that reads back as the same value. */
  exact(x: number): string;
}

export interface SortKey {
  id: string;
  desc: boolean;
}
