import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { parse, View } from 'vega';
import { compile } from 'vega-lite';
import type { PlotScene, SceneAnswer } from '../src/scene.ts';
import { sceneToVegaLite } from '../src/spec.ts';
import type { PlotColors, SpecOptions } from '../src/spec.ts';

export const colors: PlotColors = {
  observed: '#111111',
  predicted: '#ff0000',
  fit: '#fe0000',
  selected: '#0000ff',
  start: '#00ff00',
  replaced: '#ffaa00',
  other: '#888888',
  subjects: ['#a00001', '#a00002', '#a00003'],
  grid: '#dddddd',
  background: '#ffffff',
  text: '#000001',
  muted: '#555555',
};

const here = new URL('.', import.meta.url).pathname;
export const fixtureDir = join(here, 'fixtures');

export function fixtureNames(): string[] {
  return readdirSync(fixtureDir)
    .filter((f) => f.endsWith('.json'))
    .map((f) => f.replace(/\.json$/, ''))
    .sort();
}

export function loadScene(name: string): PlotScene {
  const answer = JSON.parse(readFileSync(join(fixtureDir, `${name}.json`), 'utf8')) as SceneAnswer;
  return answer.scene;
}

export interface Compiled {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  vega: any;
  warnings: string[];
}

function collector(warnings: string[]) {
  return {
    level() {
      return this;
    },
    warn(...a: unknown[]) {
      warnings.push(a.map(String).join(' '));
      return this;
    },
    error(...a: unknown[]) {
      warnings.push(`error: ${a.map(String).join(' ')}`);
      return this;
    },
    info() {
      return this;
    },
    debug() {
      return this;
    },
  };
}

/** Compiles the Vega-Lite spec of a scene, collecting every warning and error. */
export function compileScene(scene: PlotScene, options: Partial<SpecOptions> = {}, warnings: string[] = []): Compiled {
  const spec = sceneToVegaLite(scene, { colors, ...options });
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const vega = compile(spec, { logger: collector(warnings) as any }).spec;
  return { vega, warnings };
}

/** A headless view, run once. Warnings of Vega-Lite and of Vega are collected together. */
export async function runHeadless(scene: PlotScene, options: Partial<SpecOptions> = {}): Promise<{ view: View; warnings: string[] }> {
  const warnings: string[] = [];
  const { vega } = compileScene(scene, options, warnings);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const view = new View(parse(vega), { renderer: 'none', logger: collector(warnings) as any });
  await view.runAsync();
  return { view, warnings };
}

export interface SceneNode {
  marktype?: string;
  role?: string;
  name?: string;
  items?: SceneItem[];
}
export interface SceneItem {
  text?: unknown;
  datum?: Record<string, unknown>;
  strokeDash?: number[];
  items?: SceneNode[];
  [key: string]: unknown;
}

/** Every mark node of the scenegraph, depth first, with the panel (concat_N) it sits in. */
export function walk(node: SceneNode, panel: number | undefined, out: { node: SceneNode; panel: number | undefined }[]): void {
  const m = /^concat_(\d+)_group$/.exec(node.name ?? '');
  const here = m?.[1] !== undefined ? Number(m[1]) : panel;
  out.push({ node, panel: here });
  for (const item of node.items ?? []) {
    for (const child of item.items ?? []) walk(child, here, out);
  }
}

export function marks(view: View): { node: SceneNode; panel: number | undefined }[] {
  const out: { node: SceneNode; panel: number | undefined }[] = [];
  walk((view.scenegraph() as unknown as { root: SceneNode }).root, undefined, out);
  return out;
}
