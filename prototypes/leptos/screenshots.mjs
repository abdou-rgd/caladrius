// Takes the four screenshots of the Leptos prototype (light and dark, fresh and stale) at 1440x900 with the
// DevTools protocol: it starts Edge headless, opens the page, waits until the status badge is on screen,
// and saves the PNG. (`msedge --headless --screenshot` gave a blank page: it does not wait for the module.)
//
//   node prototypes/leptos/screenshots.mjs            # needs `trunk build --release` first (apps/caladrius-proto-leptos/dist)
//
// The page is served from dist/ by a small static server started here.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const dist = resolve(here, '..', '..', 'apps', 'caladrius-proto-leptos', 'dist');
const out = resolve(here, 'screens');
const edge = process.env.EDGE ?? 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const port = 9333;
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };

if (!existsSync(join(dist, 'index.html'))) {
  console.error(`no build in ${dist}: run trunk build --release first`);
  process.exit(1);
}
mkdirSync(out, { recursive: true });

const server = createServer((req, res) => {
  const path = (req.url ?? '/').split('?')[0];
  const file = join(dist, path === '/' ? 'index.html' : path.replace(/\.\./g, ''));
  try {
    const body = readFileSync(file);
    res.writeHead(200, { 'content-type': types[extname(file)] ?? 'application/octet-stream' });
    res.end(body);
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const base = `http://127.0.0.1:${server.address().port}/`;

const profile = mkdtempSync(join(tmpdir(), 'caladrius-edge-'));
const browser = spawn(edge, [`--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, '--headless=new', '--hide-scrollbars', '--no-first-run', '--disable-gpu', 'about:blank'], { stdio: 'ignore' });

async function target() {
  for (let i = 0; i < 100; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      const page = list.find((t) => t.type === 'page');
      if (page) return page.webSocketDebuggerUrl;
    } catch {
      /* the browser is starting */
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error('Edge did not open a page');
}

const socket = new WebSocket(await target());
await new Promise((ok, bad) => {
  socket.onopen = ok;
  socket.onerror = bad;
});
let next = 0;
const waiting = new Map();
socket.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id !== undefined && waiting.has(msg.id)) {
    const { ok, bad } = waiting.get(msg.id);
    waiting.delete(msg.id);
    if (msg.error) bad(new Error(JSON.stringify(msg.error)));
    else ok(msg.result);
  }
};
const send = (method, params = {}) =>
  new Promise((ok, bad) => {
    const id = ++next;
    waiting.set(id, { ok, bad });
    socket.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true })).result.value;

await send('Page.enable');
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });

let failed = false;
for (const theme of ['light', 'dark']) {
  for (const state of ['fresh', 'stale']) {
    const url = `${base}?theme=${theme}${state === 'stale' ? '&stale=1' : ''}`;
    await send('Page.navigate', { url });
    let badge = null;
    for (let i = 0; i < 100 && !badge; i++) {
      await new Promise((r) => setTimeout(r, 100));
      badge = await evaluate(`(document.querySelector('.badge-${state}') || {}).textContent || null`);
    }
    if (!badge) {
      console.error(`${theme}/${state}: the ${state} badge never showed up`);
      failed = true;
      continue;
    }
    await new Promise((r) => setTimeout(r, 400)); // the plot has been laid out
    const shot = await send('Page.captureScreenshot', { format: 'png' });
    const file = join(out, `${theme}-${state}.png`);
    writeFileSync(file, Buffer.from(shot.data, 'base64'));
    console.log(`${file} (badge "${badge}")`);
  }
}
socket.close();
browser.kill();
server.close();
process.exit(failed ? 1 : 0);
