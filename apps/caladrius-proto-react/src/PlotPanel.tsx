import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent } from 'react';
import { coord, find, pickPoint, plotRect, polyline, pxOfX, pyOfY, snapSample, stepSample, type Frame } from './plotmap.ts';
import { isManual, okOf, selectedCandidate } from './session.ts';
import type { ScenePanel, SceneSeries } from './types.ts';
import { token, useScreen, useSession } from './hooks.ts';

const MARGIN = { left: 64, right: 18, top: 16, bottom: 44 };

function useSize(ref: React.RefObject<HTMLDivElement | null>): { width: number; height: number } {
  const [size, setSize] = useState({ width: 640, height: 360 });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const read = () => setSize({ width: Math.max(200, Math.floor(el.clientWidth)), height: Math.max(160, Math.floor(el.clientHeight)) });
    read();
    const observer = new ResizeObserver(read);
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);
  return size;
}

function Markers({ frame, panel, series, radius }: { frame: Frame; panel: ScenePanel; series: SceneSeries; radius: number }) {
  return (
    <g className={`pts pts-${series.role}`}>
      {series.points.map((p, i) => (
        <circle
          key={i}
          cx={coord(pxOfX(frame, panel, p.x))}
          cy={coord(pyOfY(frame, panel, p.y))}
          r={radius}
          className={p.blq ? 'blq' : undefined}
        />
      ))}
    </g>
  );
}

