// "The environment could not run this cell" — spotting it in a diagnostic list, and the
// once-per-session latch that decides whether to say anything about it.
//
// Kept free of `vscode` (like `taskspecs.ts` and `diaglink.ts`) so `node --test` can cover
// the two things that actually break here: the message match, and the latch.

/** The part of a `vscode.Diagnostic` this file needs, so the pure half stays pure. */
export interface CodedDiagnostic {
  message: string;
}

/**
 * The message openings that mean "the environment could not run this cell", as distinct from
 * the author's code raising. That is exactly the population `doctor` can help.
 *
 * Matched on the MESSAGE, and it has to be. Until 2026-08-08 this keyed on a `TAL-KERNEL`
 * code that arrived (measured in a real Extension Host) as `{ value, target }` from the
 * language client and as a bare string from a problem matcher, so both shapes had to be
 * understood; the code catalogue is gone and the message is the whole diagnostic now, which
 * removes that fork entirely. `crates/server/src/build.rs`'s `cell_error_message` and
 * `run_print.rs`'s `failure_line` are the two producers, and `src/test/kernelfail.test.ts`
 * is the drift gate that pins these strings against them.
 */
export const KERNEL_MESSAGES = ["code cell did not run", "code cell did not complete"] as const;

/**
 * The message of the first kernel failure in `diags`, or `null` if there is none.
 *
 * The **message** rather than a boolean on purpose: the notification quotes the engine's own
 * words, so a sentence written here cannot drift from the one the author already read.
 */
export function kernelFailure(diags: readonly CodedDiagnostic[]): string | null {
  for (const d of diags) {
    if (KERNEL_MESSAGES.some((m) => d.message.includes(m))) return d.message;
  }
  return null;
}

/**
 * Fires on the first kernel failure of a session, and never again.
 *
 * The latch is the whole design. Diagnostics are republished on every keystroke, so an
 * unlatched watcher would reprint the same notification on every edit for as long as the
 * kernel stays broken — i.e. exactly while the author is trying to fix it.
 */
export class KernelPrompt {
  private fired = false;

  /** The message to show, or `null` to stay quiet. */
  offer(diags: readonly CodedDiagnostic[]): string | null {
    if (this.fired) return null;
    const message = kernelFailure(diags);
    if (message === null) return null;
    this.fired = true;
    return message;
  }
}
