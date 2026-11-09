import assert from 'node:assert/strict';
import { test } from 'node:test';
import { RowProcessor } from '../src/processor.ts';
import { ids, markFormatter, table } from './helpers.ts';

const SPECIAL = [
  1e-5, 0.00002, -3, Number.NaN, 5, null, Number.POSITIVE_INFINITY, 0, Number.NEGATIVE_INFINITY, -0.5,
];

function sortedIds(values: Array<number | null>, desc: boolean): string[] {
  const spec = table(values.map((v) => ({ x: v })));
  const p = new RowProcessor();
  p.configure(spec, markFormatter(), 'k');
  return ids(spec, p.sorted([{ id: 'x', desc }]));
}

test('ascending order is on raw values: 1e-5 < 0.00002, negatives first, missing last', () => {
  // r0 1e-5, r1 2e-5, r2 -3, r3 NaN, r4 5, r5 null, r6 +Inf, r7 0, r8 -Inf, r9 -0.5
  assert.deepEqual(sortedIds(SPECIAL, false), ['r2', 'r9', 'r7', 'r0', 'r1', 'r4', 'r3', 'r5', 'r6', 'r8']);
});

test('descending order: the same values reversed, missing still last in their own order', () => {
  assert.deepEqual(sortedIds(SPECIAL, true), ['r4', 'r1', 'r0', 'r7', 'r9', 'r2', 'r3', 'r5', 'r6', 'r8']);
});

test('NaN, null and ±Infinity go last in both directions, never first', () => {
  for (const desc of [false, true]) {
    const order = sortedIds([Number.NaN, 1, null, 2, Number.NEGATIVE_INFINITY, 3, Number.POSITIVE_INFINITY], desc);
    assert.deepEqual(new Set(order.slice(-4)), new Set(['r0', 'r2', 'r4', 'r6']));
  }
});

test('the displayed text never decides the order: 1e-5 and 0.00002 as engine text would sort the other way', () => {
  // As text, "0.00002" < "1e-5"; as numbers, 1e-5 < 0.00002.
  assert.deepEqual(sortedIds([0.00002, 1e-5], false), ['r1', 'r0']);
});

test('several columns, ties keep the original order, text sorts with numbers inside it', () => {
  const spec = table(
    [
      { g: 'S10', v: 2 },
      { g: 'S2', v: 2 },
      { g: 'S2', v: 1 },
      { g: null, v: 0 },
      { g: 'S1', v: 5 },
    ],
    { g: 'text', v: 'number' },
  );
  const p = new RowProcessor();
  p.configure(spec, markFormatter(), 'k');
  assert.deepEqual(ids(spec, p.sorted([{ id: 'g', desc: false }, { id: 'v', desc: false }])), ['r4', 'r2', 'r1', 'r0', 'r3']);
  assert.deepEqual(ids(spec, p.sorted([{ id: 'g', desc: true }, { id: 'v', desc: true }])), ['r0', 'r1', 'r2', 'r4', 'r3']);
  assert.deepEqual(ids(spec, p.sorted([{ id: 'unknown', desc: false }])), ['r0', 'r1', 'r2', 'r3', 'r4']);
  assert.deepEqual(ids(spec, p.sorted([])), ['r0', 'r1', 'r2', 'r3', 'r4']);
});

test('a text in a number column is missing, so it goes last', () => {
  const spec = table([{ x: 'BLQ' }, { x: 2 }, { x: 1 }]);
  const p = new RowProcessor();
  p.configure(spec, markFormatter(), 'k');
  assert.deepEqual(ids(spec, p.sorted([{ id: 'x', desc: false }])), ['r2', 'r1', 'r0']);
});

test('a sort calls the formatter 0 times', () => {
  const spec = table(SPECIAL.map((v) => ({ x: v, y: v })));
  const f = markFormatter();
  const p = new RowProcessor();
  p.configure(spec, f, 'k');
  p.sorted([{ id: 'x', desc: false }]);
  p.sorted([{ id: 'x', desc: true }, { id: 'y', desc: false }]);
  p.view([{ id: 'x', desc: true }], '', ['x', 'y']);
  assert.equal(f.calls, 0);
  assert.equal(p.formatted, 0);
});
