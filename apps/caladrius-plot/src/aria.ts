import type { PlotScene } from './scene.ts';

/** The text alternative of the canvas: the scene title, then the panel titles. */
export function sceneAriaLabel(scene: PlotScene): string {
  const panels = Array.isArray(scene.panels) ? scene.panels : [];
  const titles = panels.map((p) => p.title).filter((t) => t !== '');
  const head = scene.title === '' ? 'Plot' : scene.title;
  return titles.length === 0 ? head : `${head}. Panels: ${titles.join(', ')}.`;
}
