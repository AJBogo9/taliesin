// A per-key trailing debouncer, extracted vscode-free so it stays in the fast `node:test`
// loop (mirrors check.ts/paths.ts/ports.ts). `schedule(key, fn)` runs `fn` after `delayMs`
// of quiet for that key; a new schedule for the same key before it fires REPLACES the
// pending call, so a burst of keystrokes collapses into one run. This is what makes the
// on-type diagnostic refresh practical: one `taliesin check --stdin` per typing pause, not
// one per character. The timer functions are injectable so tests can drive a fake clock.

type TimerHandle = ReturnType<typeof setTimeout>;

export interface Debouncer {
  /** Run `fn` after the quiet delay for `key`, superseding any call still pending for it. */
  schedule(key: string, fn: () => void): void;
  /** Drop `key`'s pending call, if any (e.g. its document closed). */
  cancel(key: string): void;
  /** Drop every pending call (e.g. on extension deactivate). */
  cancelAll(): void;
}

export function createDebouncer(
  delayMs: number,
  set: typeof setTimeout = setTimeout,
  clear: typeof clearTimeout = clearTimeout
): Debouncer {
  const timers = new Map<string, TimerHandle>();
  const cancel = (key: string) => {
    const t = timers.get(key);
    if (t !== undefined) {
      clear(t);
      timers.delete(key);
    }
  };
  return {
    schedule(key, fn) {
      cancel(key); // supersede a still-pending run for this key
      const handle = set(() => {
        timers.delete(key);
        fn();
      }, delayMs);
      timers.set(key, handle);
    },
    cancel,
    cancelAll() {
      for (const key of [...timers.keys()]) cancel(key);
    },
  };
}
