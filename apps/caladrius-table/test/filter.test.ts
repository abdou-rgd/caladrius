import assert from 'node:assert/strict';
import { test } from 'node:test';
import { RowProcessor } from '../src/processor.ts';
import { ids, markFormatter, table } from './helpers.ts';

const spec = table(
  [
    { subject: 'Alice', v: 3.25, w: 1.23456789 },
    { subject: 'Bob', v: 10, w: null },
    { subject: 'alice b', v: 0.5, w: Number.NaN },
  ],
  { subject: 'text' },
);

function make(comma: boolean): { p: RowProcessor; f: ReturnType<typeof markFormatter> } {
  const f = markFormatter(comma);
  const p = new RowProcessor();
  p.configure(spec, f, comma ? 'c' : 'p');
  return { p, f };
}

const COLS = ['subject', 'v', 'w'];

test('the filter matches the displayed text, decimal comma included', () => {
  const comma = make(true);
  assert.deepEqual(ids(spec, comma.p.view([], '3,25', COLS)), ['r0']);
  assert.deepEqual(ids(spec, comma.p.view([], '3.25', COLS)), []);
  const point = make(false);
  assert.deepEqual(ids(spec, point.p.view([], '3.25', COLS)), ['r0']);
  assert.deepEqual(ids(spec, point.p.view([], '3,25', COLS)), []);
});

test('the filter does not match digits that are not displayed', () => {
  const { p } = make(false);
  // The formatter shows ⟨1.23456789⟩ here, so a raw-value filter and a text filter agree; the
  // check that matters is that a dash is searchable and a raw NaN is not "nan".
  assert.deepEqual(ids(spec, p.view([], 'nan', COLS)), []);
  assert.deepEqual(ids(spec, p.view([], '-', COLS)), ['r1', 'r2']);
});

test('words are all required, case does not matter, the order of the view is kept', () => {
  const { p } = make(false);
  assert.deepEqual(ids(spec, p.view([], 'ALICE', COLS)), ['r0', 'r2']);
  assert.deepEqual(ids(spec, p.view([], 'alice b', COLS)), ['r2']);
  assert.deepEqual(ids(spec, p.view([{ id: 'v', desc: false }], 'alice', COLS)), ['r2', 'r0']);
  assert.deepEqual(ids(spec, p.view([], '   ', COLS)), ['r0', 'r1', 'r2']);
});

test('a second filter calls the formatter 0 times; a new display key formats again', () => {
  const f = markFormatter();
  const p = new RowProcessor();
  p.configure(spec, f, 'p');
  p.view([], 'alice', COLS);
  const first = f.calls;
  assert.equal(first, 3 + 1 + 0 + 0 + 0, 'one call per finite number of v and w: 3 + 1');
  p.view([], 'bob', COLS);
  p.view([{ id: 'w', desc: true }], '1', COLS);
  p.view([], '3', ['v']);
  assert.equal(f.calls, first);
  p.configure(spec, f, 'q');
  p.view([], 'alice', COLS);
  assert.equal(f.calls, first * 2);
});

test('drawing cells fills the cache: filtering after a draw formats only what is missing', () => {
  const f = markFormatter();
  const p = new RowProcessor();
  p.configure(spec, f, 'p');
  p.cellText(0, 'v');
  p.cellText(0, 'v');
  assert.equal(f.calls, 1);
  p.view([], '10', ['v']);
  assert.equal(f.calls, 3);
});

test('new rows drop the cache', () => {
  const f = markFormatter();
  const p = new RowProcessor();
  p.configure(spec, f, 'p');
  p.view([], 'x', COLS);
  const n = f.calls;
  p.configure({ columns: spec.columns, rows: [...spec.rows] }, f, 'p');
  p.view([], 'x', COLS);
  assert.equal(f.calls, 2 * n);
});

test('the filter sees the displayed digits only: 1.23456789 shown with 4 digits is "1.235"', async () => {
  const { fakeFormatter } = await import('../demo/fake-formatter.ts');
  const p = new RowProcessor();
  p.configure(spec, fakeFormatter(4, false), 'd4');
  assert.deepEqual(ids(spec, p.view([], '1.235', COLS)), ['r0']);
  assert.deepEqual(ids(spec, p.view([], '456', COLS)), []);
  p.configure(spec, fakeFormatter(6, false), 'd6');
  assert.deepEqual(ids(spec, p.view([], '1.23457', COLS)), ['r0']);
  assert.deepEqual(ids(spec, p.view([], '1.235', COLS)), []);
});

test('warmStep prepares the columns one at a time, then the filter formats nothing', () => {
  const f = markFormatter();
  const p = new RowProcessor();
  p.configure(spec, f, 'p');
  assert.equal(p.warmStep(COLS), true);
  assert.equal(p.warmStep(COLS), true);
  assert.equal(p.warmStep(COLS), false);
  const n = f.calls;
  p.view([], 'alice', COLS);
  assert.equal(f.calls, n);
});
