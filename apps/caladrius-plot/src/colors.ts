import type { PlotColors } from './spec.ts';

/** Used only when a CSS variable is missing, so a page without a theme still draws something legible. */
const MISSING = 'currentColor';

/** Builds the plot colours from CSS custom properties. `get` returns the value of `--name`, or ''. */
export function colorsFromVariables(get: (name: string) => string): PlotColors {
  const v = (name: string): string => {
    const value = get(name).trim();
    return value === '' ? MISSING : value;
  };
  const subjects: string[] = [];
  for (let i = 0; i < 64; i += 1) {
    const value = get(`--c-series-subjects-${i}`).trim();
    if (value === '') break;
    subjects.push(value);
  }
  return {
    observed: v('--c-series-observed'),
    predicted: v('--c-series-fit'),
    fit: v('--c-series-fit'),
    selected: v('--c-series-selected'),
    start: v('--c-series-start'),
    replaced: v('--c-series-replaced'),
    other: v('--c-series-other'),
    subjects,
    grid: v('--c-plot-grid'),
    background: v('--c-plot-background'),
    text: v('--c-text'),
    muted: v('--c-text-muted'),
  };
}
