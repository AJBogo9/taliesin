export const meta = {
  name: 'taliesin-ui-audit',
  description:
    'Analyze captured UI screenshots + logs, dedup by root cause, adversarially verify, and write a ranked bug report.',
  whenToUse:
    'After running capture-run.mjs (and optionally probe-run.mjs). Pass args={artifactsRoot, manifest, probeResults}.',
  phases: [
    { title: 'Analyze', detail: 'one agent per page reads its screenshots + logs' },
    { title: 'Dedup', detail: 'group findings by root cause' },
    { title: 'Verify', detail: 'adversarially refute each visual/interaction finding' },
    { title: 'Report', detail: 'synthesize a ranked markdown report' },
  ],
};

// ---- args -------------------------------------------------------------------
// args.artifactsRoot    : absolute path to .work/artifacts
// One of:
//   args.manifest       : the parsed manifest.json object (has .pages[]), or
//   args.manifestPath    : absolute path to manifest.json (a bootstrap agent
//                          reads it; preferred for large runs so args stays small)
// Optionally:
//   args.probeResults / args.probeResultsPath : parsed probe-results.json / its path
// args may arrive as an object or as a JSON string depending on how it was
// passed; normalize.
const A = typeof args === 'string' ? JSON.parse(args) : args || {};
const artifactsRoot = A.artifactsRoot;

// Cost controls (balanced defaults):
//   A.model      : model for analysis/verify/report agents (default 'sonnet',
//                  keeps the run off the scarce weekly-Opus budget).
//   A.fullMatrix : if true, vision-analyze ALL 6 cells per page; default false
//                  = analyze one representative cell + any flagged cell only.
const MODEL = A.model || 'sonnet';
const FULL_MATRIX = A.fullMatrix === true;

// A cell is "flagged" (worth a dedicated look) if any cheap signal fired.
const isFlagged = (c) =>
  c.horizontalOverflow ||
  (c.pastRightCount || 0) > 0 ||
  (c.brokenImageCount || 0) > 0 ||
  (c.errorCount || 0) > 0 ||
  (c.failedRequests || 0) > 0 ||
  c.cellError ||
  c.navError;

// Balanced cell selection for a page: one base theme (light) across ALL
// viewports, so responsive/layout bugs are reliably covered without relying on
// the agent to go exploring, plus any flagged cell (which pulls in the other
// theme when a cheap signal fired there). Clean pages cost 3 vision reads
// instead of 6; FULL_MATRIX keeps all cells.
function selectCells(cells) {
  if (FULL_MATRIX) return cells;
  const themes = [...new Set(cells.map((c) => c.theme))];
  const baseTheme = themes.includes('light') ? 'light' : themes[0];
  const chosen = new Map();
  for (const c of cells)
    if (c.theme === baseTheme) chosen.set(`${c.viewport}/${c.theme}`, c);
  for (const c of cells)
    if (isFlagged(c)) chosen.set(`${c.viewport}/${c.theme}`, c);
  return [...chosen.values()];
}

const LOOSE_MANIFEST_SCHEMA = {
  type: 'object',
  properties: {
    pages: { type: 'array', items: { type: 'object' } },
    buildFailures: { type: 'array', items: { type: 'object' } },
  },
  required: ['pages'],
};
const LOOSE_PROBE_SCHEMA = {
  type: 'object',
  properties: { results: { type: 'array', items: { type: 'object' } } },
  required: ['results'],
};

let manifest = A.manifest ?? null;
let probeResults = A.probeResults ?? null;

if (!manifest && A.manifestPath) {
  manifest = await agent(
    `Read the JSON file at ${A.manifestPath} and return its parsed contents. It has a top-level "pages" array (each item has fields unit, route, sourceFile, format, viewport, theme, screenshot, meta, errorCount, failedRequests, horizontalOverflow) and a "buildFailures" array. Return the data verbatim; do NOT summarize, sample, or drop any page.`,
    { label: 'bootstrap:manifest', phase: 'Analyze', schema: LOOSE_MANIFEST_SCHEMA, agentType: 'general-purpose', model: MODEL },
  );
}
if (!probeResults && A.probeResultsPath) {
  probeResults = await agent(
    `Read the JSON file at ${A.probeResultsPath} if it exists and return it parsed (has a "results" array). If it does not exist, return {"results": []}.`,
    { label: 'bootstrap:probe', phase: 'Analyze', schema: LOOSE_PROBE_SCHEMA, agentType: 'general-purpose', model: MODEL },
  );
}

