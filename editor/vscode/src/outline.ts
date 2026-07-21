// Pure document-outline extraction for `.tmd`: the ATX-heading tree that drives the Outline
// view, breadcrumbs, and sticky scroll. No `vscode` import, so it stays in the fast `node:test`
// loop (mirrors check.ts/hover.ts). Skips headings inside fenced code blocks and the leading
// `---` front-matter block, and strips a trailing `{#id}`/`{.class}` attribute block.

export interface OutlineNode {
  title: string;
  level: number; // 1..6
  startLine: number; // 0-based
  endLine: number; // 0-based, inclusive of the section body
  children: OutlineNode[];
}

interface Flat {
  title: string;
  level: number;
  line: number;
}

// Heading text minus a trailing `{#id}`/`{.class}` attribute block and inline emphasis markers.
function cleanTitle(raw: string): string {
  const noAttr = raw.replace(/\s*\{[^}]*\}\s*$/, "").trim();
  const noEmph = noAttr.replace(/[*_`]/g, "").trim();
  return noEmph || raw.trim();
}

// The ATX headings in reading order, skipping fenced code and a leading front-matter block.
function headings(text: string): Flat[] {
  const lines = text.split("\n");
  const out: Flat[] = [];
  let inFence = false;
  let fence = "";
  let start = 0;
  // Skip a leading `---` front-matter block (its keys are not headings).
  if (lines[0]?.trim() === "---") {
    for (let i = 1; i < lines.length; i++) {
      const t = lines[i].trim();
      if (t === "---" || t === "...") {
        start = i + 1;
        break;
      }
    }
  }
  for (let i = start; i < lines.length; i++) {
    const line = lines[i];
    const fenceOpen = /^\s*(```+|~~~+)/.exec(line);
    if (fenceOpen) {
      const marker = fenceOpen[1][0];
      if (!inFence) {
        inFence = true;
        fence = marker;
      } else if (fence === marker) {
        inFence = false;
      }
      continue;
    }
    if (inFence) continue;
    const m = /^(#{1,6})\s+(.*)$/.exec(line);
    if (m) out.push({ title: cleanTitle(m[2]), level: m[1].length, line: i });
  }
  return out;
}

// Build the nested outline. Each node's `endLine` runs to the line before the next heading at
// the same or a higher level (the last heading runs to EOF), so folding covers the section.
export function outline(text: string): OutlineNode[] {
  const flat = headings(text);
  const lineCount = text.split("\n").length;
  const roots: OutlineNode[] = [];
  const stack: OutlineNode[] = [];
  for (let i = 0; i < flat.length; i++) {
    const h = flat[i];
    let end = lineCount - 1;
    for (let j = i + 1; j < flat.length; j++) {
      if (flat[j].level <= h.level) {
        end = flat[j].line - 1;
        break;
      }
    }
    const node: OutlineNode = {
      title: h.title,
      level: h.level,
      startLine: h.line,
      endLine: end,
      children: [],
    };
    while (stack.length && stack[stack.length - 1].level >= h.level) stack.pop();
    if (stack.length) stack[stack.length - 1].children.push(node);
    else roots.push(node);
    stack.push(node);
  }
  return roots;
}
