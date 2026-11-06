// Synchronous number formatting on the page: the code of `caladrius-format` (the same code the
// egui interface runs), in a WebAssembly instance of the page itself. A call costs no message and
// no await, so a table of a thousand cells is formatted while it is drawn.
//
// The module is the one of the engine worker, instantiated a second time from the same compiled
// `WebAssembly.Module` (`WorkerTransport.module()`): one download, no second file.

import type { FormatGlue } from './wasm-module.js';

/** How numbers are shown: significant digits and the decimal mark. */
export interface Display {
  digits: number;
  comma: boolean;
}

export const DEFAULT_DISPLAY: Display = { digits: 4, comma: false };

/** The glue of the page, with the call that instantiates it. */
export interface SyncGlue extends FormatGlue {
  initSync(input: { module: WebAssembly.Module }): unknown;
}

export class Formatter {
  constructor(
    private readonly glue: FormatGlue,
    private display: Display = DEFAULT_DISPLAY,
  ) {}

  /** Sets how the numbers are shown from now on. */
  setDisplay(display: Display): void {
    this.display = { ...display };
  }

  getDisplay(): Display {
    return { ...this.display };
  }

  /** Significant digits, scientific notation outside 1e-3 to 1e5; `-` for a value that is not finite. */
  number(x: number): string {
    return this.glue.formatNumber(x, this.display.digits, this.display.comma);
  }

  /** A goodness of fit, up to five decimals. */
  fitQuality(x: number): string {
    return this.glue.formatFitQuality(x, this.display.digits, this.display.comma);
  }

  /** The shortest text that reads back as the same value, with the chosen decimal mark. */
  exact(x: number): string {
    return this.glue.formatExact(x, this.display.digits, this.display.comma);
  }

  /** As `exact`, always with a point. */
  exactPoint(x: number): string {
    return this.glue.formatExactPoint(x);
  }

  /** A typed number (`3,25` and `3.25` are both 3.25); undefined if it is not a finite number. */
  parseNumber(text: string): number | undefined {
    return this.glue.parseNumber(text);
  }

  parameterLabel(name: string, extravascular: boolean): string {
    return this.glue.parameterLabel(name, extravascular);
  }

  /** The unit of a parameter; `derived` maps the derived quantities the engine names to their units. */
  parameterUnit(
    name: string,
    time: string | undefined,
    conc: string | undefined,
    derived: Record<string, string> = {},
  ): string | undefined {
    return this.glue.parameterUnit(name, time, conc, JSON.stringify(derived));
  }

  withLabels(sentence: string): string {
    return this.glue.withLabels(sentence);
  }

  plain(text: string): string {
    return this.glue.plain(text);
  }

  words(code: string): string {
    return this.glue.words(code);
  }
}

/**
 * A formatter on the glue of this page. With a compiled module (from the worker transport) the
 * glue is instantiated from it; without one the glue is already instantiated (the Node build).
 */
export function createFormatter(
  glue: SyncGlue,
  module?: WebAssembly.Module,
  display: Display = DEFAULT_DISPLAY,
): Formatter {
  if (module !== undefined) glue.initSync({ module });
  return new Formatter(glue, display);
}
