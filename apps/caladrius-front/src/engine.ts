// How the page reaches the engine. One build, two ways in:
//  - in a browser: the Web Worker transport of the bridge client (the engine is WebAssembly in a worker);
//  - in the Tauri 2 shell: the Tauri transport (the engine is in the Rust process).
// Number formatting is the same in both: the `caladrius-format` code, as WebAssembly in the page
// itself (the client's `createFormatter`), so a number is formatted while it is drawn, without a message.

import {
  TauriTransport,
  WorkerTransport,
  createFormatter,
  type BridgeGlue,
  type EngineClient,
  type Formatter,
  type SyncGlue,
} from '@caladrius/bridge-client';

export type Way = 'worker' | 'tauri';

export interface Connection {
  way: Way;
  client: EngineClient;
  fmt: Formatter;
  /** The registry of commands, as `describe()` of the module gives it. */
  describe: unknown;
}

export function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** The URL of a file the build copies next to the page (`public/`). */
export function asset(path: string): string {
  return new URL(path, document.baseURI).href;
}

async function compile(): Promise<WebAssembly.Module> {
  const response = await fetch(asset('pkg/caladrius_bridge_bg.wasm'));
  if (!response.ok) throw new Error(`the engine module could not be loaded (${response.status})`);
  return WebAssembly.compile(await response.arrayBuffer());
}

export async function connect(): Promise<Connection> {
  // A variable, not a literal: the glue is written by `npm run wasm`, not by the bundler.
  const glue = (await import(/* @vite-ignore */ asset('pkg/caladrius_bridge.js'))) as BridgeGlue & SyncGlue;
  let way: Way;
  let client: EngineClient;
  let module: WebAssembly.Module;
  if (inTauri()) {
    way = 'tauri';
    client = new TauriTransport();
    module = await compile();
  } else {
    const worker = new Worker(new URL('./engine.worker.ts', import.meta.url), { type: 'module' });
    const transport = new WorkerTransport(worker);
    way = 'worker';
    client = transport;
    module = await transport.module();
  }
  const fmt = createFormatter(glue, module);
  return { way, client, fmt, describe: JSON.parse(glue.describe()) as unknown };
}
