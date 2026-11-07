import { useEffect, useMemo, useState, type KeyboardEvent } from 'react';
import { chipsFor } from './roles.ts';
import { okOf } from './session.ts';
import type { DataRow } from './types.ts';
import { useScreen, useSession } from './hooks.ts';

interface NumberCellProps {
  label: string;
  shown: string;
  onCommit: (text: string) => void;
}

/** A cell of the table: edited in place, committed with Enter or when it loses focus, undone with Escape. */
function NumberCell({ label, shown, onCommit }: NumberCellProps) {
  const [text, setText] = useState(shown);
  useEffect(() => setText(shown), [shown]);
  const commit = () => {
    if (text.trim() !== shown) onCommit(text);
  };
  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      commit();
      e.currentTarget.blur();
    } else if (e.key === 'Escape') {
      setText(shown);
      e.currentTarget.blur();
    }
  };
  return (
    <input
      className="num"
      inputMode="decimal"
      aria-label={label}
      value={text}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={keys}
    />
  );
}

export function DataPanel() {
  const session = useSession();
  const state = useScreen();
  const rows: DataRow[] = useMemo(() => state.rows.filter((r) => r.subject === state.subject), [state.rows, state.subject]);
  const ok = okOf(state);
  const tmax = ok?.parameters.find((p) => p.name === 'tmax')?.value.value;
  const chips = useMemo(() => chipsFor(rows, state.scene, ok, tmax, session.fmt), [rows, state.scene, ok, tmax, session.fmt]);
  const fmt = session.fmt;

  const move = (e: KeyboardEvent<HTMLTableRowElement>, row: DataRow) => {
    if (e.target !== e.currentTarget) return;
    const rowEl = e.currentTarget;
    if (e.key === 'ArrowDown') {
      (rowEl.nextElementSibling as HTMLElement | null)?.focus();
      e.preventDefault();
    } else if (e.key === 'ArrowUp') {
      (rowEl.previousElementSibling as HTMLElement | null)?.focus();
      e.preventDefault();
    } else if (e.key === ' ' || e.key === 'Enter') {
      if (row.conc !== null && row.conc > 0) void session.togglePoint(row.time);
      e.preventDefault();
    }
  };

  return (
    <section className="panel data-panel" aria-label="Data">
      <div className="panel-head">
        <span>
          {state.info ? state.info.name : 'No data'}
          {state.info ? <span className="muted"> · {fmt.number(state.info.rows)} rows, subject {state.subject}</span> : null}
        </span>
      </div>
      <div className="scroll">
        <table className="data" aria-label="Time, concentration and role of each point">
          <thead>
            <tr>
              <th>Time</th>
              <th>Concentration</th>
              <th>Role</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.index} tabIndex={0} onKeyDown={(e) => move(e, row)} data-row={row.index} title="Space or Enter: take this point in or out of the terminal phase">
                <td>
                  <NumberCell label={`Time of row ${fmt.number(row.index + 1)}`} shown={fmt.exact(row.time)} onCommit={(t) => void session.editCell(row, 'time', t)} />
                </td>
                <td>
                  <NumberCell
                    label={`Concentration of row ${fmt.number(row.index + 1)}`}
                    shown={row.conc === null ? '' : fmt.exact(row.conc)}
                    onCommit={(t) => void session.editCell(row, 'concentration', t)}
                  />
                </td>
                <td className="chips">
                  {(chips.get(row.index) ?? []).map((c, i) => (
                    <span key={i} className={`chip chip-${c.kind}`}>
                      {c.text}
                    </span>
                  ))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="hint pad">Edit a cell and the analysis is stale until it runs again.</p>
    </section>
  );
}
