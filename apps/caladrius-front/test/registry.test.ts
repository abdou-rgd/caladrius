import assert from 'node:assert/strict';
import { test } from 'node:test';
import { commandInfos, enumOf } from '../src/model/registry.ts';
import { fixture } from './helpers.ts';

const describe = fixture('describe.json');

test('the commands of the recorded registry: id, title, required parameters, mutates', () => {
  const infos = commandInfos(describe);
  assert.equal(infos.length, describe.length);
  const by = new Map(infos.map((c) => [c.id, c]));
  assert.deepEqual(by.get('data.import')?.required.slice().sort(), ['csv', 'name']);
  assert.equal(by.get('data.import')?.mutates, true);
  assert.deepEqual(by.get('history.list')?.required, []);
  assert.equal(by.get('history.list')?.mutates, false);
  for (const c of infos) {
    assert.ok(c.title.length > 0 && c.title.length <= 110, c.id);
    assert.ok(!c.title.includes('\n'));
  }
});

test('enums of fit.run', () => {
  assert.deepEqual(enumOf(describe, 'fit.run', 'Weighting'), ['uniform', 'inv_y', 'inv_y2', 'inv_yhat', 'inv_yhat2']);
  assert.ok(enumOf(describe, 'fit.run', 'ModelId').includes('pk3.iv_bolus'));
  assert.deepEqual(enumOf(describe, 'fit.run', 'Nothing'), []);
  assert.deepEqual(enumOf(describe, 'no.such.command', 'ModelId'), []);
  assert.deepEqual(enumOf(null, 'fit.run', 'ModelId'), []);
  assert.deepEqual(commandInfos('not a registry'), []);
});
