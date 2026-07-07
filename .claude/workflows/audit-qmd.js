export const meta = {
  name: 'audit-qmd',
  description: 'Multi-dimension review of taliesin changes with per-finding adversarial verification',
  whenToUse: 'Auditing a taliesin change set: fans reviewers across correctness + load-bearing invariants + scope-discipline + corpus coverage + simplicity, then refutes each finding before reporting. Pass a target description as args, or omit to review the current branch diff.',
  phases: [
    { title: 'Review' },
    { title: 'Verify' },
  ],
}

// What to audit. Pass a string via args (e.g. "crates/core/src/render/deck.rs" or a
// feature description); default to the working/unpushed diff.
const target = (typeof args === 'string' && args.trim())
  ? args.trim()
  : 'the uncommitted + unpushed changes on the current branch (run `git diff` and `git diff origin/main...HEAD`; if both are empty, review the most recent commit)'

const DIMENSIONS = [
  {
    key: 'correctness',
    prompt: 'Hunt real bugs: logic errors, panics/unwrap on author input, wrong Option/Result handling, off-by-one in sourcepos or block-diff math, broken incremental-update paths, executor/warm-kernel races.',
  },
  {
    key: 'invariants',
    prompt: 'Verify the load-bearing invariants survive: every emitted block keeps data-block-id + data-sourcepos (included blocks also data-source-file); reverse-sync sourcepos stays total; the preview never gains a write-back-to-source path (single editing surface). Cite crates/core/tests/corpus.rs expectations.',
  },
  {
    key: 'scope-discipline',
    prompt: 'Flag scope creep: anything pulling toward non-HTML output (LaTeX/Typst/Word/ePub/PDF-as-parallel-format), reintroduced reveal.js/OJS/legacy shims or vocabulary, or legacy-compat tolerance. HTML is the only target; the engine is native (window.QmdDeck).',
  },
  {
    key: 'corpus-coverage',
    prompt: 'Is each new capability pinned by a target corpus doc + a test added in the same change? Find gaps in corpus/ and crates/core/tests where behavior is unverified. The corpus is the arbiter of done.',
  },
  {
    key: 'simplify',
    prompt: 'Quality only (no bug hunting): needless clones/allocations, duplicated logic that could reuse an existing helper, simpler equivalents, code that does not read like its neighbors (edition-2024 idiom, centralized workspace deps).',
  },
]

const FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          file: { type: 'string' },
          line: { type: 'string' },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          detail: { type: 'string' },
        },
        required: ['title', 'file', 'severity', 'detail'],
      },
    },
  },
  required: ['findings'],
}

const VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    isReal: { type: 'boolean' },
    confidence: { type: 'string', enum: ['high', 'medium', 'low'] },
    reason: { type: 'string' },
  },
  required: ['isReal', 'reason'],
}

const reviewPrompt = (d) =>
  `You are reviewing ${target} in the taliesin repo (a Rust .tmd -> HTML-only dev ` +
  `server). Focus ONLY on the "${d.key}" dimension.\n\n${d.prompt}\n\n` +
  `Read the diff and the changed files yourself before judging. Report concrete, ` +
  `file:line-anchored findings. Return an empty findings array if there is nothing ` +
  `real — do NOT invent issues to fill a quota.`

// Pipeline: each dimension's findings get adversarially verified the moment that
// dimension's review lands (no barrier — fast dimensions don't wait for slow ones).
const results = await pipeline(
  DIMENSIONS,
  (d) => agent(reviewPrompt(d), { label: `review:${d.key}`, phase: 'Review', schema: FINDINGS_SCHEMA }),
  (review, d) =>
    parallel(((review && review.findings) || []).map((f) => () =>
      agent(
        `Adversarially verify this taliesin review finding. Try to REFUTE it. Read the ` +
          `actual code at ${f.file}:${f.line || '?'} before deciding. Default to ` +
          `isReal=false if you cannot confirm it from the code itself.\n\n` +
          `Finding (${d.key}, ${f.severity}): ${f.title}\n${f.detail}`,
        { label: `verify:${f.file}`, phase: 'Verify', schema: VERDICT_SCHEMA },
      ).then((v) => ({ ...f, dimension: d.key, verdict: v })),
    )),
)

const all = results.flat().filter(Boolean)
const confirmed = all.filter((f) => f.verdict && f.verdict.isReal)
log(`Audit complete: ${confirmed.length} confirmed of ${all.length} raw finding(s) across ${DIMENSIONS.length} dimensions.`)

return {
  target,
  confirmed,
  rejected: all.filter((f) => !(f.verdict && f.verdict.isReal)),
}
