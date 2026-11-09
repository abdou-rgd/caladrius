// Records the engine's answers the tests read (test/fixtures/): describe(), an import, a fit, a flagged fit.
// Run after `npm run wasm`:  node test/record.mjs
// The curve and the partial derivatives of a fit are cut to a few entries (the tests do not read them).
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = join(dirname(fileURLToPath(import.meta.url)), '..');
const glue = await import(join(app, 'public/pkg/caladrius_bridge.js'));
glue.initSync({ module: await WebAssembly.compile(readFileSync(join(app, 'public/pkg/caladrius_bridge_bg.wasm'))) });
const bridge = new glue.Bridge(true);
const run = (command, params) => JSON.parse(bridge.execute(JSON.stringify({ command, params })));
const csv = (name) => readFileSync(join(app, 'public/data', name), 'utf8');
const save = (name, value) => writeFileSync(join(app, 'test/fixtures', name), `${JSON.stringify(value, null, 1)}\n`);
const cut = (answer) => {
  const ok = answer.result?.result?.outcome?.ok;
  if (ok) {
    ok.curve = ok.curve.slice(0, 3);
    ok.partials = ok.partials.map((row) => row.slice(0, 3));
  }
  return answer;
};

save('describe.json', JSON.parse(glue.describe()));

const pk3 = run('data.import', { name: 'pk3_iv_bolus', csv: csv('pk3_iv_bolus.csv') });
save('import_pk3.json', pk3);
const ws = pk3.result.worksheet.id;
const initial = { cl: 2, vc: 10, q2: 5, vp2: 20, q3: 0.5, vp3: 40 };
save('fit_run_pk3_A.json', cut(run('fit.run', { worksheet: ws, subject: 'A', model: 'pk3.iv_bolus', initial, weighting: 'inv_y2' })));
save('evaluate_pk3_A.json', run('fit.evaluate', { worksheet: ws, subject: 'A', model: 'pk3.iv_bolus', initial, weighting: 'inv_y2' }));

const theoph = run('data.import', { name: 'theoph', csv: csv('theoph.csv') });
save('import_theoph.json', theoph);
const th = theoph.result.worksheet.id;
const estimate = run('fit.initial_estimates', { worksheet: th, model: 'pk1.oral_1', subject: '1' });
save('initial_estimates_theoph_1.json', estimate);
save('fit_run_theoph_1.json', cut(run('fit.run', { worksheet: th, subject: '1', model: 'pk1.oral_1', initial: estimate.result.initial, weighting: 'inv_y2' })));
console.log('recorded');
