// Keyboard navigation of the active cell, as a pure function (the grid pattern's keys).

export interface Active {
  row: number;
  col: number;
}

export const NAV_KEYS: ReadonlySet<string> = new Set([
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Home',
  'End',
  'PageUp',
  'PageDown',
]);

function clamp(x: number, lo: number, hi: number): number {
  if (x < lo) return lo;
  if (x > hi) return hi;
  return x;
}

/**
 * The cell reached from `from` by `key`. Arrows move one cell, Home and End go to the first and
 * last column of the row, Ctrl+Home and Ctrl+End to the first and last cell of the table, Page
 * Up and Page Down move by `page` rows. The result always lies inside the table (or at 0, 0 when
 * the table is empty).
 */
export function moveActive(
  from: Active,
  key: string,
  ctrl: boolean,
  rows: number,
  cols: number,
  page: number,
): Active {
  if (rows <= 0 || cols <= 0) return { row: 0, col: 0 };
  const lastRow = rows - 1;
  const lastCol = cols - 1;
  const step = page > 1 ? page : 1;
  let { row, col } = from;
  switch (key) {
    case 'ArrowUp':
      row -= 1;
      break;
    case 'ArrowDown':
      row += 1;
      break;
    case 'ArrowLeft':
      col -= 1;
      break;
    case 'ArrowRight':
      col += 1;
      break;
    case 'PageUp':
      row -= step;
      break;
    case 'PageDown':
      row += step;
      break;
    case 'Home':
      col = 0;
      if (ctrl) row = 0;
      break;
    case 'End':
      col = lastCol;
      if (ctrl) row = lastRow;
      break;
    default:
      break;
  }
  return { row: clamp(row, 0, lastRow), col: clamp(col, 0, lastCol) };
}
