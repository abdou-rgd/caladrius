// The ARIA grid arithmetic. Under virtualization only a few rows exist in the page, so each one
// says where it is: `aria-rowindex` and `aria-colindex` are 1-based and the header row is row 1.

/** `aria-rowcount`: the rows of the current view plus the header. */
export function ariaRowCount(viewRows: number): number {
  return viewRows + 1;
}

/** `aria-rowindex` of the body row at 0-based position `position` of the view. */
export function ariaRowIndex(position: number): number {
  return position + 2;
}

export const ARIA_HEADER_ROW_INDEX = 1;

/** `aria-colindex` of the visible column at 0-based position `position`. */
export function ariaColIndex(position: number): number {
  return position + 1;
}

/** `aria-sort` of a header, from the table's sort state. */
export function ariaSort(sorted: false | 'asc' | 'desc'): 'ascending' | 'descending' | 'none' {
  if (sorted === 'asc') return 'ascending';
  if (sorted === 'desc') return 'descending';
  return 'none';
}
