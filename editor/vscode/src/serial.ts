/**
 * `fn`, run one call at a time: each call starts once every earlier one has settled, and a
 * call that fails does not stop the next.
 *
 * Kept out of `client.ts`, which cannot load outside VS Code, so the one property that
 * matters about starting the language server can be tested on its own.
 */
export function serialize<A extends unknown[]>(
  fn: (...args: A) => Promise<void>
): (...args: A) => Promise<void> {
  let last: Promise<void> = Promise.resolve();
  return (...args: A) => {
    const run = last.then(() => fn(...args));
    last = run.catch(() => undefined);
    return run;
  };
}
