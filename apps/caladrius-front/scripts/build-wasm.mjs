// Builds the engine as a WebAssembly module for the page and copies what the page loads at run time:
//   public/pkg/   caladrius_bridge.js and caladrius_bridge_bg.wasm (the engine and caladrius-format)
//   public/data/  the sample datasets offered on the start screen
// Same steps as `cargo xtask bridge`: `cargo rustc` with the `web` profile, then `wasm-bindgen --target web`.
// CARGO_TARGET_DIR is honoured; without it the workspace `target/` is used.
// `--if-missing` skips the engine build when the module is already there (for `npm run dev`).
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const app = resolve(here, '..');
const root = resolve(app, '..', '..');
const target = process.env.CARGO_TARGET_DIR ?? join(root, 'target');
const pkg = join(app, 'public', 'pkg');
const module = join(pkg, 'caladrius_bridge_bg.wasm');

function run(command, args) {
  console.error(`> ${command} ${args.join(' ')}`);
  const done = spawnSync(command, args, { cwd: root, stdio: 'inherit', shell: false });
  if (done.error || done.status !== 0) {
    const hint = command === 'wasm-bindgen' ? ' (install it: cargo install wasm-bindgen-cli --version 0.2.129 --locked)' : '';
    console.error(`caladrius-front: ${command} failed${done.error ? `: ${done.error.message}` : ''}${hint}`);
    process.exit(1);
  }
}

if (!(process.argv.includes('--if-missing') && existsSync(module))) {
  run('cargo', ['rustc', '-p', 'caladrius-bridge', '--lib', '--crate-type', 'cdylib', '--profile', 'web', '--target', 'wasm32-unknown-unknown']);
  const wasm = join(target, 'wasm32-unknown-unknown', 'web', 'caladrius_bridge.wasm');
  mkdirSync(pkg, { recursive: true });
  run('wasm-bindgen', [wasm, '--target', 'web', '--no-typescript', '--out-dir', pkg]);
}

// Sample datasets: real data from oracle/data, and the generated pk3 sample next to this script.
const data = join(app, 'public', 'data');
mkdirSync(data, { recursive: true });
for (const name of ['theoph.csv', 'indometh.csv']) copyFileSync(join(root, 'oracle', 'data', name), join(data, name));
for (const name of readdirSync(join(app, 'samples'))) copyFileSync(join(app, 'samples', name), join(data, name));
console.error(`caladrius-front: module ${statSync(module).size} bytes in ${pkg}`);
