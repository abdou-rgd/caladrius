import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { ariaColIndex, ariaRowCount, ariaRowIndex, ariaSort } from '../src/aria.ts';
import { moveActive } from '../src/nav.ts';
import { RowProcessor } from '../src/processor.ts';
import { toTsv } from '../src/tsv.ts';
import { displayText } from '../src/cell.ts';
import { markFormatter, table } from './helpers.ts';

const SRC = new URL('../src/', import.meta.url).pathname;
const sources = readdirSync(SRC).filter((f) => /\.(ts|tsx|css)$/.test(f));

test('src/ formats no number itself and does not import the bridge', () => {
  const forbidden = [
    /\.toFixed\b/, /\.toPrecision\b/, /\.toExponential\b/, /\.toLocale\w*/, /Intl\.NumberFormat/, /Math\.round\b/,
    /\bString\(/, /\.toString\(/, /\bNumber\.prototype\b/, /caladrius-bridge/, /bridge-client/,
  ];
  assert.ok(sources.length >= 8);
  for (const file of sources) {
    const text = readFileSync(join(SRC, file), 'utf8');
    for (const re of forbidden) assert.ok(!re.test(text), file + ' matches ' + re);
  }
});

test('table.css holds no colour literal: the theme variables only', () => {
  const css = readFileSync(join(SRC, 'table.css'), 'utf8');
  assert.ok(!/#[0-9a-fA-F]{3,8}\b/.test(css));
  assert.ok(!/\b(rgb|rgba|hsl|hsla)\(/.test(css));
  assert.ok(/var\(--c-text\)/.test(css));
});

test('every number cell goes through the formatter and no other digit appears', () => {
  const f = markFormatter();
  const spec = table([{ a: 1.5, b: -2, c: 1e-7 }, { a: 0, b: 12345.678, c: 1e21 }]);
  const p = new RowProcessor();
  p.configure(spec, f, 'k');
  for (const [i, row] of spec.rows.entries()) {
    for (const col of spec.columns) {
      const v = row.values[col.id];
      assert.equal(p.cellText(i, col.id), '⟨' + v + '⟩');
    }
  }
  assert.equal(f.calls, 6);
});

test('rule 6: a missing column, NaN, null, an infinity and a text give a dash or the text, never an error', () => {
  const f = markFormatter();
  const col = { id: 'x', label: 'x', kind: 'number' as const };
  assert.equal(displayText(undefined, col, f), '-');
  assert.equal(displayText(null, col, f), '-');
  assert.equal(displayText(Number.NaN, col, f), '-');
  assert.equal(displayText(Number.NEGATIVE_INFINITY, col, f), '-');
  assert.equal(displayText('BLQ', col, f), 'BLQ');
  assert.equal(f.calls, 0);
  const spec = table([{ a: 1 }, { a: 2 }]);
  const rows = [{ id: 'r', values: {} }, ...spec.rows];
  const p = new RowProcessor();
  p.configure({ columns: spec.columns, rows }, f, 'k');
  assert.equal(p.cellText(0, 'a'), '-');
  assert.equal(p.cellText(99, 'a'), '');
  assert.equal(p.cellText(0, 'nope'), '');
  assert.deepEqual(Array.from(p.view([{ id: 'a', desc: false }], 'zzz', ['a', 'nope'])), []);
});

test('rule 6: an empty table, no columns, no rows', () => {
  const f = markFormatter();
  const p = new RowProcessor();
  p.configure({ columns: [], rows: [] }, f, 'k');
  assert.equal(p.view([], 'x', []).length, 0);
  assert.equal(p.view([{ id: 'a', desc: true }], '', []).length, 0);
  assert.equal(toTsv({ columns: [], rows: [] }, 'all', f, false), '');
  assert.deepEqual(moveActive({ row: 5, col: 5 }, 'ArrowDown', false, 0, 0, 10), { row: 0, col: 0 });
});

test('ARIA arithmetic under virtualization: row 50,000 is aria-rowindex 50,001 (the header is row 1)', () => {
  assert.equal(ariaRowIndex(49_999), 50_001);
  assert.equal(ariaRowIndex(0), 2);
  assert.equal(ariaRowCount(100_000), 100_001);
  assert.equal(ariaColIndex(0), 1);
  assert.equal(ariaSort('asc'), 'ascending');
  assert.equal(ariaSort('desc'), 'descending');
  assert.equal(ariaSort(false), 'none');
});

test('keyboard navigation stays inside the table', () => {
  const at = (row: number, col: number) => ({ row, col });
  assert.deepEqual(moveActive(at(0, 0), 'ArrowUp', false, 100, 5, 10), at(0, 0));
  assert.deepEqual(moveActive(at(0, 0), 'ArrowLeft', false, 100, 5, 10), at(0, 0));
  assert.deepEqual(moveActive(at(3, 2), 'ArrowDown', false, 100, 5, 10), at(4, 2));
  assert.deepEqual(moveActive(at(3, 4), 'ArrowRight', false, 100, 5, 10), at(3, 4));
  assert.deepEqual(moveActive(at(3, 2), 'Home', false, 100, 5, 10), at(3, 0));
  assert.deepEqual(moveActive(at(3, 2), 'End', false, 100, 5, 10), at(3, 4));
  assert.deepEqual(moveActive(at(3, 2), 'Home', true, 100, 5, 10), at(0, 0));
  assert.deepEqual(moveActive(at(3, 2), 'End', true, 100, 5, 10), at(99, 4));
  assert.deepEqual(moveActive(at(3, 2), 'PageDown', false, 100, 5, 10), at(13, 2));
  assert.deepEqual(moveActive(at(95, 2), 'PageDown', false, 100, 5, 10), at(99, 2));
  assert.deepEqual(moveActive(at(3, 2), 'PageUp', false, 100, 5, 10), at(0, 2));
});
