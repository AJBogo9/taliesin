// Where a definition found in a SHADOW document really is. Kept free of `vscode` so the unit
// suite can cover it: `npm test` runs in plain node, where importing `vscode` throws. The
// types are the structural slice of `vscode.Uri`, `vscode.Location` and
// `vscode.LocationLink` this reads, so the real objects pass straight through.
//
// embedded.ts answers go-to-definition inside a code cell by asking the owning language
// against a shadow file whose lines are the `.tmd`'s own, one for one. Nothing that comes back
// may still name a shadow: F12 would take the author into a projection of their document
// instead of the document itself.

/** The part of `vscode.Uri` read here. */
export interface UriLike {
  toString(): string;
}

/** `vscode.Location`'s shape. */
export interface LocationLike<U, R> {
  uri: U;
  range: R;
}

/** `vscode.LocationLink`'s shape. */
export interface LinkLike<U, R> {
  originSelectionRange?: R;
  targetUri: U;
  targetRange: R;
  targetSelectionRange?: R;
}

export type DefinitionLike<U, R> = LocationLike<U, R> | LinkLike<U, R>;

/**
 * `found`, the definitions the owning language gave for a position in `shadow`, moved onto
 * `parent`, the `.tmd` that shadow projects. Returned as links; a location becomes the link
 * that names the same place, which is also what VS Code converts it into.
 *
 * - A target in the shadow becomes the same range in the parent (the lines map 1:1).
 * - A target anywhere else in `shadowDir` (another document's shadow) is dropped: it is never
 *   a file the author wrote, and landing in one is the same failure as landing in the shadow.
 * - Any other target (a numpy source, `lib.dom.d.ts`, an unsaved buffer the author opened)
 *   passes through unchanged.
 */
export function remapDefinitions<U extends UriLike, R>(
  found: readonly DefinitionLike<U, R>[] | null | undefined,
  shadow: UriLike,
  parent: U,
  shadowDir: string
): LinkLike<U, R>[] {
  const shadowKey = shadow.toString();
  const inShadowDir = `${shadowDir.replace(/\/$/, "")}/`;
  const out: LinkLike<U, R>[] = [];
  for (const d of found ?? []) {
    const link = "targetUri" in d ? d : { targetUri: d.uri, targetRange: d.range };
    const target = link.targetUri.toString();
    if (target === shadowKey) {
      out.push({ ...link, targetUri: parent });
    } else if (!target.startsWith(inShadowDir)) {
      out.push(link);
    }
  }
  return out;
}
