import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));

export function fixture<T = any>(name: string): T {
  return JSON.parse(readFileSync(join(here, 'fixtures', name), 'utf8')) as T;
}

export const appDir = join(here, '..');
