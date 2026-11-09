// Two small tools for the live preview: a debounce, and "the latest request wins" (the answer of a
// superseded request is dropped).

/** Runs `fn` once, `ms` after the last call; each call restarts the wait. `timers` can be given by a test. */
export interface Timers {
  set(fn: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

const realTimers: Timers = {
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

export interface Debounced {
  call(): void;
  cancel(): void;
  /** Runs now what is waiting (nothing when nothing waits). */
  flush(): void;
}

export function debounce(fn: () => void, ms: number, timers: Timers = realTimers): Debounced {
  let handle: unknown;
  let waiting = false;
  const fire = (): void => {
    waiting = false;
    handle = undefined;
    fn();
  };
  return {
    call() {
      if (waiting) timers.clear(handle);
      waiting = true;
      handle = timers.set(fire, ms);
    },
    cancel() {
      if (waiting) timers.clear(handle);
      waiting = false;
      handle = undefined;
    },
    flush() {
      if (!waiting) return;
      timers.clear(handle);
      fire();
    },
  };
}

/** A ticket is current until a later one is taken (or the guard is invalidated). */
export class LatestWins {
  private counter = 0;

  /** Starts a request: the returned function says whether it is still the latest. */
  begin(): () => boolean {
    const mine = ++this.counter;
    return () => this.counter === mine;
  }

  /** Makes every request in flight stale. */
  invalidate(): void {
    this.counter++;
  }
}
