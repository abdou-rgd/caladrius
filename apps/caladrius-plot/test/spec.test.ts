import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';
import type { PlotScene } from '../src/scene.ts';
import { NOTHING_TO_PLOT, sceneToVegaLite, seriesLook } from '../src/spec.ts';
import { aria } from './aria-helper.ts';
import { colors, compileScene, fixtureNames, loadScene, marks, runHeadless } from './helpers.ts';
import type { SceneItem } from './helpers.ts';

const names = fixtureNames();

test('the five recorded scenes are there', () => {
  assert.deepEqual(names, [
    'data_overlay_linear',
    'data_per_subject_log',
    'fit_obs_pred',
    'fit_profile_both',
    'fit_wres',
  ]);
});

for (const name of names) {
  const scene = loadScene(name);

  test(`${name}: compiles and parses with no warning, in 1 and 2 columns`, async () => {
    for (const columns of [1, 2] as const) {
      const { warnings } = await runHeadless(scene, { columns });
      assert.deepEqual(warnings, []);
    }
  });

  test(`${name}: axis values, labels and domains are the engine's`, async () => {
    const { vega, warnings } = compileScene(scene);
    assert.deepEqual(warnings, []);
    const { view } = await runHeadless(scene);
    scene.panels.forEach((panel, i) => {
      for (const [dim, axis] of [['x', panel.x], ['y', panel.y]] as const) {
        const domain = view.scale(`concat_${i}_${dim}`).domain();
        assert.deepEqual(domain, [axis.min, axis.max], `${name} panel ${i} ${dim} domain`);
      }
      // The axis `values` and `labelExpr` of the spec (the first layer carries the axes).
      const group = vega.marks[i];
      for (const [dim, axis] of [['x', panel.x], ['y', panel.y]] as const) {
        const spec = group.axes.find((a: { scale: string }) => a.scale === `concat_${i}_${dim}`);
        assert.ok(spec, 'axis present');
        assert.deepEqual(spec.values, axis.ticks.map((t) => t.value));
      }
    });
    // The text of the axis labels in the scenegraph, per panel, in tick order.
    const found = new Map<number, { x: string[]; y: string[] }>();
    for (const { node, panel } of marks(view)) {
      if (node.role !== 'axis-label' || panel === undefined) continue;
      const texts = (node.items ?? []).map((it: SceneItem) => String(it.text));
      const slot = found.get(panel) ?? { x: [], y: [] };
      // x labels share their baseline, y labels their right edge: tell them apart by the first item's `align`.
      const first = node.items?.[0];
      if (first?.align === 'right') slot.y.push(...texts);
      else slot.x.push(...texts);
      found.set(panel, slot);
    }
    scene.panels.forEach((panel, i) => {
      const got = found.get(i);
      assert.ok(got, `labels of panel ${i}`);
      assert.deepEqual(got.x, panel.x.ticks.map((t) => t.label), `${name} panel ${i} x labels`);
      assert.deepEqual(got.y, panel.y.ticks.map((t) => t.label), `${name} panel ${i} y labels`);
    });
  });

  test(`${name}: panel titles and axis titles come from the scene`, () => {
    const { vega } = compileScene(scene);
    scene.panels.forEach((panel, i) => {
      const group = vega.marks[i];
      assert.equal(group.title.text, panel.title);
      const titles = group.axes.map((a: { title?: string }) => a.title).filter((t: unknown) => t !== undefined).sort();
      assert.deepEqual(titles, [panel.x.title, panel.y.title].sort());
    });
  });

  test(`${name}: no formatting, aggregation or statistics in the spec`, () => {
    const text = JSON.stringify(sceneToVegaLite(scene, { colors }));
    for (const word of ['aggregate', 'bin', 'loess', 'regression', 'density', 'quantile', 'timeUnit', 'format', 'formatType']) {
      assert.equal(text.includes(`"${word}"`), false, `"${word}" in the spec`);
    }
  });
}

test('labelExpr writes each tick value exactly (a double survives JSON)', () => {
  const scene = loadScene('fit_obs_pred');
  const tricky = scene.panels[0]!;
  tricky.x.ticks = [{ value: -0.43581450000000005, label: 'a "quoted" label' }, { value: 1e-7, label: "b'c" }];
  const { vega, warnings } = compileScene({ ...scene, panels: [tricky] });
  assert.deepEqual(warnings, []);
  const expr: string = vega.marks[0].axes.find((a: { scale: string; encode?: { labels?: unknown } }) => a.scale === 'concat_0_x' && a.encode?.labels !== undefined).encode.labels.update.text.signal;
  assert.ok(expr.includes('-0.43581450000000005'));
  assert.ok(expr.includes('1e-7'));
});

