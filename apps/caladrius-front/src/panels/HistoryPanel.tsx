import { useState, type ReactNode } from 'react';
import { useSessionState } from '../session/hooks.ts';

/** The log of engine calls, newest first. The live preview's calls are hidden unless asked for. */
export function HistoryPanel(): ReactNode {
  const { log } = useSessionState();
  const [quiet, setQuiet] = useState(false);
  const shown = quiet ? log : log.filter((e) => !e.quiet);
  return (
    <div className="panel">
      <div className="row small">
        <label className="row">
          <input type="checkbox" checked={quiet} onChange={(e) => setQuiet(e.target.checked)} />
          <span className="muted">Show the calls of the live preview</span>
        </label>
        <span className="spacer" />
        <span className="muted">{shown.length} calls</span>
      </div>
      {shown.length === 0 ? <p className="muted">Nothing yet: each command the page sends to the engine is listed here with its seal.</p> : null}
      <ul className="log" aria-label="History">
        {shown.map((e) => (
          <li key={e.n}>
            {e.json !== null || e.message !== null ? (
              <details>
                <summary>
                  <Line entry={e} />
                </summary>
                {e.message !== null ? <p className="err small" style={{ margin: '2px 0' }}>{e.message}</p> : null}
                {e.json !== null ? <pre>{e.json}</pre> : null}
              </details>
            ) : (
              <div style={{ display: 'flex', gap: 'var(--space-large)', alignItems: 'baseline' }}>
                <Line entry={e} />
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Line(props: { entry: ReturnType<typeof useSessionState>['log'][number] }): ReactNode {
  const { entry } = props;
  return (
    <>
      <span className="n">{entry.n}</span>
      <span className="cmd">{entry.command}</span>
      <span className={entry.ok ? 'muted' : 'err'}>{entry.ok ? 'ok' : (entry.code ?? 'error')}</span>
      <span className="mono muted">{entry.seal === null ? '' : entry.seal.slice(0, 8)}</span>
    </>
  );
}
