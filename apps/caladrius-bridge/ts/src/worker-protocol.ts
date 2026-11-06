// The messages between the page and the engine worker. Every message to the worker is a request
// with an id; every message from it is the answer with the same id, or an error with it.

/** Page to worker. */
export interface ToWorker {
  id: number;
  request: string;
}

/** Worker to page. */
export type FromWorker =
  /** The module is compiled and the engine is made. The page may instantiate the module too. */
  | { kind: 'ready'; module: WebAssembly.Module }
  /** The worker could not start (the module did not load). */
  | { kind: 'failed'; message: string }
  | { kind: 'answer'; id: number; answer: string }
  | { kind: 'error'; id: number; message: string };
