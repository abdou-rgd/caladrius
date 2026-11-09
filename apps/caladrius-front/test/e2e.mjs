// End-to-end check in Chromium (Playwright), against `vite preview` of the build (started here) or any
// running server (FRONT_URL, for example the dev server on http://127.0.0.1:5173/).
//
//   cd apps && npm run build -w @caladrius/front && npm run test:e2e -w @caladrius/front
//
// What it does: the pk3 sample, subject A, 3 compartments, IV bolus, the `cl` set, typed starting values, the
// preview, Fit, then the estimates read with "Copy exact values" and compared EXACTLY (every digit) with
// `caladrius-cli fit.run` on the same CSV and parameters. The same for a pk1 fit of theophylline subject 1 with
// `fit.initial_estimates`. Screenshots go to screens/ (SCREENS=0 to skip).
//
// The native CLI: CLI=<path> (default target/agent-interface/debug/caladrius-cli in the repository).
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { playwright } from './pw.mjs';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = resolve(app, '..', '..');
const cli = process.env.CLI ?? join(root, 'target', 'agent-interface', 'debug', 'caladrius-cli');
const screens = join(app, 'screens');
const shots = process.env.SCREENS !== '0';
if (!existsSync(cli)) throw new Error(`the native CLI is not built: ${cli} (CARGO_TARGET_DIR=target/agent-interface cargo build -p caladrius-cli)`);

// ---- the server ----------------------------------------------------------------------------------
let server;
let url = process.env.FRONT_URL;
if (url === undefined) {
  if (!existsSync(join(app, 'dist', 'index.html'))) throw new Error('no build: run `npm run build -w @caladrius/front` first (or set FRONT_URL)');
  const port = 4173 + Math.floor(Math.random() * 200);
  server = spawn('npx', ['vite', 'preview', '--host', '127.0.0.1', '--port', String(port), '--strictPort'], { cwd: app, stdio: 'ignore' });
  url = `http://127.0.0.1:${port}/`;
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(url)).ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 100));
  }
}

// ---- the native answer ------------------------------------------------------------------------------
function native(command, csv, params) {
  const args = [command, '--csv', join(app, 'public', 'data', csv), '--no-seal'];
  for (const [k, v] of Object.entries(params)) args.push('--param', `${k}=${typeof v === 'string' ? v : JSON.stringify(v)}`);
  return JSON.parse(execFileSync(cli, args, { maxBuffer: 1 << 28, stdio: ['ignore', 'pipe', 'ignore'] }).toString());
}

// ---- the page ---------------------------------------------------------------------------------------
const { chromium } = playwright();
const browser = await chromium.launch();
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, permissions: ['clipboard-read', 'clipboard-write'] });
const errors = [];
let passed = 0;
const check = (name, ok, detail = '') => {
  if (!ok) throw new Error(`FAILED: ${name} ${detail}`);
  passed++;
  console.log(`ok  ${name}${detail ? `  ${detail}` : ''}`);
};

async function open(theme) {
  const page = await context.newPage();
  page.on('pageerror', (e) => errors.push(`pageerror ${e}`));
  page.on('console', (m) => m.type() === 'error' && errors.push(`console ${m.text()}`));
  await page.goto(`${url}?theme=${theme}`);
  await page.waitForSelector('[data-engine="ready"] [data-testid="engine-status"].ready', { timeout: 30000 });
  return page;
}

