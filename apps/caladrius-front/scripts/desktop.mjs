// Opens the desktop window: the engine in the Rust process (Tauri 2), the page of this front.
//   npm run desktop -w @caladrius/front
// Steps: the engine module for the page's formatter (`pkg/`, built when missing), `vite build`, then the
// shell with `cargo run --release --features custom-protocol --manifest-path apps/caladrius-tauri/Cargo.toml`
// (the feature makes the window serve the embedded `dist/` instead of the dev server at `devUrl`).
// CARGO_TARGET_DIR is honoured by every cargo call (and by the wasm step, through build-wasm.mjs).
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const app = resolve(here, '..');
const root = resolve(app, '..', '..');

function run(command, args, cwd) {
  console.error(`> ${command} ${args.join(' ')}`);
  const done = spawnSync(command, args, { cwd, stdio: 'inherit', shell: false });
  if (done.error || done.status !== 0) {
    console.error(`caladrius-front: ${command} failed${done.error ? `: ${done.error.message}` : ` (exit ${done.status})`}`);
    process.exit(done.status ?? 1);
  }
}

run(process.execPath, [resolve(here, 'build-wasm.mjs'), '--if-missing'], app);
// `vite` is hoisted by the npm workspace: find it where node does.
const vite = resolve(dirname(createRequire(import.meta.url).resolve('vite/package.json')), 'bin', 'vite.js');
run(process.execPath, [vite, 'build'], app);
run('cargo', ['run', '--release', '--features', 'custom-protocol', '--manifest-path', resolve(root, 'apps', 'caladrius-tauri', 'Cargo.toml')], root);
