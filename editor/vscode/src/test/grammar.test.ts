// Offline TextMate tokenization tests for the Taliesin (.tmd) grammar.
//
// Runs fully headless (no VS Code download, no network): it loads the grammar under test
// (syntaxes/tmd.tmLanguage.json) into a real vscode-textmate + vscode-oniguruma registry,
// alongside the built-in MIT markdown/python/yaml grammars that ship inside the .vscode-test
// VS Code download, and asserts token SCOPES on fixture lines. This is the CI-safe gate for the
// grammar (the manual F5 visual check is the author's; e2e asserts language *registration*).
//
// Grammars are located by glob under .vscode-test so the pinned VS Code version can change
// without editing paths. Missing embedded sub-grammars resolve to null (no color, not an error).
import { test } from "node:test";
import assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { globSync } from "glob";
import * as vsctm from "vscode-textmate";
import * as oniguruma from "vscode-oniguruma";

const EXT_ROOT = path.join(__dirname, "..", "..");
const VST = path.join(EXT_ROOT, ".vscode-test");

/** First file matching a glob under .vscode-test (robust to the pinned VS Code version). */
function findGrammar(rel: string): string | null {
  const hits = globSync(`**/${rel}`, { cwd: VST, absolute: true });
  return hits.length ? hits[0] : null;
}

// scopeName -> grammar file on disk. Our grammars + the download's bundled base grammars.
const GRAMMAR_FILES: Record<string, string | null> = {
  "text.tmd.markdown": path.join(EXT_ROOT, "syntaxes", "tmd.tmLanguage.json"),
  "text.tmd.markdown.injection": path.join(EXT_ROOT, "syntaxes", "tmd.injection.tmLanguage.json"),
  "text.html.markdown": findGrammar("markdown-basics/syntaxes/markdown.tmLanguage.json"),
  "source.python": findGrammar("python/syntaxes/MagicPython.tmLanguage.json"),
  "source.yaml": findGrammar("yaml/syntaxes/yaml.tmLanguage.json"),
};

// External injection grammars, keyed by the host scope they inject into (mirrors how VS Code
// resolves contributes.grammars `injectTo` — the offline registry needs this told explicitly).
const INJECTIONS: Record<string, string[]> = {
  "text.tmd.markdown": ["text.tmd.markdown.injection"],
};

let registryPromise: Promise<vsctm.Registry> | null = null;
function getRegistry(): Promise<vsctm.Registry> {
  if (registryPromise) return registryPromise;
  const wasm = fs.readFileSync(
    path.join(EXT_ROOT, "node_modules", "vscode-oniguruma", "release", "onig.wasm")
  );
  // Pass the Buffer (an ArrayBufferView) directly — `.buffer` can carry a nonzero byteOffset.
  const onigLib = oniguruma.loadWASM(wasm).then(() => ({
    createOnigScanner: (patterns: string[]) => new oniguruma.OnigScanner(patterns),
    createOnigString: (s: string) => new oniguruma.OnigString(s),
  }));
  registryPromise = Promise.resolve(
    new vsctm.Registry({
      onigLib,
      getInjections: (scopeName: string) => INJECTIONS[scopeName],
      loadGrammar: async (scopeName: string) => {
        const file = GRAMMAR_FILES[scopeName];
        if (!file || !fs.existsSync(file)) return null; // unknown/missing → graceful null
        const content = fs.readFileSync(file, "utf8");
        return vsctm.parseRawGrammar(content, file);
      },
    })
  );
  return registryPromise;
}

interface Tok {
  line: number;
  text: string;
  scopes: string[];
}

