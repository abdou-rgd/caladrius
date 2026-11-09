// A plain store: one state value, `subscribe`, `getState` (React reads it with `useSyncExternalStore`).
// No library. The state it holds is serializable data (types.ts).

export class Store<S> {
  private state: S;
  private readonly listeners = new Set<() => void>();

  constructor(initial: S) {
    this.state = initial;
  }

  getState = (): S => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** Replaces the state by `update(state)`; listeners are told when the value changed. */
  set(update: (state: S) => S): void {
    const next = update(this.state);
    if (next === this.state) return;
    this.state = next;
    for (const listener of [...this.listeners]) listener();
  }

  patch(partial: Partial<S>): void {
    this.set((s) => ({ ...s, ...partial }));
  }
}
