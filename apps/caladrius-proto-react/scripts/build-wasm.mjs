// Builds the engine as a WebAssembly module for the page and copies what the page loads at run time:
//   public/pkg/   caladrius_bridge.js and caladrius_bridge_bg.wasm (what `cargo xtask bridge` also writes)
//   public/data/  the two datasets of the brief (edge_blq.csv, nca50.csv)
// The same steps as xtask/src/bridge.rs: `cargo rustc` with the `web` profile, then `wasm-bindgen --target web`.
// CARGO_TARGET_DIR is honoured; without it the workspace `target/` is used.
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const app = resolve(here, '..');
const root = resolve(app, '..', '..');
const target = process.env.CARGO_TARGET_DIR ?? join(root, 'target');

function run(command, args) {
  console.error(`> ${command} ${args.join(' ')}`);
  const done = spawnSync(command, args, { cwd: root, stdio: 'inherit', shell: false });
  if (done.error || done.status !== 0) {
    console.error(`proto-react: ${command} failed${done.error ? `: ${done.error.message}` : ''}`);
    process.exit(1);
  }
}

run('cargo', ['rustc', '-p', 'caladrius-bridge', '--lib', '--crate-type', 'cdylib', '--profile', 'web', '--target', 'wasm32-unknown-unknown']);
const wasm = join(target, 'wasm32-unknown-unknown', 'web', 'caladrius_bridge.wasm');
const pkg = join(app, 'public', 'pkg');
mkdirSync(pkg, { recursive: true });
run('wasm-bindgen', [wasm, '--target', 'web', '--no-typescript', '--out-dir', pkg]);

const data = join(app, 'public', 'data');
mkdirSync(data, { recursive: true });
copyFileSync(join(root, 'oracle', 'data', 'edge_blq.csv'), join(data, 'edge_blq.csv'));
copyFileSync(join(root, 'prototypes', 'nca50', 'nca50.csv'), join(data, 'nca50.csv'));
console.error(`proto-react: module ${statSync(join(pkg, 'caladrius_bridge_bg.wasm')).size} bytes in ${pkg}`);
