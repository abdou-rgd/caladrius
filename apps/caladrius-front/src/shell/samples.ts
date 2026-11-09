import { asset } from '../engine.ts';
import { DEFAULT_DATASETS } from '../session/session.ts';
import type { Session } from '../session/session.ts';

/** Fetches one of the sample CSVs the build copies to `public/data/` and imports it. */
export async function loadSample(session: Session, id: string): Promise<void> {
  const sample = DEFAULT_DATASETS.find((d) => d.id === id);
  if (sample === undefined) return;
  try {
    const response = await fetch(asset(sample.file));
    if (!response.ok) throw new Error(`${sample.file}: ${response.status}`);
    await session.importCsv(sample.id, await response.text(), { compartments: sample.compartments, route: sample.route, sample: sample.id });
  } catch (cause) {
    session.store.set((s) => ({ ...s, data: { ...s.data, busy: false, error: `The sample could not be loaded: ${cause instanceof Error ? cause.message : String(cause)}` } }));
  }
}
