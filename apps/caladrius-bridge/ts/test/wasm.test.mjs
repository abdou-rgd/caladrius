// The client on the real module. It runs when CALADRIUS_BRIDGE_PKG names the directory that
// `cargo xtask bridge` writes for Node (<target>/bridge/node); otherwise it is skipped, so that
// `npm test` needs no Rust toolchain.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { test } from 'node:test';
import { TextClient, createFormatter } from '../dist/index.js';

const dir = process.env.CALADRIUS_BRIDGE_PKG;
const skip = dir ? false : 'CALADRIUS_BRIDGE_PKG is not set (run cargo xtask bridge, then point it at <target>/bridge/node)';

test('formatting on the real module gives the text of caladrius-format', { skip }, () => {
  const glue = createRequire(import.meta.url)(path.resolve(dir, 'caladrius_bridge.js'));
  const format = createFormatter(glue);
  assert.equal(format.number(12.3456), '12.35');
  assert.equal(format.number(100000), '1.000e5');
  assert.equal(format.number(Number.NaN), '-');
  format.setDisplay({ digits: 3, comma: true });
  assert.equal(format.number(12.3456), '12,3');
  assert.equal(format.fitQuality(0.99994), '0,99994');
  assert.equal(format.parseNumber('3,25'), 3.25);
  assert.equal(format.parseNumber('abc'), undefined);
  assert.equal(format.exactPoint(2.5), '2.5');
});

test('a client on the real engine gives a sealed answer', { skip }, async () => {
  const glue = createRequire(import.meta.url)(path.resolve(dir, 'caladrius_bridge.js'));
  const engine = new glue.Bridge(false);
  const client = new (class extends TextClient {
    async send(request) {
      return engine.execute(request);
    }
    close() {}
  })();
  const imported = await client.execute('data.import', { name: 't', csv: 'subject,time,conc,dose\n1,0,0,100\n1,1,6,100\n1,2,5,100\n' });
  assert.equal(imported.ok, true);
  assert.equal(imported.seal.sha256.length, 64);
  const refused = await client.execute('no.such.command', {});
  assert.equal(refused.ok, false);
  assert.equal(refused.error.code, 'unknown_command');
  assert.equal(refused.seal.sha256.length, 64);
});
