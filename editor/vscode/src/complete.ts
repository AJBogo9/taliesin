// Pure completion-context detection + best-effort live-candidate scans. No `vscode` import,
// so it stays in the fast `node:test` loop. The static vocabulary is Rust-authoritative
// (fetched via `taliesin vocab`); this file only decides WHICH list applies and harvests
// document-defined ids / .bib keys, which are suggestion-only (check remains the arbiter).

export type CompletionContext =
  | { kind: "none" }
  | { kind: "frontmatter-key"; parent: string | null }
  | { kind: "frontmatter-value"; key: string; typed: string }
  | { kind: "cell-option" }
  | { kind: "div-class" }
  | { kind: "xref"; typed: string }
  | { kind: "cite" }
  | { kind: "shortcode-path"; shortcode: "embed" | "include"; typed: string };

// Front-matter parents whose immediate children have their own vocabulary.
const NESTED_PARENTS = ["execute", "listing", "about", "hero", "prose-lint", "theorems"];

// Are we inside the leading `---` front-matter block at `docPrefix`'s end?
function inFrontmatter(docPrefix: string): boolean {
  const lines = docPrefix.split("\n");
  if (lines.length === 0 || lines[0].trim() !== "---") return false;
  // Closed if any line AFTER the opener (before the current line) is a lone `---` or `...`.
  for (let i = 1; i < lines.length - 1; i++) {
    const t = lines[i].trim();
    if (t === "---" || t === "...") return false;
  }
  return true;
}

