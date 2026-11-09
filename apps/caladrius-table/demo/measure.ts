// Measures on the 100,000 x 20 table, driven from the page (Playwright calls
// `window.__caladriusMeasure()`): first render, long tasks and frame times while scrolling 100
// screens at a steady speed. The sort and filter times are the Event Timing durations (the INP
// definition: input delay + processing + time to the next paint) of real clicks and keystrokes
// sent by the runner; `installEventLog` records them.

export function installEventLog(): void {
  const log: Array<{ name: string; duration: number }> = [];
  (window as unknown as Record<string, unknown>).__caladriusEvents = log;
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) log.push({ name: e.name, duration: e.duration });
  }).observe({ type: 'event', durationThreshold: 16 } as PerformanceObserverInit);
}

export interface Measure {
  env: string;
  rows: number;
  cols: number;
  first_render_ms: number;
  scroll_long_tasks: number;
  scroll_frame_ms_p50: number;
  scroll_frame_ms_p95: number;
}

const frame = (): Promise<number> => new Promise((r) => requestAnimationFrame((t) => r(t)));
const twoFrames = async (): Promise<void> => {
  await frame();
  await frame();
};

function quantile(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0;
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))] ?? 0;
}

export async function runMeasure(
  root: HTMLElement,
  firstRenderMs: number,
  rows: number,
  cols: number,
): Promise<Measure> {
  const grid = root.querySelector<HTMLElement>('[role="grid"]');
  if (grid === null) throw new Error('the big table is not on the page');
  // Scroll 100 screens at a steady speed (one screen per 4 frames), watching long tasks.
  let longTasks = 0;
  const observer = new PerformanceObserver((list) => {
    longTasks += list.getEntries().length;
  });
  observer.observe({ entryTypes: ['longtask'] });
  const frames: number[] = [];
  const screen = grid.clientHeight;
  let last = await frame();
  for (let i = 0; i < 400; i += 1) {
    grid.scrollTop += screen / 4;
    const t = await frame();
    frames.push(t - last);
    last = t;
  }
  await twoFrames();
  observer.disconnect();
  grid.scrollTop = 0;
  await twoFrames();
  frames.sort((a, b) => a - b);

  return {
    env: navigator.userAgent,
    rows,
    cols,
    first_render_ms: firstRenderMs,
    scroll_long_tasks: longTasks,
    scroll_frame_ms_p50: quantile(frames, 0.5),
    scroll_frame_ms_p95: quantile(frames, 0.95),
  };
}
