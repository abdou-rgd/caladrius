import { statusOf, subjectResult } from './session.ts';
import { useScreen, useSession } from './hooks.ts';

export interface TopBarProps {
  datasets: string[];
  onDataset: (name: string) => void;
}

export function TopBar({ datasets, onDataset }: TopBarProps) {
  const session = useSession();
  const state = useScreen();
  const status = statusOf(state);
  const result = subjectResult(state);
  const label = state.view?.label ?? 'New NCA';
  return (
    <header className="topbar" role="toolbar" aria-label="Analysis">
      <strong className="brand">Caladrius</strong>
      <label className="field">
        <span>Dataset</span>
        <select value={state.datasetName} onChange={(e) => onDataset(e.target.value)} aria-label="Dataset">
          {datasets.map((d) => (
            <option key={d} value={d}>
              {d}
            </option>
          ))}
        </select>
      </label>
      <span className="analysis-label" title="The analysis">
        {label}
      </span>
      <label className="field">
        <span>Subject</span>
        <select
          value={state.subject}
          onChange={(e) => void session.selectSubject(e.target.value)}
          aria-label="Subject"
          aria-keyshortcuts="n p"
        >
          {(state.info?.subjects ?? []).map((s) => (
            <option key={s} value={s}>
              {s}
            </option>
          ))}
        </select>
      </label>
      <button type="button" className="primary" onClick={() => void session.run()} aria-keyshortcuts="r">
        Run
      </button>
      <span className={`badge badge-${status}`} role="status" aria-label="Status of the result">
        {status === 'none' ? 'not run' : status}
      </span>
      {result && 'error' in result.outcome ? <span className="hint">no result for this subject</span> : null}
      <span className="spacer" />
      {state.busy > 0 ? <span className="hint">working...</span> : null}
    </header>
  );
}
