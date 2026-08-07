// Build every unit of Taliesin's rendered surface to static HTML, and nothing else.
//
//   node build-sweep.mjs --out <dir> [--filter corpus/] [--jobs N] [--bin <taliesin>]
//
// `capture-run.mjs` already does this as its first phase, but it then screenshots, which
// is the slow, browser-bound, occasionally-wedging part. When the goal is a HUMAN reading
// the pages in a real browser (the corpus sweep), the screenshots are redundant: you want
// the builds served, not a contact sheet. This is that half on its own.
//
// Point --out somewhere OUTSIDE the repo. `crates/core/tests/retired_names.rs` walks the
// filesystem rather than `git ls-files`, so a build tree left inside the working copy
// fails the retired-brand gate: the bundled fonts are inlined as base64, and some of that
// payload spells the retired brand by chance. Measured: 90 spurious hits, every one of
// them font data. (This comment cannot quote the token for the same reason.)

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { allUnits } from './lib/units.mjs';
import { buildUnit } from './lib/build.mjs';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i === -1 ? fallback : process.argv[i + 1];
}

const outDir = path.resolve(arg('out', path.join(os.tmpdir(), 'taliesin-sweep')));
const buildRoot = path.join(outDir, 'build');
const filter = arg('filter', '');
const jobs = Number(arg('jobs', String(Math.max(2, Math.min(8, os.cpus().length - 2)))));
const bin =
  arg('bin', process.env.TALIESIN_BIN) || path.join(REPO_ROOT, 'target/release/taliesin');

if (!fs.existsSync(bin)) {
  console.error(`no taliesin binary at ${bin} (build it, or pass --bin)`);
  process.exit(2);
}

const units = allUnits(REPO_ROOT).filter((u) => u.source.startsWith(filter));
fs.mkdirSync(buildRoot, { recursive: true });

console.log(`[build-sweep] bin=${bin}`);
console.log(`[build-sweep] units=${units.length} jobs=${jobs} out=${buildRoot}`);

let next = 0;
let done = 0;
const failures = [];

async function worker() {
  while (next < units.length) {
    const unit = units[next++];
    const res = await buildUnit(unit, { bin, buildRoot, repoRoot: REPO_ROOT });
    done++;
    if (res.ok) {
      console.log(`[${done}/${units.length}] ok   ${unit.source}`);
    } else {
      failures.push({ unit: unit.source, status: res.status, log: res.log });
      console.log(`[${done}/${units.length}] FAIL ${unit.source} (${res.status})`);
    }
  }
}

await Promise.all(Array.from({ length: jobs }, worker));

if (failures.length) {
  console.log(`\n${failures.length} unit(s) failed to build:`);
  for (const f of failures) {
    console.log(`\n--- ${f.unit} (${f.status}) ---\n${(f.log || '').trim().slice(-1200)}`);
  }
  process.exit(1);
}
console.log(`\n[build-sweep] all ${units.length} units built into ${buildRoot}`);
