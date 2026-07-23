/*!tali-askai v1*/
// Ask AI — client-side hand-off to the student's own logged-in AI.
// Spec: notes/2026-07-23-ask-ai-handoff-design.md. Read-only; no backend.
// One fragment in the concatenated code-enhance <script> (shares global scope), so
// every symbol is `taliAsk`-prefixed. Top-level functions are both tsc-visible globals
// and runtime window properties (used by the browser test harness).

/**
 * Entry point; registered in 09-register.js. Idempotent; skips decks.
 * @param {Document | Element} [root]
 */
function taliInitAskAi(root) {
  if (typeof document === 'undefined') return;
  if (document.querySelector('.tali-deck')) return; // decks are not reading views
  var host = document.body;
  if (!host || host.getAttribute('data-tali-askai') === 'on') return;
  host.setAttribute('data-tali-askai', 'on');
  void root; // reserved for future scoped re-init
  // Wiring added in later tasks.
}
