import test from "node:test";
import assert from "node:assert/strict";
import { createDebouncer } from "../debounce";

// A deterministic stand-in for the timer functions the debouncer calls, so the tests
// don't depend on wall-clock sleeps. `tick(ms)` advances the clock and fires anything due.
function fakeClock() {
  let now = 0;
  let seq = 0;
  const pending = new Map<number, { at: number; fn: () => void }>();
  return {
    set: ((fn: () => void, ms: number) => {
      const id = ++seq;
      pending.set(id, { at: now + ms, fn });
      return id;
    }) as unknown as typeof setTimeout,
    clear: ((id: number) => {
      pending.delete(id);
    }) as unknown as typeof clearTimeout,
    tick(ms: number) {
      now += ms;
      for (const [id, t] of [...pending]) {
        if (t.at <= now) {
          pending.delete(id);
          t.fn();
        }
      }
    },
    get size() {
      return pending.size;
    },
  };
}

test("debounce: fires the callback after the quiet delay", () => {
  const clock = fakeClock();
  const d = createDebouncer(300, clock.set, clock.clear);
  let calls = 0;
  d.schedule("a", () => calls++);
  clock.tick(299);
  assert.equal(calls, 0, "does not fire before the delay elapses");
  clock.tick(1);
  assert.equal(calls, 1, "fires once the delay elapses");
});

test("debounce: a burst of schedules for one key coalesces to a single run", () => {
  const clock = fakeClock();
  const d = createDebouncer(300, clock.set, clock.clear);
  let calls = 0;
  const last = { v: 0 };
  // Five rapid keystrokes: each reschedules, so only the last callback should fire.
  for (let i = 1; i <= 5; i++) {
    clock.tick(50); // 50ms between keystrokes, well under the 300ms window
    d.schedule("a", () => {
      calls++;
      last.v = i;
    });
  }
  clock.tick(300);
  assert.equal(calls, 1, "the burst collapses to one call");
  assert.equal(last.v, 5, "the surviving call is the most recent one");
});

test("debounce: cancel prevents a pending callback from firing", () => {
  const clock = fakeClock();
  const d = createDebouncer(300, clock.set, clock.clear);
  let calls = 0;
  d.schedule("a", () => calls++);
  d.cancel("a");
  clock.tick(1000);
  assert.equal(calls, 0, "a cancelled key never fires");
});

test("debounce: distinct keys are independent", () => {
  const clock = fakeClock();
  const d = createDebouncer(300, clock.set, clock.clear);
  const fired: string[] = [];
  d.schedule("a", () => fired.push("a"));
  clock.tick(100);
  d.schedule("b", () => fired.push("b"));
  clock.tick(200); // a's window (300) elapses; b's (started at 100) has 100 left
  assert.deepEqual(fired, ["a"], "a fired; b is still pending");
  clock.tick(100);
  assert.deepEqual(fired, ["a", "b"], "b fires on its own schedule");
});

test("debounce: cancelAll clears every pending key", () => {
  const clock = fakeClock();
  const d = createDebouncer(300, clock.set, clock.clear);
  let calls = 0;
  d.schedule("a", () => calls++);
  d.schedule("b", () => calls++);
  d.cancelAll();
  clock.tick(1000);
  assert.equal(calls, 0, "nothing fires after cancelAll");
});
