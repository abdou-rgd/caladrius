import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import { COMPARTMENTS, ROUTES, SETS, modelId, offeredModels, parameterLabel, parameterNames, parameterUnit, setOf } from '../src/model/parameterSets.ts';
import { enumOf } from '../src/model/registry.ts';
import { appDir, fixture } from './helpers.ts';

const describe = fixture('describe.json');
const engineModels = enumOf(describe, 'fit.run', 'ModelId');

test('every model id the sets build is in the ModelId enum of the recorded describe()', () => {
  assert.equal(engineModels.length, 18);
  for (const c of COMPARTMENTS) for (const r of ROUTES) assert.ok(engineModels.includes(modelId(c, r.id)), modelId(c, r.id));
});

test('every ModelId of the engine is offered (no model is left out of the pk1, pk2, pk3 families)', () => {
  assert.equal(offeredModels(engineModels).length, engineModels.length);
});

test('a model the engine does not list is not offered', () => {
  const without = engineModels.filter((m) => m !== 'pk3.oral_0');
  const offered = offeredModels(without);
  assert.equal(offered.length, 17);
  assert.ok(!offered.some((o) => o.compartments === 3 && o.route === 'oral_0'));
  assert.deepEqual(offeredModels([]), []);
});

test('the parameter names of each set, as the ModelId description gives them', () => {
  const description: string = describe.find((c: any) => c.id === 'fit.run').params_schema.$defs.ModelId.description;
  // pk1: v and cl or k. pk2: cl,vc,q,vp or k10,k12,k21,vc. pk3: cl,vc,q2,vp2,q3,vp3 or k10,k12,k21,k13,k31,vc.
  for (const c of COMPARTMENTS) {
    for (const s of SETS[c]) assert.ok(description.includes(s.names.slice().sort().join(',')) || description.includes(s.names.join(',')) || c === 1, `${c} ${s.id}`);
  }
  assert.deepEqual(setOf(3, 'cl').names, ['cl', 'vc', 'q2', 'vp2', 'q3', 'vp3']);
  assert.deepEqual(setOf(3, 'k').names, ['k10', 'k12', 'k21', 'k13', 'k31', 'vc']);
  assert.deepEqual(setOf(2, 'cl').names, ['cl', 'vc', 'q', 'vp']);
  assert.deepEqual(setOf(1, 'k').names, ['v', 'k']);
  assert.equal(setOf(1, 'unknown').id, 'cl', 'an unknown set falls back to the first');
});

test('routes add ka, tlag and dur', () => {
  assert.deepEqual(parameterNames(1, 'oral_1_lag', 'cl'), ['v', 'cl', 'ka', 'tlag']);
  assert.deepEqual(parameterNames(2, 'iv_infusion', 'k'), ['k10', 'k12', 'k21', 'vc', 'dur']);
  assert.deepEqual(parameterNames(3, 'oral_0_lag', 'cl'), ['cl', 'vc', 'q2', 'vp2', 'q3', 'vp3', 'dur', 'tlag']);
  assert.deepEqual(parameterNames(3, 'iv_bolus', 'cl'), ['cl', 'vc', 'q2', 'vp2', 'q3', 'vp3']);
});

test('labels and units', () => {
  assert.equal(parameterLabel('cl', false), 'CL');
  assert.equal(parameterLabel('cl', true), 'CL/F');
  assert.equal(parameterLabel('ka', true), 'ka');
  assert.equal(parameterLabel('mystery', true), 'mystery');
  const units = { time: 'h', conc: 'mg/L', derived: { v: 'L', cl: 'L/h' } };
  assert.equal(parameterUnit('vp2', units), 'L');
  assert.equal(parameterUnit('q3', units), 'L/h');
  assert.equal(parameterUnit('k13', units), '1/h');
  assert.equal(parameterUnit('tlag', units), 'h');
  assert.equal(parameterUnit('cl', { derived: {} }), undefined);
  assert.equal(parameterUnit('ka', { derived: {} }), undefined);
});

// Against the engine itself: every set of every model is accepted by `fit.evaluate` (a worksheet of the pk3 sample).
const pkg = join(appDir, 'public', 'pkg');
const csv = join(appDir, 'public', 'data', 'pk3_iv_bolus.csv');
test('the engine accepts every parameter set of every offered model', { skip: !existsSync(join(pkg, 'caladrius_bridge_bg.wasm')) || !existsSync(csv) }, async () => {
  const glue: any = await import(join(pkg, 'caladrius_bridge.js'));
  glue.initSync({ module: await WebAssembly.compile(readFileSync(join(pkg, 'caladrius_bridge_bg.wasm'))) });
  const bridge = new glue.Bridge(false);
  const run = (command: string, params: unknown) => JSON.parse(bridge.execute(JSON.stringify({ command, params })));
  const ws = run('data.import', { name: 'pk3', csv: readFileSync(csv, 'utf8') }).result.worksheet.id;
  const value: Record<string, number> = { v: 10, cl: 1, vc: 10, vp: 10, vp2: 10, vp3: 10, q: 1, q2: 1, q3: 0.5, k: 0.1, k10: 0.1, k12: 0.2, k21: 0.1, k13: 0.05, k31: 0.02, ka: 1.5, tlag: 0.1, dur: 0.5 };
  let n = 0;
  for (const { compartments, route } of offeredModels(engineModels)) {
    for (const s of SETS[compartments]) {
      const initial = Object.fromEntries(parameterNames(compartments, route, s.id).map((name) => [name, value[name]]));
      const answer = run('fit.evaluate', { worksheet: ws, subject: 'A', model: modelId(compartments, route), initial, weighting: 'inv_y2' });
      assert.equal(answer.ok, true, `${modelId(compartments, route)} ${s.id}: ${JSON.stringify(answer.error)}`);
      n++;
    }
  }
  assert.equal(n, 36);
});
