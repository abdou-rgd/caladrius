import assert from 'node:assert/strict';
import { test } from 'node:test';
import { pickPoint, plotRect, pxOfX, pyOfY, selectable, snapSample, stepSample, xOfPx, yOfPy, type Frame } from '../src/plotmap.ts';
import { logPanel } from './fake.ts';

const frame: Frame = { width: 640, height: 360, left: 64, right: 18, top: 16, bottom: 44 };
const near = (a: number, b: number, eps = 1e-9) => assert.ok(Math.abs(a - b) <= eps, `${a} is not ${b}`);

test('the semi-log map is one affine map of the scene units (the scene is already in log10)', () => {
  const panel = logPanel();
  const r = plotRect(frame);
  near(pxOfX(frame, panel, 0), r.x0);
  near(pxOfX(frame, panel, 24), r.x1);
  near(pyOfY(frame, panel, -1), r.y1);
  near(pyOfY(frame, panel, 1), r.y0);
  // The middle of the y range (log10 = 0, that is 1 on the axis) is the middle of the rectangle.
  near(pyOfY(frame, panel, 0), (r.y0 + r.y1) / 2);
});

test('a pixel maps back to the scene units', () => {
  const panel = logPanel();
  for (const t of [0, 1, 6.5, 24]) near(xOfPx(frame, panel, pxOfX(frame, panel, t)), t);
  for (const y of [-1, -0.3, 0.8]) near(yOfPy(frame, panel, pyOfY(frame, panel, y)), y);
});

test('points hidden by the log axis (zero concentrations) are not samples a click can pick', () => {
  const panel = logPanel();
  assert.deepEqual(selectable(panel).map((p) => p.x), [0.5, 1, 2, 4, 6, 8]);
  // A click on the column of the zero at t = 12 picks nothing, wherever it is on that column.
  const px = pxOfX(frame, panel, 12);
  assert.equal(pickPoint(frame, panel, px, pyOfY(frame, panel, -1), 12), null);
  assert.equal(snapSample(frame, panel, px)?.x, 8, 'the nearest sample is the last one drawn');
});

test('a click within the hit radius picks the sample, a click outside it does not', () => {
  const panel = logPanel();
  const target = selectable(panel).find((p) => p.x === 4);
  assert.ok(target);
  const px = pxOfX(frame, panel, 4);
  const py = pyOfY(frame, panel, target.y);
  assert.equal(pickPoint(frame, panel, px + 5, py - 5, 12)?.x, 4);
  assert.equal(pickPoint(frame, panel, px + 40, py, 12), null);
});

test('stepping a handle moves it by one sample and stays in the list', () => {
  const panel = logPanel();
  assert.equal(stepSample(panel, 4, 1), 6);
  assert.equal(stepSample(panel, 4, -1), 2);
  assert.equal(stepSample(panel, 0.5, -1), 0.5);
  assert.equal(stepSample(panel, 8, 1), 8);
  assert.equal(stepSample(panel, 3, 1), null);
});
