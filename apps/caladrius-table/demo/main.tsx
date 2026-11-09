import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { DataTable, type TableSpec } from '../src/index.ts';
import '../src/table.css';
import { fakeFormatter } from './fake-formatter.ts';
import { fitEstimatesTable } from './fixture.ts';
import { installEventLog, runMeasure } from './measure.ts';
import { syntheticTable } from './synthetic.ts';
import { applyTheme } from './theme.ts';

const t0 = performance.now();
installEventLog();
const params = new URLSearchParams(location.search);
const mode = params.get('theme') === 'dark' ? 'dark' : 'light';
const view = params.get('view') ?? 'both';
const bigRows = Number(params.get('rows') ?? '100000');
const bigCols = Number(params.get('cols') ?? '20');
applyTheme(mode);

const fit = fitEstimatesTable();
const LONG = 'A label of two hundred characters '.repeat(6).slice(0, 200);
// Edge cases (rule 6): nothing here may throw.
const edge: Array<{ label: string; spec: TableSpec }> = [
  { label: 'No columns', spec: { columns: [], rows: [{ id: 'a', values: {} }] } },
  { label: 'No rows', spec: { columns: [{ id: 'x', label: 'X', kind: 'number' }], rows: [] } },
  {
    label: 'Odd values',
    spec: {
      columns: [
        { id: 'x', label: LONG, kind: 'number' },
        { id: 'y', label: 'Y', kind: 'number' },
        { id: 't', label: 'Text', kind: 'text' },
      ],
      rows: [
        { id: 'a', values: { x: Number.NaN, y: Number.POSITIVE_INFINITY, t: LONG } },
        { id: 'b', values: { y: null } },
        { id: 'c', values: { x: 1.5, y: 'BLQ', t: 'tab\there' }, flag: 'Demo flag on a row.' },
        { id: 'c', values: { x: -2, y: 0, t: 'duplicate id' } },
      ],
    },
  },
];
const big = syntheticTable(bigRows, bigCols);

function App(): React.JSX.Element {
  const [digits, setDigits] = useState(4);
  const [comma, setComma] = useState(false);
  const [dense, setDense] = useState(true);
  const formatter = fakeFormatter(digits, comma);
  // The display key changes with the digits and the decimal mark, as in the app.
  const displayKey = String(digits) + (comma ? ',' : '.');
  const [selectedId, setSelectedId] = useState('');

  useEffect(() => {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        const ms = performance.now() - t0;
        (window as unknown as Record<string, unknown>).__caladriusMeasure = () => runMeasure(document.body, ms, bigRows, bigCols);
      }),
    );
  }, []);

  return (
    <main style={{ padding: 'var(--space-panel-margin)', fontFamily: 'system-ui, sans-serif', display: 'flex', flexDirection: 'column', gap: 'var(--space-large)', height: '100vh', boxSizing: 'border-box' }}>
      <header style={{ display: 'flex', gap: 'var(--space-large)', alignItems: 'center', fontSize: 'var(--font-body)' }}>
        <strong style={{ fontSize: 'var(--font-heading)' }}>@caladrius/table</strong>
        <span style={{ color: 'var(--c-text-muted)' }}>demo formatter (fake, not the engine's): </span>
        <label>
          digits{' '}
          <select value={digits} onChange={(e) => setDigits(Number(e.target.value))}>
            {[3, 4, 5, 6].map((d) => (
              <option key={d}>{d}</option>
            ))}
          </select>
        </label>
        <label>
          <input type="checkbox" checked={comma} onChange={(e) => setComma(e.target.checked)} /> decimal comma
        </label>
        <label>
          <input type="checkbox" checked={dense} onChange={(e) => setDense(e.target.checked)} /> dense
        </label>
        <span style={{ color: 'var(--c-text-muted)' }}>selected row: {selectedId || 'none'}</span>
      </header>
      {view === 'boom' ? (
        // A formatter that throws: the error boundary says so in one sentence.
        <DataTable
          spec={edge[2]?.spec ?? fit}
          formatter={{ number: () => { throw new Error('boom'); }, exact: () => '' }}
          displayKey="boom"
          label="Broken table"
        />
      ) : null}
      {view === 'edge' ? (
        <section style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-large)' }}>
          {edge.map((e) => (
            <DataTable key={e.label} spec={e.spec} formatter={formatter} displayKey={displayKey} label={e.label} dense={dense} />
          ))}
        </section>
      ) : null}
      {view === 'both' || view === 'fit' ? (
        <section>
          <h2 style={{ fontSize: 'var(--font-heading)', margin: '0 0 var(--space-medium)' }}>Fit estimates (recorded fit.run, pk3.iv_bolus, subject A)</h2>
          <DataTable spec={fit} formatter={formatter} displayKey={displayKey} label="Fit estimates" dense={dense} onSelectRow={setSelectedId} />
        </section>
      ) : null}
      {view === 'both' || view === 'big' ? (
        <section style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
          <h2 style={{ fontSize: 'var(--font-heading)', margin: '0 0 var(--space-medium)' }}>
            Synthetic table, {bigRows} rows x {bigCols} columns (demo only)
          </h2>
          <DataTable spec={big} formatter={formatter} displayKey={displayKey} label="Synthetic table" dense={dense} height="100%" />
        </section>
      ) : null}
    </main>
  );
}

const el = document.getElementById('root');
if (el !== null) createRoot(el).render(<StrictMode><App /></StrictMode>);
