// What is kept between visits, in localStorage: the layout of the panels (data, not code) and the
// display and theme choices. A value that cannot be read is ignored.

import type { Display } from '@caladrius/bridge-client';
import type { ThemeChoice } from '../session/types.ts';

const LAYOUT = 'caladrius.layout.v1';
const SETTINGS = 'caladrius.settings.v1';

function read(key: string): unknown {
  try {
    const text = localStorage.getItem(key);
    return text === null ? null : (JSON.parse(text) as unknown);
  } catch {
    return null;
  }
}

function write(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage may be full or refused: the app works without it.
  }
}

export function loadLayout(): unknown {
  return read(LAYOUT);
}

export function saveLayout(layout: unknown): void {
  write(LAYOUT, layout);
}

export function forgetLayout(): void {
  try {
    localStorage.removeItem(LAYOUT);
  } catch {
    // nothing to forget
  }
}

export interface Settings {
  display: Display;
  theme: ThemeChoice;
}

export const DEFAULT_SETTINGS: Settings = { display: { digits: 4, comma: false }, theme: 'system' };

export function loadSettings(): Settings {
  const raw = read(SETTINGS) as { display?: { digits?: unknown; comma?: unknown }; theme?: unknown } | null;
  const stored = raw?.display?.digits;
  const digits = typeof stored === 'number' && Number.isInteger(stored) && stored >= 2 && stored <= 8 ? stored : DEFAULT_SETTINGS.display.digits;
  const comma = raw?.display?.comma === true;
  const theme = raw?.theme === 'light' || raw?.theme === 'dark' || raw?.theme === 'system' ? raw.theme : DEFAULT_SETTINGS.theme;
  return { display: { digits, comma }, theme };
}

export function saveSettings(settings: Settings): void {
  write(SETTINGS, settings);
}
