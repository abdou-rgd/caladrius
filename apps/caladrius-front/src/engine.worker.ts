// The engine worker: loads the WebAssembly module of `caladrius-bridge` and answers one request at a time.
// The module and its glue are in `public/pkg/` (`npm run wasm`).

import { serve, type BridgeGlue, type LoadedBridge, type WorkerScope } from '@caladrius/bridge-client';

async function load(): Promise<LoadedBridge> {
  const glueUrl = new URL(/* @vite-ignore */ '../pkg/caladrius_bridge.js', import.meta.url).href;
  const wasmUrl = new URL(/* @vite-ignore */ '../pkg/caladrius_bridge_bg.wasm', import.meta.url).href;
  const glue = (await import(/* @vite-ignore */ glueUrl)) as BridgeGlue;
  const response = await fetch(wasmUrl);
  if (!response.ok) throw new Error(`the engine module could not be loaded (${response.status})`);
  const module = await WebAssembly.compile(await response.arrayBuffer());
  glue.initSync({ module });
  return { glue, module };
}

serve(self as unknown as WorkerScope, load);
