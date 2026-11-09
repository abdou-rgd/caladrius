import assert from 'node:assert/strict';
import { test } from 'node:test';
import { TauriTransport, TransportError, decodeAnswer, encodeRequest } from '../dist/index.js';

const SEALED = JSON.stringify({
  at: null,
  command: 'history.list',
  error: null,
  ok: true,
  result: { total: 0 },
  seal: { format: 1, sha256: 'a'.repeat(64) },
});

test('a request is the command, with params only when there are some', () => {
  assert.equal(encodeRequest('history.list'), '{"command":"history.list"}');
  assert.equal(encodeRequest('nca.run', { worksheet: 1 }), '{"command":"nca.run","params":{"worksheet":1}}');
});

test('an answer with the six keys is read', () => {
  const answer = decodeAnswer(SEALED);
  assert.equal(answer.ok, true);
  assert.equal(answer.seal.sha256.length, 64);
  assert.equal(answer.error, null);
});

test('text that is not an answer is refused', () => {
  for (const text of ['not json', '[1]', 'null', '{}', '{"ok":true}']) {
    assert.throws(() => decodeAnswer(text), TransportError, text);
  }
  const noOk = JSON.stringify({ at: null, command: null, error: null, ok: 'yes', result: null, seal: null });
  assert.throws(() => decodeAnswer(noOk), /ok/);
});

test('the Tauri transport calls engine_execute with the request text', async () => {
  const calls = [];
  const client = new TauriTransport(async (command, args) => {
    calls.push([command, args]);
    return SEALED;
  });
  const answer = await client.execute('history.list');
  assert.deepEqual(calls, [['engine_execute', { request: '{"command":"history.list"}' }]]);
  assert.equal(answer.result.total, 0);
  assert.equal(await client.send('{"command":"history.list"}'), SEALED);
});

test('the Tauri transport reports a refused command and a reply that is not text', async () => {
  const refused = new TauriTransport(async () => {
    throw 'Command engine_execute not allowed by ACL';
  });
  await assert.rejects(refused.send('{}'), /not allowed by ACL/);
  const odd = new TauriTransport(async () => ({ not: 'text' }));
  await assert.rejects(odd.execute('history.list'), TransportError);
});
