import assert from 'node:assert/strict';
import { test } from 'node:test';
import { colorsFromVariables } from '../src/colors.ts';

test('colours are read from the CSS variables, subjects until the first gap', () => {
  const vars: Record<string, string> = {
    '--c-series-observed': ' #010101 ',
    '--c-series-fit': '#020202',
    '--c-series-subjects-0': '#a',
    '--c-series-subjects-1': '#b',
    '--c-plot-grid': '#030303',
  };
  const c = colorsFromVariables((n) => vars[n] ?? '');
  assert.equal(c.observed, '#010101');
  assert.equal(c.fit, '#020202');
  assert.equal(c.predicted, '#020202');
  assert.deepEqual(c.subjects, ['#a', '#b']);
  assert.equal(c.grid, '#030303');
  assert.equal(c.background, 'currentColor'); // missing variable: a legible fallback, not a throw
});
