// The module script of the engine worker: `new Worker(new URL('./worker-entry.js', ...), { type:
// 'module' })`. It expects `caladrius_bridge.js` and `caladrius_bridge_bg.wasm` (the output of
// `wasm-bindgen --target web`, which `cargo xtask bridge` writes to <target>/bridge/web) next to it,
// under `pkg/`.

import { serve, type WorkerScope } from './worker.js';
import type { BridgeGlue, LoadedBridge } from './wasm-module.js';

async function load(): Promise<LoadedBridge> {
  // A variable, not a literal: the glue is made by the build of the module, not by tsc.
  const glueUrl = new URL('../pkg/caladrius_bridge.js', import.meta.url).href;
  const wasmUrl = new URL('../pkg/caladrius_bridge_bg.wasm', import.meta.url);
  const glue = (await import(glueUrl)) as BridgeGlue;
  const module = await WebAssembly.compileStreaming(fetch(wasmUrl));
  glue.initSync({ module });
  return { glue, module };
}

serve(self as unknown as WorkerScope, load);
