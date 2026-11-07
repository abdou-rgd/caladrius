// The measure mode of the wave 3 brief (the same contract for the three prototypes).
//
//  1. Load nca50.csv (50 subjects of 20 points).
//  2. 100 drags. Drag k takes subject 1 + k % 50 and sets the λz start so that the range holds the
//     last 3, 4, 5 or 6 points (CYCLE[k % 4]). Choosing the subject comes first and is not timed; the
//     timed part is the handler a pointer drag of the start handle calls (`setRangeStart`) up to the
//     frame that shows the new result.
//  3. `input_to_paint_ms`: handler called -> `nca.run` and `plot.scene` answered -> state rendered ->
//     the first frame after it (a requestAnimationFrame callback, then a message, which runs after
//     the paint). `frame_ms`: the time between that frame and the one before it.
//  4. Every result cell on screen is compared with the value formatted from the answer.
//  5. Wait 3 seconds (the memory is sampled then), then exit; a browser page stays open.

import { okOf, statusOf, subjectResult, type Session, type State } from './session.ts';
import { allCells, headlineCells, type Cell, type Fmt, type Units } from './results.ts';

export const DRAGS = 100;
export const CYCLE: readonly number[] = [3, 4, 5, 6];

export interface MeasureReport {
  stack: 'react';
  version: string;
  first_frame_ms: number;
  drags: number;
  input_to_paint_ms: number[];
  frame_ms: number[];
  cells_checked: number;
  cells_equal: number;
  errors: string[];
}

/** The frame that shows what has just been rendered: after the next paint. */
export function nextPaint(timeoutMs = 2000): Promise<{ at: number; timedOut: boolean }> {
  return new Promise((resolve) => {
    let done = false;
    const finish = (timedOut: boolean) => {
      if (done) return;
      done = true;
      resolve({ at: performance.now(), timedOut });
    };
    const timer = setTimeout(() => finish(true), timeoutMs);
    requestAnimationFrame(() => {
      const channel = new MessageChannel();
      channel.port1.onmessage = () => {
        clearTimeout(timer);
        finish(false);
      };
      channel.port2.postMessage(0);
    });
  });
}

export function unitsOf(state: State): Units {
  const cols = state.info?.columns ?? [];
  const unit = (role: string) => cols.find((c) => c.role === role)?.unit ?? undefined;
  return { time: unit('time'), conc: unit('concentration'), derived: state.info?.derived_units ?? {} };
}

/** What every cell on screen should say, by its `data-cell` key, formatted from the answer. */
export function expectedCells(state: State, fmt: Fmt): Map<string, string> {
  const out = new Map<string, string>();
  const subject = subjectResult(state);
  const ok = okOf(state);
  if (!subject || !ok) return out;
  const units = unitsOf(state);
  const put = (prefix: string, cells: Cell[]) => {
    for (const c of cells) {
      out.set(`${prefix}:${c.key}`, c.text);
      out.set(`${prefix}-unit:${c.key}`, c.unit);
    }
  };
  put('h', headlineCells(subject, ok, fmt, units, true));
  put('a', allCells(subject, ok, fmt, units, true));
  ok.lambda_z_candidates.forEach((c, i) => {
    out.set(`c${i}:n`, fmt.number(c.n_points));
    out.set(`c${i}:from`, fmt.number(c.time_first));
    out.set(`c${i}:to`, fmt.number(c.time_last));
    out.set(`c${i}:lambda_z`, fmt.number(c.lambda_z));
    out.set(`c${i}:adj`, c.adj_r_squared === null ? '-' : fmt.fitQuality(c.adj_r_squared));
  });
  return out;
}

/** Compares the cells in the document with the answer. */
export function compareCells(state: State, fmt: Fmt, root: ParentNode): { checked: number; equal: number; errors: string[] } {
  const expected = expectedCells(state, fmt);
  const errors: string[] = [];
  let checked = 0;
  let equal = 0;
  for (const el of Array.from(root.querySelectorAll<HTMLElement>('[data-cell]'))) {
    const key = el.dataset['cell'] ?? '';
    const want = expected.get(key);
    checked += 1;
    const got = (el.textContent ?? '').trim();
    if (want !== undefined && got === want.trim()) equal += 1;
    else errors.push(`cell ${key}: shows "${got}", the answer formats to "${want ?? '(none)'}"`);
  }
  return { checked, equal, errors };
}

export interface MeasureOptions {
  stack: 'react';
  version: string;
  /** Milliseconds from the start of the process (desktop) or of the page (browser) to the first frame. */
  firstFrameMs: number;
  csv: string;
}

export async function runMeasure(session: Session, root: ParentNode, options: MeasureOptions): Promise<MeasureReport> {
  const report: MeasureReport = {
    stack: options.stack,
    version: options.version,
    first_frame_ms: options.firstFrameMs,
    drags: DRAGS,
    input_to_paint_ms: [],
    frame_ms: [],
    cells_checked: 0,
    cells_equal: 0,
    errors: [],
  };
  try {
    await session.loadDataset('nca50', options.csv, '1');
    let lastFrame = (await nextPaint()).at;
    const subjects = session.getState().info?.subjects ?? [];
    for (let k = 0; k < DRAGS; k += 1) {
      const subject = String(1 + (k % 50));
      if (!subjects.includes(subject)) {
        report.errors.push(`drag ${k}: no subject ${subject}`);
        continue;
      }
      await session.selectSubject(subject);
      const samples = session.samples();
      const n = CYCLE[k % CYCLE.length] ?? 3;
      const start = samples[samples.length - n];
      if (start === undefined) {
        report.errors.push(`drag ${k}: subject ${subject} has fewer than ${n} samples`);
        continue;
      }
      await nextPaint();
      const t0 = performance.now();
      await session.setRangeStart(start);
      const painted = await nextPaint();
      report.input_to_paint_ms.push(painted.at - t0);
      report.frame_ms.push(painted.at - lastFrame);
      lastFrame = painted.at;
      if (painted.timedOut) report.errors.push(`drag ${k}: no frame within the time limit`);
      const state = session.getState();
      if (state.error) report.errors.push(`drag ${k}: ${state.error}`);
      else if (statusOf(state) !== 'fresh') report.errors.push(`drag ${k}: the result is not fresh`);
    }
    // Every cell on screen, with "All parameters" open.
    const details = Array.from(root.querySelectorAll<HTMLDetailsElement>('details[data-measure-open]'));
    for (const d of details) d.open = true;
    await nextPaint();
    const compared = compareCells(session.getState(), session.fmt, root);
    report.cells_checked = compared.checked;
    report.cells_equal = compared.equal;
    report.errors.push(...compared.errors.slice(0, 20));
  } catch (error) {
    report.errors.push(`the measure stopped: ${error instanceof Error ? error.message : String(error)}`);
  }
  return report;
}
