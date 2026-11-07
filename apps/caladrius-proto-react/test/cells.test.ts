import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { expectedCells } from '../src/measure.ts';
import { allCells, headlineCells, resultsTsv } from '../src/results.ts';
import { Session, okOf, subjectResult } from '../src/session.ts';
import { fake, markFmt } from './fake.ts';

async function state() {
  const f = fake();
  const session = new Session(f.client, markFmt);
  await session.loadDataset('edge_blq', '', '3');
  return session.getState();
}

const units = { time: 'h', conc: 'mg/L', derived: {} };

test('every result cell is a string made by the formatter of the bridge', async () => {
  const s = await state();
  const subject = subjectResult(s);
  const ok = okOf(s);
  assert.ok(subject && ok);
  const headline = headlineCells(subject, ok, markFmt, units, true);
  const all = allCells(subject, ok, markFmt, units, true);
  const cells = [...headline, ...all];
  assert.ok(cells.length >= 8);
  // The headline labels come from the bridge; "All parameters" shows the engine names as they are.
  for (const c of headline) assert.match(c.label, /^«/);
  for (const c of all) assert.equal(c.label, c.key);
  for (const c of cells) {
    if (c.calculated) assert.match(c.text, /^«q?-?[0-9.]+»$/, `${c.key} was not formatted by the bridge`);
    else assert.equal(c.text, 'not calculated');
    assert.match(c.unit, /^«/);
  }
  assert.equal(cells.find((c) => c.key === 'adj.r.squared')?.text, '«q0.99917»', 'a fit quality goes through fitQuality');
});

test('the candidate list goes through the formatter too', async () => {
  const s = await state();
  const expected = expectedCells(s, markFmt);
  const candidates = [...expected].filter(([key]) => /^c\d+:/.test(key));
  assert.ok(candidates.length >= 10);
  for (const [key, text] of candidates) assert.match(text, /^«.*»$/, key);
});

test('Copy is tab-separated text with a header row, with the digits of the screen', async () => {
  const s = await state();
  const subject = subjectResult(s);
  const ok = okOf(s);
  assert.ok(subject && ok);
  const lines = resultsTsv(headlineCells(subject, ok, markFmt, units, true)).split('\n');
  assert.equal(lines[0], 'Parameter\tValue\tUnit');
  assert.ok(lines.slice(1).every((l) => l.split('\t').length === 3));
  assert.ok(lines.some((l) => l.includes('«6.3»')));
});

test('the sources never format a number themselves', () => {
  const banned = /\.toFixed\(|\.toPrecision\(|\.toExponential\(|\.toLocaleString\(|Intl\.NumberFormat/;
  const walk = (dir: string): string[] => readdirSync(dir).flatMap((n) => (statSync(join(dir, n)).isDirectory() ? walk(join(dir, n)) : [join(dir, n)]));
  const sources = walk(fileURLToPath(new URL('../src', import.meta.url))).filter((p) => /\.(ts|tsx)$/.test(p));
  assert.ok(sources.length > 8);
  for (const p of sources) assert.doesNotMatch(readFileSync(p, 'utf8'), banned, `${p} formats a number`);
});