/** Tokenize .tmd source (multi-line, threading the rule stack) into flat tokens. */
async function tokenizeTmd(src: string): Promise<Tok[]> {
  const registry = await getRegistry();
  const grammar = await registry.loadGrammar("text.tmd.markdown");
  assert.ok(grammar, "text.tmd.markdown grammar must load");
  const out: Tok[] = [];
  let ruleStack = vsctm.INITIAL;
  const lines = src.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const r = grammar!.tokenizeLine(lines[i], ruleStack);
    for (const t of r.tokens) {
      out.push({ line: i, text: lines[i].substring(t.startIndex, t.endIndex), scopes: t.scopes });
    }
    ruleStack = r.ruleStack;
  }
  return out;
}

/** Scopes of the first token whose text contains `needle` (optionally on a given line). */
function scopesOf(toks: Tok[], needle: string, line?: number): string[] {
  const t = toks.find((x) => x.text.includes(needle) && (line === undefined || x.line === line));
  assert.ok(t, `no token containing ${JSON.stringify(needle)}${line !== undefined ? ` on line ${line}` : ""}`);
  return t!.scopes;
}

/** True if any token covering `needle` has a scope that starts with `scopePrefix`. */
function hasScope(toks: Tok[], needle: string, scopePrefix: string, line?: number): boolean {
  return toks
    .filter((x) => x.text.includes(needle) && (line === undefined || x.line === line))
    .some((x) => x.scopes.some((s) => s.startsWith(scopePrefix)));
}

/**
 * True if the token covering `needle` was tokenized by a REAL embedded sub-grammar for `.lang`
 * (a rule scope ending in `.lang`, e.g. `support.function.builtin.python`). Note: an embedded
 * grammar's own root scopeName (`source.python`) is NOT pushed onto the stack, so we look for its
 * rule scopes instead — the proof that `include: source.<lang>` actually ran.
 */
function embedsLang(toks: Tok[], needle: string, lang: string): boolean {
  return toks
    .filter((x) => x.text.includes(needle))
    .some((x) => x.scopes.some((s) => s.endsWith("." + lang) && !s.startsWith("meta.embedded")));
}

// ---------------------------------------------------------------------------
// Phase 0 — the grammar registers and inherits CommonMark via `include: text.html.markdown`.
// ---------------------------------------------------------------------------

test("base grammars are discoverable in the .vscode-test download", () => {
  assert.ok(GRAMMAR_FILES["text.html.markdown"], "bundled markdown grammar found under .vscode-test");
});

test("Phase 0: the tmd grammar loads and its top scope is text.tmd.markdown", async () => {
  const toks = await tokenizeTmd("# Hello\n");
  assert.ok(
    toks.every((t) => t.scopes[0] === "text.tmd.markdown"),
    "every token carries the owned top scope text.tmd.markdown (not markdown's)"
  );
});

test("Phase 0: inherited markdown — heading + bold get markdown scopes", async () => {
  const toks = await tokenizeTmd("# Title\n\nsome **bold** text\n");
  assert.ok(hasScope(toks, "Title", "markup.heading"), "heading inherited from text.html.markdown");
  assert.ok(hasScope(toks, "bold", "markup.bold"), "bold inherited from text.html.markdown");
});

test("Phase 0: inherited markdown — a BARE ```python fence embeds source.python", async () => {
  // The base markdown grammar already handles bare info strings; the BRACED {python} form is a
  // Phase-1 delta (added later). This pins the inherited baseline so the Phase-1 delta is a real add.
  const toks = await tokenizeTmd("```python\nprint(1)\n```\n");
  assert.ok(
    hasScope(toks, "print", "meta.embedded.block.python"),
    "bare fenced python body embeds via the inherited markdown grammar"
  );
});

// ---------------------------------------------------------------------------
// Phase 1 — braced executable cells + #|/// |/%%| cell options.
// ---------------------------------------------------------------------------

test("Phase 1: a braced ```{python} cell embeds source.python (the delta over base markdown)", async () => {
  const toks = await tokenizeTmd("```{python}\nprint(1)\n```\n");
  assert.ok(hasScope(toks, "print", "meta.embedded.block.python"), "braced {python} body embeds python");
  assert.ok(embedsLang(toks, "print", "python"), "inner tokens are tokenized by the real python grammar");
  assert.ok(hasScope(toks, "python", "keyword.other.taliesin.cell"), "the {python} header is scoped as a cell keyword");
});

