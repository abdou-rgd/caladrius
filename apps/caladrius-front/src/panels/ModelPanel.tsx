import type { ReactNode } from 'react';
import { COMPARTMENTS, ROUTES, SETS, modelId, offeredModels, parameterLabel, parameterNames, parameterUnit, routeInfo, setOf } from '../model/parameterSets.ts';
import type { Compartments, Route } from '../model/parameterSets.ts';
import { useApp, useSessionState } from '../session/hooks.ts';
import type { WorksheetInfo } from '../session/types.ts';

function unitsOf(worksheet: WorksheetInfo | null): { time?: string; conc?: string; derived: Record<string, string> } {
  const unit = (role: string): string | undefined => worksheet?.columns.find((c) => c.role === role)?.unit ?? undefined;
  const time = unit('time');
  const conc = unit('concentration');
  return { ...(time !== undefined ? { time } : {}), ...(conc !== undefined ? { conc } : {}), derived: worksheet?.derived_units ?? {} };
}

export function ModelPanel(): ReactNode {
  const { session, fmt } = useApp();
  const state = useSessionState();
  const { input, preview, data } = state;
  const ready = state.engine.status === 'ready' && data.worksheet !== null && state.subject !== null;

  const offered = offeredModels(state.models);
  const compartments = COMPARTMENTS.filter((c) => offered.some((o) => o.compartments === c));
  const routes = ROUTES.filter((r) => state.models.includes(modelId(input.compartments, r.id)));
  const route = routeInfo(input.route);
  const set = setOf(input.compartments, input.set);
  const names = parameterNames(input.compartments, input.route, input.set);
  const units = unitsOf(data.worksheet);
  const typed = (name: string): string => input.initial[name] ?? '';
  const invalid = (name: string): boolean => typed(name).trim() !== '' && fmt.parseNumber(typed(name).trim()) === undefined;
  const complete = session.currentKey() !== null;
  const pk1 = input.compartments === 1;

  return (
    <div className="panel">
      <h3 className="section">Model</h3>
      <div className="row">
        <span className="muted small">Compartments</span>
        <div className="seg" role="radiogroup" aria-label="Compartments">
          {compartments.map((c: Compartments) => (
            <button key={c} role="radio" aria-checked={input.compartments === c} onClick={() => session.setCompartments(c)}>
              {c}
            </button>
          ))}
        </div>
        <select className="field" aria-label="Route" value={input.route} onChange={(e) => session.setRoute(e.target.value as Route)}>
          {routes.map((r) => (
            <option key={r.id} value={r.id}>
              {r.label}
            </option>
          ))}
        </select>
      </div>
      <div className="row">
        <span className="muted small">Parameters</span>
        <div className="seg" role="radiogroup" aria-label="Parameter set">
          {SETS[input.compartments].map((s) => (
            <button key={s.id} role="radio" aria-checked={set.id === s.id} onClick={() => session.setParameterSet(s.id)} title={s.names.join(', ')}>
              {s.label}
            </button>
          ))}
        </div>
        <label className="row">
          <span className="muted small">Weighting</span>
          <select className="field" aria-label="Weighting" value={input.weighting} onChange={(e) => session.setWeighting(e.target.value)}>
            {state.weightings.map((w) => (
              <option key={w} value={w}>
                {w}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="row">
        <label className="row">
          <span className="muted small">Dose</span>
          <input
            className={`field${input.dose.trim() !== '' && fmt.parseNumber(input.dose.trim()) === undefined ? ' invalid' : ''}`}
            aria-label="Dose"
            inputMode="decimal"
            placeholder="from the data"
            value={input.dose}
            size={12}
            onChange={(e) => session.setDose(e.target.value)}
          />
        </label>
        <span className="muted small">{preview.dose !== null ? `Dose used: ${fmt.number(preview.dose)}` : ''}</span>
      </div>

      <h3 className="section">Initial estimates</h3>
      <div className="params" role="group" aria-label="Initial estimates">
        {names.map((name) => (
          <ParamRow key={name} name={name} label={parameterLabel(name, route.extravascular)} unit={parameterUnit(name, units)} value={typed(name)} invalid={invalid(name)} onChange={(t) => session.setInitial(name, t)} />
        ))}
      </div>

      <div className="row">
        {pk1 ? (
          <button className="btn" disabled={!ready || state.estimate.busy} onClick={() => void session.estimateInitial()} title="Ask the engine for starting values from the profile">
            Estimate
          </button>
        ) : (
          <span className="muted small">Enter a value for each parameter: the engine does not generate starting values for {input.compartments} compartments.</span>
        )}
        <span className="spacer" />
        <button className="btn primary" disabled={!ready || !complete || state.fitBusy} onClick={() => void session.runFit()}>
          {state.fitBusy ? 'Fitting…' : 'Fit'}
        </button>
      </div>
      {state.estimate.error !== null ? (
        <div className="banner error" role="alert">
          {state.estimate.error}
        </div>
      ) : null}
      {preview.message !== null ? (
        <div className={preview.status === 'error' ? 'banner error' : 'muted small'} role={preview.status === 'error' ? 'alert' : undefined} aria-label="Preview message">
          {preview.message}
        </div>
      ) : null}
      {preview.wrss !== null ? (
        <dl className="kv" aria-label="Start of the fit">
          <dt>Weighted sum of squares at the start</dt>
          <dd data-testid="start-wrss">{fmt.number(preview.wrss)}</dd>
          <dt>Observations</dt>
          <dd>{preview.nObservations ?? '-'}</dd>
        </dl>
      ) : null}
      {state.fitError !== null ? (
        <div className="banner error" role="alert">
          {state.fitError}
        </div>
      ) : null}
    </div>
  );
}

function ParamRow(props: { name: string; label: string; unit: string | undefined; value: string; invalid: boolean; onChange: (text: string) => void }): ReactNode {
  return (
    <>
      <label htmlFor={`init-${props.name}`}>{props.label}</label>
      <input
        id={`init-${props.name}`}
        className={`field${props.invalid ? ' invalid' : ''}`}
        aria-label={props.name}
        aria-invalid={props.invalid}
        inputMode="decimal"
        autoComplete="off"
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
      />
      <span className="unit">{props.unit ?? ''}</span>
    </>
  );
}
