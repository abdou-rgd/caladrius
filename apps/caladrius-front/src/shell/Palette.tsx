import { Command } from 'cmdk';
import { useMemo, useState, type ReactNode } from 'react';
import { useApp, useSessionState } from '../session/hooks.ts';
import type { CommandInfo } from '../model/registry.ts';

export interface Action {
  id: string;
  title: string;
  run: () => void;
}

/** A minimal command palette (the full one is T-186): the app's own actions, then the engine's commands. */
export function Palette(props: { open: boolean; onOpenChange: (open: boolean) => void; actions: Action[] }): ReactNode {
  const { session } = useApp();
  const { commands } = useSessionState();
  const [detail, setDetail] = useState<CommandInfo | null>(null);
  const close = (): void => {
    props.onOpenChange(false);
    setDetail(null);
  };
  const engine = useMemo(() => commands, [commands]);

  return (
    <Command.Dialog
      open={props.open}
      onOpenChange={(o) => {
        props.onOpenChange(o);
        if (!o) setDetail(null);
      }}
      label="Command palette"
      loop
    >
      {detail === null ? (
        <>
          <Command.Input placeholder="Type a command…" aria-label="Command" />
          <Command.List>
            <Command.Empty>No command matches.</Command.Empty>
            <Command.Group heading="Application">
              {props.actions.map((a) => (
                <Command.Item
                  key={a.id}
                  value={`${a.title} ${a.id}`}
                  onSelect={() => {
                    close();
                    a.run();
                  }}
                >
                  {a.title}
                  <span className="id">{a.id}</span>
                </Command.Item>
              ))}
            </Command.Group>
            <Command.Group heading="Engine commands">
              {engine.map((c) => (
                <Command.Item
                  key={c.id}
                  value={`${c.title} ${c.id}`}
                  onSelect={() => {
                    if (c.required.length === 0) {
                      close();
                      void session.runCommand(c.id);
                    } else setDetail(c);
                  }}
                >
                  {c.title}
                  <span className="id">{c.id}</span>
                </Command.Item>
              ))}
            </Command.Group>
          </Command.List>
        </>
      ) : (
        <div className="palette-detail">
          <div>
            <strong className="mono">{detail.id}</strong> needs {detail.required.map((r) => `\`${r}\``).join(', ')}. The parameter form comes with the full palette.
          </div>
          <div>{detail.description}</div>
          <div className="muted">Example</div>
          <pre>{JSON.stringify(detail.example, null, 1)}</pre>
          <div>
            <button className="btn" onClick={() => setDetail(null)}>
              Back
            </button>
          </div>
        </div>
      )}
    </Command.Dialog>
  );
}
