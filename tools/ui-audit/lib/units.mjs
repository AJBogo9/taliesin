// Enumerate the "build units" that make up Taliesin's full rendered surface.
//
// A unit is one thing you hand to `taliesin build`:
//   - a SITE project (a directory with a _site.yml) -> many HTML pages, or
//   - a STANDALONE single .tmd -> one HTML page.
//
// Route<->source mapping in Taliesin is a literal, structure-preserving
// extension swap (.tmd <-> .html) with no slugification anywhere, so after a
// build we can just glob *.html and swap the extension back onto the unit's
// source root to recover the source file. See the design spec.

import fs from 'node:fs';
import path from 'node:path';

// The six multi-page projects. `kind` is the reported `format` for their pages
// (books have `chapters:` in _site.yml; websites are directory-walked).
export const SITE_UNITS = [
  { slug: 'site', source: 'site', kind: 'website' },
  { slug: 'docs-guide', source: 'docs/guide', kind: 'book' },
  { slug: 'docs-internals', source: 'docs/internals', kind: 'book' },
  { slug: 'bayesian-website', source: 'corpus/bayesian-website', kind: 'website' },
  { slug: 'demo-book', source: 'corpus/demo-book', kind: 'book' },
  { slug: 'tech-blog', source: 'corpus/tech-blog', kind: 'website' },
];

// Decks that exist only as `{{< embed >}}` targets. They are NOT standalone
// units (the owning site build emits them as tour.html / demo.html inside its
// output tree, so they get captured there). Listed here so the site enumerator
// can label their route as `deck`.
export const EMBED_DECK_SOURCES = new Set([
  'docs/guide/tour.tmd',
  'docs/guide/demo.tmd',
  'site/demo.tmd',
]);
export const EMBED_DECK_BASENAMES = new Set(['tour.html', 'demo.html']);

// A path segment starting with `_` or `.` marks a partial/hidden file that the
// site walker skips (e.g. corpus/_includes/*, subsections/_intro.tmd). We apply
// the same rule so a naive glob never treats an include target as a page.
function hasHiddenSegment(relPath) {
  return relPath
    .split(path.sep)
    .some((seg) => seg.startsWith('_') || seg.startsWith('.'));
}

function isUnderSiteRoot(relPath) {
  return SITE_UNITS.some(
    (u) => relPath === u.source || relPath.startsWith(u.source + path.sep),
  );
}

function* walk(dir) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const ent of entries) {
    const abs = path.join(dir, ent.name);
    if (ent.isDirectory()) yield* walk(abs);
    else if (ent.isFile()) yield abs;
  }
}

// Read a .tmd's front matter and decide its format. Only decks matter for
// standalone capture (a deck renders very differently); everything else is a
// plain article/page.
export function detectFormat(absTmdPath) {
  let text = '';
  try {
    text = fs.readFileSync(absTmdPath, 'utf8').slice(0, 4000);
  } catch {
    return 'article';
  }
  if (!text.startsWith('---')) return 'article';
  const end = text.indexOf('\n---', 3);
  const fm = end === -1 ? text : text.slice(0, end);
  const m = fm.match(/^\s*format:\s*([A-Za-z0-9_-]+)/m);
  const fmt = (m?.[1] || '').toLowerCase();
  if (fmt === 'deck' || fmt === 'reveal' || fmt === 'slides') return 'deck';
  return 'article';
}

// Standalone corpus docs = every corpus/**/*.tmd that is NOT under a site-unit
// root and has no `_`/`.`-prefixed path segment. This reproduces the exclude
// list (all excludes are `_`-prefixed partials) while staying robust as the
// corpus grows.
export function discoverStandalones(repoRoot) {
  const corpus = path.join(repoRoot, 'corpus');
  const out = [];
  for (const abs of walk(corpus)) {
    if (!abs.endsWith('.tmd')) continue;
    const rel = path.relative(repoRoot, abs);
    if (hasHiddenSegment(rel)) continue;
    if (isUnderSiteRoot(rel)) continue;
    out.push(rel);
  }
  return out.sort();
}

// Build a stable slug from a source rel-path, e.g.
//   corpus/posts/em-algorithm/index.tmd -> posts__em-algorithm__index
function standaloneSlug(relTmd) {
  return relTmd
    .replace(/^corpus\//, '')
    .replace(/\.tmd$/, '')
    .split('/')
    .join('__');
}

// The full unit list: 6 site projects + N standalone docs. Each unit:
//   { slug, source, type: 'site'|'standalone', kind|format }
export function allUnits(repoRoot) {
  const site = SITE_UNITS.map((u) => ({ ...u, type: 'site' }));
  const standalone = discoverStandalones(repoRoot).map((rel) => ({
    slug: standaloneSlug(rel),
    source: rel,
    type: 'standalone',
    format: detectFormat(path.join(repoRoot, rel)),
  }));
  return [...site, ...standalone];
}

// Simple `--only` matcher: substring OR shell-ish glob against slug and source.
export function matchesGlob(unit, pattern) {
  if (!pattern) return true;
  const rx = new RegExp(
    '^' +
      pattern
        .replace(/[.+^${}()|[\]\\]/g, '\\$&')
        .replace(/\*/g, '.*')
        .replace(/\?/g, '.') +
      '$',
  );
  return (
    rx.test(unit.slug) ||
    rx.test(unit.source) ||
    unit.slug.includes(pattern) ||
    unit.source.includes(pattern)
  );
}
