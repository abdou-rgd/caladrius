import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { diagnosticsSpec, estimatesSpec, flaggedCells } from '../src/results/fitTable.ts';
import { parameterLabel, parameterNames, parameterUnit } from '../src/model/parameterSets.ts';
import { appDir, fixture } from './helpers.ts';

const pk3 = fixture('fit_run_pk3_A.json').result;
const theoph = fixture('fit_run_theoph_1.json').result;
const ok = pk3.result.outcome.ok;
const units = { time: 'h', conc: 'mg/L', derived: { v: 'L', cl: 'L/h' } };
const labels = (extravascular: boolean) => (name: string) => ({ label: parameterLabel(name, extravascular), unit: parameterUnit(name, units) });
const words = (c: string) => c.replace(/_/g, ' ');

test('the estimates table of the pk3 fit: six rows in the model order, every value cell the raw number', () => {
  const order = parameterNames(3, 'iv_bolus', 'cl');
  const spec = estimatesSpec(ok, labels(false), new Map(), order);
  assert.deepEqual(spec.columns.map((c) => c.id), ['parameter', 'unit', 'estimate', 'se', 'cv', 'ci_lo', 'ci_hi']);
  assert.deepEqual(spec.rows.map((r) => r.id), order);
  assert.deepEqual(spec.rows.map((r) => r.values['parameter']), ['CL', 'Vc', 'Q2', 'Vp2', 'Q3', 'Vp3']);
  let cells = 0;
  for (const row of spec.rows) {
    const map: Record<string, string> = { estimate: 'estimate', se: 'se', cv: 'cv_percent', ci_lo: 'ci_lo', ci_hi: 'ci_hi' };
    for (const [column, key] of Object.entries(map)) {
      const cell = row.values[column];
      assert.equal(typeof cell, 'number', `${row.id}.${column}`);
      assert.ok(Object.is(cell, ok.values[`${key}.${row.id}`]), `${row.id}.${column} is the engine's value, bit for bit`);
      cells++;
    }
  }
  assert.equal(cells, 30);
  assert.equal(spec.rows[0]?.values['unit'], 'L/h');
  assert.equal(spec.rows[1]?.values['unit'], 'L');
  assert.equal(pk3.result.flag_messages.length, 0);
  assert.ok(spec.rows.every((r) => r.cellFlags === undefined), 'no engine flag, no flagged cell');
});

test('without the model order, the engine order is kept; an unknown name still gets a row', () => {
  const spec = estimatesSpec(ok, labels(false));
  assert.deepEqual(spec.rows.map((r) => r.id), ok.parameters);
  const partial = estimatesSpec(ok, labels(false), new Map(), ['vp3', 'cl']);
  assert.deepEqual(partial.rows.slice(0, 2).map((r) => r.id), ['vp3', 'cl']);
  assert.equal(partial.rows.length, 6);
});

test('a value the engine did not give is null, never a made-up number', () => {
  const thin = { ...ok, values: { 'estimate.cl': 2 } };
  const spec = estimatesSpec(thin, labels(false), new Map(), ['cl']);
  assert.equal(spec.rows[0]?.values['estimate'], 2);
  assert.equal(spec.rows[0]?.values['se'], null);
});

test('flagged cells are the engine flags and its sentences (theophylline: ka interval contains zero)', () => {
  const tok = theoph.result.outcome.ok;
  const flagged = flaggedCells(tok.flags, theoph.result.flag_messages, words);
  assert.deepEqual([...flagged.keys()], ['ka']);
  assert.deepEqual(Object.keys(flagged.get('ka') ?? {}).sort(), ['ci_hi', 'ci_lo']);
  assert.match(Object.values(flagged.get('ka') ?? {})[0] ?? '', /ka.*contains 0/);
  const spec = estimatesSpec(tok, labels(true), flagged, ['v', 'k', 'ka']);
  assert.deepEqual(spec.rows.map((r) => r.id), ['v', 'k', 'ka']);
  assert.equal(spec.rows[2]?.cellFlags?.['ci_lo'], theoph.result.flag_messages[0]);
  assert.equal(spec.rows[0]?.cellFlags, undefined);
  assert.equal(spec.rows[0]?.values['parameter'], 'V/F');
});

test('flags that do not pair with sentences fall back to the code in words; a CV over the threshold is not decided here', () => {
  const flagged = flaggedCells([{ code: 'high_cv', parameter: 'vp3' }], [], words);
  assert.equal(flagged.get('vp3')?.['cv'], 'high cv');
  // The pk3 fit has a CV of 34.7 % for vp3 and no flag: the front does not compare numbers, so nothing is flagged.
  assert.equal(ok.values['cv_percent.vp3'] > 30, true);
  const spec = estimatesSpec(ok, labels(false), flaggedCells(ok.flags, [], words));
  assert.ok(spec.rows.every((r) => r.cellFlags === undefined));
  assert.equal(flaggedCells([{ code: 'something_new' }], ['x'], words).size, 0);
});

test('the diagnostics table: the scalar keys, raw values, known ones first', () => {
  const spec = diagnosticsSpec(ok, words);
  const ids = spec.rows.map((r) => r.id);
  assert.deepEqual(ids.slice(0, 4), ['wrss', 's', 'df', 'n']);
  const scalars = Object.keys(ok.values).filter((k) => !k.includes('.'));
  assert.deepEqual([...ids].sort(), [...scalars].sort());
  for (const row of spec.rows) assert.ok(Object.is(row.values['value'], ok.values[row.id]), row.id);
  assert.equal(spec.rows[0]?.values['quantity'], 'Weighted sum of squares');
  const extra = diagnosticsSpec({ ...ok, values: { ...ok.values, new_measure: 1.5 } }, words);
  assert.equal(extra.rows.at(-1)?.values['quantity'], 'new measure');
});

// The rule of the front (T-178): it never computes, rounds or formats a result number.
test('the sources never round or format a number themselves', () => {
  const forbidden = /\.toFixed\(|\.toPrecision\(|\.toExponential\(|Math\.(round|floor|ceil|trunc|log|log10|exp|pow|sqrt)\(|Intl\.NumberFormat|\.toLocaleString\(|parseFloat\(|toString\(\d/;
  const walk = (dir: string): string[] =>
    readdirSync(dir).flatMap((n) => {
      const p = join(dir, n);
      return statSync(p).isDirectory() ? walk(p) : /\.(ts|tsx)$/.test(n) ? [p] : [];
    });
  for (const file of walk(join(appDir, 'src'))) {
    const text = readFileSync(file, 'utf8').split('\n');
    text.forEach((line, i) => {
      // Layout arithmetic (pixels) is allowed where the line says so; nothing else.
      if (/\/\/ layout/.test(line)) return;
      assert.ok(!forbidden.test(line), `${file}:${i + 1}: ${line.trim()}`);
    });
  }
});