export function PlotPanel() {
  const session = useSession();
  const state = useScreen();
  const box = useRef<HTMLDivElement | null>(null);
  const { width, height } = useSize(box);
  const frame: Frame = { width, height, ...MARGIN };
  const panel = state.scene?.panels[0];
  const ok = okOf(state);
  const fmt = session.fmt;
  const [drag, setDrag] = useState<{ which: 'start' | 'end'; time: number } | null>(null);
  const svg = useRef<SVGSVGElement | null>(null);
  const range = session.range();
  const hit = token('--size-hit-radius', 12);
  const marker = token('--size-marker-medium', 5);

  const local = (e: { clientX: number; clientY: number }): { x: number; y: number } => {
    const r = svg.current?.getBoundingClientRect();
    return { x: e.clientX - (r?.left ?? 0), y: e.clientY - (r?.top ?? 0) };
  };

  const downHandle = (which: 'start' | 'end') => (e: PointerEvent<SVGGElement>) => {
    if (!range) return;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    setDrag({ which, time: which === 'start' ? range.start : range.end });
    e.stopPropagation();
  };
  const moveHandle = (e: PointerEvent<SVGGElement>) => {
    if (!drag || !panel) return;
    const snapped = snapSample(frame, panel, local(e).x);
    if (snapped && snapped.x !== drag.time) setDrag({ which: drag.which, time: snapped.x });
  };
  const upHandle = (e: PointerEvent<SVGGElement>) => {
    if (!drag) return;
    const done = drag;
    setDrag(null);
    (e.currentTarget as Element).releasePointerCapture?.(e.pointerId);
    const current = done.which === 'start' ? range?.start : range?.end;
    if (done.time !== current) void (done.which === 'start' ? session.setRangeStart(done.time) : session.setRangeEnd(done.time));
  };
  const keyHandle = (which: 'start' | 'end') => (e: KeyboardEvent<SVGGElement>) => {
    if (!panel || !range) return;
    const from = which === 'start' ? range.start : range.end;
    const step = e.key === 'ArrowLeft' || e.key === 'ArrowDown' ? -1 : e.key === 'ArrowRight' || e.key === 'ArrowUp' ? 1 : 0;
    if (step === 0) return;
    e.preventDefault();
    const next = stepSample(panel, from, step);
    if (next !== null && next !== from) void (which === 'start' ? session.setRangeStart(next) : session.setRangeEnd(next));
  };

  const clickPlot = (e: MouseEvent<SVGSVGElement>) => {
    if (!panel) return;
    const { x, y } = local(e);
    const p = pickPoint(frame, panel, x, y, hit);
    if (p) void session.togglePoint(p.x);
  };

  const rect = plotRect(frame);
  const startX = drag?.which === 'start' ? drag.time : range?.start;
  const endX = drag?.which === 'end' ? drag.time : range?.end;
  const profile = panel ? find(panel, 'profile') : undefined;
  const selected = selectedCandidate(state);
  const clipId = 'plot-clip';

  const ordered = useMemo(() => {
    if (!panel) return [];
    const order: SceneSeries['role'][] = ['predicted', 'profile', 'lambda_z_fit', 'observed', 'subject', 'excluded', 'replaced', 'lambda_z_used', 'lambda_z_start'];
    return [...panel.series].sort((a, b) => order.indexOf(a.role) - order.indexOf(b.role));
  }, [panel]);

  return (
    <section className="panel plot-panel" aria-label="Plot">
      <div className="panel-head">
        <span>{state.scene?.title ?? 'Plot'}</span>
        <span className="spacer" />
        <div role="group" aria-label="Axis" className="seg">
          <button type="button" aria-pressed={state.log} onClick={() => void session.setLog(true)}>
            Semi-log
          </button>
          <button type="button" aria-pressed={!state.log} onClick={() => void session.setLog(false)}>
            Linear
          </button>
        </div>
        <button type="button" disabled={!isManual(state)} onClick={() => void session.backToAutomatic()}>
          Back to automatic
        </button>
      </div>
      <div className="plot-box" ref={box}>
        {panel ? (
          <svg
            ref={svg}
            width={width}
            height={height}
            viewBox={`0 0 ${width} ${height}`}
            role="img"
            aria-label={`${panel.title}. Click a point to take it in or out of the terminal phase; drag the two handles to set the range.`}
            onClick={clickPlot}
          >
            <defs>
              <clipPath id={clipId}>
                <rect x={rect.x0} y={rect.y0} width={rect.x1 - rect.x0} height={rect.y1 - rect.y0} />
              </clipPath>
            </defs>
            <rect className="plot-bg" x={rect.x0} y={rect.y0} width={rect.x1 - rect.x0} height={rect.y1 - rect.y0} />
            {panel.x.ticks.map((t, i) => (
              <g key={`x${i}`}>
                <line className="grid" x1={coord(pxOfX(frame, panel, t.value))} x2={coord(pxOfX(frame, panel, t.value))} y1={rect.y0} y2={rect.y1} />
                <text className="tick" x={coord(pxOfX(frame, panel, t.value))} y={rect.y1 + 16} textAnchor="middle">
                  {t.label}
                </text>
              </g>
            ))}
            {panel.y.ticks.map((t, i) => (
              <g key={`y${i}`}>
                <line className="grid" y1={coord(pyOfY(frame, panel, t.value))} y2={coord(pyOfY(frame, panel, t.value))} x1={rect.x0} x2={rect.x1} />
                <text className="tick" y={coord(pyOfY(frame, panel, t.value) + 4)} x={rect.x0 - 6} textAnchor="end">
                  {t.label}
                </text>
              </g>
            ))}
            <text className="axis-title" x={(rect.x0 + rect.x1) / 2} y={height - 6} textAnchor="middle">
              {panel.x.title}
            </text>
            <text className="axis-title" transform={`translate(14 ${(rect.y0 + rect.y1) / 2}) rotate(-90)`} textAnchor="middle">
              {panel.y.title}
            </text>
            <g clipPath={`url(#${clipId})`}>
              {ordered.map((s, i) =>
                s.style === 'line' ? (
                  <polyline key={i} className={`line line-${s.role}`} points={polyline(frame, panel, s)} fill="none" />
                ) : (
                  <Markers key={i} frame={frame} panel={panel} series={s} radius={s.role === 'lambda_z_used' || s.role === 'lambda_z_start' ? marker + 1.5 : marker} />
                ),
              )}
              {startX !== undefined && endX !== undefined ? (
                <rect className="range-band" x={pxOfX(frame, panel, startX)} y={rect.y0} width={Math.max(0, pxOfX(frame, panel, endX) - pxOfX(frame, panel, startX))} height={rect.y1 - rect.y0} />
              ) : null}
            </g>
            {startX !== undefined && endX !== undefined
              ? (['start', 'end'] as const).map((which) => {
                  const t = which === 'start' ? startX : endX;
                  const x = pxOfX(frame, panel, t);
                  return (
                    <g
                      key={which}
                      className={`handle handle-${which}`}
                      tabIndex={0}
                      role="slider"
                      aria-label={which === 'start' ? 'Start of the terminal phase' : 'End of the terminal phase'}
                      aria-valuenow={t}
                      aria-valuetext={`t = ${fmt.number(t)}`}
                      onPointerDown={downHandle(which)}
                      onPointerMove={moveHandle}
                      onPointerUp={upHandle}
                      onKeyDown={keyHandle(which)}
                      onClick={(e) => e.stopPropagation()}
                    >
                      <line x1={coord(x)} x2={coord(x)} y1={rect.y0} y2={rect.y1} />
                      <rect x={coord(x - 6)} y={coord(rect.y0)} width={12} height={22} rx={3} />
                    </g>
                  );
                })
              : null}
          </svg>
        ) : (
          <p className="empty">{state.error ?? (state.ready ? 'No plot for this subject.' : 'Loading the engine...')}</p>
        )}
      </div>
      <div className="legend" aria-label="Legend">
        {(state.scene?.legend.entries ?? []).map((e, i) => (
          <span key={i} className={`key key-${e.role}`}>
            <i /> {e.name}
          </span>
        ))}
        {state.scene?.legend.below_limit ? <span className="muted">{state.scene.legend.below_limit}</span> : null}
      </div>
      {panel?.hidden && panel.hidden.count > 0 ? (
        <p className="hint pad" data-testid="hidden-note">
          {panel.hidden.note}
        </p>
      ) : null}
      {state.scene?.notes.map((n, i) => (
        <p key={i} className="hint pad">
          {fmt.withLabels(n)}
        </p>
      ))}
      {profile ? null : null}
      <div className="candidates">
        <table aria-label="Candidate ranges for the terminal phase">
          <thead>
            <tr>
              <th>Points</th>
              <th>From</th>
              <th>To</th>
              <th>λz</th>
              <th>Adj. R²</th>
            </tr>
          </thead>
          <tbody>
            {(ok?.lambda_z_candidates ?? []).map((c, i) => (
              <tr
                key={i}
                tabIndex={0}
                className={c.selected ? 'selected' : undefined}
                aria-selected={c.selected}
                onClick={() => void session.pickCandidate(c)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    void session.pickCandidate(c);
                  }
                }}
              >
                <td data-cell={`c${i}:n`}>{fmt.number(c.n_points)}</td>
                <td data-cell={`c${i}:from`}>{fmt.number(c.time_first)}</td>
                <td data-cell={`c${i}:to`}>{fmt.number(c.time_last)}</td>
                <td data-cell={`c${i}:lambda_z`}>{fmt.number(c.lambda_z)}</td>
                <td data-cell={`c${i}:adj`}>{c.adj_r_squared === null ? '-' : fmt.fitQuality(c.adj_r_squared)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {selected ? null : <p className="hint pad">No candidate is selected.</p>}
      </div>
    </section>
  );
}
