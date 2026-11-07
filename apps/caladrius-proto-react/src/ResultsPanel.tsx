import { useState } from 'react';
import { unitsOf } from './measure.ts';
import { allCells, headlineCells, resultsTsv, terminalPhaseSentence, type Cell } from './results.ts';
import { isManual, okOf, selectedCandidate, staleSentence, statusOf, subjectResult } from './session.ts';
import { useScreen, useSession } from './hooks.ts';

function Rows({ cells, prefix }: { cells: Cell[]; prefix: 'h' | 'a' }) {
  return (
    <>
      {cells.map((c) => (
        <tr key={c.key} className={c.flagged ? 'flagged' : undefined} title={c.sentence}>
          <th scope="row">{c.label}</th>
          <td className={`value${c.calculated ? '' : ' absent'}`} data-cell={`${prefix}:${c.key}`}>
            {c.text}
          </td>
          <td className="unit" data-cell={`${prefix}-unit:${c.key}`}>
            {c.unit}
          </td>
          <td className="flagmark">{c.flagged ? <span title="A flag concerns this parameter" aria-label="flagged">!</span> : null}</td>
        </tr>
      ))}
    </>
  );
}

export function ResultsPanel() {
  const session = useSession();
  const state = useScreen();
  const fmt = session.fmt;
  const [copied, setCopied] = useState('');
  const status = statusOf(state);
  const subject = subjectResult(state);
  const ok = okOf(state);
  const units = unitsOf(state);
  const headline = subject && ok ? headlineCells(subject, ok, fmt, units, true) : [];
  const all = subject && ok ? allCells(subject, ok, fmt, units, true) : [];
  const selected = selectedCandidate(state);

  const copy = async (cells: Cell[]) => {
    const text = resultsTsv(cells);
    try {
      await navigator.clipboard.writeText(text);
      setCopied('Copied as tab-separated text, with a header row.');
    } catch {
      setCopied('The clipboard is not available here.');
    }
  };

  const failure = subject && 'error' in subject.outcome ? subject.outcome.error : null;
  const seal = state.seal;

  return (
    <section className="panel results-panel" aria-label="Results">
      <div className="panel-head">
        <span>Results · subject {state.subject}</span>
        <span className="spacer" />
        <button type="button" disabled={headline.length === 0} onClick={() => void copy(headline)}>
          Copy
        </button>
      </div>
      <div className="scroll pad">
        {state.error ? (
          <p className="error" role="alert">
            {state.error}
          </p>
        ) : null}
        {failure ? (
          <p className="error" role="alert">
            {fmt.withLabels(failure)}
          </p>
        ) : null}
        {status === 'stale' ? (
          <p className="stale-note" role="status">
            {staleSentence(state.staleReasons)}
          </p>
        ) : null}
        {ok ? (
          <>
            <p className="phase">{terminalPhaseSentence(selected, isManual(state), units.time, fmt)}</p>
            {subject?.flag_messages && subject.flag_messages.length > 0 ? (
              <ul className="flags" aria-label="Flags">
                {subject.flag_messages.map((m, i) => (
                  <li key={i}>{fmt.withLabels(m)}</li>
                ))}
              </ul>
            ) : null}
            <table className="results" aria-label="Headline parameters">
              <thead>
                <tr>
                  <th>Parameter</th>
                  <th>Value</th>
                  <th>Unit</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                <Rows cells={headline} prefix="h" />
              </tbody>
            </table>
            <details data-measure-open>
              <summary>All parameters</summary>
              <div className="row-actions">
                <button type="button" onClick={() => void copy(all)}>
                  Copy all
                </button>
              </div>
              <table className="results" aria-label="All parameters">
                <tbody>
                  <Rows cells={all} prefix="a" />
                </tbody>
              </table>
            </details>
          </>
        ) : failure || state.error ? null : (
          <p className="empty">{state.ready ? 'Run the analysis to see the results.' : 'Loading the engine...'}</p>
        )}
        <p className="hint" role="status" aria-live="polite">
          {copied}
        </p>
      </div>
      <footer className="seal" aria-label="Seal">
        {seal ? (
          <>
            <span>seal {seal.sha256.slice(0, 12)}</span>
            <span>engine {seal.engine ?? '?'}</span>
            <span>command {state.command ?? '-'}</span>
          </>
        ) : (
          <span className="muted">no seal yet</span>
        )}
      </footer>
    </section>
  );
}
