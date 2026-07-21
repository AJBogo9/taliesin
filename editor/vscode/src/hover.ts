// Pure hover-target classification + resolution helpers. No `vscode` import, so it stays in
// the fast `node:test` loop (mirrors check.ts/complete.ts). The vscode `HoverProvider` is a
// thin shell over this: classify the token under the cursor, then look up its meaning in the
// same Rust-authoritative data the completion path uses (`taliesin symbols`/`vocab`, the .bib).

// Front-matter parents whose immediate children have their own vocabulary (mirrors complete.ts).
const NESTED_PARENTS = ["execute", "listing", "about", "hero", "prose-lint", "theorems"];

export type HoverTarget =
  | { kind: "none" }
  // `@fig-2` / `@thm-x` etc. `id` is the token after `@`; [start,end) spans `@id` on the line.
  | { kind: "xref"; id: string; start: number; end: number }
  // `[@key]`. `key` is the citation key; [start,end) spans `@key` (inside the brackets).
  | { kind: "cite"; key: string; start: number; end: number }
  // A YAML front-matter key. `parent` is its nested owner (`execute` for `echo:`) or null.
  | { kind: "frontmatter-key"; key: string; parent: string | null; start: number; end: number }
  // `{{< include PATH >}}` / `{{< embed PATH >}}`. `path` is the target; [start,end) spans it.
  | { kind: "include"; path: string; start: number; end: number };

// The [start, end) line range of the front-matter body (the key lines between the fences),
// or null when there is no closed `---` block. Line indices are 0-based over `lines`.
function frontmatterBody(lines: string[]): { start: number; end: number } | null {
  if (lines[0]?.trim() !== "---") return null;
  for (let i = 1; i < lines.length; i++) {
    const t = lines[i].trim();
    if (t === "---" || t === "...") return { start: 1, end: i };
  }
  return null; // unterminated: don't treat anything as front matter
}

// The nearest less-indented ancestor key (a recognized nested parent) above `line`.
function nestedParentOf(lines: string[], line: number, indent: number): string | null {
  if (indent === 0) return null;
  for (let i = line - 1; i >= 0; i--) {
    const raw = lines[i];
    if (raw.trim() === "") continue;
    const lineIndent = raw.length - raw.trimStart().length;
    if (lineIndent < indent) {
      const m = /^([\w-]+):/.exec(raw.trim());
      const key = m ? m[1] : null;
      return key && NESTED_PARENTS.includes(key) ? key : null;
    }
  }
  return null;
}

// Does [s, e) contain `char` (inclusive of the endpoints, so the cursor sitting just after
// the last char of a token still hovers it, matching VS Code's word-range behaviour)?
function covers(s: number, e: number, char: number): boolean {
  return char >= s && char <= e;
}

/**
 * Classify the hover token at (0-based `line`, `char`) in `docText`. Citation wins over
 * xref (a `[@key]` contains an `@`); a front-matter KEY is recognized only inside the
 * `---` block and only when the cursor is on the key token, never its value.
 */
export function classifyHover(docText: string, line: number, char: number): HoverTarget {
  const lines = docText.split("\n");
  const lineText = lines[line] ?? "";

  // Citation `[@key]` first (its `@` must not be read as an xref).
  {
    const re = /\[@([\w:.\-]+)\]/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(lineText)) !== null) {
      const start = m.index + 1; // the `@`
      const end = start + 1 + m[1].length; // `@` + key
      if (covers(start, end, char)) return { kind: "cite", key: m[1], start, end };
    }
  }

  // Cross-reference `@id`, where `@` is not preceded by a word char or `[` (so an email
  // local-part and a `[@cite]` are both excluded).
  {
    const re = /(^|[^\w@[])@([\w-]+)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(lineText)) !== null) {
      const start = m.index + m[1].length; // the `@`
      const end = start + 1 + m[2].length; // `@` + id
      if (covers(start, end, char)) return { kind: "xref", id: m[2], start, end };
    }
  }

  // Include / embed shortcode: `{{< include PATH >}}` — the PATH token (go-to-definition
  // jumps to the file). Classified before front matter so a shortcode line is never misread.
  {
    const re = /\{\{<\s*(?:include|embed)\s+([^\s>]+)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(lineText)) !== null) {
      const start = m.index + m[0].length - m[1].length;
      const end = start + m[1].length;
      if (covers(start, end, char)) return { kind: "include", path: m[1], start, end };
    }
  }

  // Front-matter key: inside the `---` body, on the `key` of a `key:` line.
  const body = frontmatterBody(lines);
  if (body && line >= body.start && line < body.end) {
    const m = /^(\s*)([\w-]+):/.exec(lineText);
    if (m) {
      const start = m[1].length;
      const end = start + m[2].length;
      if (covers(start, end, char)) {
        const parent = nestedParentOf(lines, line, start);
        return { kind: "frontmatter-key", key: m[2], parent, start, end };
      }
    }
  }

  return { kind: "none" };
}

// The raw BibTeX entry (`@type{key, … }`) for `key`, brace-balanced so a `{…}` inside a
// field value doesn't cut it short; null when the key is absent. Reads the .bib text the
// completion path already locates via `frontmatterBibPaths`.
export function bibEntryFor(bibText: string, key: string): string | null {
  const esc = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`@\\w+\\s*\\{\\s*${esc}\\s*,`, "g");
  const m = re.exec(bibText);
  if (!m) return null;
  const entryStart = m.index;
  const braceOpen = bibText.indexOf("{", entryStart);
  if (braceOpen < 0) return null;
  let depth = 0;
  for (let i = braceOpen; i < bibText.length; i++) {
    const c = bibText[i];
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return bibText.slice(entryStart, i + 1).trim();
    }
  }
  return bibText.slice(entryStart).trim(); // unbalanced .bib: give back what we have
}

// The 0-based {line, col} where cross-reference id `id` is DEFINED in this document: the first
// occurrence preceded by `#` (a `{#fig-x}` attribute) or `label:` (a `#| label: fig-x` cell),
// never `@id` (a reference). null when the id isn't defined here (e.g. it lives in another
// file) — the caller then offers no definition rather than jump to a guess.
export function definitionSite(text: string, id: string): { line: number; col: number } | null {
  const esc = id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`(?:#|label:\\s*)(${esc})(?![\\w-])`, "g");
  const m = re.exec(text);
  if (!m) return null;
  const idOffset = m.index + m[0].length - m[1].length;
  const before = text.slice(0, idOffset);
  const line = before.split("\n").length - 1;
  const col = idOffset - (before.lastIndexOf("\n") + 1);
  return { line, col };
}

// The offset of the BibTeX entry `@type{key,` for `key` in `bibText`, or null when absent.
// Sibling of `bibEntryFor`; the caller converts the offset to a document position.
export function bibEntryOffset(bibText: string, key: string): number | null {
  const esc = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`@\\w+\\s*\\{\\s*${esc}\\s*,`, "g");
  const m = re.exec(bibText);
  return m ? m.index : null;
}
