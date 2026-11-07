import react from '@vitejs/plugin-react';
import { execSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig, type Plugin } from 'vite';
// @ts-expect-error a plain .mjs module shared with the tests, without declarations
import { themeCss } from './build/theme-css.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const themeFile = resolve(here, '../../crates/caladrius-ui/src/theme.json');
const clientSrc = resolve(here, '../caladrius-bridge/ts/src');

/** `import 'virtual:theme.css'`: the CSS variables made from theme.json at build time. */
function theme(): Plugin {
  const id = 'virtual:theme.css';
  return {
    name: 'caladrius-theme',
    resolveId(source) {
      return source === id ? `\0${id}` : undefined;
    },
    load(loaded) {
      if (loaded !== `\0${id}`) return undefined;
      this.addWatchFile(themeFile);
      return themeCss(JSON.parse(readFileSync(themeFile, 'utf8')));
    },
  };
}

function version(): string {
  try {
    return execSync('git describe --always --dirty', { cwd: here }).toString().trim();
  } catch {
    return 'unknown';
  }
}

export default defineConfig({
  base: './',
  define: { __VERSION__: JSON.stringify(version()) },
  plugins: [react(), theme()],
  resolve: {
    alias: {
      // The TypeScript client of the bridge, from source (its own `dist` is not needed).
      '@caladrius/bridge-client': resolve(clientSrc, 'index.ts'),
      '@tauri-apps/api': resolve(here, 'node_modules/@tauri-apps/api'),
    },
  },
  worker: { format: 'es' },
  build: { target: 'es2022', sourcemap: false },
  server: { fs: { allow: [resolve(here, '../..')] } },
});
