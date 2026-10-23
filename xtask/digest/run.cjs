// Runs the workload of `cargo xtask digest --wasm` in the WebAssembly module, under Node.
//
//   node run.cjs <folder of caladrius_digest.js> <workload.json> [<file for the canonical text>]
//
// The module is the crate caladrius-digest (the engine, built for wasm32-unknown-unknown and wrapped
// by `wasm-bindgen --target nodejs`). It gets the text of the workload that the native build ran,
// gives back the canonical text of the results, and hashes it with the same SHA-256 code. The digest
// goes to the standard output; whatever else there is to say goes to the standard error.
'use strict';

const fs = require('node:fs');
const path = require('node:path');

const [moduleDir, workloadPath, dumpPath] = process.argv.slice(2);
if (!moduleDir || !workloadPath) {
  process.stderr.write('usage: node run.cjs <module folder> <workload.json> [<canonical text file>]\n');
  process.exit(2);
}

const digest = require(path.resolve(moduleDir, 'caladrius_digest.js'));
const workload = fs.readFileSync(workloadPath, 'utf8');

const started = process.hrtime.bigint();
const text = digest.run_workload(workload);
const seconds = Number(process.hrtime.bigint() - started) / 1e9;

if (dumpPath) {
  fs.mkdirSync(path.dirname(path.resolve(dumpPath)), { recursive: true });
  fs.writeFileSync(dumpPath, text);
}

const steps = text.split('\n').length - 1;
process.stderr.write(`digest: wasm32 under Node ${process.version}, ${steps} steps, ${seconds.toFixed(2)} s\n`);
process.stdout.write(`${digest.sha256_text(text)}\n`);
