import assert from 'node:assert/strict';
import { test } from 'node:test';
import { toTsv } from '../src/tsv.ts';
import type { TableSpec } from '../src/types.ts';
import { markFormatter } from './helpers.ts';

const spec: TableSpec = {
  columns: [
    { id: 'p', label: 'Parameter', kind: 'text' },
    { id: 'v', label: 'Cmax', unit: 'mg/L', kind: 'number' },
    { id: 'n', label: 'Note\ttab', kind: 'text' },
  ],
  rows: [
    { id: 'a', values: { p: 'cl', v: 2.5, n: 'ok' } },
    { id: 'b', values: { p: 'vc', v: Number.NaN, n: 'two\nlines' } },
    { id: 'c', values: { p: 'q', v: null } },
  ],
};

test('TSV: header row, tabs, "\\n" between rows, byte for byte', () => {
  const f = markFormatter();
  assert.equal(
    toTsv(spec, 'all', f, false),
    'Parameter\tCmax (mg/L)\tNote tab\ncl\t⟨2.5⟩\tok\nvc\t\ttwo lines\nq\t\t',
  );
});

test('exact copy goes through Formatter.exact and nothing else', () => {
  const f = markFormatter();
  assert.equal(toTsv(spec, ['a'], f, true), 'Parameter\tCmax (mg/L)\tNote tab\ncl\t⟪2.5⟫\tok');
});

test('selected rows are copied in the order given; unknown ids are skipped; none gives the header only', () => {
  const f = markFormatter();
  assert.equal(toTsv(spec, ['c', 'zz', 'a'], f, false), 'Parameter\tCmax (mg/L)\tNote tab\nq\t\t\ncl\t⟨2.5⟩\tok');
  assert.equal(toTsv(spec, [], f, false), 'Parameter\tCmax (mg/L)\tNote tab');
});

test('a 200-character label stays whole in the header', () => {
  const label = 'L'.repeat(200);
  const out = toTsv({ columns: [{ id: 'x', label, kind: 'text' }], rows: [] }, 'all', markFormatter(), false);
  assert.equal(out, label);
});