test("Phase 1: a ```{js} cell embeds its language", async () => {
  const js = await tokenizeTmd("```{js}\nconst x = 1\n```\n");
  assert.ok(hasScope(js, "const", "meta.embedded.block.js"), "{js} body embeds js");
});

test("Phase 1: #| cell options get a directive scope, not a plain comment", async () => {
  const toks = await tokenizeTmd("```{python}\n#| echo: false\nprint(1)\n```\n");
  assert.ok(hasScope(toks, "#|", "keyword.control.directive.tmd"), "the #| marker is a directive keyword");
  assert.ok(hasScope(toks, "echo", "entity.name.tag.tmd"), "the option key is scoped like a tag/key");
  // the space-before-pipe form + the //| (js) and %%| (mermaid) forms:
  const spaced = await tokenizeTmd("```{python}\n# | echo: false\n```\n");
  assert.ok(hasScope(spaced, "# |", "keyword.control.directive.tmd"), "'# |' (space) is tolerated");
  const jsopt = await tokenizeTmd("```{js}\n//| input: scene\n```\n");
  assert.ok(hasScope(jsopt, "//|", "keyword.control.directive.tmd"), "//| (js) marker recognized");
});

test("Phase 1: {=html} raw-output is NOT a cell (excluded; falls through to markdown)", async () => {
  const toks = await tokenizeTmd("```{=html}\n<b>hi</b>\n```\n");
  assert.ok(!hasScope(toks, "hi", "meta.embedded.block.python"), "{=html} is not a python cell");
  assert.ok(!hasScope(toks, "html", "keyword.other.taliesin.cell"), "{=html} does not get the taliesin cell keyword");
});

// ---------------------------------------------------------------------------
// Phase 2 — front matter, math, divs, shortcodes, xref, cite.
// ---------------------------------------------------------------------------

test("Phase 2: a mid-doc --- stays a thematic break (never front matter)", async () => {
  // Leading `---` YAML front matter is handled by the INHERITED markdown grammar (its #frontMatter
  // rule is \A-anchored and sets meta.embedded.block.frontmatter → yaml via our embeddedLanguages);
  // \A only fires at true document start, which an isolated tokenizeLine harness can't reproduce, so
  // that positive case is an F5 check. What the harness CAN pin (and the real risk) is that a mid-doc
  // `---` is NOT swallowed as front matter — it stays a markdown thematic break.
  const toks = await tokenizeTmd("text\n\n---\n\nmore\n");
  const midDash = toks.find((t) => t.text.includes("---") && t.line === 2);
  assert.ok(midDash, "found the mid-doc ---");
  assert.ok(
    !midDash!.scopes.some((s) => s.includes("frontmatter")),
    "mid-doc --- is NOT front matter"
  );
});

test("Phase 2: ::: div fence + {.class #id} attrs are scoped", async () => {
  const toks = await tokenizeTmd('::: {.callout-note #warn title="Heads up"}\nbody\n:::\n');
  assert.ok(hasScope(toks, ":::", "keyword.control.tmd.div"), "::: colons scoped as a div keyword");
  assert.ok(hasScope(toks, ".callout-note", "entity.name.tag.tmd.div-class"), ".class scoped");
  assert.ok(hasScope(toks, "#warn", "entity.other.attribute-name.id.tmd"), "#id scoped");
});