test('subject series take the colour of their slot, modulo the palette', () => {
  assert.equal(seriesLook('subject', 0, colors).color, colors.subjects[0]);
  assert.equal(seriesLook('subject', 4, colors).color, colors.subjects[1]);
  assert.equal(seriesLook('subject', undefined, colors).color, colors.subjects[0]);
  assert.equal(seriesLook('subject', 2, { ...colors, subjects: [] }).color, colors.other);
  assert.equal(seriesLook('predicted', undefined, colors).color, colors.fit);
  assert.equal(seriesLook('lambda_z_fit', undefined, colors).color, colors.fit);
  assert.equal(seriesLook('lambda_z_used', undefined, colors).color, colors.selected);
  assert.equal(seriesLook('lambda_z_start', undefined, colors).color, colors.start);
  assert.deepEqual(seriesLook('excluded', undefined, colors), { color: colors.muted, hollow: true });
  assert.equal(seriesLook('replaced', undefined, colors).color, colors.replaced);
});

test('the colours drawn are the theme colours', async () => {
  const { view } = await runHeadless(loadScene('data_overlay_linear'));
  const strokes = new Set<string>();
  for (const { node } of marks(view)) {
    if (node.marktype !== 'line' && node.marktype !== 'symbol') continue;
    for (const it of node.items ?? []) {
      for (const key of ['stroke', 'fill']) {
        const c = it[key];
        if (typeof c === 'string' && c !== 'transparent') strokes.add(c);
      }
    }
  }
  for (const c of colors.subjects) assert.ok(strokes.has(c), `subject colour ${c}`);
});

test('a guide is drawn as a rule from `from` to `to`', async () => {
  for (const [name, kind] of [['fit_wres', 'zero'], ['fit_obs_pred', 'identity']] as const) {
    const scene = loadScene(name);
    const guide = scene.panels[0]?.guide;
    assert.equal(guide?.kind, kind);
    const { view } = await runHeadless(scene);
    const rules = marks(view).filter(({ node, panel }) => node.marktype === 'rule' && panel === 0 && (node.items ?? []).some((i) => Array.isArray(i.strokeDash)));
    assert.equal(rules.length, 1, `${name}: one guide rule`);
    const datum = rules[0]?.node.items?.[0]?.datum;
    assert.deepEqual([datum?.x, datum?.y, datum?.x2, datum?.y2], [guide?.from[0], guide?.from[1], guide?.to[0], guide?.to[1]]);
  }
});

test('series are drawn in the point order of the scene (order field = index)', () => {
  const scene = loadScene('fit_profile_both');
  const { vega } = compileScene(scene);
  const text = JSON.stringify(vega);
  assert.ok(text.includes('"order"'));
  const rows = JSON.stringify(sceneToVegaLite(scene, { colors }));
  assert.ok(rows.includes('"order":999'), 'predicted curve carries order 0..999');
});

test('BLQ points are open triangles', () => {
  const scene = loadScene('fit_profile_both');
  const obs = scene.panels[0]!.series[0]!;
  obs.points[2] = { ...obs.points[2]!, blq: true };
  const text = JSON.stringify(sceneToVegaLite(scene, { colors }));
  assert.ok(text.includes('"triangle-up"'));
});

test('an overlay on the semi-log panel drops non-positive y, keeps the others, and maps to decades', async () => {
  const scene = loadScene('fit_profile_both');
  assert.equal(scene.panels[1]?.y.log, true);
  const overlay = [{
    name: 'preview',
    points: [
      { x: 0, y: 10 },
      { x: 1, y: 0 },
      { x: 2, y: -3 },
      { x: 3, y: 1 },
      { x: 4, y: 0.01 },
    ],
  }];
  const { view, warnings } = await runHeadless(scene, { overlay });
  assert.deepEqual(warnings, []);
  const dashed = marks(view).filter(({ node }) => node.marktype === 'line' && (node.items ?? []).some((i) => Array.isArray(i.strokeDash) && i.strokeDash.length === 2 && i.strokeDash[0] === 6));
  assert.equal(dashed.length, 2, 'the overlay is drawn on both panels');
  const linear = dashed.find((d) => d.panel === 0)!;
  const semilog = dashed.find((d) => d.panel === 1)!;
  const ys = (m: typeof linear) => (m.node.items ?? []).map((i) => i.datum?.y);
  assert.deepEqual(ys(linear), [10, 0, -3, 1, 0.01]);
  assert.deepEqual(ys(semilog), [10, 1, 0.01]);
  const decades = (semilog.node.items ?? []).map((i) => i.datum?.yp as number);
  assert.deepEqual(decades.map((d) => Math.round(d * 1e9) / 1e9), [1, 0, -2]);
});

