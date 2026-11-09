// The global filter. It matches the text the user reads, from the formatted-text cache, never
// the raw values and never by calling the formatter.

/** The words of a query: lower case, split on white space; a row must contain every word. */
export function queryWords(query: string): string[] {
  return query.toLowerCase().split(/\s+/).filter((w) => w.length > 0);
}

/**
 * Which rows match: `mask[row]` is 1 when the row contains every word in at least one of the
 * columns (`lower[c][row]` is the lower-cased text of row `row` in column `c`). The scan runs in
 * row order, so it reads the cache sequentially whatever the sort.
 */
export function filterMask(
  rows: number,
  words: readonly string[],
  lower: readonly (readonly (string | undefined)[])[],
): Uint8Array {
  const mask = new Uint8Array(rows);
  for (let row = 0; row < rows; row += 1) {
    let all = true;
    for (let w = 0; w < words.length && all; w += 1) {
      const word = words[w] ?? '';
      let found = false;
      for (let c = 0; c < lower.length; c += 1) {
        const text = lower[c]?.[row];
        if (text !== undefined && text.includes(word)) {
          found = true;
          break;
        }
      }
      all = found;
    }
    if (all) mask[row] = 1;
  }
  return mask;
}

/** The rows of `order` that the mask keeps, in the same order. */
export function applyMask(order: Uint32Array, mask: Uint8Array): Uint32Array {
  const out = new Uint32Array(order.length);
  let n = 0;
  for (let k = 0; k < order.length; k += 1) {
    const row = order[k] ?? 0;
    if (mask[row] === 1) {
      out[n] = row;
      n += 1;
    }
  }
  return out.slice(0, n);
}
