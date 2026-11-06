// The Web Worker transport: the engine runs in a worker, one request at a time, and the page
// awaits each answer.

import { TextClient, TransportError } from './protocol.js';
import type { FromWorker, ToWorker } from './worker-protocol.js';

/** What the transport needs of a worker (a real `Worker` has it; a test can stand in). */
export interface WorkerLike {
  postMessage(message: unknown): void;
  addEventListener(type: 'message', listener: (event: { data: unknown }) => void): void;
  addEventListener(type: 'error', listener: (event: { message?: string }) => void): void;
  terminate(): void;
}

interface Pending {
  resolve(answer: string): void;
  reject(error: Error): void;
}

function isFromWorker(data: unknown): data is FromWorker {
  return typeof data === 'object' && data !== null && typeof (data as { kind?: unknown }).kind === 'string';
}

export class WorkerTransport extends TextClient {
  private readonly pending = new Map<number, Pending>();
  private nextId = 1;
  private failure: Error | undefined;
  private readonly started: Promise<WebAssembly.Module>;

  constructor(private readonly worker: WorkerLike) {
    super();
    let ready: (module: WebAssembly.Module) => void = () => {};
    let broken: (error: Error) => void = () => {};
    this.started = new Promise((resolve, reject) => {
      ready = resolve;
      broken = reject;
    });
    // A page that never asks for the module must not see an unhandled rejection from it.
    this.started.catch(() => {});

    worker.addEventListener('message', (event) => {
      const message = event.data;
      if (!isFromWorker(message)) return;
      switch (message.kind) {
        case 'ready':
          ready(message.module);
          break;
        case 'failed':
          this.fail(new TransportError(`the engine worker did not start: ${message.message}`), broken);
          break;
        case 'answer':
          this.settle(message.id)?.resolve(message.answer);
          break;
        case 'error':
          this.settle(message.id)?.reject(new TransportError(`the engine worker failed: ${message.message}`));
          break;
      }
    });
    worker.addEventListener('error', (event) => {
      this.fail(new TransportError(`the engine worker crashed: ${event.message ?? 'no message'}`), broken);
    });
  }

  /**
   * The compiled module of the engine. The page instantiates it again for synchronous formatting
   * (`createFormatter`), so the module is downloaded and compiled once.
   */
  module(): Promise<WebAssembly.Module> {
    return this.started;
  }

  send(request: string): Promise<string> {
    if (this.failure) return Promise.reject(this.failure);
    const id = this.nextId++;
    return new Promise<string>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      const message: ToWorker = { id, request };
      this.worker.postMessage(message);
    });
  }

  close(): void {
    this.fail(new TransportError('the engine worker was closed'), () => {});
    this.worker.terminate();
  }

  private settle(id: number): Pending | undefined {
    const entry = this.pending.get(id);
    this.pending.delete(id);
    return entry;
  }

  /** From now on every call fails with this error, and the calls waiting fail with it too. */
  private fail(error: Error, broken: (error: Error) => void): void {
    this.failure ??= error;
    broken(error);
    for (const entry of this.pending.values()) entry.reject(error);
    this.pending.clear();
  }
}
