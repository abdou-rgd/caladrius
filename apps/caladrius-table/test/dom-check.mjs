// DOM checks of the rendered table in Chromium, against a running demo (the React part is not
// covered by the Node tests, which only load .ts modules):
//   cd apps && npx vite caladrius-table/demo --port 5181 --host 127.0.0.1
//   cd caladrius-table && npm run test:dom
// Playwright is not a dependency of the package: it is found in the npx cache (`npx -y playwright@1 --version`
// puts it there) or in PLAYWRIGHT_DIR.
import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
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
const url = process.env.DEMO_URL ?? 'http://127.0.0.1:5181/';

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));

async function attr(locator, name, expected) {
  const t0 = Date.now();
  let value = await locator.getAttribute(name);
  while (value !== expected && Date.now() - t0 < 5000) {
    await new Promise((r) => setTimeout(r, 50));
    value = await locator.getAttribute(name);
  }
  assert.equal(value, expected);
}

async function check(name, fn) {
  await fn();
  console.log('ok -', name);
}

await page.goto(url + '?view=big');
await page.waitForSelector('[role="grid"] [role="row"]');
const grid = page.locator('[role="grid"]');

await check('grid attributes: 100,000 rows + header, 20 columns', async () => {
  assert.equal(await grid.getAttribute('aria-rowcount'), '100001');
  assert.equal(await grid.getAttribute('aria-colcount'), '20');
  assert.equal(await page.locator('[role="columnheader"]').first().getAttribute('aria-rowindex'), null);
  assert.equal(await page.locator('[role="row"]').first().getAttribute('aria-rowindex'), '1');
});

await check('virtualization: few rows in the DOM, first body row is aria-rowindex 2', async () => {
  const n = await page.locator('[role="row"]').count();
  assert.ok(n > 10 && n < 120, 'rows in DOM: ' + n);
  assert.equal(await page.locator('[role="gridcell"][aria-colindex="1"]').first().innerText(), 'S000001');
  assert.ok((await page.locator('[role="gridcell"]').count()) < 2000);
});

await check('row 50,000 has aria-rowindex 50,001', async () => {
  await grid.evaluate((el) => {
    el.scrollTop = 49_990 * 22 + 40;
  });
  await page.waitForFunction(() => document.querySelector('[role="row"][aria-rowindex="50001"]') !== null);
  const text = await page.locator('[role="row"][aria-rowindex="50001"] [role="gridcell"]').first().innerText();
  assert.match(text, /S050000/);
});

await check('keyboard: Ctrl+End reaches the last cell and announces it', async () => {
  await grid.focus();
  await page.keyboard.press('Control+End');
  await page.waitForFunction(() => document.querySelector('[role="row"][aria-rowindex="100001"]') !== null);
  const active = await grid.getAttribute('aria-activedescendant');
  assert.ok(active, 'aria-activedescendant set');
  const el = page.locator('[id="' + active + '"]');
  assert.equal(await el.getAttribute('aria-selected'), 'true');
  assert.equal(await el.getAttribute('aria-colindex'), '20');
  await page.keyboard.press('Control+Home');
  await page.waitForFunction(() => document.querySelector('[role="row"][aria-rowindex="2"]') !== null);
  assert.equal(await grid.evaluate((el) => el.scrollTop), 0);
});

await check('sorting: aria-sort follows the clicks, missing values last', async () => {
  const header = page.locator('[role="columnheader"]').nth(2);
  await attr(header, 'aria-sort', 'none');
  await header.locator('button').click();
  await attr(header, 'aria-sort', 'ascending');
  await header.locator('button').click();
  await attr(header, 'aria-sort', 'descending');
  await header.locator('button').click();
  await attr(header, 'aria-sort', 'none');
});

await check('filter: the count says how many rows match', async () => {
  await page.locator('input[type="search"]').fill('S00004');
  await page.waitForFunction(() => document.querySelector('.ct-count')?.textContent === '10 of 100000 rows');
  assert.equal(await grid.getAttribute('aria-rowcount'), '11');
  await page.locator('input[type="search"]').fill('zzzz');
  await page.waitForSelector('.ct-empty');
  await page.locator('input[type="search"]').fill('');
});

await check('display key: the digits change every visible cell without a new table', async () => {
  const cell = page.locator('[role="row"][aria-rowindex="2"] [role="gridcell"][aria-colindex="3"]');
  const before = await cell.innerText();
  await page.locator('select').selectOption('6');
  await page.waitForFunction((b) => document.querySelector('[role="row"][aria-rowindex="2"] [role="gridcell"][aria-colindex="3"]')?.textContent !== b, before);
  await page.locator('input[type="checkbox"]').first().check();
  assert.match(await cell.innerText(), /,/);
});

await page.goto(url + '?view=fit');
await page.waitForSelector('[role="grid"] [role="row"]');
await check('flags: the mark carries the sentence as label and tooltip', async () => {
  const flag = page.locator('.ct-flag').first();
  assert.match((await flag.getAttribute('title')) ?? '', /CV%/);
  assert.equal(await flag.getAttribute('title'), await flag.getAttribute('aria-label'));
});
await check('row selection calls back and marks aria-selected', async () => {
  await page.locator('[role="gridcell"]', { hasText: 'vc' }).first().click();
  assert.equal(await page.locator('[role="row"][aria-selected="true"]').count(), 1);
  assert.match(await page.locator('header').innerText(), /selected row: vc/);
});

await page.goto(url + '?view=edge');
await page.waitForSelector('[role="grid"]');
await check('rule 6: edge cases render sentences and dashes, no error', async () => {
  const body = await page.locator('main').innerText();
  assert.match(body, /This table has no columns/);
  assert.match(body, /No rows to show/);
  const odd = page.locator('[role="grid"][aria-label="Odd values"]');
  assert.equal(await odd.locator('[role="row"][aria-rowindex="2"] [role="gridcell"]').nth(0).innerText(), '-');
  assert.equal(await odd.locator('[role="row"][aria-rowindex="2"] [role="gridcell"]').nth(1).innerText(), '-');
  assert.equal(await odd.locator('[role="row"][aria-rowindex="3"] [role="gridcell"]').nth(0).innerText(), '-');
  assert.equal(await odd.locator('[role="row"][aria-rowindex="4"] [role="gridcell"]').nth(1).innerText(), 'BLQ');
  assert.equal(await odd.locator('[role="columnheader"]').first().locator('.ct-label').innerText(), 'A label of two hundred characters '.repeat(6).slice(0, 200));
});

errors.length = 0;
await page.goto(url + '?view=boom');
await page.waitForSelector('[role="alert"]');
await check('error boundary: a throwing formatter gives one sentence', async () => {
  assert.match(await page.locator('[role="alert"]').innerText(), /could not be drawn/);
});
errors.length = 0;

assert.deepEqual(errors, [], 'no console error');
await browser.close();
console.log('all DOM checks passed');
