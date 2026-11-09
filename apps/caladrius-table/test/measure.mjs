// Measures on the 100,000 x 20 demo in Chromium: `npm run measure` (needs a production build of
// the demo served by `vite preview`; see README). Writes measure-local.json.
import { readdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { homedir } from 'node:os';
import { join } from 'node:path';

function findPlaywright() {
  if (process.env.PLAYWRIGHT_DIR) return process.env.PLAYWRIGHT_DIR;
  const base = join(homedir(), '.npm', '_npx');
  for (const d of readdirSync(base)) {
    try {
      const dir = join(base, d, 'node_modules', 'playwright');
      createRequire(import.meta.url).resolve(join(dir, 'package.json'));
      return dir;
    } catch {}
  }
  throw new Error('playwright not found: run `npx -y playwright@1 --version` once');
}
const { chromium } = createRequire(import.meta.url)(findPlaywright());
const url = process.env.DEMO_URL ?? 'http://127.0.0.1:5182/';

const maxDuration = (events, names) => Math.max(0, ...events.filter((e) => names.includes(e.name)).map((e) => e.duration));

// Real clicks and keystrokes; the durations are the page's Event Timing entries (INP definition).
async function run(browser, warm) {
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  await page.goto(url + '?view=big&theme=light');
  await page.waitForFunction(() => typeof window.__caladriusMeasure === 'function');
  const base = await page.evaluate(() => window.__caladriusMeasure());
  const events = () => page.evaluate(() => window.__caladriusEvents.splice(0));
  await events();
  const header = page.locator('[role="columnheader"] button').nth(5);
  const search = page.locator('input[type="search"]');
  const settle = () => page.waitForTimeout(900);
  const sortOnce = async () => {
    await header.click();
    await settle();
    return maxDuration(await events(), ['click']);
  };
  const filterOnce = async (q) => {
    await search.fill(q);
    await settle();
    return maxDuration(await events(), ['input']);
  };
  const out = { ...base };
  out.sort_ms = await sortOnce(); // first sort of this column: builds its keys
  await sortOnce();
  await sortOnce(); // back to unsorted
  out.sort_again_ms = await sortOnce(); // same sort again, keys cached
  if (warm) {
    await search.focus(); // the columns are prepared in idle time
    await page.waitForTimeout(4000);
    await events();
  }
  out.filter_ms = await filterOnce('12');
  await filterOnce('');
  out.filter_again_ms = await filterOnce('34');
  await page.close();
  return out;
}

const browser = await chromium.launch();
const cold = await run(browser, false);
const warm = await run(browser, true);
const version = browser.version();
await browser.close();
const out = {
  env: 'chromium ' + version + ' headless, Linux, production build of the demo',
  rows: cold.rows,
  cols: cold.cols,
  first_render_ms: Math.round(cold.first_render_ms),
  scroll_long_tasks: cold.scroll_long_tasks,
  scroll_frame_ms_p50: Math.round(cold.scroll_frame_ms_p50 * 10) / 10,
  scroll_frame_ms_p95: Math.round(cold.scroll_frame_ms_p95 * 10) / 10,
  sort_ms: Math.round(cold.sort_ms),
  filter_ms: Math.round(cold.filter_ms),
  extra: {
    note: 'filter_ms is the first filter with a cold cache (no focus on the box); filter_after_focus_ms is the first filter after the box had focus for 4 s (columns prepared in idle time); the "again" values reuse the sort keys / the formatted-text cache.',
    sort_again_ms: Math.round(cold.sort_again_ms),
    filter_again_ms: Math.round(cold.filter_again_ms),
    filter_after_focus_ms: Math.round(warm.filter_ms),
    warm_run_scroll_long_tasks: warm.scroll_long_tasks,
    formatter: 'fake demo formatter (toPrecision); the real one is the bridge wasm Formatter: measure in M2',
  },
};
writeFileSync(new URL('../measure-local.json', import.meta.url), JSON.stringify(out, null, 2) + '\n');
console.log(JSON.stringify(out, null, 2));
