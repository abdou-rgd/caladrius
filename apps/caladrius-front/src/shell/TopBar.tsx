import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useApp, useSessionState } from '../session/hooks.ts';
import type { ThemeChoice } from '../session/types.ts';

const DIGITS = [2, 3, 4, 5, 6, 7, 8];

export function TopBar(props: { onPalette: () => void; onResetLayout: () => void }): ReactNode {
  const { session } = useApp();
  const state = useSessionState();
  const { engine, display, theme, data } = state;
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent): void => {
      if (box.current !== null && !box.current.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', close);
    document.addEventListener('keydown', esc);
    return () => {
      document.removeEventListener('mousedown', close);
      document.removeEventListener('keydown', esc);
    };
  }, [open]);

  const statusText = engine.status === 'ready' ? `Engine ready${engine.way !== null ? ` (${engine.way})` : ''}` : engine.status === 'loading' ? 'Loading the engine…' : 'Engine error';
  return (
    <>
      <header className="topbar">
        <span className="brand">Caladrius</span>
        <span className="worksheet-name" data-testid="worksheet-name">
          {data.worksheet !== null ? <strong>{data.worksheet.name}</strong> : 'No worksheet'}
        </span>
        <span className="spacer" />
        <div className="topbar-group">
          <button className="btn small" onClick={props.onPalette} title="Command palette (Ctrl+K)">
            Commands <span className="muted">Ctrl+K</span>
          </button>
          <div className="menu" ref={box}>
            <button className="btn small" aria-haspopup="true" aria-expanded={open} onClick={() => setOpen(!open)}>
              Display
            </button>
            {open ? (
              <div className="menu-pop" role="dialog" aria-label="Display settings">
                <label>
                  Significant digits
                  <select className="field" aria-label="Significant digits" value={display.digits} onChange={(e) => session.setDisplay({ ...display, digits: Number(e.target.value) })}>
                    {DIGITS.map((d) => (
                      <option key={d} value={d}>
                        {d}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Decimal comma
                  <input type="checkbox" aria-label="Decimal comma" checked={display.comma} onChange={(e) => session.setDisplay({ ...display, comma: e.target.checked })} />
                </label>
                <label>
                  Theme
                  <select className="field" aria-label="Theme" value={theme} onChange={(e) => session.setTheme(e.target.value as ThemeChoice)}>
                    <option value="system">System</option>
                    <option value="light">Light</option>
                    <option value="dark">Dark</option>
                  </select>
                </label>
                <button className="btn small" onClick={props.onResetLayout}>
                  Reset layout
                </button>
              </div>
            ) : null}
          </div>
          <span className={`status ${engine.status}`} role="status" data-testid="engine-status" title={engine.message ?? ''}>
            {statusText}
          </span>
        </div>
      </header>
      {engine.status === 'error' && engine.message !== null ? (
        <div className="transport-error" role="alert">
          {engine.message}
        </div>
      ) : null}
    </>
  );
}
