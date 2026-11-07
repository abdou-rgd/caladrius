// `npm run desktop`: builds the engine module and the page, then builds and opens the window of the Tauri shell.
import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(app, '..', '..');
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';

function run(command, args, cwd) {
  console.error(`> ${command} ${args.join(' ')}`);
  const done = spawnSync(command, args, { cwd, stdio: 'inherit', shell: process.platform === 'win32' && command === npm });
  if (done.error || done.status !== 0) {
    console.error(`proto-react: ${command} failed${done.error ? `: ${done.error.message}` : ''}`);
    process.exit(done.status ?? 1);
  }
}

run(npm, ['run', 'wasm'], app);
run(npm, ['run', 'build'], app);
run('cargo', ['run', '--release', '--manifest-path', 'apps/caladrius-tauri/Cargo.toml'], root);
