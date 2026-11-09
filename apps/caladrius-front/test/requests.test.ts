import assert from 'node:assert/strict';
import { test } from 'node:test';
import { evaluateRequest, fitPlotRequest, fitRunRequest, initialEstimatesRequest, inputKey, missingSentence, parseInitial, rawPlotRequest, sceneRequest, simulateRequest } from '../src/session/requests.ts';

const inputs = { worksheet: 1, subject: 'A', model: 'pk3.iv_bolus', initial: { cl: 2, vc: 10 }, weighting: 'inv_y2' };
// What `Formatter.parseNumber` does: both decimal marks, undefined when it is not a finite number.
const parse = (t: string): number | undefined => {
  const x = Number(t.replace(',', '.'));
  return t !== '' && Number.isFinite(x) ? x : undefined;
};

test('fit.evaluate and fit.run requests', () => {
  assert.deepEqual(evaluateRequest(inputs), { worksheet: 1, subject: 'A', model: 'pk3.iv_bolus', initial: { cl: 2, vc: 10 }, weighting: 'inv_y2' });
  assert.equal('dose' in evaluateRequest(inputs), false, 'no dose typed: the data dose is used');
  assert.equal(evaluateRequest({ ...inputs, dose: 100 })['dose'], 100);
  assert.equal('analysis' in fitRunRequest(inputs), false);
  assert.equal('analysis' in fitRunRequest(inputs, null), false);
  assert.equal(fitRunRequest(inputs, 7)['analysis'], 7, 're-run with the analysis: the same object is updated');
});

test('fit.initial_estimates request names no initial values and no weighting', () => {
  assert.deepEqual(initialEstimatesRequest({ worksheet: 3, subject: '1', model: 'pk1.oral_1', dose: undefined }), { worksheet: 3, subject: '1', model: 'pk1.oral_1' });
  assert.equal(initialEstimatesRequest({ worksheet: 3, subject: '1', model: 'pk1.oral_1', dose: 320 })['dose'], 320);
});

test('model.simulate request: a grid from 0 to the end of the raw plot, 200 points', () => {
  assert.deepEqual(simulateRequest('pk3.iv_bolus', 100, { cl: 2 }, 100.79), { model: 'pk3.iv_bolus', dose: 100, params: { cl: 2 }, grid: { start: 0, end: 100.79, points: 200 } });
});

test('plot requests', () => {
  assert.deepEqual(rawPlotRequest(1, 'A'), { worksheet: 1, layout: 'overlay', scale: 'both', subjects: ['A'] });
  assert.equal(rawPlotRequest(1, 'A', 5)['analysis'], 5);
  assert.deepEqual(fitPlotRequest(3, 'profile'), { fit: 3, view: 'profile', scale: 'both' });
  assert.deepEqual(fitPlotRequest(3, 'weighted_residuals_time', 9), { fit: 3, view: 'weighted_residuals_time', scale: 'linear', analysis: 9 });
  assert.deepEqual(sceneRequest(4), { analysis: 4 });
});

test('typed initial values: the decimal comma, missing, invalid', () => {
  const names = ['cl', 'vc', 'q2'];
  assert.deepEqual(parseInitial({ cl: '2', vc: '10,5', q2: ' 0.5 ' }, names, parse), { ok: true, values: { cl: 2, vc: 10.5, q2: 0.5 } });
  const bad = parseInitial({ cl: '2', vc: 'abc' }, names, parse);
  assert.equal(bad.ok, false);
  if (!bad.ok) {
    assert.deepEqual(bad.missing, ['q2']);
    assert.deepEqual(bad.invalid, ['vc']);
    assert.equal(missingSentence(bad), 'Enter a value for q2; vc is not a number');
  }
  const none = parseInitial({}, ['v'], parse);
  assert.equal(none.ok === false && missingSentence(none), 'Enter a value for v');
  assert.equal(parseInitial({ cl: 'Infinity' }, ['cl'], (t) => Number(t)).ok, false, 'not finite is not accepted');
});

test('the input key does not depend on the order of the parameters, and changes with every input', () => {
  const a = inputKey(inputs);
  assert.equal(inputKey({ ...inputs, initial: { vc: 10, cl: 2 } }), a);
  assert.notEqual(inputKey({ ...inputs, subject: 'B' }), a);
  assert.notEqual(inputKey({ ...inputs, weighting: 'uniform' }), a);
  assert.notEqual(inputKey({ ...inputs, initial: { cl: 2, vc: 11 } }), a);
  assert.notEqual(inputKey({ ...inputs, dose: 50 }), a);
  assert.notEqual(inputKey({ ...inputs, model: 'pk3.oral_1' }), a);
});
