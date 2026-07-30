// The paste gestures' pure decisions, kept free of `vscode` so they are unit-testable.
//
// The unit suite runs in plain node, where the `vscode` module does not exist, so every module the
// suite covers has to be import-free of it. That is why `paths.ts`, `map.ts` and `ports.ts` are
// shaped this way, and the routing below belongs in the same layer: it is the client's own
// judgement, and the one part of the paste path a Rust test cannot see.
//
// Everything the SERVER decides (the figure shape, the pipe table, the citation key) lives in
// `lsp_insert.rs`. Nothing here knows what a figure looks like.

/** Kinds the server understands. Mirrors `InsertKind` in `lsp_insert.rs`. */
export type InsertKind = "image" | "htmlTable" | "tsvTable" | "bibtex" | "dataset" | "asset";

/** Mirrors `InsertEditResult` in `lsp_insert.rs` (camelCase over the wire). */
export interface InsertEditResult {
  text: string;
  isSnippet: boolean;
  /** A file the client must write before applying `text`, relative to the document. */
  writeFile?: string;
  /** An append to another file, carried in the same undo as the paste. */
  append?: { path: string; text: string };
  /** Why the build will not ship this reference. Set only for `asset`. */
  outside?: string;
}

/** Clipboard image flavours the server can name an extension for (`image_extension`). */
export const IMAGE_MIMES = [
  "image/png",
  "image/jpeg",
  "image/svg+xml",
  "image/webp",
  "image/gif",
] as const;

/**
 * Which FLAVOUR a paste routes to, from the mime types on the clipboard.
 *
 * This answers only the question the mime list can answer. It deliberately does not decide between
 * a URL, a BibTeX entry and a TSV grid: all three arrive as `text/plain` and can only be told
 * apart by reading the text, which the provider does next. An earlier draft folded the URL
 * decision in here using the selection, and it was wrong in a way worth recording: it returned
 * `null` for `text/plain` with no selection, which silently made the BibTeX and TSV routes
 * unreachable in the ordinary case of pasting with nothing selected.
 *
 * `null` means no gesture applies, which lets the plain paste win.
 */
export function classifyPaste(mimes: readonly string[]): "image" | "htmlTable" | "text" | null {
  if (IMAGE_MIMES.some((m) => mimes.includes(m))) return "image";
  // HTML beats the plain-text copy a spreadsheet puts alongside it: it is the flavour that
  // actually says "this is a table".
  if (mimes.includes("text/html")) return "htmlTable";
  if (mimes.includes("text/plain")) return "text";
  return null;
}

/** Whether `text` is a single absolute http(s) URL, so pasting it over a selection makes a link. */
export function isUrl(text: string): boolean {
  const t = text.trim();
  if (t.length === 0 || /\s/.test(t)) return false;
  try {
    const u = new URL(t);
    return u.protocol === "http:" || u.protocol === "https:";
  } catch {
    return false;
  }
}

/** Whether `text` looks like a BibTeX entry, i.e. `@type{key,`. */
export function isBibtex(text: string): boolean {
  return /^\s*@[A-Za-z]+\s*\{/.test(text);
}

/** Whether clipboard HTML actually contains a table, rather than merely being HTML. */
export function hasHtmlTable(html: string): boolean {
  return /<table[\s>]/i.test(html);
}
