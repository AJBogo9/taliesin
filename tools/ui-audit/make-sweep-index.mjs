// Generate a browsable index over a `capture-run.mjs --out <dir>` build tree.
//
// The capture harness already builds every unit to static HTML; this walks that
// tree and emits one page linking every route of every unit, so a human can
// click through the whole rendered surface in order instead of remembering
// which of 69 units they have already looked at.
//
//   node make-sweep-index.mjs --out .work-sweep [--filter corpus/]
//
// Routes are recovered the same way the harness recovers them: a build's *.html
// paths swap back onto the unit's source root (Taliesin's route<->source mapping
// is a literal extension swap, no slugification).

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { allUnits } from './lib/units.mjs';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i === -1 ? fallback : process.argv[i + 1];
}

const outDir = path.resolve(REPO_ROOT, 'tools/ui-audit', arg('out', '.work-sweep'));
// Default to the WHOLE rendered surface, not just `corpus/`. The manual (`docs/guide`,
// `docs/internals`) and the marketing site are what a stranger sees first, and a
// corpus-only sweep never opens them. `--filter corpus/` narrows it back.
const filter = arg('filter', '');
const buildRoot = path.join(outDir, 'build');

// `draft: true` pages are excluded from `build` by design and exist only under `preview`,
// so a build-tree sweep silently omits them. Named here so the index can SAY they are
// missing rather than leaving a hole the reader has to notice.
const DRAFT_ONLY = [
  'corpus/course/problems.tmd',
  'corpus/demo-book/appendix.tmd',
  'corpus/tech-blog/posts/draft-example/index.tmd',
];

