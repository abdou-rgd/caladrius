import assert from 'node:assert/strict';
import { test } from 'node:test';
import { pxOfX, snapSample, type Frame } from '../src/plotmap.ts';
import { Session, staleSentence, statusOf } from '../src/session.ts';
import { fake, markFmt } from './fake.ts';

async function loaded() {
  const f = fake();
  const session = new Session(f.client, markFmt);
  await session.loadDataset('edge_blq', 'subject,time,conc,dose\n', '3');
  return { f, session };
}

const runs = (f: ReturnType<typeof fake>) => f.calls.filter((c) => c.command === 'nca.run');
const manualOf = (c: { params: Record<string, unknown> } | undefined) =>
  ((c?.params['options'] ?? {}) as { lambda_z_selection?: { manual?: unknown } }).lambda_z_selection?.manual;

test('loading the data runs the first subject as the egui page does (worksheet, subject, route, dose)', async () => {
  const { f } = await loaded();
  const first = runs(f)[0];
  assert.ok(first);
  assert.deepEqual(first.params, { worksheet: 1, subject: '3', route: 'extravascular', dose: null });
});

test('a drag of the start handle calls nca.run with the range, the analysis and the options', async () => {
  const { f, session } = await loaded();
  // The pointer is released near the column of t = 2: the handle snaps to the sample, then the handler runs.
  const frame: Frame = { width: 640, height: 360, left: 64, right: 18, top: 16, bottom: 44 };
  const panel = session.getState().scene?.panels[0];
  assert.ok(panel);
  const snapped = snapSample(frame, panel, pxOfX(frame, panel, 2) + 3);
  assert.equal(snapped?.x, 2);
  await session.setRangeStart(snapped?.x ?? Number.NaN);
  const last = runs(f).at(-1);
  assert.ok(last);
  assert.equal(last.params['worksheet'], 1);
  assert.equal(last.params['subject'], '3');
  assert.equal(last.params['route'], 'extravascular');
  assert.equal(last.params['analysis'], 7);
  assert.deepEqual(manualOf(last), { range: { start: 2, end: 8 } });
  assert.equal(f.calls.at(-1)?.command, 'plot.scene', 'the scene is asked again after the run');
});

test('a range of fewer than two samples is refused with a sentence, and nothing is sent', async () => {
  const { f, session } = await loaded();
  const before = runs(f).length;
  await session.setRange(4, 4.2);
  assert.equal(runs(f).length, before);
  assert.match(session.getState().error ?? '', /at least 2 points/);
});

test('a click on a sample sends the hand-picked times; "Back to automatic" sends null', async () => {
  const { f, session } = await loaded();
  await session.togglePoint(2);
  assert.deepEqual(manualOf(runs(f).at(-1)), { times: [2, 4, 6, 8] });
  await session.backToAutomatic();
  assert.equal(manualOf(runs(f).at(-1)), null);
});

test('a candidate row sends its range', async () => {
  const { f, session } = await loaded();
  const subject = session.getState().view?.result?.subjects[0];
  assert.ok(subject && 'ok' in subject.outcome);
  const second = subject.outcome.ok.lambda_z_candidates[1];
  assert.ok(second);
  await session.pickCandidate(second);
  assert.deepEqual(manualOf(runs(f).at(-1)), { range: { start: 2, end: 8 } });
});

test('editing a cell marks the analysis stale, and the screen says why', async () => {
  const { f, session } = await loaded();
  assert.equal(statusOf(session.getState()), 'fresh');
  const row = session.getState().rows[2];
  assert.ok(row);
  await session.editCell(row, 'concentration', '6.4');
  const set = f.calls.find((c) => c.command === 'data.set_cell');
  assert.deepEqual(set?.params, { worksheet: 1, column: 'conc', row: 2, value: '6.4' });
  const state = session.getState();
  assert.equal(statusOf(state), 'stale');
  assert.deepEqual(state.staleReasons, ['input_changed']);
  assert.match(staleSentence(state.staleReasons), /the data changed/);
  assert.equal(state.rows[2]?.conc, 6.4);
});

test('an engine error is kept as its own sentence', async () => {
  const f = fake();
  const failing = {
    execute: async (c: string, p?: unknown) =>
      c === 'nca.run' ? { ok: false, command: null, result: null, error: { code: 'x', message: 'The dose is missing.' }, seal: null } : f.client.execute(c, p),
  };
  const session = new Session(failing as never, markFmt);
  await session.loadDataset('edge_blq', '', '3');
  assert.equal(session.getState().error, 'The dose is missing.');
});
