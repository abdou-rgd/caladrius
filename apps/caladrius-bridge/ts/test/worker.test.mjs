import assert from 'node:assert/strict';
import { test } from 'node:test';
import { TransportError, WorkerTransport, serve } from '../dist/index.js';

// A worker made of two queues: what the page posts reaches the scope of `serve` and back, each
// message in its own tick as a real worker would.
function pair() {
  const toWorker = [];
  const toPage = [];
  const scope = {
    postMessage: (m) => queueMicrotask(() => toPage.forEach((l) => l({ data: m }))),
    addEventListener: (type, l) => type === 'message' && toWorker.push(l),
  };
  const worker = {
    posted: [],
    terminated: false,
    errorListeners: [],
    postMessage(m) {
      this.posted.push(m);
      queueMicrotask(() => toWorker.forEach((l) => l({ data: m })));
    },
    addEventListener(type, l) {
      if (type === 'message') toPage.push(l);
      else this.errorListeners.push(l);
    },
    terminate() {
      this.terminated = true;
    },
  };
  return { scope, worker };
}

const MODULE = { fake: 'compiled module' };

function fakeLoad(log = []) {
  return async () => ({
    module: MODULE,
    glue: {
      Bridge: class {
        constructor(provenance) {
          log.push(['new', provenance]);
        }
        execute(request) {
          log.push(['execute', request]);
          const { command } = JSON.parse(request);
          return JSON.stringify({ at: null, command, error: null, ok: true, result: { n: log.length }, seal: null });
        }
      },
    },
  });
}

test('answers come back to the call that asked, in order', async () => {
  const { scope, worker } = pair();
  const log = [];
  serve(scope, fakeLoad(log), { provenance: false });
  const client = new WorkerTransport(worker);
  const [a, b, c] = await Promise.all([client.execute('one'), client.execute('two'), client.execute('three')]);
  assert.deepEqual([a.command, b.command, c.command], ['one', 'two', 'three']);
  assert.deepEqual(log[0], ['new', false]);
  assert.deepEqual(
    log.slice(1).map((entry) => JSON.parse(entry[1]).command),
    ['one', 'two', 'three'],
  );
  assert.deepEqual(worker.posted.map((m) => m.id), [1, 2, 3]);
});

test('the compiled module is given to the page once the worker is ready', async () => {
  const { scope, worker } = pair();
  serve(scope, fakeLoad());
  const client = new WorkerTransport(worker);
  assert.equal(await client.module(), MODULE);
});

test('a worker that does not start fails the calls and the module', async () => {
  const { scope, worker } = pair();
  serve(scope, async () => {
    throw new Error('no such file');
  });
  const client = new WorkerTransport(worker);
  await assert.rejects(client.module(), /did not start: no such file/);
  await assert.rejects(client.send('{}'), TransportError);
});

test('a crash fails the calls waiting and every later call', async () => {
  const { worker } = pair();
  const client = new WorkerTransport(worker);
  const waiting = client.send('{"command":"x"}');
  worker.errorListeners.forEach((l) => l({ message: 'boom' }));
  await assert.rejects(waiting, /crashed: boom/);
  await assert.rejects(client.send('{}'), /crashed: boom/);
});

test('closing terminates the worker and fails what is waiting', async () => {
  const { worker } = pair();
  const client = new WorkerTransport(worker);
  const waiting = client.send('{}');
  client.close();
  assert.equal(worker.terminated, true);
  await assert.rejects(waiting, /closed/);
});

test('an engine that throws gives an error for that call and the next call works', async () => {
  const { scope, worker } = pair();
  let n = 0;
  serve(scope, async () => ({
    module: MODULE,
    glue: {
      Bridge: class {
        execute() {
          if (n++ === 0) throw new Error('trap');
          return JSON.stringify({ at: null, command: null, error: null, ok: true, result: null, seal: null });
        }
      },
    },
  }));
  const client = new WorkerTransport(worker);
  await assert.rejects(client.execute('first'), /trap/);
  assert.equal((await client.execute('second')).ok, true);
});