// Pull each corpus doc's "Exercises" cell out of corpus/README.md so the index
// says what a unit is FOR, not just that it exists. The table is
// `| path | category | exercises | source |`; paths are repo-relative-ish
// (`posts/em-algorithm/`), so match by prefix against the unit source.
function readCorpusNotes() {
  const notes = [];
  let md = '';
  try {
    md = fs.readFileSync(path.join(REPO_ROOT, 'corpus/README.md'), 'utf8');
  } catch {
    return notes;
  }
  for (const line of md.split('\n')) {
    if (!line.startsWith('|')) continue;
    // Split on UNESCAPED pipes only. Several cells document cell options as `#\| label:`,
    // and a naive `split('|')` cuts the row there — the description then ends mid-sentence
    // with no sign anything was lost.
    const cells = line
      .split(/(?<!\\)\|/)
      .map((c) => c.replace(/\\\|/g, '|').trim());
    if (cells.length < 5) continue;
    const key = cells[1].replace(/`/g, '').replace(/\/$/, '');
    if (!key || key === 'Path' || key.startsWith('---')) continue;
    notes.push({ key, category: cells[2], exercises: cells[3] });
  }
  return notes;
}

const NOTES = readCorpusNotes();

function noteFor(source) {
  const rel = source.replace(/^corpus\//, '').replace(/\.tmd$/, '').replace(/\/index$/, '');
  // Longest matching key wins (`posts/em-algorithm` over `posts`).
  let best = null;
  for (const n of NOTES) {
    const k = n.key.replace(/\.tmd$/, '').replace(/\/index$/, '');
    if (rel === k || rel.startsWith(k + '/')) {
      if (!best || k.length > best.key.length) best = { ...n, key: k };
    }
  }
  return best;
}

function walkHtml(dir, base = dir) {
  const out = [];
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const ent of entries) {
    const abs = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      if (ent.name.startsWith('_') || ent.name.startsWith('.')) continue;
      out.push(...walkHtml(abs, base));
    } else if (ent.isFile() && ent.name.endsWith('.html')) {
      out.push(path.relative(base, abs));
    }
  }
  return out;
}

// The non-HTML products of a build that a human still wants to eyeball: Atom
// feeds, the sitemap, the Cmd-K search index and the two agent-facing text
// projections. They are page *outputs* too, and a whole-corpus look that only
// opens `.html` never checks them.
const SIDE_ARTIFACTS = [
  ['sitemap.xml', 'sitemap'],
  ['llms.txt', 'llms.txt'],
  ['llms-full.txt', 'llms-full.txt'],
  ['search-index.js', 'search index'],
];

function sideArtifacts(dir) {
  const found = [];
  for (const [file, label] of SIDE_ARTIFACTS) {
    if (fs.existsSync(path.join(dir, file))) found.push({ file, label });
  }
  // Atom feeds are named after the listing page that produced them.
  try {
    for (const f of fs.readdirSync(dir)) {
      if (f.endsWith('.xml') && f !== 'sitemap.xml') found.push({ file: f, label: `feed: ${f}` });
    }
  } catch {
    /* unbuilt */
  }
  return found;
}

function titleOf(absHtml) {
  try {
    const head = fs.readFileSync(absHtml, 'utf8').slice(0, 8000);
    const m = head.match(/<title>([\s\S]*?)<\/title>/i);
    if (!m) return null;
    return m[1].replace(/\s+/g, ' ').trim() || null;
  } catch {
    return null;
  }
}

const esc = (s) =>
  String(s ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');

// `exercises` cells carry inline markdown (`code`, **bold**); render the two
// that actually appear so the index reads as prose rather than source.
const mdInline = (s) =>
  esc(s)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

const units = allUnits(REPO_ROOT)
  .filter((u) => u.source.startsWith(filter))
  .map((u) => {
    const dir = path.join(buildRoot, u.slug);
    const built = fs.existsSync(dir);
    const routes = built
      ? walkHtml(dir)
          .sort((a, b) => {
            // index.html first, then alphabetical.
            const ai = a === 'index.html' ? 0 : 1;
            const bi = b === 'index.html' ? 0 : 1;
            return ai - bi || a.localeCompare(b);
          })
          .map((r) => ({ route: r, title: titleOf(path.join(dir, r)) }))
      : [];
    const side = built ? sideArtifacts(dir) : [];
    return { ...u, built, routes, side, note: noteFor(u.source) };
  });

const corpusUnits = units.filter((u) => u.source.startsWith('corpus/'));
const sites = corpusUnits.filter((u) => u.type === 'site');
const standalones = corpusUnits.filter((u) => u.type === 'standalone');
// The rest of the rendered surface: the two dogfooded manuals and the marketing site.
const others = units.filter((u) => !u.source.startsWith('corpus/'));
const totalPages = units.reduce((n, u) => n + u.routes.length, 0);
const missing = units.filter((u) => !u.built);

function unitCard(u, i) {
  const kind = u.kind || u.format || '';
  const rows = u.routes
    .map(
      (r) =>
        `<li><a href="${esc(u.slug)}/${esc(r.route)}">${esc(r.title || r.route)}</a>` +
        `<span class="route">${esc(r.route)}</span></li>`,
    )
    .join('\n');
  return `<section class="unit" id="${esc(u.slug)}">
  <h3><span class="n">${i}</span> ${esc(u.source)}
    <span class="badge ${esc(u.type)}">${esc(u.type)}</span>
    ${kind ? `<span class="badge kind">${esc(kind)}</span>` : ''}
    <span class="badge count">${u.routes.length} page${u.routes.length === 1 ? '' : 's'}</span>
  </h3>
  ${u.note ? `<p class="note"><strong>${mdInline(u.note.category)}.</strong> ${mdInline(u.note.exercises)}</p>` : ''}
  ${u.built ? `<ul class="routes">${rows}</ul>` : '<p class="missing">NOT BUILT</p>'}
  ${
    u.side.length
      ? `<p class="side">also: ${u.side
          .map((s) => `<a href="${esc(u.slug)}/${esc(s.file)}">${esc(s.label)}</a>`)
          .join(' · ')}</p>`
      : ''
  }
</section>`;
}

let n = 0;
const html = `<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Taliesin corpus sweep</title>
<style>
  :root { color-scheme: light dark; --fg:#111; --dim:#666; --line:#ddd; --bg:#fff; --accent:#2a4b8d; }
  @media (prefers-color-scheme: dark) {
    :root { --fg:#e6e6e6; --dim:#9a9a9a; --line:#333; --bg:#141414; --accent:#8fb0ee; }
  }
  * { box-sizing: border-box; }
  body { margin:0; padding:2rem 1.25rem 6rem; background:var(--bg); color:var(--fg);
         font:15px/1.55 ui-sans-serif,system-ui,-apple-system,Segoe UI,Roboto,sans-serif;
         max-width: 62rem; margin-inline:auto; }
  h1 { font-size:1.6rem; margin:0 0 .25rem; }
  h2 { font-size:1.15rem; margin:2.5rem 0 .5rem; padding-bottom:.3rem; border-bottom:2px solid var(--line); }
  h3 { font-size:.98rem; margin:0 0 .35rem; font-weight:600; display:flex; flex-wrap:wrap; gap:.4rem; align-items:center; }
  .lede { color:var(--dim); margin:0 0 1.5rem; }
  .unit { border:1px solid var(--line); border-radius:8px; padding:.85rem 1rem; margin:.6rem 0; }
  .n { color:var(--dim); font-variant-numeric:tabular-nums; font-weight:400; min-width:2.2rem; }
  .badge { font-size:.7rem; text-transform:uppercase; letter-spacing:.04em; padding:.1rem .4rem;
           border:1px solid var(--line); border-radius:99px; color:var(--dim); font-weight:500; }
  .badge.site { border-color:var(--accent); color:var(--accent); }
  .note { margin:.2rem 0 .6rem; color:var(--dim); font-size:.85rem; }
  ul.routes { list-style:none; margin:0; padding:0; display:grid;
              grid-template-columns:repeat(auto-fill,minmax(19rem,1fr)); gap:.15rem .9rem; }
  ul.routes li { display:flex; gap:.5rem; align-items:baseline; min-width:0; }
  ul.routes a { color:var(--accent); text-decoration:none; white-space:nowrap;
                overflow:hidden; text-overflow:ellipsis; }
  ul.routes a:hover { text-decoration:underline; }
  .route { color:var(--dim); font-size:.72rem; font-family:ui-monospace,monospace;
           margin-left:auto; white-space:nowrap; }
  .missing { color:#c33; font-weight:600; margin:.2rem 0; }
  .side { margin:.5rem 0 0; font-size:.75rem; color:var(--dim); }
  .side a { color:var(--dim); }
  code { font-family:ui-monospace,monospace; font-size:.9em; }
  .summary { display:flex; gap:1.5rem; flex-wrap:wrap; padding:.8rem 1rem; border:1px solid var(--line);
             border-radius:8px; background:color-mix(in srgb, var(--fg) 4%, transparent); }
  .summary div { font-size:.8rem; color:var(--dim); }
  .summary b { display:block; font-size:1.5rem; color:var(--fg); font-variant-numeric:tabular-nums; }
</style></head><body>
<h1>Taliesin corpus sweep</h1>
<p class="lede">Every document of the rendered surface, built and browsable: the corpus, the two
dogfooded manuals, and the marketing site. Click a page to open the real rendered HTML &mdash;
interactive features work (reactive cells, Cmd-K search, scrollspy), so judge them live
rather than from a screenshot.</p>
<div class="summary">
  <div><b>${units.length}</b> units</div>
  <div><b>${sites.length}</b> corpus projects</div>
  <div><b>${standalones.length}</b> corpus docs</div>
  <div><b>${others.length}</b> docs + site</div>
  <div><b>${totalPages}</b> pages</div>
  ${missing.length ? `<div><b style="color:#c33">${missing.length}</b> not built</div>` : ''}
</div>

<h2>Corpus &mdash; site projects <span class="badge">${sites.length}</span></h2>
${sites.map((u) => unitCard(u, ++n)).join('\n')}

<h2>Corpus &mdash; standalone documents <span class="badge">${standalones.length}</span></h2>
${standalones.map((u) => unitCard(u, ++n)).join('\n')}

${
  others.length
    ? `<h2>The manual and the marketing site <span class="badge">${others.length}</span></h2>
<p class="lede">Not corpus, but the same rendered surface &mdash; and the first thing a
stranger sees. <code>site/</code> is composed with the two books and the gallery exhibits
by <code>tools/build-site.sh</code>.</p>
${others.map((u) => unitCard(u, ++n)).join('\n')}`
    : ''
}

<h2>Not in this sweep <span class="badge">${DRAFT_ONLY.length}</span></h2>
<p class="lede">These carry <code>draft: true</code>, which <code>build</code> excludes by
design; they render only under <code>taliesin preview</code>, where they appear badged.</p>
<ul class="routes">
${DRAFT_ONLY.map((d) => `  <li><code>${esc(d)}</code></li>`).join('\n')}
</ul>
</body></html>
`;

const dest = path.join(buildRoot, 'index.html');
fs.writeFileSync(dest, html);
console.log(
  `wrote ${dest}\n  ${units.length} units (${sites.length} site + ${standalones.length} standalone), ` +
    `${totalPages} pages${missing.length ? `, ${missing.length} NOT BUILT: ${missing.map((u) => u.slug).join(', ')}` : ''}`,
);