if (!manifest || !Array.isArray(manifest.pages)) {
  return { error: 'no manifest: pass args.manifest or args.manifestPath.' };
}
if (!manifest.pages.length) {
  return { error: 'manifest has zero pages; run capture-run.mjs first.' };
}

// Batching: run the workflow on a subset of units from a single capture so the
// cost can be paced across sessions/days. A.onlyUnits = ['tech-blog', ...]
// (matches the manifest's `unit` slug). A.onlyUnits is a substring match.
if (Array.isArray(A.onlyUnits) && A.onlyUnits.length) {
  const before = manifest.pages.length;
  manifest.pages = manifest.pages.filter((p) =>
    A.onlyUnits.some((u) => p.unit === u || p.unit.includes(u)),
  );
  log(
    `Batch filter onlyUnits=${JSON.stringify(A.onlyUnits)}: ${manifest.pages.length}/${before} cells`,
  );
  if (!manifest.pages.length) {
    return { error: `onlyUnits matched no cells: ${JSON.stringify(A.onlyUnits)}` };
  }
}

// ---- schemas ----------------------------------------------------------------
const ANALYZE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          viewport: { type: 'string' },
          theme: { type: 'string' },
          bugClass: { type: 'string', enum: ['visual', 'console', 'network'] },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          title: { type: 'string' },
          description: { type: 'string' },
          evidence: { type: 'string' },
          suspected: { type: 'string' },
        },
        required: ['bugClass', 'severity', 'title', 'description'],
      },
    },
  },
  required: ['findings'],
};

const CLUSTER_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    clusters: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          title: { type: 'string' },
          bugClass: { type: 'string' },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          rootCause: { type: 'string' },
          suspected: { type: 'string' },
          instanceIndexes: { type: 'array', items: { type: 'number' } },
        },
        required: ['title', 'severity', 'instanceIndexes'],
      },
    },
  },
  required: ['clusters'],
};

const VERDICT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    verdict: { type: 'string', enum: ['confirmed', 'refuted', 'uncertain'] },
    severity: { type: 'string', enum: ['high', 'medium', 'low'] },
    reason: { type: 'string' },
    rootCause: { type: 'string' },
    sourceLocation: { type: 'string' },
  },
  required: ['verdict', 'reason'],
};

// ---- group cells into pages -------------------------------------------------
const groups = new Map();
for (const r of manifest.pages) {
  const key = r.unit + r.route;
  if (!groups.has(key)) {
    groups.set(key, {
      unit: r.unit,
      route: r.route,
      sourceFile: r.sourceFile,
      format: r.format,
      cells: [],
    });
  }
  groups.get(key).cells.push({
    viewport: r.viewport,
    theme: r.theme,
    screenshot: `${artifactsRoot}/${r.screenshot}`,
    meta: `${artifactsRoot}/${r.meta}`,
    errorCount: r.errorCount,
    failedRequests: r.failedRequests,
    horizontalOverflow: r.horizontalOverflow,
    pastRightCount: r.pastRightCount || 0,
    brokenImageCount: r.brokenImageCount || 0,
    cellError: r.cellError || null,
    navError: r.navError || null,
  });
}
const pageGroups = [...groups.values()];
log(`Analyzing ${pageGroups.length} pages (${manifest.pages.length} cells)`);

