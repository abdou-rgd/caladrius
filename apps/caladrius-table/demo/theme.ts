// The demo's stand-in for the app's theme CSS builder: theme.json becomes CSS variables on <html>.
// Names: --c-<color key, "_" as "-">, arrays as --c-<key>-<i>, --space-*, --radius-*, --size-*,
// --font-* (pixels, with their unit), --stroke-*.
import theme from '../../../crates/caladrius-ui/src/theme.json';

type Mode = 'light' | 'dark';

function dash(key: string): string {
  return key.replace(/_/g, '-');
}

export function applyTheme(mode: Mode): void {
  const t = theme[mode];
  const root = document.documentElement;
  const set = (name: string, value: string): void => root.style.setProperty(name, value);
  for (const [k, v] of Object.entries(t.colors)) {
    if (Array.isArray(v)) v.forEach((c, i) => set('--c-' + dash(k) + '-' + i, c));
    else set('--c-' + dash(k), v);
  }
  for (const [k, v] of Object.entries(t.spacing)) set('--space-' + dash(k), v + 'px');
  for (const [k, v] of Object.entries(t.radius)) set('--radius-' + dash(k), v + 'px');
  for (const [k, v] of Object.entries(t.size)) set('--size-' + dash(k), v + 'px');
  for (const [k, v] of Object.entries(t.font)) set('--font-' + dash(k), v + 'px');
  for (const [k, v] of Object.entries(t.stroke)) set('--stroke-' + dash(k), v + 'px');
  root.style.background = t.colors.background;
  root.style.color = t.colors.text;
  root.style.colorScheme = mode;
}
