import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Formatter, createFormatter } from '../dist/index.js';

// A glue that records how it is called.
function recorder() {
  const calls = [];
  const glue = {
    calls,
    initSync: (input) => calls.push(['initSync', input]),
    formatNumber: (x, d, c) => `n:${x}:${d}:${c}`,
    formatFitQuality: (x, d, c) => `q:${x}:${d}:${c}`,
    formatExact: (x, d, c) => `e:${x}:${d}:${c}`,
    formatExactPoint: (x) => `p:${x}`,
    parseNumber: (t) => (t === '3,25' ? 3.25 : undefined),
    parameterLabel: (n, ev) => `${n}/${ev}`,
    parameterUnit: (n, t, c, d) => `${n}|${t}|${c}|${d}`,
    withLabels: (s) => s,
    plain: (s) => s,
    words: (s) => s,
  };
  return glue;
}

test('the display goes to every call, and can be changed', () => {
  const formatter = new Formatter(recorder());
  assert.equal(formatter.number(1.5), 'n:1.5:4:false');
  formatter.setDisplay({ digits: 3, comma: true });
  assert.equal(formatter.number(1.5), 'n:1.5:3:true');
  assert.equal(formatter.fitQuality(0.9), 'q:0.9:3:true');
  assert.equal(formatter.exact(2), 'e:2:3:true');
  assert.equal(formatter.exactPoint(2), 'p:2');
  assert.deepEqual(formatter.getDisplay(), { digits: 3, comma: true });
});

test('numbers are read and units are asked with the derived units as JSON', () => {
  const formatter = new Formatter(recorder());
  assert.equal(formatter.parseNumber('3,25'), 3.25);
  assert.equal(formatter.parseNumber('abc'), undefined);
  assert.equal(formatter.parameterUnit('auc', 'h', 'mg/L', { auc: 'h*mg/L' }), 'auc|h|mg/L|{"auc":"h*mg/L"}');
  assert.equal(formatter.parameterUnit('cmax', undefined, undefined), 'cmax|undefined|undefined|{}');
});

test('with a compiled module the glue is instantiated from it, once, without a download', () => {
  const glue = recorder();
  const module = { fake: 'module' };
  createFormatter(glue, module);
  assert.deepEqual(glue.calls, [['initSync', { module }]]);
  const ready = recorder();
  createFormatter(ready);
  assert.deepEqual(ready.calls, []);
});
