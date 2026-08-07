// "The environment could not run this cell" — spotting it in a diagnostic list, and the
// once-per-session latch that decides whether to say anything about it.
//
// Kept free of `vscode` (like `taskspecs.ts` and `diaglink.ts`) so `node --test` can cover
// the two things that actually break here: the wrapped `code` shape, and the latch.

/**
 * The code shapes a diagnostic's `code` really arrives in.
 *
 * MEASURED in a real Extension Host, not assumed. `check.rs` wires `code_description` from
 * each code's docs URL, so vscode-languageclient converts a Taliesin diagnostic into
 * `code: { value: "TAL-KERNEL", target: Uri }` — an **object**, never a bare string. A
 * problem matcher, the other producer, writes a plain string. Both have to be understood,
 * because nothing downstream can tell which one published a given diagnostic.
 */
export type DiagnosticCode = string | number | { value: string | number } | undefined | null;

/** The part of a `vscode.Diagnostic` this file needs, so the pure half stays pure. */
export interface CodedDiagnostic {
  code?: DiagnosticCode;
  message: string;
}

/**
 * The code for a cell that never ran because the environment could not run it.
 *
 * `crates/core/src/diagnostics/codes.rs` gives it to both "code cell did not run" and "code
 * cell did not complete", and to nothing else: it is the environment failing, as distinct
 * from the author's code raising. That is exactly the population `doctor` can help.
 */
export const KERNEL_CODE = "TAL-KERNEL";

/** The code as a plain string, whichever of the two shapes it arrived in. */
function codeValue(code: DiagnosticCode): string | null {
  if (typeof code === "string") return code;
  if (typeof code === "number") return String(code);
  if (code && typeof code === "object" && "value" in code) return String(code.value);
  return null;
}

/**
 * The message of the first kernel failure in `diags`, or `null` if there is none.
 *
 * The **message** rather than a boolean on purpose: the notification quotes the engine's own
 * words. The cause and the fix live in the Rust diagnostic catalogue, and a sentence written
 * here would be a second copy of them, free to drift.
 */
export function kernelFailure(diags: readonly CodedDiagnostic[]): string | null {
  for (const d of diags) {
    if (codeValue(d.code) === KERNEL_CODE) return d.message;
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
