import assert from 'node:assert/strict';
import { test } from 'node:test';
import { LatestWins, debounce, type Timers } from '../src/session/latestWins.ts';

/** Timers the test moves by hand. */
function fake(): Timers & { now: number; advance(ms: number): void; pending(): number } {
  let now = 0;
  let next = 1;
  const queue = new Map<number, { at: number; fn: () => void }>();
  return {
    get now() {
      return now;
    },
    set(fn, ms) {
      const id = next++;
      queue.set(id, { at: now + ms, fn });
      return id;
    },
    clear(handle) {
      queue.delete(handle as number);
    },
    advance(ms) {
      now += ms;
      for (const [id, t] of [...queue].sort((a, b) => a[1].at - b[1].at)) {
        if (t.at <= now) {
          queue.delete(id);
          t.fn();
        }
      }
    },
    pending: () => queue.size,
  };
}

test('debounce runs once, 150 ms after the last call', () => {
  const timers = fake();
  let calls = 0;
  const d = debounce(() => calls++, 150, timers);
  d.call();
  timers.advance(100);
  d.call();
  timers.advance(100);
  assert.equal(calls, 0, 'restarted by the second call');
  timers.advance(60);
  assert.equal(calls, 1);
  timers.advance(1000);
  assert.equal(calls, 1);
});

test('debounce: cancel and flush', () => {
  const timers = fake();
  let calls = 0;
  const d = debounce(() => calls++, 150, timers);
  d.call();
  d.cancel();
  timers.advance(500);
  assert.equal(calls, 0);
  assert.equal(timers.pending(), 0);
  d.call();
  d.flush();
  assert.equal(calls, 1);
  d.flush();
  assert.equal(calls, 1, 'nothing waits, nothing runs');
});

test('debounce with the real timers', async () => {
  let calls = 0;
  const d = debounce(() => calls++, 20);
  d.call();
  d.call();
  d.call();
  await new Promise((r) => setTimeout(r, 80));
  assert.equal(calls, 1);
});

test('latest wins: only the last request is current, and invalidate makes all stale', async () => {
  const guard = new LatestWins();
  const applied: string[] = [];
  const request = async (name: string, ms: number): Promise<void> => {
    const current = guard.begin();
    await new Promise((r) => setTimeout(r, ms));
    if (current()) applied.push(name);
  };
  // The first request is slower than the second: its answer arrives last and is dropped.
  await Promise.all([request('first', 40), request('second', 5)]);
  assert.deepEqual(applied, ['second']);
  const a = guard.begin();
  guard.invalidate();
  assert.equal(a(), false);
  const b = guard.begin();
  assert.equal(b(), true);
});