// ---- Phase 1: analyze (barrier: dedup needs all findings) -------------------
phase('Analyze');
const perPage = await parallel(
  pageGroups.map((g) => () => {
    const cells = selectCells(g.cells);
    const subsetNote = FULL_MATRIX
      ? ''
      : `\n(Showing one theme across all viewports + any flagged cells: ${cells.length} of ${g.cells.length}. COMPARE the viewports against each other for responsive breakage. Omitted cells are the other theme on a page with no cheap-signal issue.)`;
    const cellList = cells
      .map(
        (c) =>
          `- ${c.viewport}/${c.theme}: screenshot=${c.screenshot} meta=${c.meta}` +
          `  (flags: consoleErrors=${c.errorCount}, failedReq=${c.failedRequests}, hOverflow=${c.horizontalOverflow})`,
      )
      .join('\n');
    const deckNote =
      g.format === 'deck'
        ? '\nNOTE: this is a DECK captured in its default scroll/reader view; do NOT flag "only one slide visible" or reader-mode layout as a bug.'
        : '';
    const prompt = `You are auditing ONE rendered page of Taliesin (a .tmd->HTML document tool) for VISUAL/LAYOUT bugs a reader would notice. Console/JS and network errors are harvested separately and mechanically, so do NOT report those here.

Page: ${g.unit} ${g.route}
Source: ${g.sourceFile}  Format: ${g.format}${deckNote}

For EACH cell below, use Read on the screenshot PNG (it renders as an image). You may also Read the meta JSON for its domFlags (horizontalOverflow, pastRight, brokenImages) as hints.

Report only concrete VISUAL defects a reader would see:
- content overflowing / clipped / cut off, horizontal scroll leak
- overlapping or unreadable text, text colliding with other elements
- broken or missing images (blank/placeholder where an image should render)
- theme not applied (e.g. dark text on a dark background, unstyled flash)
- badly broken responsive layout (one viewport clearly broken vs another)
- broken tables / figures / callouts / code blocks / math
Tie every finding to a specific viewport/theme cell and set bugClass to "visual". Ignore subjective taste, minor spacing, or intentional design. If the page looks fine, return findings: [].

CAPTURE CAVEAT: these are FULL-PAGE screenshots. A position:fixed / sticky element (a floating corner button, nav bar, cookie banner) is painted at the DOCUMENT bottom in full-page capture, NOT at its real viewport position. So an apparent overlap between such a fixed corner/edge control and the last content on the page is almost always a capture artifact, not a real bug. Do not report it unless the collision is clearly between normal in-flow elements.

Cells:${subsetNote}
${cellList}`;
    return agent(prompt, {
      label: `analyze:${g.unit}${g.route}`,
      phase: 'Analyze',
      schema: ANALYZE_SCHEMA,
      agentType: 'general-purpose',
      model: MODEL,
    }).then((res) => ({ group: g, findings: res?.findings ?? [] }));
  }),
);

// flatten with page context
const allFindings = [];
for (const pp of perPage.filter(Boolean)) {
  for (const f of pp.findings) {
    allFindings.push({
      ...f,
      unit: pp.group.unit,
      route: pp.group.route,
      sourceFile: pp.group.sourceFile,
      format: pp.group.format,
    });
  }
}
log(`Collected ${allFindings.length} raw findings`);

// ---- Phase 2: dedup ---------------------------------------------------------
phase('Dedup');

// Console/network FACTS harvested mechanically from the capture logs (never at
// an agent's discretion), grouped by normalized message.
const normalize = (s) =>
  (s || '')
    .replace(/https?:\/\/[^\s)]+/g, 'URL')
    .replace(/127\.0\.0\.1:\d+/g, 'HOST')
    .replace(/[0-9a-f]{6,}/gi, '#')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 200);
const factGroups = {};
let factCount = 0;
for (const r of manifest.pages) {
  for (const c of r.consoleErrors || []) {
    factCount++;
    const key = `console:${normalize(c.text)}`;
    (factGroups[key] ||= {
      bugClass: 'console',
      message: c.text,
      instances: [],
    }).instances.push({ unit: r.unit, route: r.route, viewport: r.viewport, theme: r.theme });
  }
  for (const n of r.networkFailures || []) {
    factCount++;
    const msg = `${n.status || n.error || 'failed'} ${n.url}`;
    const key = `network:${normalize(msg)}`;
    (factGroups[key] ||= {
      bugClass: 'network',
      message: msg,
      instances: [],
    }).instances.push({ unit: r.unit, route: r.route, viewport: r.viewport, theme: r.theme });
  }
}
const factClusters = Object.values(factGroups);
log(`Console/network facts: ${factCount} -> ${factClusters.length} groups`);

// Visual findings clustered by root cause via one agent.
const visualFindings = allFindings.filter((f) => f.bugClass === 'visual');
let visualClusters = [];
if (visualFindings.length) {
  const indexed = visualFindings.map((f, i) => ({ i, ...f }));
  const res = await agent(
    `These are ${visualFindings.length} visual UI findings from a Taliesin audit, each with an index "i". Many are the SAME underlying bug repeated across pages (shared CSS/component). Cluster them by ROOT CAUSE. For each cluster: a clear title, bugClass "visual", the WORST severity among its members, the suspected root cause / component / selector, and instanceIndexes = the list of member "i" values. Do not drop any finding; every index must appear in exactly one cluster.\n\nFindings:\n${JSON.stringify(indexed, null, 1)}`,
    { label: 'dedup:visual', phase: 'Dedup', schema: CLUSTER_SCHEMA, agentType: 'general-purpose', model: MODEL },
  );
  visualClusters = (res?.clusters ?? []).map((c) => ({
    ...c,
    instances: (c.instanceIndexes || [])
      .map((i) => visualFindings[i])
      .filter(Boolean),
  }));
}
log(`Visual findings: ${visualFindings.length} -> ${visualClusters.length} clusters`);

