import { createContext, useContext, useSyncExternalStore } from 'react';
import type { Formatter } from '@caladrius/bridge-client';
import type { Session } from './session.ts';
import type { SessionState } from './types.ts';

export interface App {
  session: Session;
  fmt: Formatter;
}

export const AppContext = createContext<App | null>(null);

export function useApp(): App {
  const app = useContext(AppContext);
  if (app === null) throw new Error('no session: the panel is outside <AppContext.Provider>');
  return app;
}

/** The whole state; a panel re-renders on any change (the state is small). */
export function useSessionState(): SessionState {
  const { session } = useApp();
  return useSyncExternalStore(session.store.subscribe, session.store.getState);
}
