import { useMemo, useState, type ReactNode } from 'react';
import { DataTable } from '@caladrius/table';
import { estimatesSpec, diagnosticsSpec, flaggedCells } from '../results/fitTable.ts';
import { parameterLabel, parameterUnit } from '../model/parameterSets.ts';
import { useApp, useSessionState } from '../session/hooks.ts';

const ROW = 26;

export function ResultsPanel(): ReactNode {
  const { session, fmt } = useApp();
  const state = useSessionState();
  const { fit, fitError, fitBusy } = state;
  const [copied, setCopied] = useState<string | null>(null);
  const displayKey = `${state.display.digits}-${state.display.comma}`;
  const ok = fit?.body.outcome.ok;

  const worksheet = state.data.worksheet;

  const estimates = useMemo(() => {
    if (ok === undefined || fit === null) return null;
    const units = {
      time: worksheet?.columns.find((c) => c.role === 'time')?.unit ?? undefined,
      conc: worksheet?.columns.find((c) => c.role === 'concentration')?.unit ?? undefined,
      derived: worksheet?.derived_units ?? {},
    };
    const flagged = flaggedCells(ok.flags, fit.body.flag_messages, (c) => fmt.words(c));
    return estimatesSpec(ok, (name) => ({ label: parameterLabel(name, fit.extravascular), unit: parameterUnit(name, units) }), flagged, fit.order);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ok, fit, worksheet]);
  const diagnostics = useMemo(() => (ok === undefined ? null : diagnosticsSpec(ok, (c) => fmt.words(c))), [ok, fmt]);

  if (fit === null) {
    return (
      <div className="panel">
        {fitBusy ? <p className="muted">Fitting…</p> : <p className="muted">The estimates appear here once the model has been fitted.</p>}
        {fitError !== null ? (
          <div className="banner error" role="alert">
            {fitError}
          </div>
        ) : null}
      </div>
    );
  }

  const tableHeight = ROW * ((estimates?.rows.length ?? 6) + 1) + 64;
  const key = session.currentKey();
  const inputsChanged = key !== null && key !== fit.key;
  const stale = fit.engineStatus === 'stale';
  const { body } = fit;
  const seal = fit.seal;
  const copySeal = async (): Promise<void> => {
    if (seal === null) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(seal));
      setCopied('Seal copied');
    } catch {
      setCopied('The browser did not allow copying');
    }
  };

  return (
    <div className="panel" data-testid="results">
      <div className="row">
        <strong data-testid="status-message">{fmt.plain(body.status_message)}</strong>
        <span className="muted small">{fit.body.subject !== undefined ? `subject ${String(body.subject)}` : ''}</span>
        {stale ? <span className="badge stale" title="The data changed after this fit">Stale: the data changed</span> : null}
        {!stale && inputsChanged ? <span className="badge stale" title="The inputs differ from the ones this fit was run with">Stale: inputs changed since the fit</span> : null}
        {fitBusy ? <span className="muted small">Fitting…</span> : null}
      </div>
      {fitError !== null ? (
        <div className="banner error" role="alert">
          {fitError}
        </div>
      ) : null}
      {body.outcome.error !== undefined ? (
        <div className="banner error" role="alert">
          {fmt.plain(body.outcome.error)}
        </div>
      ) : null}
      {body.flag_messages.length > 0 ? (
        <div className="banner" aria-label="Check these" data-testid="flags">
          <strong>Check these</strong>
          <ul>
            {body.flag_messages.map((m) => (
              <li key={m}>{fmt.plain(m)}</li>
            ))}
          </ul>
        </div>
      ) : null}
      {estimates !== null && diagnostics !== null ? (
        <div className="results-grid">
          <div>
            <h3 className="section">Estimates</h3>
            <DataTable spec={estimates} formatter={fmt} displayKey={displayKey} label="Fit estimates" dense height={tableHeight} />
          </div>
          <div>
            <h3 className="section">Goodness of fit</h3>
            <DataTable spec={diagnostics} formatter={fmt} displayKey={displayKey} label="Fit diagnostics" dense height={tableHeight} />
          </div>
        </div>
      ) : null}
      <div className="row small muted" data-testid="seal">
        <span>Seal</span>
        <code className="mono">{seal === null ? 'none' : seal.sha256.slice(0, 12)}</code>
        {seal?.engine !== undefined ? <span>engine {seal.engine}</span> : null}
        <button className="btn small" disabled={seal === null} onClick={() => void copySeal()}>
          Copy seal
        </button>
        {copied !== null ? <span role="status">{copied}</span> : null}
      </div>
    </div>
  );
}
