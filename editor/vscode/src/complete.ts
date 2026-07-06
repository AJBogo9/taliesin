// Pure completion-context detection + best-effort live-candidate scans. No `vscode` import,
// so it stays in the fast `node:test` loop. The static vocabulary is Rust-authoritative
// (fetched via `taliesin vocab`); this file only decides WHICH list applies and harvests
// document-defined ids / .bib keys, which are suggestion-only (check remains the arbiter).

export type CompletionContext =
  | { kind: "none" }
  | { kind: "frontmatter-key"; parent: string | null }
  | { kind: "cell-option" }
  | { kind: "div-class" }
  | { kind: "xref"; typed: string }
  | { kind: "cite" };

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

  // Front-matter key: inside the `---` block, at a key position (only a partial word so far).
  if (inFrontmatter(docPrefix) && /^\s*[\w-]*$/.test(linePrefix)) {
    return { kind: "frontmatter-key", parent: nestedParent(docPrefix) };
  }

  return { kind: "none" };
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
