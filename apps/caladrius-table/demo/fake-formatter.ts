// A FAKE formatter, for the demo and the tests only (the app passes the bridge's `Formatter`,
// the code of caladrius-format). It shows `digits` significant digits and is not the engine's.
import type { NumberFormatter } from '../src/types.ts';

export function fakeFormatter(digits: number, comma: boolean): NumberFormatter {
  const mark = (s: string): string => (comma ? s.replace('.', ',') : s);
  return {
    number: (x) => mark(x.toPrecision(digits)),
    exact: (x) => mark(x.toPrecision(17)),
  };
}