const state = (page) => page.evaluate(() => JSON.parse(JSON.stringify(window.__caladrius.session.state)));
async function until(page, what, test, ms = 20000) {
  const t0 = Date.now();
  for (;;) {
    const s = await state(page);
    const v = test(s);
    if (v) return s;
    if (Date.now() - t0 > ms) throw new Error(`timeout waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 50));
  }
}
const shot = async (page, name) => {
  if (!shots) return;
  mkdirSync(screens, { recursive: true });
  await page.screenshot({ path: join(screens, name) });
};

/** The text of "Copy exact values" of the table named `label`, as rows of cells. */
async function exactTable(page, label) {
  const table = page.locator(`[data-testid="results"] .ct-root:has([aria-label="${label}"])`).first();
  await table.getByRole('button', { name: 'Copy exact values' }).click();
  await page.waitForTimeout(150);
  const text = await page.evaluate(() => navigator.clipboard.readText());
  return text.trim().split('\n').map((line) => line.split('\t'));
}

/** Compares the estimates table with the native fit: every value, exactly. Returns [matching, total]. */
function compareEstimates(rows, ok, order, labelOf) {
  const [head, ...body] = rows;
  assert.deepEqual(head.slice(2), ['Estimate', 'SE', 'CV%', 'CI low', 'CI high'], `header ${head}`);
  const kinds = ['estimate', 'se', 'cv_percent', 'ci_lo', 'ci_hi'];
  let matching = 0;
  let total = 0;
  const bad = [];
  assert.equal(body.length, order.length, 'one row per parameter');
  for (const row of body) {
    const name = order.find((n) => labelOf(n) === row[0]);
    assert.ok(name, `unknown row ${row[0]}`);
    kinds.forEach((kind, i) => {
      total++;
      const native = ok.values[`${kind}.${name}`];
      const shown = row[2 + i];
      if (Number(shown) === native) matching++;
      else bad.push(`${kind}.${name}: page ${shown}, native ${native}`);
    });
  }
  return { matching, total, bad };
}

try {
  // ---------------- pk3 -----------------------------------------------------------------------------
  const page = await open('light');
  await page.click('[data-sample="pk3_iv_bolus"]');
  await until(page, 'the pk3 worksheet and its raw plot', (s) => s.data.worksheet?.name === 'pk3_iv_bolus' && s.rawPlot.scene !== null);
  check('sample imported', true, '42 rows, 3 subjects');
  assert.match(await page.locator('[aria-label="Notes from the engine"]').innerText(), /What the engine read/);
  await page.selectOption('select[aria-label="Subject"]', 'A');
  await page.getByRole('radio', { name: '3', exact: true }).click();
  await page.selectOption('select[aria-label="Route"]', 'iv_bolus');
  await page.getByRole('radio', { name: 'Clearances' }).click();
  const initial = { cl: 2, vc: 10, q2: 5, vp2: 20, q3: 0.5, vp3: 40 };
  for (const [name, value] of Object.entries(initial)) await page.fill(`input[aria-label="${name}"]`, String(value));
  const live = await until(page, 'the live preview', (s) => s.preview.status === 'ok' && s.preview.curve !== null);
  check('live preview', live.preview.curve.length === 200 && live.preview.wrss !== null, `${live.preview.curve.length} points`);
  await page.waitForTimeout(400);
  await shot(page, 'preview-light.png');

  // The preview is latest-wins: typing fast ends on the last value.
  await page.fill('input[aria-label="cl"]', '3');
  await page.fill('input[aria-label="cl"]', '2.5');
  await page.fill('input[aria-label="cl"]', '2');
  const settled = await until(page, 'the last preview', (s) => s.preview.status === 'ok');
  assert.equal(settled.preview.wrss, live.preview.wrss, 'the preview shows the answer for the last typed value');
  check('latest wins', true);

  // A refusal of the engine shows its sentence under the inputs, and the Fit button waits for a valid input.
  await page.fill('input[aria-label="vp3"]', '-1');
  await page.waitForSelector('[aria-label="Preview message"][role="alert"]');
  const refusal = await page.locator('[aria-label="Preview message"]').innerText();
  check('an engine refusal is shown where the action was taken', /vp3/.test(refusal) && /-1/.test(refusal), refusal);
  await page.fill('input[aria-label="vp3"]', '40');
  await until(page, 'the preview again', (s) => s.preview.status === 'ok' && s.preview.curve !== null);
  await page.fill('input[aria-label="Dose"]', 'abc');
  check('a dose that is not a number is refused before any call', (await page.locator('input[aria-label="Dose"].invalid').count()) === 1);
  await page.fill('input[aria-label="Dose"]', '');
  await until(page, 'the preview after the dose', (s) => s.preview.status === 'ok');

  await page.getByRole('button', { name: 'Fit', exact: true }).click();
  const fitted = await until(page, 'the fit', (s) => s.fit !== null && s.plots.profile.scene !== null && s.plots.residuals.scene !== null);
  check('fit converged', fitted.fit.body.status_message === 'converged', fitted.fit.body.status_message);
  await page.waitForSelector('[data-testid="results"] .ct-root');
  await page.waitForTimeout(500);
  await shot(page, 'fit-pk3-light.png');
  await page.getByRole('tab', { name: 'Residuals' }).click();
  await page.waitForSelector('[data-testid="plot-area"] canvas');
  await page.waitForTimeout(500);
  await shot(page, 'residuals-pk3-light.png');
  await page.getByRole('tab', { name: 'Fit', exact: true }).click();

  const expected = native('fit.run', 'pk3_iv_bolus.csv', { subject: 'A', model: 'pk3.iv_bolus', initial, weighting: 'inv_y2' });
  const ok = expected.result.outcome.ok;
  const labels = { cl: 'CL', vc: 'Vc', q2: 'Q2', vp2: 'Vp2', q3: 'Q3', vp3: 'Vp3' };
  const table = await exactTable(page, 'Fit estimates');
  const cmp = compareEstimates(table, ok, Object.keys(initial), (n) => labels[n]);
  console.log(`pk3 estimates table vs native: ${cmp.matching} of ${cmp.total} values identical`);
  cmp.bad.forEach((b) => console.log('   differs:', b));
  assert.equal(cmp.matching, cmp.total, 'exact comparison');
  check('pk3 estimates identical to caladrius-cli', true, `${cmp.matching} of ${cmp.total}`);

  const diag = await exactTable(page, 'Fit diagnostics');
  let dm = 0;
  let dt = 0;
  const names = { 'Weighted sum of squares': 'wrss', AIC: 'aic', SBC: 'sbc', 'Condition number': 'condition_number', 'Residual standard deviation S': 's' };
  for (const [label, value] of diag.slice(1)) {
    const key = names[label];
    if (key === undefined) continue;
    dt++;
    if (Number(value) === ok.values[key]) dm++;
  }
  console.log(`pk3 diagnostics vs native: ${dm} of ${dt} values identical`);
  assert.equal(dm, dt);
  assert.ok(dt >= 5, 'diagnostics read');
  check('pk3 diagnostics identical', true, `${dm} of ${dt}`);

  const sealText = await page.locator('[data-testid="seal"]').innerText();
  check('seal shown', /Seal\s+[0-9a-f]{12}/.test(sealText) && /engine 0\./.test(sealText), sealText.replace(/\s+/g, ' '));
  await page.getByRole('button', { name: 'Copy seal' }).click();
  const copied = JSON.parse(await page.evaluate(() => navigator.clipboard.readText()));
  check('copy seal', copied.sha256 === fitted.fit.seal.sha256 && copied.sha256.length === 64);

  // Stale: change an input after the fit.
  await page.fill('input[aria-label="cl"]', '2.2');
  await page.waitForSelector('.badge.stale');
  check('stale badge after an input change', true);

  // Display: digits and the decimal comma re-render the numbers without an engine call.
  await page.fill('input[aria-label="cl"]', '2');
  const before = (await state(page)).log.length;
  const cell = () => page.locator('[data-testid="results"] [role="row"]').nth(1).innerText();
  const textBefore = await cell();
  await page.getByRole('button', { name: 'Display' }).click();
  await page.selectOption('select[aria-label="Significant digits"]', '7');
  await page.waitForTimeout(200);
  const textDigits = await cell();
  await page.getByLabel('Decimal comma').check();
  await page.waitForTimeout(200);
  const textComma = await cell();
  check('digits change the numbers', textBefore !== textDigits, `${textBefore.replace(/\s+/g, ' ')} -> ${textDigits.replace(/\s+/g, ' ')}`);
  check('decimal comma', /\d,\d/.test(textComma), textComma.replace(/\s+/g, ' '));
  const after = (await state(page)).log.length;
  assert.equal(after - before <= 3, true, 'display changes made no engine call besides the live preview');
  await page.getByLabel('Decimal comma').uncheck();
  await page.selectOption('select[aria-label="Significant digits"]', '4');
  await page.keyboard.press('Escape');

  // Palette and History.
  await page.keyboard.press('Control+k');
  await page.waitForSelector('[cmdk-dialog]');
  await page.fill('[cmdk-input]', 'history.list');
  await page.waitForTimeout(150);
  await shot(page, 'palette-light.png');
  await page.keyboard.press('Enter');
  await until(page, 'a palette command in the log', (s) => s.log.some((e) => e.command === 'history.list' && e.json !== null));
  await page.locator('.dv-tab', { hasText: 'History' }).click();
  await page.waitForSelector('ul[aria-label="History"] li', { timeout: 5000 });
  check('palette runs an engine command, History lists it', (await page.locator('ul[aria-label="History"] li', { hasText: 'history.list' }).count()) > 0);
  await shot(page, 'history-light.png');
  await page.locator('.dv-tab', { hasText: 'Results' }).click();

  // Layout: saved on every change, restored after a reload, reset on request.
  const saved = await page.evaluate(() => localStorage.getItem('caladrius.layout.v1'));
  check('the layout is saved as data', saved !== null && JSON.parse(saved).panels && Object.keys(JSON.parse(saved).panels).length === 5, saved === null ? 'nothing saved' : `${saved.length} bytes of JSON`);
  await page.reload();
  await page.waitForSelector('[data-engine="ready"] [data-testid="engine-status"].ready');
  check('the five panels are back after a reload', (await page.locator('.dv-tab').count()) === 5);
  await page.getByRole('button', { name: 'Display' }).click();
  await page.getByRole('button', { name: 'Reset layout' }).click();
  await page.keyboard.press('Escape');

  // Dark.
  const dark = await open('dark');
  await dark.click('[data-sample="pk3_iv_bolus"]');
  await until(dark, 'worksheet', (s) => s.data.worksheet !== null && s.rawPlot.scene !== null);
  await dark.getByRole('radio', { name: '3', exact: true }).click();
  for (const [name, value] of Object.entries(initial)) await dark.fill(`input[aria-label="${name}"]`, String(value));
  await dark.getByRole('button', { name: 'Fit', exact: true }).click();
  await until(dark, 'the fit', (s) => s.fit !== null && s.plots.profile.scene !== null);
  await dark.waitForTimeout(700);
  await shot(dark, 'fit-pk3-dark.png');
  await dark.close();

  // ---------------- pk1, theophylline, fit.initial_estimates ------------------------------------------
  const t = await open('light');
  await t.click('[data-sample="theoph"]');
  await until(t, 'the theophylline worksheet', (s) => s.data.worksheet?.name === 'theoph' && s.rawPlot.scene !== null);
  await t.selectOption('select[aria-label="Subject"]', '1');
  await t.getByRole('radio', { name: '1', exact: true }).click();
  await t.selectOption('select[aria-label="Route"]', 'oral_1');
  await t.getByRole('button', { name: 'Estimate' }).click();
  const est = await until(t, 'estimates in the inputs', (s) => s.input.initial.v !== undefined && s.input.initial.ka !== undefined);
  const start = native('fit.initial_estimates', 'theoph.csv', { model: 'pk1.oral_1', subject: '1' });
  check('Estimate fills the inputs from fit.initial_estimates', Object.entries(start.initial).every(([k, v]) => Number(est.input.initial[k]) === v), JSON.stringify(est.input.initial));
  await t.waitForTimeout(500);
  await t.getByRole('button', { name: 'Fit', exact: true }).click();
  const tf = await until(t, 'the pk1 fit', (s) => s.fit !== null && s.plots.profile.scene !== null);
  await t.waitForSelector('[data-testid="results"] .ct-root');
  await t.waitForTimeout(400);
  const nativeFit = native('fit.run', 'theoph.csv', { subject: '1', model: 'pk1.oral_1', initial: start.initial, weighting: 'inv_y2' });
  const labels1 = { v: 'V/F', k: 'k', ka: 'ka' };
  const t1 = await exactTable(t, 'Fit estimates');
  const c1 = compareEstimates(t1, nativeFit.result.outcome.ok, Object.keys(labels1), (n) => labels1[n]);
  console.log(`pk1 estimates table vs native: ${c1.matching} of ${c1.total} values identical`);
  c1.bad.forEach((b) => console.log('   differs:', b));
  assert.equal(c1.matching, c1.total);
  check('pk1 (theophylline subject 1) identical to caladrius-cli', true, `${c1.matching} of ${c1.total}`);
  const flagged = await t.locator('[data-testid="flags"]').count();
  check('the engine flags are shown when it raises any', flagged === (nativeFit.result.flag_messages.length > 0 ? 1 : 0), `${nativeFit.result.flag_messages.length} flags`);
  await shot(t, 'fit-pk1-theoph-light.png');

  check('no page or console error', errors.length === 0, errors.join(' | '));
  console.log(`\n${passed} checks passed`);
} finally {
  await browser.close();
  server?.kill();
}
