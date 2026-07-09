#!/usr/bin/env node
// Split artifacts/manifest.json into per-unit part files so the audit Workflow
// can bootstrap it. A workflow script has NO filesystem access, so it reads the
// manifest through an agent -- but a full-corpus manifest (~530 cells) is ~66K
// tokens, past the 64K agent-output ceiling, so a single reader agent cannot
// echo it back (it fails with "response exceeded the output token maximum").
//
// The fix: shard the manifest by unit (the heaviest unit is ~13K tokens, 5x
// under the cap) and let the workflow read the shards in parallel, one small
// reader agent per unit, then concatenate. This also makes the bootstrap fast
// instead of one giant serial read.
//
// Writes <artifacts>/parts/<slug>.json (a bare array of that unit's page
// records) plus <artifacts>/parts/_index.json = { parts:[filename...],
// buildFailures:[...], unitCount, cellCount }. The workflow takes
// args.manifestPartsDir pointing at the parts/ dir.
//
// Usage: node split-manifest.mjs [--out <dir>]   (default .work)

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

let outDir = path.join(__dirname, '.work');
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  if (a === '--out') outDir = path.resolve(process.argv[++i]);
  else {
    console.error(`unknown flag: ${a}`);
    process.exit(2);
  }
}

const artifactsRoot = path.join(outDir, 'artifacts');
const manifestPath = path.join(artifactsRoot, 'manifest.json');
if (!fs.existsSync(manifestPath)) {
  console.error(`no manifest: ${manifestPath} (run capture-run.mjs or rebuild-manifest.mjs first)`);
  process.exit(1);
}

const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
if (!Array.isArray(manifest.pages) || !manifest.pages.length) {
  console.error('manifest has no pages');
  process.exit(1);
}

const partsDir = path.join(artifactsRoot, 'parts');
fs.rmSync(partsDir, { recursive: true, force: true });
fs.mkdirSync(partsDir, { recursive: true });

// Group by unit, preserving manifest order.
const byUnit = new Map();
for (const p of manifest.pages) {
  if (!byUnit.has(p.unit)) byUnit.set(p.unit, []);
  byUnit.get(p.unit).push(p);
}

const slug = (u) => u.replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^_+|_+$/g, '') || 'unit';
const parts = [];
const used = new Set();
for (const [unit, pages] of byUnit) {
  let name = `${slug(unit)}.json`;
  let n = 1;
  while (used.has(name)) name = `${slug(unit)}-${n++}.json`;
  used.add(name);
  fs.writeFileSync(path.join(partsDir, name), JSON.stringify(pages, null, 2));
  parts.push({ unit, file: name, cells: pages.length });
}

const index = {
  parts,
  buildFailures: manifest.buildFailures || [],
  unitCount: byUnit.size,
  cellCount: manifest.pages.length,
  rebuiltFromArtifacts: !!manifest.rebuiltFromArtifacts,
};
fs.writeFileSync(path.join(partsDir, '_index.json'), JSON.stringify(index, null, 2));

console.log(
  `[split] ${byUnit.size} units, ${manifest.pages.length} cells -> ${partsDir}\n` +
    `        pass args.manifestPartsDir="${partsDir}" to the audit workflow`,
);
