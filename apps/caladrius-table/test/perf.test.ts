import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fakeFormatter } from '../demo/fake-formatter.ts';
import { syntheticTable } from '../demo/synthetic.ts';
import { RowProcessor } from '../src/processor.ts';

// 100,000 x 20 on the pure modules, in Node (the browser measures are in the demo, see
// measure-local.json). Thresholds are generous here; the 200 ms goal is checked in the browser.
test('100,000 x 20: sort and filter, first and again', () => {
  const spec = syntheticTable(100_000, 20);
  let calls = 0;
  const inner = fakeFormatter(4, false);
  const formatter = { number: (x: number) => ((calls += 1), inner.number(x)), exact: inner.exact };
  const p = new RowProcessor();
  p.configure(spec, formatter, 'k');
  const ids = spec.columns.map((c) => c.id);
  const time = (f: () => unknown): number => {
    const t = performance.now();
    f();
    return Math.round(performance.now() - t);
  };
  const sortFirst = time(() => p.sorted([{ id: 'p5', desc: false }]));
  const sortOther = time(() => p.sorted([{ id: 'p6', desc: true }]));
  const sortText = time(() => p.sorted([{ id: 'group', desc: false }, { id: 'p7', desc: false }]));
  assert.equal(calls, 0, 'sorting formats nothing');
  const filterFirst = time(() => p.view([], '12', ids));
  const first = calls;
  const filterAgain = time(() => p.view([], '34', ids));
  const filterSorted = time(() => p.view([{ id: 'p5', desc: false }], '5', ids));
  assert.equal(calls, first, 'a second filter formats nothing');
  assert.ok(first > 1_500_000, 'the first filter formatted the numbers once');
  console.log(JSON.stringify({ sortFirst, sortOther, sortText, filterFirst, filterAgain, filterSorted, firstFormatterCalls: first }));
  assert.ok(sortFirst < 400 && sortOther < 400 && sortText < 600);
  assert.ok(filterAgain < 400 && filterSorted < 400);
});
