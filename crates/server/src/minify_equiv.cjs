// Assert that minifying JS changed NOTHING but comments and whitespace.
//
// Usage: node --expose-internals minify_equiv.cjs <original.js> <minified.js>
// Exit 0 = equivalent; exit 1 = a real difference, printed with context.
//
// Why this exists: `node --check` only proves the output PARSES, which is not
// evidence of correctness for this minifier's bug class. A nested-template or
// regex-context slip rewrites a token's VALUE while leaving the token COUNT
// identical, so the result parses clean and is silently wrong. Comparing the
// token streams catches exactly that.
//
// Uses Node's OWN bundled acorn (no network, no dependency, no lockfile). That is
// an internal path: if a Node upgrade moves it, this must FAIL LOUDLY rather than
// skip, or the guard silently regresses to zero coverage — the failure mode this
// whole guard exists to prevent.

const fs = require("fs");

let acorn;
try {
  acorn = require("internal/deps/acorn/acorn/dist/acorn");
} catch (e) {
  console.error(
    "FATAL: Node's bundled acorn is not reachable at " +
      "internal/deps/acorn/acorn/dist/acorn (needs --expose-internals).\n" +
      "If a Node upgrade moved it, find the new path — do not delete this guard.\n" +
      "Underlying error: " + e.message,
  );
  process.exit(2);
}

const [, , origPath, minPath] = process.argv;
if (!origPath || !minPath) {
  console.error("usage: minify_equiv.cjs <original.js> <minified.js>");
  process.exit(2);
}

const OPTS = { ecmaVersion: "latest", sourceType: "script" };

// Comments and whitespace are the only things the minifier may remove, and acorn's
// tokenizer already drops both, so a faithful minify yields an identical stream.
function tokens(src, label) {
  const out = [];
  try {
    for (const t of acorn.tokenizer(src, OPTS)) {
      // `type.label` + `value` together catch a changed token VALUE (the identical-
      // count bug class), not just a changed shape.
      out.push({ label: t.type.label, value: String(t.value), start: t.start });
    }
  } catch (e) {
    console.error(`FAIL: ${label} does not tokenize: ${e.message}`);
    process.exit(1);
  }
  return out;
}

const orig = fs.readFileSync(origPath, "utf8");
const min = fs.readFileSync(minPath, "utf8");

// A parse of each side, so a minified file that tokenizes but no longer parses
// (e.g. a truncated string literal flipping quote parity) is still caught.
for (const [src, label, path] of [
  [orig, "original", origPath],
  [min, "minified", minPath],
]) {
  try {
    acorn.parse(src, OPTS);
  } catch (e) {
    console.error(`FAIL: ${label} (${path}) does not parse: ${e.message}`);
    process.exit(1);
  }
}

const a = tokens(orig, "original");
const b = tokens(min, "minified");

if (a.length !== b.length) {
  console.error(`FAIL: token count differs: original ${a.length}, minified ${b.length}`);
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i].label !== b[i].label || a[i].value !== b[i].value) {
      console.error(`  first divergence at token ${i}:`);
      console.error(`    original: ${a[i].label} ${JSON.stringify(a[i].value)}`);
      console.error(`    minified: ${b[i].label} ${JSON.stringify(b[i].value)}`);
      break;
    }
  }
  process.exit(1);
}

for (let i = 0; i < a.length; i++) {
  if (a[i].label !== b[i].label || a[i].value !== b[i].value) {
    console.error(`FAIL: token ${i} changed (count identical, so this parses clean):`);
    console.error(`  original: ${a[i].label} ${JSON.stringify(a[i].value)}`);
    console.error(`  minified: ${b[i].label} ${JSON.stringify(b[i].value)}`);
    const ctx = orig.slice(Math.max(0, a[i].start - 60), a[i].start + 60);
    console.error(`  context: ...${ctx.replace(/\n/g, "\\n")}...`);
    process.exit(1);
  }
}

console.log(`ok: ${a.length} tokens identical`);
