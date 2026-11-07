// The role chips of the data table. The roles come from the NCA scene of `plot.scene`: the series a
// time falls in (`lambda_z_start`, `lambda_z_used`, `excluded`, `replaced`, `observed`), the points
// below the limit it marks, and the points a logarithmic axis cannot show (listed under `hidden`).
// Nothing is worked out here: a row with no place in the scene says so.

import type { Fmt } from './results.ts';
import type { DataRow, NcaOk, Scene, SceneRoleName } from './types.ts';

export type ChipKind = 'start' | 'used' | 'excluded' | 'replaced' | 'observed' | 'blq' | 'tmax' | 'removed' | 'none';

export interface Chip {
  kind: ChipKind;
  text: string;
}

/** The roles of a table row, strongest first. */
const RANK: SceneRoleName[] = ['lambda_z_start', 'lambda_z_used', 'excluded', 'replaced', 'observed'];

const KIND: Partial<Record<SceneRoleName, ChipKind>> = {
  lambda_z_start: 'start',
  lambda_z_used: 'used',
  excluded: 'excluded',
  replaced: 'replaced',
  observed: 'observed',
};

/** The role of each time of the subject, from the first panel of the scene. */
export function rolesByTime(scene: Scene | null): Map<number, { role: SceneRoleName; blq: boolean }> {
  const out = new Map<number, { role: SceneRoleName; blq: boolean }>();
  const panel = scene?.panels[0];
  if (!scene || !panel) return out;
  const consider = (time: number, role: SceneRoleName, blq: boolean) => {
    const have = out.get(time);
    if (!have || RANK.indexOf(role) < RANK.indexOf(have.role)) out.set(time, { role, blq: blq || (have?.blq ?? false) });
    else if (blq && have) have.blq = true;
  };
  for (const s of panel.series) {
    if (!RANK.includes(s.role)) continue;
    for (const p of s.points) consider(p.x, s.role, p.blq === true);
  }
  const nameToRole = new Map(scene.legend.entries.map((e) => [e.name, e.role]));
  for (const h of panel.hidden?.points ?? []) {
    const role = nameToRole.get(h.series);
    if (role && RANK.includes(role)) consider(h.x, role, false);
  }
  return out;
}

/** The legend name of a role, as the engine wrote it. */
function nameOf(scene: Scene, role: SceneRoleName): string {
  return scene.legend.entries.find((e) => e.role === role)?.name ?? role;
}

/** The chips of every row of the subject (keyed by the row number of the worksheet). */
export function chipsFor(rows: DataRow[], scene: Scene | null, ok: NcaOk | null, tmax: number | undefined, fmt: Fmt): Map<number, Chip[]> {
  const roles = rolesByTime(scene);
  const removed = new Map((ok?.removed ?? []).map((r) => [r.time, r.reason]));
  const out = new Map<number, Chip[]>();
  for (const row of rows) {
    const chips: Chip[] = [];
    const found = roles.get(row.time);
    if (scene && found) {
      const kind = KIND[found.role] ?? 'observed';
      chips.push({ kind, text: nameOf(scene, found.role) });
      if (found.blq) chips.push({ kind: 'blq', text: 'BLQ' });
    } else if (removed.has(row.time)) {
      chips.push({ kind: 'removed', text: fmt.words(removed.get(row.time) ?? '') });
    }
    if (tmax !== undefined && row.time === tmax) chips.push({ kind: 'tmax', text: 'Tmax' });
    if (chips.length === 0) chips.push({ kind: 'none', text: '-' });
    out.set(row.index, chips);
  }
  return out;
}
