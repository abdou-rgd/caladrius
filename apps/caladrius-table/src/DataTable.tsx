import {
  columnPinningFeature,
  columnSizingFeature,
  columnVisibilityFeature,
  createColumnHelper,
  rowSortingFeature,
  tableFeatures,
  useTable,
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  useCallback,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';
import { ARIA_HEADER_ROW_INDEX, ariaColIndex, ariaRowCount, ariaRowIndex, ariaSort } from './aria.ts';
import { TableBoundary } from './boundary.tsx';
import { NAV_KEYS, moveActive, type Active } from './nav.ts';
import { RowProcessor } from './processor.ts';
import { toTsv } from './tsv.ts';
import type { ColumnSpec, NumberFormatter, RowSpec, SortKey, TableSpec } from './types.ts';

// TanStack Table v9 owns the column model and its state (sorting, sizing, pinning, visibility).
// The rows are not its business: no row model is registered, `data` stays empty, and the rows of
// the view come from `RowProcessor` as a permutation, drawn by TanStack Virtual.
const features = tableFeatures({
  rowSortingFeature,
  columnSizingFeature,
  columnPinningFeature,
  columnVisibilityFeature,
});
const columnHelper = createColumnHelper<typeof features, RowSpec>();
const NO_ROWS: RowSpec[] = [];

export interface DataTableProps {
  spec: TableSpec;
  formatter: NumberFormatter;
  displayKey: string;
  label: string;
  height?: number | string;
  dense?: boolean;
  onSelectRow?: (id: string) => void;
}

/** Pixels of the theme, read once from the CSS variables (the fallbacks only serve a page without the theme). */
interface Metrics {
  rowHeight: number;
  cellWidth: number;
  gap: number;
}
const FALLBACK_METRICS: Metrics = { rowHeight: 22, cellWidth: 96, gap: 4 };

function readMetrics(el: HTMLElement): Metrics {
  const style = getComputedStyle(el);
  const px = (name: string, fallback: number): number => {
    const v = Number.parseFloat(style.getPropertyValue(name));
    return Number.isFinite(v) && v > 0 ? v : fallback;
  };
  return {
    rowHeight: px('--size-row-height', FALLBACK_METRICS.rowHeight),
    cellWidth: px('--size-cell-width', FALLBACK_METRICS.cellWidth),
    gap: px('--space-small', FALLBACK_METRICS.gap),
  };
}

function schedule(fn: () => void): void {
  if (typeof requestIdleCallback === 'function') requestIdleCallback(fn, { timeout: 200 });
  else setTimeout(fn, 0);
}

export function DataTable(props: DataTableProps): React.JSX.Element {
  // A new set of columns starts a new table: sort, pinning and active cell belong to the columns.
  const signature = props.spec.columns.map((c) => c.id + (c.frozen === true ? '*' : '')).join('\u0000');
  return (
    <TableBoundary label={props.label}>
      <Grid key={signature} {...props} />
    </TableBoundary>
  );
}

function Grid(props: DataTableProps): React.JSX.Element {
  const { spec, formatter, displayKey, label, dense = false, onSelectRow } = props;
  const gridId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const processor = useRef<RowProcessor>(new RowProcessor());
  processor.current.configure(spec, formatter, displayKey);
  const proc = processor.current;

  const [metrics, setMetrics] = useState<Metrics>(FALLBACK_METRICS);
  useLayoutEffect(() => {
    if (rootRef.current !== null) setMetrics(readMetrics(rootRef.current));
  }, []);
  const rowHeight = dense ? metrics.rowHeight : metrics.rowHeight + 2 * metrics.gap;

  const specById = useMemo(() => new Map(spec.columns.map((c) => [c.id, c] as const)), [spec.columns]);
  const columns = useMemo(
    () =>
      spec.columns.map((c) =>
        columnHelper.accessor((row): unknown => row.values[c.id], {
          id: c.id,
          header: c.label,
          size: c.width !== undefined && c.width > 0 ? c.width : metrics.cellWidth,
          enableSorting: true,
          sortDescFirst: false,
        }),
      ),
    [spec.columns, metrics.cellWidth],
  );
  const frozen = useMemo(() => spec.columns.filter((c) => c.frozen === true).map((c) => c.id), [spec.columns]);

  const table = useTable({
    features,
    columns,
    data: NO_ROWS,
    manualSorting: true,
    getRowId: (row) => row.id,
    initialState: { columnPinning: { start: frozen, end: [] } },
  });

  const sorting: SortKey[] = table.state.sorting.map((s) => ({ id: s.id, desc: s.desc }));
  const startColumns = table.getStartVisibleLeafColumns();
  const centerColumns = table.getCenterVisibleLeafColumns();
  const orderedColumns = useMemo(() => [...startColumns, ...centerColumns], [startColumns, centerColumns]);
  const columnIds = useMemo(() => orderedColumns.map((c) => c.id), [orderedColumns]);
  const startTotal = table.getStartTotalSize();
  const totalWidth = table.getTotalSize();

  const [query, setQuery] = useState('');
  const sortingKey = sorting.map((s) => (s.desc ? '-' : '+') + s.id).join(',');
  // The view: sort on raw values, then the global filter on the cached displayed text.
  const view = useMemo(
    () => proc.view(sorting, query, columnIds),
    // `sorting` is read through its key; `spec.rows` and `displayKey` invalidate the cache.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [proc, spec.rows, spec.columns, displayKey, formatter, sortingKey, query, columnIds],
  );
  const total = spec.rows.length;

  // Focusing the filter box starts preparing the columns for it, one per idle slice, so that the
  // first filter finds the formatted text ready (the first one is the only slow one otherwise).
  const warming = useRef(0);
  const warm = useCallback((): void => {
    const token = (warming.current += 1);
    const step = (): void => {
      if (token !== warming.current) return;
      if (proc.warmStep(columnIds)) schedule(step);
    };
    schedule(step);
  }, [proc, columnIds]);

  const rowVirtualizer = useVirtualizer({
    count: view.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 8,
  });
  const colVirtualizer = useVirtualizer({
    horizontal: true,
    count: centerColumns.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => centerColumns[i]?.getSize() ?? metrics.cellWidth,
    overscan: 2,
    paddingStart: startTotal,
  });
  // The virtualizer caches sizes: tell it when the row height or the columns change.
  useLayoutEffect(() => {
    rowVirtualizer.measure();
  }, [rowVirtualizer, rowHeight]);
  useLayoutEffect(() => {
    colVirtualizer.measure();
  }, [colVirtualizer, centerColumns, startTotal]);

  const rowItems = rowVirtualizer.getVirtualItems();
  const colItems = colVirtualizer.getVirtualItems();
  const firstColStart = colItems[0]?.start ?? startTotal;

  const [active, setActive] = useState<Active>({ row: 0, col: 0 });
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const anchor = useRef<number>(0);
  const [status, setStatus] = useState('');

  const cellId = (row: number, col: number): string => gridId + '-' + row + '-' + col;
  const activeRendered =
    view.length > 0 &&
    columnIds.length > 0 &&
    rowItems.some((r) => r.index === active.row) &&
    (active.col < startColumns.length || colItems.some((c) => c.index === active.col - startColumns.length));

  const rowIdAt = useCallback((position: number): string | undefined => {
    const index = view[position];
    return index === undefined ? undefined : proc.rowId(index);
  }, [view, proc]);

  const selectRowAt = (position: number, e: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }): void => {
    const id = rowIdAt(position);
    if (id === undefined) return;
    if (e.shiftKey) {
      const lo = Math.min(anchor.current, position);
      const hi = Math.max(anchor.current, position);
      const next = new Set<string>();
      for (let p = lo; p <= hi; p += 1) {
        const rid = rowIdAt(p);
        if (rid !== undefined) next.add(rid);
      }
      setSelected(next);
    } else if (e.ctrlKey || e.metaKey) {
      const next = new Set(selected);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      setSelected(next);
      anchor.current = position;
    } else {
      setSelected(new Set([id]));
      anchor.current = position;
    }
    onSelectRow?.(id);
  };

  const copy = (exact: boolean): void => {
    const ids: string[] = [];
    for (let p = 0; p < view.length; p += 1) {
      const id = rowIdAt(p);
      if (id !== undefined && (selected.size === 0 || selected.has(id))) ids.push(id);
    }
    const shown: TableSpec = {
      columns: columnIds.flatMap((id) => {
        const c = specById.get(id);
        return c === undefined ? [] : [c];
      }),
      rows: spec.rows,
    };
    try {
      const text = toTsv(shown, ids, formatter, exact);
      void navigator.clipboard.writeText(text).then(
        () => setStatus((exact ? 'Copied exact values of ' : 'Copied ') + ids.length + (ids.length === 1 ? ' row' : ' rows')),
        () => setStatus('The clipboard refused the copy. Allow clipboard access for this page, then try again.'),
      );
    } catch {
      setStatus('The copy failed. Check that the table has values, then try again.');
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>): void => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLButtonElement) return;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && (e.key === 'c' || e.key === 'C')) {
      e.preventDefault();
      copy(e.shiftKey);
      return;
    }
    if (mod && (e.key === 'a' || e.key === 'A')) {
      e.preventDefault();
      const all = new Set<string>();
      for (let p = 0; p < view.length; p += 1) {
        const id = rowIdAt(p);
        if (id !== undefined) all.add(id);
      }
      setSelected(all);
      return;
    }
    if (e.key === 'Escape') {
      setSelected(new Set());
      return;
    }
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      selectRowAt(active.row, e);
      return;
    }
    if (!NAV_KEYS.has(e.key)) return;
    e.preventDefault();
    const page = Math.max(1, Math.floor((scrollRef.current?.clientHeight ?? rowHeight) / rowHeight) - 1);
    const next = moveActive(active, e.key, mod, view.length, columnIds.length, page);
    setActive(next);
    if (e.shiftKey) selectRowAt(next.row, { ctrlKey: false, metaKey: false, shiftKey: true });
    rowVirtualizer.scrollToIndex(next.row);
    if (next.col >= startColumns.length) colVirtualizer.scrollToIndex(next.col - startColumns.length);
    else if (e.key === 'Home' && scrollRef.current !== null) scrollRef.current.scrollLeft = 0;
  };

  const headerCells = orderedColumns.map((column, position) => ({ column, position }));
  const renderHeader = (column: (typeof orderedColumns)[number], position: number): React.JSX.Element => {
    const cs = specById.get(column.id);
    const sorted = column.getIsSorted();
    const pinned = column.getIsPinned() === 'start';
    const sortIndex = column.getSortIndex();
    return (
      <div
        key={column.id}
        role="columnheader"
        className={'ct-cell ct-head-cell' + (pinned ? ' ct-pinned' : '') + (cs?.kind === 'number' ? ' ct-num' : '')}
        aria-colindex={ariaColIndex(position)}
        aria-sort={ariaSort(sorted)}
        style={{ width: column.getSize(), ...(pinned ? { left: column.getStart('start') } : {}) }}
      >
        <button type="button" className="ct-sort" onClick={column.getToggleSortingHandler()} title={cs?.label ?? column.id}>
          <span className="ct-label">{cs?.label ?? column.id}</span>
          {cs?.unit !== undefined && cs.unit !== '' ? <span className="ct-unit">{cs.unit}</span> : null}
          <span className="ct-sort-mark" aria-hidden="true">
            {sorted === 'asc' ? '▲' : sorted === 'desc' ? '▼' : ''}
            {sorted !== false && sorting.length > 1 ? sortIndex + 1 : ''}
          </span>
        </button>
      </div>
    );
  };

  const renderCell = (
    column: (typeof orderedColumns)[number],
    position: number,
    viewRow: number,
    rowIndex: number,
    row: RowSpec,
  ): React.JSX.Element => {
    const cs = specById.get(column.id);
    const text = proc.cellText(rowIndex, column.id);
    const pinned = column.getIsPinned() === 'start';
    const cellFlag = row.cellFlags?.[column.id];
    const rowFlag = position === 0 ? row.flag : undefined;
    const isActive = active.row === viewRow && active.col === position;
    const flags = [rowFlag, cellFlag].filter((f): f is string => f !== undefined);
    return (
      <div
        key={column.id}
        id={cellId(viewRow, position)}
        role="gridcell"
        className={
          'ct-cell' +
          (cs?.kind === 'number' ? ' ct-num' : '') +
          (cs?.kind === 'flag' ? ' ct-flagcol' : '') +
          (pinned ? ' ct-pinned' : '') +
          (isActive ? ' ct-active' : '')
        }
        aria-colindex={ariaColIndex(position)}
        aria-selected={isActive}
        data-r={viewRow}
        data-c={position}
        style={{ width: column.getSize(), ...(pinned ? { left: column.getStart('start') } : {}) }}
        title={cs?.kind === 'number' ? undefined : text}
        onMouseDown={(e: MouseEvent) => {
          setActive({ row: viewRow, col: position });
          selectRowAt(viewRow, e);
        }}
      >
        {flags.map((f) => (
          <span key={f} className="ct-flag" role="img" aria-label={f} title={f}>
            !
          </span>
        ))}
        <span className="ct-text">{text}</span>
      </div>
    );
  };

  const hasColumns = columnIds.length > 0;
  return (
    <div
      ref={rootRef}
      className="ct-root"
      data-density={dense ? 'dense' : 'comfortable'}
      style={{ height: props.height ?? 'auto' }}
    >
      <div className="ct-toolbar">
        <input
          type="search"
          className="ct-search"
          placeholder="Filter the rows shown"
          aria-label={'Filter ' + label}
          value={query}
          onFocus={warm}
          onChange={(e) => setQuery(e.target.value)}
        />
        <span className="ct-count" aria-live="polite">
          {view.length === total ? total + (total === 1 ? ' row' : ' rows') : view.length + ' of ' + total + ' rows'}
        </span>
        <span className="ct-status" role="status">
          {status}
        </span>
        <button type="button" className="ct-button" onClick={() => copy(false)} disabled={!hasColumns || view.length === 0}>
          Copy
        </button>
        <button type="button" className="ct-button" onClick={() => copy(true)} disabled={!hasColumns || view.length === 0}>
          Copy exact values
        </button>
      </div>
      <div
        ref={scrollRef}
        className="ct-grid"
        role="grid"
        tabIndex={0}
        aria-label={label}
        aria-rowcount={ariaRowCount(view.length)}
        aria-colcount={columnIds.length}
        aria-multiselectable="true"
        aria-activedescendant={activeRendered ? cellId(active.row, active.col) : undefined}
        onKeyDown={onKeyDown}
      >
        <div className="ct-inner" style={{ width: totalWidth }}>
          <div className="ct-head" role="rowgroup">
            <div className="ct-row ct-head-row" role="row" aria-rowindex={ARIA_HEADER_ROW_INDEX} style={{ width: totalWidth, height: rowHeight }}>
              {headerCells.slice(0, startColumns.length).map(({ column, position }) => renderHeader(column, position))}
              <div className="ct-spacer" style={{ width: Math.max(0, firstColStart - startTotal) }} />
              {colItems.map((item) => {
                const column = centerColumns[item.index];
                return column === undefined ? null : renderHeader(column, startColumns.length + item.index);
              })}
            </div>
          </div>
          <div className="ct-body" role="rowgroup" style={{ height: rowVirtualizer.getTotalSize() }}>
            {rowItems.map((item) => {
              const rowIndex = view[item.index];
              const row = rowIndex === undefined ? undefined : spec.rows[rowIndex];
              if (rowIndex === undefined || row === undefined) return null;
              const isSelected = selected.has(row.id);
              return (
                <div
                  key={item.index}
                  role="row"
                  className={'ct-row' + (isSelected ? ' ct-selected' : '') + (row.flag !== undefined ? ' ct-flagged' : '') + (item.index % 2 === 1 ? ' ct-odd' : '')}
                  aria-rowindex={ariaRowIndex(item.index)}
                  aria-selected={isSelected}
                  style={{ top: item.start, height: item.size, width: totalWidth }}
                >
                  {startColumns.map((column, p) => renderCell(column, p, item.index, rowIndex, row))}
                  <div className="ct-spacer" style={{ width: Math.max(0, firstColStart - startTotal) }} />
                  {colItems.map((ci) => {
                    const column = centerColumns[ci.index];
                    return column === undefined ? null : renderCell(column, startColumns.length + ci.index, item.index, rowIndex, row);
                  })}
                </div>
              );
            })}
          </div>
          {!hasColumns ? <div className="ct-empty">This table has no columns.</div> : null}
          {hasColumns && total === 0 ? <div className="ct-empty">No rows to show: run the analysis or change the selection.</div> : null}
          {hasColumns && total > 0 && view.length === 0 ? <div className="ct-empty">No row matches the filter. Clear it to see all the rows.</div> : null}
        </div>
      </div>
    </div>
  );
}

export type { ColumnSpec };