// Count ``` fence lines before the cursor; an odd count means we are inside a code cell.
function inCodeCell(docPrefix: string): boolean {
  const lines = docPrefix.split("\n");
  let fences = 0;
  // Exclude the current (last) line: the `#|` line itself is inside the cell the opener began.
  for (let i = 0; i < lines.length - 1; i++) {
    if (/^\s*```/.test(lines[i])) fences++;
  }
  return fences % 2 === 1;
}

// The nearest less-indented ancestor key (ending in `:`) above an indented current line.
function nestedParent(docPrefix: string): string | null {
  const lines = docPrefix.split("\n");
  const current = lines[lines.length - 1];
  const indent = current.length - current.trimStart().length;
  if (indent === 0) return null;
  for (let i = lines.length - 2; i >= 0; i--) {
    const line = lines[i];
    if (line.trim() === "") continue;
    const lineIndent = line.length - line.trimStart().length;
    if (lineIndent < indent) {
      const m = /^([\w-]+):/.exec(line.trim());
      const key = m ? m[1] : null;
      return key && NESTED_PARENTS.includes(key) ? key : null;
    }
  }
  return null;
}

export function detectContext(linePrefix: string, docPrefix: string): CompletionContext {
  // Shortcode file argument: `{{< embed ` / `{{< include ` then the first (path) token.
  // Only while typing that FIRST token — a space after it moves on to named args (title=…),
  // which are not paths. Checked before the `@`/`:::` rules so a path can't be misread.
  const sc = /\{\{<\s*(embed|include)\s+([^\s>]*)$/.exec(linePrefix);
  if (sc) return { kind: "shortcode-path", shortcode: sc[1] as "embed" | "include", typed: sc[2] };

  // Citation FIRST: `[@` contains `@`, so it must win over the xref rule.
  if (/\[@[^\]]*$/.test(linePrefix)) return { kind: "cite" };

  // Cross-reference: `@` not preceded by a word char (so an email local-part is skipped).
  const xref = /(^|[^\w@])@([\w-]*)$/.exec(linePrefix);
  if (xref) return { kind: "xref", typed: xref[2] };

  // Fenced-div class: `:::{.` or `::: {.` then a partial class name.
  if (/:::\s*\{\.[\w-]*$/.test(linePrefix)) return { kind: "div-class" };

  // Cell option: a `#|` / `//|` / `%%|` directive line, key position, inside a code cell.
  if (/^\s*(#\||\/\/\||%%\|)\s*[\w-]*$/.test(linePrefix) && inCodeCell(docPrefix)) {
    return { kind: "cell-option" };
  }

  // Front-matter, inside the `---` block.
  if (inFrontmatter(docPrefix)) {
    // Value position: `key:` then the value token being typed. Vocab-agnostic — the provider
    // offers values only for the keys that have a closed set (format/theme via `taliesin
    // vocab`), so a valueless key like `author:` simply yields nothing.
    const val = /^\s*([\w-]+):\s*(\S*)$/.exec(linePrefix);
    if (val) return { kind: "frontmatter-value", key: val[1], typed: val[2] };
    // Key position: only a partial word so far (no colon yet).
    if (/^\s*[\w-]*$/.test(linePrefix)) {
      return { kind: "frontmatter-key", parent: nestedParent(docPrefix) };
    }
  }

  return { kind: "none" };
}

// One cross-reference target, as `taliesin symbols --format json` emits it.
export interface XrefSymbol {
  id: string;
  kind: string;
  number: string;
}

// Parse `taliesin symbols --format json`. Never throws: a missing binary, an old binary
// with no `symbols` command, or a `{"error": …}` envelope all degrade to "no symbols",
// and the caller falls back to the buffer scan rather than dropping completions.
export function parseSymbolsJson(stdout: string): XrefSymbol[] {
  const text = stdout.trim();
  if (text === "") return [];
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return [];
  }
  if (!Array.isArray(value)) return [];
  return value
    .filter((s): s is XrefSymbol => !!s && typeof (s as XrefSymbol).id === "string")
    .map((s) => ({
      id: s.id,
      kind: typeof s.kind === "string" ? s.kind : "",
      number: typeof s.number === "string" ? s.number : "",
    }));
}

// Union the two views of a document's cross-reference targets, sorted and deduplicated.
//
// Neither view is sufficient alone. `taliesin symbols` reads the file on DISK and knows
// the whole registry, including the cell labels (`#| label: fig-scree`) that no regex can
// see and the numbers Taliesin resolved. `harvestAnchorIds` reads the LIVE buffer, so it
// alone sees an anchor the author typed a moment ago and has not saved. Merging keeps a
// just-typed anchor completable without giving up the cell-labeled ones.
//
// `labels` maps a kind prefix to its rendered label (`fig` -> `Figure`), from `vocab`.
export function mergeXrefTargets(
  bufferIds: string[],
  symbols: XrefSymbol[],
  labels: Record<string, string>
): { id: string; detail: string }[] {
  const detail = new Map<string, string>();
  for (const id of bufferIds) detail.set(id, "cross-reference target");
  for (const s of symbols) {
    const label = labels[s.kind];
    // An unknown kind means this binary knows a prefix the vocabulary does not; say
    // nothing rather than render "undefined 1".
    detail.set(s.id, label && s.number ? `${label} ${s.number}` : "cross-reference target");
  }
  return [...detail.keys()].sort().map((id) => ({ id, detail: detail.get(id)! }));
}

// Build/vcs dirs never worth offering as a `{{< embed/include >}}` target.
const IGNORE_DIRS = [".git", "target", "node_modules", "_site", "_freeze"];

export interface DirEntry {
  name: string;
  isDir: boolean;
}
export interface PathCandidate {
  value: string;
  detail: string;
}

// Candidates for a `{{< embed/include <path> >}}` file argument: the `.tmd` files and
// descendable subdirs in the directory of `typed`, filtered by its leaf and returned as
// insert-values relative to the document (dirs suffixed `/` so you can keep descending).
// `entries` is that directory's listing (the caller reads it); `fileDetail` labels the
// `.tmd` hits ("deck / page" for embed, "partial" for include). Pure + vscode-free, so it
// stays in the fast `node:test` loop.
export function shortcodePathCandidates(
  entries: DirEntry[],
  typed: string,
  fileDetail: string
): PathCandidate[] {
  const slash = typed.lastIndexOf("/");
  const dirPart = slash >= 0 ? typed.slice(0, slash + 1) : "";
  const leaf = slash >= 0 ? typed.slice(slash + 1) : typed;
  const dirs: PathCandidate[] = [];
  const files: PathCandidate[] = [];
  for (const e of entries) {
    if (!e.name.startsWith(leaf)) continue;
    // Hide dotfiles unless the user is explicitly typing a dot prefix.
    if (e.name.startsWith(".") && !leaf.startsWith(".")) continue;
    if (e.isDir) {
      if (IGNORE_DIRS.includes(e.name)) continue;
      dirs.push({ value: `${dirPart}${e.name}/`, detail: "directory" });
    } else if (e.name.endsWith(".tmd")) {
      files.push({ value: `${dirPart}${e.name}`, detail: fileDetail });
    }
  }
  dirs.sort((a, b) => a.value.localeCompare(b.value));
  files.sort((a, b) => a.value.localeCompare(b.value));
  return [...dirs, ...files];
}

// Harvest `{#id}` anchors (heading ids + figure/table/etc. labels) from the buffer, for
// @xref completion. Suggestion-only; the provider filters by the typed prefix.
export function harvestAnchorIds(docText: string): string[] {
  const ids = new Set<string>();
  const re = /\{#([\w-]+)\}/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(docText)) !== null) ids.add(m[1]);
  return [...ids];
}

// Harvest BibTeX citation keys (`@type{key,`) from a .bib file's text.
export function harvestBibKeys(bibText: string): string[] {
  const keys = new Set<string>();
  const re = /@\w+\s*\{\s*([^,\s}]+)\s*,/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(bibText)) !== null) keys.add(m[1]);
  return [...keys];
}

// Read the front-matter `bibliography:` field (scalar or list) as raw path strings.
export function frontmatterBibPaths(docText: string): string[] {
  const lines = docText.split("\n");
  if (lines[0]?.trim() !== "---") return [];
  const out: string[] = [];
  for (let i = 1; i < lines.length; i++) {
    const t = lines[i].trim();
    if (t === "---" || t === "...") break;
    const scalar = /^bibliography:\s*(.+)$/.exec(lines[i]);
    if (scalar && scalar[1].trim() !== "") {
      out.push(scalar[1].trim().replace(/^["']|["']$/g, ""));
      continue;
    }
    if (/^bibliography:\s*$/.test(lines[i])) {
      // A YAML list follows: subsequent `  - path` lines.
      for (let j = i + 1; j < lines.length; j++) {
        const t2 = lines[j].trim();
        if (t2 === "---" || t2 === "...") break;
        const item = /^\s*-\s*(.+)$/.exec(lines[j]);
        if (!item) break;
        out.push(item[1].trim().replace(/^["']|["']$/g, ""));
      }
    }
  }
  return out;
}