test('tooltips are the text of the app, carried in a tip field', () => {
  const scene = loadScene('data_overlay_linear');
  const calls: [string, number, number][] = [];
  const spec = sceneToVegaLite(scene, {
    colors,
    tooltip: (series, point, panel) => {
      calls.push([series, point.x, panel]);
      return `${series}@${panel}`;
    },
  });
  const text = JSON.stringify(spec);
  assert.ok(text.includes('"tip":"A@0"'));
  assert.ok(text.includes('"tooltip":{"field":"tip","type":"nominal"}'));
  assert.ok(calls.length >= 42);
  const without = JSON.stringify(sceneToVegaLite(scene, { colors }));
  assert.equal(without.includes('"tooltip"'), false);
});

test('rule 6: no panel, no series, an empty series, a NaN: a sentence or nothing, never a throw', async () => {
  const base = loadScene('fit_profile_both');
  const empty: PlotScene = { title: 'Nothing', panels: [], legend: { entries: [] }, notes: [] };
  const noSeries: PlotScene = { ...base, panels: [{ ...base.panels[0]!, series: [] }] };
  const emptySeries: PlotScene = { ...base, panels: [{ ...base.panels[0]!, series: [{ name: 's', role: 'observed', style: 'line_points', points: [] }] }] };
  const nan: PlotScene = {
    ...base,
    panels: [{ ...base.panels[0]!, series: [{ name: 's', role: 'observed', style: 'line_points', points: [{ x: 1, y: Number.NaN }, { x: Number.POSITIVE_INFINITY, y: 2 }, { x: 3, y: 4 }] }] }],
  };
  const emptyPanel: PlotScene = { ...base, panels: [{ ...base.panels[0]!, series: [], empty: 'No observation to draw.' }] };
  const badAxis: PlotScene = { ...base, panels: [{ ...base.panels[0]!, x: { ...base.panels[0]!.x, min: Number.NaN } }] };
  const badOverlay = [{ name: 'o', points: [{ x: Number.NaN, y: 1 }, { x: 1, y: Number.NaN }] }];
  const broken = { title: 'x' } as unknown as PlotScene;

  for (const [label, scene] of [['empty', empty], ['noSeries', noSeries], ['emptySeries', emptySeries], ['nan', nan], ['emptyPanel', emptyPanel], ['badAxis', badAxis], ['broken', broken]] as const) {
    const { warnings } = await runHeadless(scene, { overlay: badOverlay });
    assert.deepEqual(warnings, [], label);
  }
  assert.ok(JSON.stringify(sceneToVegaLite(empty, { colors })).includes(NOTHING_TO_PLOT));
  assert.ok(JSON.stringify(sceneToVegaLite(emptyPanel, { colors })).includes('No observation to draw.'));
  // Only the finite point of the NaN series is drawn.
  const { view } = await runHeadless(nan);
  const circles = marks(view).filter(({ node }) => node.marktype === 'symbol').flatMap(({ node }) => node.items ?? []);
  assert.deepEqual(circles.filter((c) => c.datum?.series === 's').map((c) => c.datum?.x), [3]);
});

test('the aria label names the scene and its panels', () => {
  const scene = loadScene('fit_profile_both');
  assert.equal(aria(scene), 'Observed and fitted curve of Fit pk3.iv_bolus to pk3, subject A. Panels: Linear, Semi-log.');
});

test('no number is formatted or computed in src/ but the one scale mapping', () => {
  const dir = new URL('../src/', import.meta.url).pathname;
  const banned = ['toFixed', 'toPrecision', 'Intl.NumberFormat', 'toLocaleString', 'Math.round', 'Math.log', 'Math.pow', 'Math.exp', 'format:', 'formatType'];
  for (const file of readdirSync(dir)) {
    if (!/\.(ts|tsx)$/.test(file) || file === 'scene.ts') continue;
    const text = readFileSync(join(dir, file), 'utf8');
    for (const word of banned) {
      assert.equal(text.includes(word), false, `${file} contains ${word}`);
    }
    // The only arithmetic on data: log(datum.…) / LN10, in spec.ts.
    const lines = text.split('\n').filter((l) => /\blog\(|LN10/.test(l) && !l.trim().startsWith('//') && !l.trim().startsWith('*'));
    if (file === 'spec.ts') assert.equal(lines.length, 1, `one scale mapping, found: ${lines.join(' | ')}`);
    else assert.deepEqual(lines, [], file);
  }
});
