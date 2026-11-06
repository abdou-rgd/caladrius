// The shape of the glue that `wasm-bindgen --target web` writes for `caladrius-bridge`
// (`caladrius_bridge.js`), as far as this client uses it. The real glue is not compiled with the
// client: the page and the worker import it by its URL.

/** The engine of a worker: one instance, one history. */
export interface WasmBridge {
  execute(request: string): string;
  free?(): void;
}

/** The formatting functions of `caladrius-format`, as the module exports them. */
export interface FormatGlue {
  formatNumber(x: number, digits: number, comma: boolean): string;
  formatFitQuality(x: number, digits: number, comma: boolean): string;
  formatExact(x: number, digits: number, comma: boolean): string;
  formatExactPoint(x: number): string;
  parseNumber(text: string): number | undefined;
  parameterLabel(name: string, extravascular: boolean): string;
  parameterUnit(
    name: string,
    time: string | undefined,
    conc: string | undefined,
    derived: string,
  ): string | undefined;
  withLabels(sentence: string): string;
  plain(text: string): string;
  words(code: string): string;
}

/** The whole glue. */
export interface BridgeGlue extends FormatGlue {
  Bridge: new (provenance: boolean) => WasmBridge;
  describe(): string;
  /** Instantiates the module from a compiled one, without a download. */
  initSync(input: { module: WebAssembly.Module }): unknown;
}

/** The glue, with the module compiled from the file next to it. */
export interface LoadedBridge {
  glue: BridgeGlue;
  module: WebAssembly.Module;
}