test("Phase 2 (injection): inline $…$ and display $$…$$ math get a math scope", async () => {
  const inl = await tokenizeTmd("Euler: $e^{i\\pi}+1=0$ is nice\n");
  assert.ok(hasScope(inl, "e", "markup.math.inline.tmd"), "inline $…$ math region scoped");
  const dis = await tokenizeTmd("$$\\int_0^1 x\\,dx$$ {#eq-area}\n");
  assert.ok(hasScope(dis, "int", "markup.math.display.tmd"), "display $$…$$ math scoped");
  assert.ok(hasScope(dis, "eq-area", "entity.name.label.tmd"), "the {#eq-…} label is scoped");
});

// `package.json` paints the math delimiters bold, because no bundled theme defines a rule for
// them. That contribution is a pair of scope STRINGS, so a typo in either one is silently inert:
// the manifest test still passes, and the delimiters stay invisible. This is the gate that
// notices — every scope the manifest paints must be one the tokenizer actually emits on real
// `$…$` and `$$…$$`, matched exactly rather than by prefix.
test("Phase 2 (injection): the manifest's math-delimiter scopes are scopes the grammar emits", async () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(EXT_ROOT, "package.json"), "utf8"));
  const rules = manifest.contributes.configurationDefaults["editor.tokenColorCustomizations"]
    .textMateRules as { scope: string | string[] }[];
  const painted = rules.flatMap((r) => (typeof r.scope === "string" ? [r.scope] : r.scope));

  const toks = [
    ...(await tokenizeTmd("Euler: $e^{i\\pi}+1=0$ is nice\n")),
    ...(await tokenizeTmd("$$\\int_0^1 x\\,dx$$\n")),
  ];
  const emitted = new Set(toks.flatMap((t) => t.scopes));
  for (const scope of painted) {
    assert.ok(emitted.has(scope), `package.json paints \`${scope}\`, which the grammar never emits`);
  }
  // And both delimiters really are what carries them, so the rule lands on the `$` rather than
  // on the body the `#math_body` patterns already colour.
  assert.deepStrictEqual(
    toks.filter((t) => t.scopes.includes("punctuation.definition.math.begin.tmd")).map((t) => t.text),
    ["$", "$$"]
  );
});

test("Phase 2 (injection): math-body inner tokens are highlighted natively (no external LaTeX grammar)", async () => {
  // Multi-line so the `% note` comment stays on an interior line (a `%` runs to EOL, so it must
  // not share a line with the closing `$$` or `\end{…}`). Empirically the form that tokenizes
  // cleanly: `$$\begin{aligned}` opens, comment on the middle line, `\end{aligned}$$` closes.
  const src =
    "$$\\begin{aligned}\n" +
    "\\int_0^1 x^2 \\,dx &= \\frac{1}{3} % note\n" +
    "\\end{aligned}$$\n";
  const toks = await tokenizeTmd(src);

  // Commands.
  assert.ok(hasScope(toks, "\\int", "keyword.control.tmd.math"), "\\int is a math command");
  assert.ok(hasScope(toks, "\\frac", "keyword.control.tmd.math"), "\\frac is a math command");
  // Environments.
  assert.ok(hasScope(toks, "\\begin", "keyword.control.tmd.math.environment"), "\\begin is an environment keyword");
  assert.ok(hasScope(toks, "\\end", "keyword.control.tmd.math.environment"), "\\end is an environment keyword");
  assert.ok(hasScope(toks, "aligned", "support.class.tmd.math.environment"), "the environment name is scoped");
  // Operators.
  assert.ok(hasScope(toks, "^", "keyword.operator.tmd.math"), "^ is a math operator");
  assert.ok(hasScope(toks, "_", "keyword.operator.tmd.math"), "_ is a math operator");
  assert.ok(hasScope(toks, "&", "keyword.operator.tmd.math.align"), "& is an alignment operator");
  // Escape, comment, number.
  assert.ok(hasScope(toks, "\\,", "constant.character.escape.tmd.math"), "\\, is an escaped symbol");
  assert.ok(hasScope(toks, "% note", "comment.line.percentage.tmd.math"), "% note is a comment");
  assert.ok(hasScope(toks, "3", "constant.numeric.tmd.math"), "a digit is numeric");
  // The region scope still coexists with the new inner scopes.
  assert.ok(hasScope(toks, "\\int", "meta.embedded.math.tmd"), "region contentName preserved under inner scopes");

  // \% is the load-bearing guard: the escaped-symbol rule must beat the comment rule so
  // a literal percent sign in math is an escape, not the start of a % comment.
  const esc = await tokenizeTmd("$a \\% b$\n");
  assert.ok(hasScope(esc, "\\%", "constant.character.escape.tmd.math"), "\\% is an escaped symbol");
  assert.ok(!hasScope(esc, "\\%", "comment.line"), "\\% is NOT a comment");
});

