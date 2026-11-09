import { useMemo, useRef, useState, type ReactNode } from 'react';
import { DataTable, type CellValue, type TableSpec } from '@caladrius/table';
import { loadSample } from '../shell/samples.ts';
import { DEFAULT_DATASETS } from '../session/session.ts';
import { useApp, useSessionState } from '../session/hooks.ts';
import type { WorksheetInfo } from '../session/types.ts';

/** The columns and rows of the worksheet as a table: the units are the engine's, the rows are `data.preview`'s. */
export function worksheetSpec(worksheet: WorksheetInfo, rows: (string | number | null)[][]): TableSpec {
  return {
    columns: worksheet.columns.map((c, i) => ({
      id: String(i),
      label: c.name,
      ...(c.unit !== null ? { unit: c.unit } : {}),
      kind: c.type === 'number' ? ('number' as const) : ('text' as const),
      ...(i === 0 ? { frozen: true } : {}),
    })),
    rows: rows.map((r, index) => ({
      id: String(index),
      values: Object.fromEntries(worksheet.columns.map((_c, i) => [String(i), (r[i] ?? null) as CellValue])),
    })),
  };
}

export function DataPanel(): ReactNode {
  const { session, fmt } = useApp();
  const state = useSessionState();
  const { data, subject } = state;
  const worksheet = data.worksheet;
  const picker = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);
  const displayKey = `${state.display.digits}-${state.display.comma}`;

  const spec = useMemo(() => (worksheet === null ? null : worksheetSpec(worksheet, data.rows)), [worksheet, data.rows]);

  const importFile = async (file: File): Promise<void> => {
    const text = await file.text();
    await session.importCsv(file.name.replace(/\.[^.]+$/, ''), text);
  };

  return (
    <div
      className={`panel dropzone${over ? ' over' : ''}`}
      onDragOver={(e) => {
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setOver(false);
        const file = e.dataTransfer.files[0];
        if (file !== undefined) void importFile(file);
      }}
    >
      <div className="row">
        <button className="btn" onClick={() => picker.current?.click()} disabled={state.engine.status !== 'ready' || data.busy}>
          Open CSV…
        </button>
        <input
          ref={picker}
          type="file"
          accept=".csv,.tsv,.txt,text/csv"
          aria-label="CSV file"
          hidden
          onChange={(e) => {
            const file = e.target.files?.[0];
            if (file !== undefined) void importFile(file);
            e.target.value = '';
          }}
        />
        <span className="muted small">or a sample:</span>
        {DEFAULT_DATASETS.map((d) => (
          <button key={d.id} className="btn small" disabled={state.engine.status !== 'ready' || data.busy} onClick={() => void loadSample(session, d.id)} title={d.label} data-sample={d.id}>
            {d.id === 'pk3_iv_bolus' ? 'pk3' : d.id}
          </button>
        ))}
      </div>
      {data.error !== null ? (
        <div className="banner error" role="alert">
          {data.error}
        </div>
      ) : null}
      {worksheet === null ? (
        <p className="muted">Drop a CSV file here, open one, or load a sample. The engine reads the roles and units from the headers.</p>
      ) : (
        <>
          <div className="row">
            <label className="row">
              <span className="muted small">Subject</span>
              <select className="field" aria-label="Subject" value={String(subject ?? '')} onChange={(e) => {
                const picked = worksheet.subjects.find((s) => String(s) === e.target.value);
                if (picked !== undefined) void session.selectSubject(picked);
              }}>
                {worksheet.subjects.map((s) => (
                  <option key={String(s)} value={String(s)}>
                    {String(s)}
                  </option>
                ))}
              </select>
            </label>
            <span className="muted small">
              {worksheet.name}: {worksheet.subjects.length} subject{worksheet.subjects.length === 1 ? '' : 's'}, {worksheet.rows} rows
            </span>
          </div>
          {worksheet.unit_warnings.length > 0 ? (
            <details className="banner small" aria-label="Unit warnings">
              <summary>Unit warnings ({worksheet.unit_warnings.length}): derived units cannot be named</summary>
              <ul style={{ margin: 0, paddingLeft: 'var(--space-large)' }}>
                {worksheet.unit_warnings.map((w) => (
                  <li key={w.message}>{fmt.plain(w.message)}</li>
                ))}
              </ul>
            </details>
          ) : null}
          {data.notes.length > 0 ? (
            <details className="small muted" aria-label="Notes from the engine">
              <summary>What the engine read from the file ({data.notes.length})</summary>
              <ul style={{ margin: 0, paddingLeft: 'var(--space-large)' }}>
                {data.notes.map((n) => (
                  <li key={n}>{fmt.plain(n)}</li>
                ))}
              </ul>
            </details>
          ) : null}
          {spec !== null ? (
            <div className="table-slot">
              <DataTable spec={spec} formatter={fmt} displayKey={displayKey} label="Worksheet" dense height="100%" />
            </div>
          ) : null}
          {data.rows.length < worksheet.rows ? (
            <p className="muted small" style={{ margin: 0 }}>
              The table shows the first {data.rows.length} of {worksheet.rows} rows; reading the whole worksheet is a later engine command.
            </p>
          ) : null}
        </>
      )}
    </div>
  );
}
