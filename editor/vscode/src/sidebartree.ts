// The shape of the three sidebar trees, as pure functions of what the server replied.
//
// Split from `sidebar.ts` for the same reason `pastekind.ts` is split from `insert.ts`: this
// half imports no `vscode`, so `node --test` exercises it with no Extension Host, and the tree
// shape is where the bugs are. The provider that turns these rows into `TreeItem`s lives next
// door.
//
// Everything here is a projection of what Rust already decided, over `taliesin/projectOutline`
// and `taliesin/projectRefs`. There is no second implementation of what a heading is or which
// page owns an anchor.

import * as path from "node:path";

/** One heading, as `taliesin/projectOutline` reports it. */
export interface OutlineHeading {
  line: number;
  level: number;
  text: string;
}

/** One page and its headings, in reading order. */
export interface OutlinePage {
  path: string;
  headings: OutlineHeading[];
}

/** A numbered float (figure, table, equation) and where it is defined. */
export interface OutlineFloat {
  id: string;
  path: string;
  line: number;
  title: string;
  number: string;
}

/** The `taliesin/projectOutline` reply. `null` for a document outside any project. */
export type OutlineReply = {
  root: string;
  pages: OutlinePage[];
  floats: OutlineFloat[];
} | null;

/** One place a cross-reference is written. */
export interface RefUse {
  path: string;
  line: number;
  col: number;
}

/** One cross-reference target and everything pointing at it. */
export interface RefTarget {
  id: string;
  resolved: boolean;
  definedIn: string | null;
  definedLine: number | null;
  uses: RefUse[];
}

/** The `taliesin/projectRefs` reply. `null` for a document outside any project. */
export type RefsReply = { root: string; targets: RefTarget[] } | null;

/**
 * One row in any of the three trees. Deliberately not a `vscode.TreeItem`: keeping the shape
 * plain is what lets the builders below be tested with no editor running, and the provider
 * converts at the boundary.
 */
export interface TreeRow {
  label: string;
  description?: string;
  /** The file a click opens. Absent on a pure grouping row. */
  path?: string;
  /** 0-based line to reveal. */
  line?: number;
  /**
   * Whether a row that HAS children starts shut. Absent means open, which is the right
   * default for a heading inside the chapter you are editing and the wrong one for a whole
   * book: every row used to be `Expanded`, and a twelve-chapter outline opened as several
   * hundred lines. Ignored on a leaf.
   */
  collapsed?: boolean;
  children: TreeRow[];
}

/**
 * The whole book as a tree: one row per page, headings nested by level beneath it.
 *
 * Levels are closed with a stack rather than by requiring `level + 1`, because `# A` followed
 * by `### C` is legal Markdown and common in real documents. A builder that insists on
 * consecutive levels drops C entirely, which is worse than showing it one level shallow.
 */
export function outlineTree(reply: OutlineReply, activePath?: string): TreeRow[] {
  if (!reply) return [];
  return reply.pages.map((page) => {
    const root: TreeRow = {
      label: path.basename(page.path),
      path: page.path,
      line: 0,
      // The page being edited opens; every other page in the book is one row until asked.
      // The headings beneath it are left open, because an outline whose sections are all
      // shut shows a filename and answers nothing.
      collapsed: page.path !== activePath,
      children: [],
    };
    // Each entry is a row still open for children, with the heading level that opened it.
    const stack: { level: number; row: TreeRow }[] = [{ level: 0, row: root }];
    for (const h of page.headings) {
      while (stack.length > 1 && stack[stack.length - 1].level >= h.level) stack.pop();
      const row: TreeRow = { label: h.text, path: page.path, line: h.line, children: [] };
      stack[stack.length - 1].row.children.push(row);
      stack.push({ level: h.level, row });
    }
    return root;
  });
}

/**
 * Cross-references grouped into exactly two rows, Resolved and Dangling.
 *
 * Both rows are always present, even at zero: a view that hides its empty half reads as broken
 * rather than as clean. A target that is *defined and never referenced* stays under Resolved
 * with no children, because that is normal authoring, not a problem. Only a use whose target
 * exists nowhere is dangling.
 */
export function refsTree(reply: RefsReply): TreeRow[] {
  const targets = reply?.targets ?? [];
  const row = (t: RefTarget): TreeRow => ({
    label: t.id,
    description: t.uses.length === 1 ? "1 use" : `${t.uses.length} uses`,
    path: t.definedIn ?? t.uses[0]?.path,
    line: t.definedLine ?? t.uses[0]?.line,
    // The id and its use count are the answer; the list of use sites is the follow-up.
    collapsed: true,
    children: t.uses.map((u) => ({
      label: `${path.basename(u.path)}:${u.line + 1}`,
      path: u.path,
      line: u.line,
      children: [],
    })),
  });
  const resolved = targets.filter((t) => t.resolved).map(row);
  const dangling = targets.filter((t) => !t.resolved).map(row);
  // Dangling is the half that wants doing something about, so it is the half that opens.
  // Resolved is usually the long one and is usually fine, and its count says so already.
  return [
    { label: `Resolved (${resolved.length})`, collapsed: true, children: resolved },
    { label: `Dangling (${dangling.length})`, collapsed: false, children: dangling },
  ];
}

/** The numbered-float index: every figure, table and equation with its number. */
export function floatsTree(reply: OutlineReply): TreeRow[] {
  if (!reply) return [];
  return reply.floats.map((f) => ({
    label: f.id,
    description: f.number || f.title || undefined,
    path: f.path,
    line: f.line,
    children: [],
  }));
}
