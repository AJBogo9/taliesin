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

// The multi-page projects, DISCOVERED rather than declared: every directory
// holding a `_site.yml`.
//
// This was a hardcoded list of six until 2026-08-05, and it had gone stale:
// fifteen `_site.yml` projects existed, so twelve corpus projects (debug,
// analyst, course, descent, tarn, ...) fell through to the standalone
// enumerator and were captured page-by-page as single documents. Their nav,
// chapter sidebar, cross-page links and site chrome were therefore never
// rendered in a capture at all, and a whole-corpus audit silently skipped them.
// Derive, don't declare.
//
// `kind` is the reported `format` for a project's pages: a book declares a
// top-level `chapters:` in its `_site.yml`, everything else is directory-walked.
const PRUNED_DIRS = new Set(['target', 'node_modules', 'dist']);

function* walkDirs(dir) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const ent of entries) {
    if (!ent.isDirectory()) continue;
    // `_`/`.` prefixes are build output (`_site/`, `.work/`) or partials.
    if (PRUNED_DIRS.has(ent.name)) continue;
    if (ent.name.startsWith('_') || ent.name.startsWith('.')) continue;
    const abs = path.join(dir, ent.name);
    yield abs;
    yield* walkDirs(abs);
  }
}

function declaresChapters(siteYmlPath) {
  try {
    return /^chapters:/m.test(fs.readFileSync(siteYmlPath, 'utf8'));
  } catch {
    return false;
  }
}

// corpus/single-page-report -> single-page-report, docs/guide -> docs-guide.
// Keeps the slugs the six declared units already had, so existing `--only`
// invocations and any banked artifact paths still match.
function siteSlug(source) {
  return source.replace(/^corpus\//, '').split('/').join('-');
}

export function discoverSiteUnits(repoRoot) {
  const found = [];
  for (const abs of walkDirs(repoRoot)) {
    if (!fs.existsSync(path.join(abs, '_site.yml'))) continue;
    found.push(path.relative(repoRoot, abs));
  }
  // Drop a project nested inside another: the outer build emits it (`mounts:`),
  // so capturing it twice would double-count and misattribute its routes.
  const roots = found.filter(
    (r) => !found.some((o) => o !== r && r.startsWith(o + path.sep)),
  );
  return roots.sort().map((source) => ({
    slug: siteSlug(source),
    source,
    kind: declaresChapters(path.join(repoRoot, source, '_site.yml'))
      ? 'book'
      : 'website',
  }));
}

// A path segment starting with `_` or `.` marks a partial/hidden file that the
// site walker skips (e.g. corpus/_includes/*, subsections/_intro.tmd). We apply
// the same rule so a naive glob never treats an include target as a page.
function hasHiddenSegment(relPath) {
  return relPath
    .split(path.sep)
    .some((seg) => seg.startsWith('_') || seg.startsWith('.'));
}

function isUnderSiteRoot(relPath, siteUnits) {
  return siteUnits.some(
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

// Standalone corpus docs = every corpus/**/*.tmd that is NOT under a site-unit
// root and has no `_`/`.`-prefixed path segment. This reproduces the exclude
// list (all excludes are `_`-prefixed partials) while staying robust as the
// corpus grows.
export function discoverStandalones(repoRoot, siteUnits = discoverSiteUnits(repoRoot)) {
  const corpus = path.join(repoRoot, 'corpus');
  const out = [];
  for (const abs of walk(corpus)) {
    if (!abs.endsWith('.tmd')) continue;
    const rel = path.relative(repoRoot, abs);
    if (hasHiddenSegment(rel)) continue;
    if (isUnderSiteRoot(rel, siteUnits)) continue;
    out.push(rel);
  }
  return out.sort();
}

// Rough page-count estimate, used to load-balance shards (LPT) so the heavy
// multi-page books/sites spread across shard processes instead of two colliding
// on one. Exact counts need a build; counting the non-hidden .tmd sources under
// a site (~= its pages) and 1 for a standalone is enough for balancing.
export function estimatePages(unit, repoRoot) {
  if (unit.type === 'standalone') return 1;
  const root = path.join(repoRoot, unit.source);
  let n = 0;
  for (const abs of walk(root)) {
    if (!abs.endsWith('.tmd')) continue;
    if (hasHiddenSegment(path.relative(repoRoot, abs))) continue;
    n++;
  }
  return Math.max(1, n);
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
  const siteUnits = discoverSiteUnits(repoRoot);
  const site = siteUnits.map((u) => ({ ...u, type: 'site' }));
  const standalone = discoverStandalones(repoRoot, siteUnits).map((rel) => ({
    slug: standaloneSlug(rel),
    source: rel,
    type: 'standalone',
    format: 'article',
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