// ---- Phase 3: adversarial verify (visual clusters only) ---------------------
phase('Verify');
const verified = await parallel(
  visualClusters.map((c) => () => {
    const inst = c.instances[0] || {};
    const shot =
      manifest.pages.find(
        (p) =>
          p.unit === inst.unit &&
          p.route === inst.route &&
          p.viewport === inst.viewport &&
          p.theme === inst.theme,
      )?.screenshot ||
      manifest.pages.find((p) => p.unit === inst.unit && p.route === inst.route)
        ?.screenshot;
    const shotAbs = shot ? `${artifactsRoot}/${shot}` : '(no screenshot)';
    const prompt = `Adversarially verify a candidate Taliesin UI bug. Your job is to REFUTE it. Default to "refuted" if uncertain, or if it is intended/expected behavior.

Candidate: ${c.title}
Severity(claimed): ${c.severity}
Root cause(claimed): ${c.rootCause || '(none)'}
Suspected source: ${c.suspected || '(none)'}
Example instance: ${inst.unit} ${inst.route} @ ${inst.viewport}/${inst.theme}
Representative screenshot (Read it): ${shotAbs}

You MAY Grep/Read the repo to check intent vs defect, e.g.:
  crates/core/src/render/emit.rs, divs.rs, figure.rs, deck.rs
  crates/core/assets/css/base.css, crates/core/assets/css/dark.css
  crates/core/src/render/theme.rs, web-client/client.js
Return "confirmed" ONLY if it is a genuine defect a reader would see; "refuted" if not real / intended; "uncertain" if truly undecidable.`;
    return agent(prompt, {
      label: `verify:${c.title}`.slice(0, 60),
      phase: 'Verify',
      schema: VERDICT_SCHEMA,
      agentType: 'general-purpose',
      model: MODEL,
    }).then((v) => ({ cluster: c, verdict: v }));
  }),
);

const confirmed = verified
  .filter(Boolean)
  .filter((v) => v.verdict && v.verdict.verdict !== 'refuted')
  .map((v) => ({
    title: v.cluster.title,
    severity: v.verdict.severity || v.cluster.severity,
    verdict: v.verdict.verdict,
    rootCause: v.verdict.rootCause || v.cluster.rootCause || '',
    sourceLocation: v.verdict.sourceLocation || v.cluster.suspected || '',
    reason: v.verdict.reason,
    instanceCount: v.cluster.instances.length,
    examples: v.cluster.instances.slice(0, 6),
  }));
log(`Verified: ${confirmed.length}/${visualClusters.length} visual clusters survived refutation`);

// ---- Phase 4: report --------------------------------------------------------
phase('Report');
const reportData = {
  summary: {
    pagesAudited: pageGroups.length,
    cells: manifest.pages.length,
    rawFindings: allFindings.length,
    confirmedVisual: confirmed.length,
    consoleNetworkGroups: factClusters.length,
    buildFailures: manifest.buildFailures?.length || 0,
  },
  confirmedVisual: confirmed,
  consoleNetwork: factClusters.map((g) => ({
    bugClass: g.bugClass,
    message: g.message,
    count: g.instances.length,
    examples: g.instances.slice(0, 6),
  })),
  buildFailures: manifest.buildFailures || [],
  probeResults: probeResults?.results || null,
};

const reportMarkdown = await agent(
  `Write a concise, ranked Markdown bug report for a Taliesin UI audit. Use these sections:
1. **Summary** — the counts.
2. **Confirmed visual/layout bugs** — most severe first; each: title, severity, affected pages (count + a few examples as unit+route @ viewport/theme), root cause, suspected source location, and the verifier's reasoning.
3. **Console / JS errors** — grouped, with counts.
4. **Failed network requests** — grouped, with counts.
5. **Interaction probe results** — pass/fail per feature (or "not run").
6. **Build failures** — if any.
Be factual and skimmable. Return the raw markdown body ONLY: no preamble, no sign-off, and do NOT wrap it in a \`\`\`markdown code fence.

Data:
${JSON.stringify(reportData, null, 1)}`,
  { label: 'report', phase: 'Report', agentType: 'general-purpose', model: MODEL },
);

return {
  reportMarkdown,
  summary: reportData.summary,
  confirmedVisual: confirmed,
  consoleNetwork: reportData.consoleNetwork,
  probeResults: reportData.probeResults,
  buildFailures: reportData.buildFailures,
};
