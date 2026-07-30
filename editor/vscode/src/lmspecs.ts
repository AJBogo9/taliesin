// Which Taliesin capabilities are offered to VS Code's language-model tool surface.
//
// `taliesin mcp` already exposes six tools; this is the subset safe to hand a model that may
// invoke them without asking. No `vscode` import, so `node --test` can check it against both
// the manifest and `mcp.rs`.

/** One offered tool: the name VS Code knows it by, and the subcommand behind it. */
export interface LmToolSpec {
  name: string;
  /** The `taliesin` subcommand. First element is the subcommand itself. */
  cli: string[];
  /** Whether the tool takes a file or directory path (`vocab` does not). */
  takesPath: boolean;
}

/**
 * The **read-only** four, plus `vocab`.
 *
 * `build` is deliberately excluded even though the MCP server offers it: it writes `_site/`
 * **and executes code cells**. A model reaching for it unprompted is a surprise write and a
 * surprise kernel run, which is not something an editor should do on a hunch. An agent that
 * genuinely wants to build can still be pointed at the MCP server, where the user opted in.
 */
export const LM_TOOLS: LmToolSpec[] = [
  { name: "taliesin_check", cli: ["check"], takesPath: true },
  { name: "taliesin_read", cli: ["read"], takesPath: true },
  { name: "taliesin_symbols", cli: ["symbols"], takesPath: true },
  { name: "taliesin_map", cli: ["map"], takesPath: true },
  { name: "taliesin_vocab", cli: ["vocab"], takesPath: false },
];

/** Subcommands a model must never be handed: they write, execute, or publish. */
export const WITHHELD_SUBCOMMANDS = ["build", "publish", "preview", "serve", "dev", "new", "init"];
