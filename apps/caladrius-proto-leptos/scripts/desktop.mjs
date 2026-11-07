// `node apps/caladrius-proto-leptos/scripts/desktop.mjs`: builds the page with trunk, then builds and opens the
// window of the Tauri shell with the Leptos page as its front.
//
// The shell (apps/caladrius-tauri) embeds the folder named by `build.frontendDist` of its tauri.conf.json: the
// React build by default. The environment variable TAURI_CONFIG (JSON merged over tauri.conf.json by Tauri)
// is the one switch: here it points `frontendDist` at the Leptos build. Without it the shell is the React one.
// The shell is built into its own target folder, so the React shell and this one never overwrite each other.
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(app, '..', '..');

function run(command, args, env = {}) {
  console.error(`> ${command} ${args.join(' ')}`);
  const done = spawnSync(command, args, { cwd: root, stdio: 'inherit', env: { ...process.env, ...env } });
  if (done.error || done.status !== 0) {
    console.error(`proto-leptos: ${command} failed${done.error ? `: ${done.error.message}` : ''}`);
    process.exit(done.status ?? 1);
  }
}

run('trunk', ['build', '--release', '--config', 'apps/caladrius-proto-leptos/Trunk.toml']);
const front = JSON.stringify({ build: { frontendDist: '../caladrius-proto-leptos/dist' } });
// A target folder of its own, unless CARGO_TARGET_DIR already names one.
const folder = process.env.CARGO_TARGET_DIR ? [] : ['--target-dir', 'target/shell-leptos'];
run('cargo', ['run', '--release', '--manifest-path', 'apps/caladrius-tauri/Cargo.toml', ...folder], { TAURI_CONFIG: front });