test("Phase 2 (injection): {{< shortcode >}} name is scoped", async () => {
  const toks = await tokenizeTmd('See {{< include parts.tmd >}} here\n');
  assert.ok(hasScope(toks, "include", "keyword.control.tmd.shortcode"), "the shortcode name is a control keyword");
});

test("Phase 2 (injection): @xref refs scoped; email is NOT a ref", async () => {
  const toks = await tokenizeTmd("see @fig-scree and @sec-intro but not bob@rem-server.com\n");
  assert.ok(hasScope(toks, "@fig-scree", "markup.other.reference.tmd"), "bare @fig- is a reference");
  assert.ok(hasScope(toks, "@sec-intro", "markup.other.reference.tmd"), "bare @sec- is a reference");
  const email = toks.find((t) => t.text.includes("rem-server"));
  assert.ok(
    email && !email.scopes.some((s) => s.startsWith("markup.other.reference")),
    "bob@rem-server.com is NOT tokenized as a reference (word-boundary guard)"
  );
});

// Seven of the twelve prefixes outlive the constructs that could define them — prp/exm/rem
// since 2026-08-03, thm/lem/cor/def since 2026-08-08 with the theorem environments — and they
// stay XREF-ONLY so a dangling @thm-x is still reported as a broken cross-reference instead of
// degrading silently
// to text (cite/render.rs's XREF_LABELS doc comment). 97d8a697 dropped three of them from this
// grammar's regex while 5330fd4a restored them in XREF_LABELS, so the two drifted out of step —
// this pins them back together.
test("Phase 2 (injection): retired-target refs are scoped too (xref-only, no live construct)", async () => {
  const toks = await tokenizeTmd(
    "see @prp-cauchy and @exm-euler and @rem-note but not bob@rem-server.com\n"
  );
  assert.ok(hasScope(toks, "@prp-cauchy", "markup.other.reference.tmd"), "bare @prp- is a reference");
  assert.ok(hasScope(toks, "@exm-euler", "markup.other.reference.tmd"), "bare @exm- is a reference");
  assert.ok(hasScope(toks, "@rem-note", "markup.other.reference.tmd"), "bare @rem- is a reference");
  const email = toks.find((t) => t.text.includes("rem-server"));
  assert.ok(
    email && !email.scopes.some((s) => s.startsWith("markup.other.reference")),
    "bob@rem-server.com is still NOT a reference now that rem- is back in the alternation"
  );
});

test("Phase 2 (injection): [@cite] citation keys scoped", async () => {
  const toks = await tokenizeTmd("as shown [@bishop2006, p. 12] and [@a; @b]\n");
  assert.ok(hasScope(toks, "@bishop2006", "constant.other.citekey.tmd"), "the cite key is scoped");
});

test("Phase 2 (injection): $ and @ inside a code cell are NOT math/refs", async () => {
  const toks = await tokenizeTmd("```{python}\ncost = 5  # $5 and email@rem-x\n```\n");
  assert.ok(!hasScope(toks, "$5", "markup.math"), "a $ inside a python cell is not math");
  assert.ok(!hasScope(toks, "email@rem", "markup.other.reference"), "an @ inside a python cell is not a ref");
});
