// One status bar item: is a preview running for this project, and how healthy is it.
//
// **Live kernel and cache state is deliberately absent.** The original idea placed it here on
// the grounds that a status bar "can talk to a running preview". It cannot, not without
// widening the webview relay, which carries exactly four message types on purpose
// (`tali-goto` and `tali-page` up, `tali-cursor` and `tali-navigate` down). Growing that
// protocol is a decision worth making on its own merits, not a side effect of a status bar.
// The gap is recorded in DETECTION-DEBT.md.
//
// Split from `statusbar.ts` (no `vscode` import here) so `node --test` can check the wording
// with no Extension Host, the same way `pastekind.ts` is split from `insert.ts`.

export interface StatusState {
  /** The port a preview for this project is serving on, or `null` if none is running. */
  previewPort: number | null;
  /** Problems the last `check` found, or `null` when it has not run. */
  problems: number | null;
}

/** The status bar label. */
export function statusText(state: StatusState): string {
  const parts = ["$(book) Taliesin"];
  if (state.previewPort !== null) parts.push(`:${state.previewPort}`);
  // `null` (not run) and `0` (clean) both render nothing. Zero is worth saying only beside
  // something that could be non-zero, and `null` shown as "0 problems" would claim a clean
  // project nothing has verified.
  if (state.problems !== null && state.problems > 0) {
    parts.push(`· ${state.problems} problem${state.problems === 1 ? "" : "s"}`);
  }
  return parts.join(" ");
}

/** The hover text, which is where the states the label collapses get spelled out. */
export function statusTooltip(state: StatusState): string {
  const preview =
    state.previewPort === null
      ? "No preview running. Click to open a preview."
      : `Preview running on port ${state.previewPort}. Click to focus it.`;
  const health =
    state.problems === null
      ? "check has not run yet."
      : state.problems === 0
        ? "check found no problems."
        : `check found ${state.problems} problem${state.problems === 1 ? "" : "s"}.`;
  return `${preview}\n${health}`;
}
