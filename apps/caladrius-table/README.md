# @caladrius/table

Result tables of the Caladrius interface: engine values shown as they are. Built on TanStack Table v9
(column model and state) and TanStack Virtual (rows and columns drawn on demand). The package never
computes, rounds or formats a number: every number goes through the `NumberFormatter` it is given (the
bridge's `Formatter` in the app), and a test greps `src/` to keep it that way.

```tsx
import { DataTable, toTsv, type TableSpec } from '@caladrius/table';
import '@caladrius/table/table.css';

<DataTable spec={spec} formatter={formatter} displayKey={displayKey} label="Fit estimates" dense onSelectRow={…} />
```

- `displayKey` changes when the digits or the decimal mark change: it drops the formatted-text cache.
- Sorting is on raw values (an index permutation); null, NaN and ±Infinity go last in both directions.
- The global filter matches the displayed text, from a per-column cache; a second filter formats nothing.
- Copy: the selected rows (or the whole view) as TSV with a header row; "Copy exact values" uses `Formatter.exact`.
- Styles: CSS variables only (`--c-*`, `--space-*`, `--radius-*`, `--font-*`, `--size-*`, values with their `px`).

## Commands (from `apps/`, one `node_modules`)

```sh
npm test -w @caladrius/table          # tsc + node:test (pure modules)
npm run build -w @caladrius/table     # type check + library build (dist/)
npm run demo -w @caladrius/table      # demo at http://127.0.0.1:5173 (add --host 127.0.0.1 if needed)
npm run test:dom -w @caladrius/table  # DOM checks in Chromium against a running demo (see the file header)
npm run measure -w @caladrius/table   # 100,000 x 20 measures in Chromium -> measure-local.json
```

Demo views: `?view=fit`, `?view=big`, `?view=edge`, `?view=boom`; `?theme=dark`. The demo's formatter is a fake.
Dependencies and licences: `@tanstack/react-table` 9.3.1 (MIT), `@tanstack/react-virtual` 3.14.26 (MIT),
and their MIT dependencies `@tanstack/table-core`, `@tanstack/virtual-core`, `@tanstack/store`, `@tanstack/react-store`
(and `use-sync-external-store`).
