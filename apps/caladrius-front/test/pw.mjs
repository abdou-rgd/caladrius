// Finds Playwright without making it a dependency: in the npx cache (`npx -y playwright@1 --version` puts it there)
// or in PLAYWRIGHT_DIR.
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { homedir } from 'node:os';
import { join } from 'node:path';

export function playwright() {
  const require = createRequire(import.meta.url);
  const dirs = [];
  if (process.env.PLAYWRIGHT_DIR) dirs.push(process.env.PLAYWRIGHT_DIR);
  try {
    const base = join(homedir(), '.npm', '_npx');
    for (const d of readdirSync(base)) dirs.push(join(base, d, 'node_modules', 'playwright'));
  } catch {}
  for (const dir of dirs) {
    try {
      require.resolve(join(dir, 'package.json'));
      return require(dir);
    } catch {}
  }
  throw new Error('playwright not found: run `npx -y playwright@1 --version` once, or set PLAYWRIGHT_DIR');
}
