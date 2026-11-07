import { createContext, useContext, useSyncExternalStore } from 'react';
import type { Session, State } from './session.ts';

export const SessionContext = createContext<Session | null>(null);

export function useSession(): Session {
  const session = useContext(SessionContext);
  if (!session) throw new Error('no session: the component is outside <SessionContext>');
  return session;
}

/** The state of the screen; the component renders again when it changes. */
export function useScreen(): State {
  const session = useSession();
  return useSyncExternalStore(session.subscribe, session.getState);
}

/** A design token of the theme (a CSS variable made from theme.json), as a number of pixels. */
export function token(name: string, fallback: number): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name);
  const value = Number.parseFloat(raw);
  return Number.isFinite(value) ? value : fallback;
}
