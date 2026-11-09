// The worker side: loads the module, makes one engine, answers each request in order.

import type { FromWorker, ToWorker } from './worker-protocol.js';
import type { LoadedBridge } from './wasm-module.js';

/** What a worker global scope offers here (kept small so a test can stand in for it). */
export interface WorkerScope {
  postMessage(message: unknown): void;
  addEventListener(type: 'message', listener: (event: { data: unknown }) => void): void;
}

export interface ServeOptions {
  /** Seals carry the engine version and commit. On by default; the check of the bridge turns it off. */
  provenance?: boolean;
}

function isRequest(data: unknown): data is ToWorker {
  if (typeof data !== 'object' || data === null) return false;
  const record = data as Record<string, unknown>;
  return typeof record['id'] === 'number' && typeof record['request'] === 'string';
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Starts the engine of this worker. Requests that arrive while the module loads wait for it, and
 * are answered in the order they came.
 */
export function serve(
  scope: WorkerScope,
  load: () => Promise<LoadedBridge>,
  options: ServeOptions = {},
): void {
  const post = (message: FromWorker): void => scope.postMessage(message);
  const started = load().then(({ glue, module }) => {
    const engine = new glue.Bridge(options.provenance ?? true);
    post({ kind: 'ready', module });
    return engine;
  });
  started.catch((error: unknown) => post({ kind: 'failed', message: messageOf(error) }));

  let queue: Promise<void> = Promise.resolve();
  scope.addEventListener('message', (event) => {
    const data = event.data;
    if (!isRequest(data)) return;
    queue = queue.then(async () => {
      try {
        const engine = await started;
        post({ kind: 'answer', id: data.id, answer: engine.execute(data.request) });
      } catch (error) {
        post({ kind: 'error', id: data.id, message: messageOf(error) });
      }
    });
  });
}
