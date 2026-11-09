import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Session, type Backend, type Numbers } from '../src/session/session.ts';
import type { Timers } from '../src/session/latestWins.ts';
import { fixture } from './helpers.ts';

const describe = fixture('describe.json');
const importAnswer = fixture('import_pk3.json');
const evaluateAnswer = fixture('evaluate_pk3_A.json');

const numbers: Numbers = {
  parseNumber: (t) => {
    const x = Number(t.replace(',', '.'));
    return t.trim() !== '' && Number.isFinite(x) ? x : undefined;
  },
  exact: (x) => String(x),
  setDisplay: () => undefined,
  plain: (t) => t.replace(/`/g, ''),
};

/** Timers that fire when the test says so. */
function manual(): Timers & { fire(): void } {
  let waiting: (() => void) | undefined;
  return {
    set(fn) {
      waiting = fn;
      return 1;
    },
    clear() {
      waiting = undefined;
    },
    fire() {
      const f = waiting;
      waiting = undefined;
      f?.();
    },
  };
}

const answer = (command: string, result: unknown, ok = true, error: unknown = null) => ({ at: null, command, error, ok, result, seal: ok ? { format: 1, sha256: `${command}-seal` } : null });

interface Call {
  command: string;
  params: any;
  resolve(a: unknown): void;
}

/** A backend whose answers the test releases one by one. */
function controlled(): { backend: Backend; calls: Call[] } {
  const calls: Call[] = [];
  return {
    calls,
    backend: {
      execute: (command, params) => new Promise((resolve) => calls.push({ command, params, resolve: resolve as (a: unknown) => void })),
    },
  };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

test('the state is plain data: it reads back from JSON as it was', () => {
  const session = new Session(controlled().backend, numbers, { digits: 4, comma: false }, 'system');
  session.engineReady(describe, 'worker');
  const state = session.state;
  assert.deepEqual(JSON.parse(JSON.stringify(state)), state);
  assert.equal(state.engine.status, 'ready');
  assert.equal(state.models.length, 18);
  assert.ok(state.weightings.includes('inv_y2'));
  assert.equal(state.input.weighting, 'inv_y2');
});

test('run records the answer in the log: command, ok, seal, error', async () => {
  const { backend, calls } = controlled();
  const session = new Session(backend, numbers, { digits: 4, comma: false }, 'system');
  const done = session.run('history.list', {});
  calls[0]?.resolve(answer('history.list', {}));
  await done;
  const failed = session.run('fit.run', {});
  calls[1]?.resolve(answer('fit.run', null, false, { code: 'invalid_parameters', message: 'invalid parameters for `fit.run`' }));
  const bad = await failed;
  assert.equal(bad.ok, false);
  const [newest, older] = session.state.log;
  assert.equal(newest?.command, 'fit.run');
  assert.equal(newest?.ok, false);
  assert.equal(newest?.code, 'invalid_parameters');
  assert.equal(older?.seal, 'history.list-seal');
  assert.equal(session.sentence(bad), 'invalid parameters for fit.run', 'the sentence loses its backticks');
  assert.deepEqual(JSON.parse(JSON.stringify(session.state)), session.state);
});

test('a transport failure is an answer with code transport, and the engine status says it', async () => {
  const backend: Backend = { execute: () => Promise.reject(new Error('the engine worker crashed')) };
  const session = new Session(backend, numbers, { digits: 4, comma: false }, 'system');
  const a = await session.run('nca.run', {});
  assert.equal(a.ok, false);
  assert.equal(a.error?.code, 'transport');
  assert.equal(session.state.engine.status, 'error');
  assert.match(session.state.engine.message ?? '', /crashed/);
  assert.equal(session.state.log[0]?.code, 'transport');
});

test('the preview is debounced and the latest request wins: an older answer arriving last is dropped', async () => {
  const { backend, calls } = controlled();
  const timers = manual();
  const session = new Session(backend, numbers, { digits: 4, comma: false }, 'system', timers);
  session.engineReady(describe, 'worker');
  // A worksheet and a subject are in place (set directly: the import is not what this test is about).
  session.store.set((s) => ({ ...s, data: { ...s.data, worksheet: importAnswer.result.worksheet }, subject: 'A', rawPlot: { ...s.rawPlot, xMax: 100 } }));
  session.setCompartments(3);
  const values = { cl: '2', vc: '10', q2: '5', vp2: '20', q3: '0.5', vp3: '40' };
  for (const [k, v] of Object.entries(values)) session.setInitial(k, v);
  assert.equal(calls.length, 0, 'nothing is sent while the debounce waits');
  timers.fire();
  await flush();
  assert.equal(calls.length, 1);
  assert.equal(calls[0]?.command, 'fit.evaluate');
  assert.equal(calls[0]?.params.model, 'pk3.iv_bolus');
  assert.deepEqual(calls[0]?.params.initial, { cl: 2, vc: 10, q2: 5, vp2: 20, q3: 0.5, vp3: 40 });
  assert.equal(session.state.preview.status, 'pending');

  // The person types again: a second request starts while the first is still in flight.
  session.setInitial('cl', '3');
  timers.fire();
  await flush();
  assert.equal(calls.length, 2);
  assert.equal(calls[1]?.params.initial.cl, 3);

  // The second answers first, the first answers last: the first is dropped.
  calls[1]?.resolve(answer('fit.evaluate', { ...evaluateAnswer.result, wrss: 22 }));
  await flush();
  assert.equal(calls[2]?.command, 'model.simulate');
  assert.equal(calls[2]?.params.grid.end, 100);
  assert.equal(calls[2]?.params.params.cl, 3);
  calls[0]?.resolve(answer('fit.evaluate', { ...evaluateAnswer.result, wrss: 11 }));
  await flush();
  assert.equal(calls.length, 3, 'the superseded answer starts nothing');
  calls[2]?.resolve(answer('model.simulate', { times: [0, 50, 100], conc: [10, 1, 0.1] }));
  await flush();
  const { preview } = session.state;
  assert.equal(preview.status, 'ok');
  assert.equal(preview.wrss, 22, 'the answer of the last request');
  assert.deepEqual(preview.curve, [{ x: 0, y: 10 }, { x: 50, y: 1 }, { x: 100, y: 0.1 }]);
  assert.equal(session.state.log.filter((e) => e.quiet).length, 3, 'preview calls are logged and marked quiet');
});

test('an incomplete input says what to enter, and a refusal shows the engine sentence', async () => {
  const { backend, calls } = controlled();
  const timers = manual();
  const session = new Session(backend, numbers, { digits: 4, comma: false }, 'system', timers);
  session.engineReady(describe, 'worker');
  session.store.set((s) => ({ ...s, data: { ...s.data, worksheet: importAnswer.result.worksheet }, subject: 'A' }));
  session.setCompartments(3);
  session.setInitial('cl', '2');
  timers.fire();
  await flush();
  assert.equal(calls.length, 0, 'no engine call for an incomplete input');
  assert.equal(session.state.preview.message, 'Enter a value for vc, q2, vp2, q3, vp3');
  for (const [k, v] of Object.entries({ vc: '10', q2: '5', vp2: '20', q3: '0.5', vp3: '-1' })) session.setInitial(k, v);
  timers.fire();
  await flush();
  calls[0]?.resolve(answer('fit.evaluate', null, false, { code: 'fit_error', message: 'the initial estimate of `vp3` (-1) lies outside its bounds' }));
  await flush();
  assert.equal(session.state.preview.status, 'error');
  assert.equal(session.state.preview.message, 'the initial estimate of vp3 (-1) lies outside its bounds');
  assert.equal(session.state.preview.curve, null);
});

test('a model the engine does not list is replaced by one it does', () => {
  const session = new Session(controlled().backend, numbers, { digits: 4, comma: false }, 'system');
  const registry = JSON.parse(JSON.stringify(describe));
  const fit = registry.find((c: any) => c.id === 'fit.run');
  fit.params_schema.$defs.ModelId.enum = fit.params_schema.$defs.ModelId.enum.filter((m: string) => m !== 'pk3.iv_bolus');
  session.engineReady(registry, 'worker');
  session.setCompartments(3);
  session.setRoute('iv_bolus');
  assert.notEqual(`pk${session.state.input.compartments}.${session.state.input.route}`, 'pk3.iv_bolus');
  assert.equal(session.state.input.compartments, 3, 'the same number of compartments, another route');
});
