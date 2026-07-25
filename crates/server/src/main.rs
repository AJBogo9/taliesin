//! taliesin — dev server & CLI entry point.
//!
//!   - `taliesin preview <file.tmd> [port]` live preview server (aliases: dev, serve)
//!   - `taliesin build  <file.tmd> [out]`   render a self-contained HTML file
//!   - `taliesin render <file.tmd>`         one-shot full HTML page to stdout
//!   - `taliesin blocks <file.tmd>`         list block ids + sourcepos (debugging)

mod build;
mod build_budget;
mod check;
mod cli;
mod complete;
mod doctor;
mod exec;
mod freeze;
mod headless_js;
mod interactive;
mod interpreter;
mod kernel;
mod log;
mod lsp;
mod lsp_complete;
mod lsp_nav;
mod lsp_outline;
mod lsp_pos;
mod mcp;
mod minify;
mod preview_diag;
mod protocol;
mod publish;
mod query;
mod runtime_dirs;
mod serve;
mod serve_site;
#[cfg(test)]
mod testutil;
mod warm_pool;
mod zip;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    // `taliesin <cmd> --help` (or `-h`): print that subcommand's focused help and
    // succeed, before the command's own arg parsing (which would otherwise treat
    // `--help` as an unknown flag + then error on the missing positional). Only fires
    // when the first token is a real subcommand with a dedicated help page.
    if let Some(cmd) = args.get(1).map(String::as_str)
        && args[2..].iter().any(|a| a == "--help" || a == "-h")
        && let Some(help) = subcommand_help(cmd)
    {
        print!("{help}");
        return ExitCode::SUCCESS;
    }
    match args.get(1).map(String::as_str) {
        Some("render") => query::cmd_render(args.get(2)),
        Some("read") => query::cmd_read(&args),
        Some("build") => {
            runtime_dirs::sweep_stale_runtime_dirs();
            build::cmd_build(&args)
        }
        Some("publish") => publish::cmd_publish(&args),
        Some("blocks") => query::cmd_blocks(args.get(2)),
        Some("schema") => query::cmd_schema(&args),
        Some("vocab") => query::cmd_vocab(),
        Some("symbols") => query::cmd_symbols(&args),
        Some("map") => query::cmd_map(&args),
        Some("check") => check::cmd_check(&args),
        Some("doctor") => doctor::cmd_doctor(&args),
        Some("mcp") => mcp::cmd_mcp(&args),
        Some("lsp") => lsp::cmd_lsp(&args),
        Some("init") => cli::cmd_init(&args),
        Some("new") => cli::cmd_new(&args),
        // `preview`/`dev` are vite-style aliases for the live server.
        Some("serve" | "preview" | "dev") => {
            runtime_dirs::sweep_stale_runtime_dirs();
            cli::cmd_serve(&args)
        }
        Some("completions") => complete::cmd_completions(&args),
        // Hidden: the shell-completion shims call this at runtime. Not in COMMANDS
        // (underscore-prefixed => excluded from did-you-mean + the dispatch guard).
        Some("__complete") => complete::cmd_complete(&args),
        Some("--version" | "-V") => {
            println!(
                "taliesin {} ({})",
                taliesin_core::VERSION,
                env!("TALIESIN_GIT_SHA")
            );
            ExitCode::SUCCESS
        }
        // `taliesin help <cmd>` is the same request as `taliesin <cmd> --help`, which the
        // intercept above already serves. Without this arm the `help` verb matched first
        // and printed top-level usage, silently ignoring the subcommand.
        Some("help") if args.get(2).and_then(|c| subcommand_help(c)).is_some() => {
            print!("{}", subcommand_help(&args[2]).unwrap());
            ExitCode::SUCCESS
        }
        // No command, or an explicit help request: print usage and succeed.
        Some("--help" | "-h" | "help") | None => {
            usage();
            ExitCode::SUCCESS
        }
        // An unrecognized command is an error (non-zero), not a silent success.
        // Suggest the nearest valid command (reusing core's Levenshtein helper).
        Some(other) => {
            let hint = match taliesin_core::closest(other, COMMANDS) {
                Some(c) => format!("unknown command: `{other}` (did you mean `{c}`?)"),
                None => format!("unknown command: `{other}`"),
            };
            log::error(&hint);
            usage();
            ExitCode::FAILURE
        }
    }
}

/// Every subcommand name (aliases included), for the unknown-command did-you-mean.
const COMMANDS: &[&str] = &[
    "render",
    "read",
    "build",
    "blocks",
    "schema",
    "vocab",
    "symbols",
    "check",
    "doctor",
    "map",
    "mcp",
    "lsp",
    "init",
    "new",
    "serve",
    "preview",
    "dev",
    "publish",
    "help",
    "completions",
];

/// The `ENV:` block of `usage()`. A const so `env_help_lists_every_runtime_env_var` can
/// diff it against the variables the code actually reads: `TALIESIN_MERMAID_URL` shipped
/// user-facing but undocumented because nothing tied the two together.
const ENV_HELP: &str = "\
ENV: TALIESIN_PYTHON (python kernel), TALIESIN_R (r kernel),
     TALIESIN_CELL_TIMEOUT (per-cell seconds; 0 disables),
     TALIESIN_RENDER_TIMEOUT (per-render seconds; default 30, 0 disables),
     TALIESIN_JS_TIMEOUT (read --run {js} headless-Chrome settle seconds; default 10),
     TALIESIN_OPEN (=--open), TALIESIN_HOST (=--host), TALIESIN_NO_CLEAR,
     TALIESIN_NO_CACHE (skip the _freeze/ execution cache),
     TALIESIN_NO_EXEC (=--no-exec, never run code cells),
     TALIESIN_MERMAID_URL (override the url the live preview lazy-loads mermaid from)
";

fn usage() {
    println!(
        "taliesin {} ({})",
        taliesin_core::VERSION,
        env!("TALIESIN_GIT_SHA")
    );
    println!("A fast .tmd -> HTML renderer and live preview server.");
    println!("Docs: https://github.com/AJBogo9/taliesin");
    println!();
    println!("USAGE:");
    println!("  taliesin <command> <file.tmd | dir> [args]");
    println!("  (a directory argument is a multi-page SITE project: an _site.yml + .tmd pages)");
    println!();
    // Grouped by purpose (git/cargo/gh style; clig.dev): the everyday three sit apart from the
    // ten an author rarely types. Flush-left section headers keep each command line unindented.
    println!("COMMANDS:");
    println!();
    println!("Author");
    println!("  init   [dir]               scaffold a starter site you can preview right away");
    println!("                             (writes _site.yml + index.tmd; default: current dir)");
    println!("  new <post|page|deck|paper> <slug> [--dir <root>] [--draft] [--tour] [--json]");
    println!("                             scaffold one document, correct on its first save");
    println!();
    println!("Preview & build");
    println!("  preview <file.tmd | dir> [port] [--host] [--open] [--no-exec]");
    println!("                             live preview server (aliases: dev, serve;");
    println!("                             a dir previews the whole SITE with nav + hot reload;");
    println!("                             default port 4321, replacing this project's own");
    println!("                             running preview and stepping past anyone else's;");
    println!("                             --host exposes it on your LAN with a QR code");
    println!("                             to open on a phone; --open launches a browser;");
    println!("                             --no-exec previews untrusted docs as source,");
    println!("                             never running their code cells)");
    println!(
        "  build  <file.tmd | dir> [out.html] [--out <dir>] [--strict] [--bare] [--jobs <N>] [--format json]"
    );
    println!("                             render a self-contained HTML file (a dir builds the");
    println!("                             whole SITE to _site/); default <name>.html beside");
    println!("                             the source; --out <dir> writes a portable folder;");
    println!("                             --strict exits non-zero on a cell error or located");
    println!("                             warning; --bare emits zero-JS, CSS-only single-doc");
    println!(
        "                             HTML; --jobs <N> caps parallel page renders (site build)"
    );
    println!(
        "  publish <dir> [--project-name <name>] [--out <dir>] [--public] [--no-strict] [--dry-run] [--format json]"
    );
    println!("                             build a site/book + deploy it to Cloudflare Pages");
    println!("                             behind a shared passcode (strict by default);");
    println!("                             --public deploys un-gated; --dry-run skips the deploy");
    println!();
    println!("Inspect");
    println!(
        "  check <file|dir> [--format human|json] [--errors-only|--strict] [--require-kernel] [--explain <CODE>]"
    );
    println!("                             list located diagnostics; exits non-zero if any");
    println!(
        "                             (--explain <CODE> prints a diagnostic code's cause + fix)"
    );
    println!("  doctor [dir] [--format human|json]  audit the environment for running code cells");
    println!("                             (interpreters, ipykernel/IRkernel, active conda/venv)");
    println!("  map   <dir> [--format human|json]  whole-project outline: pages, nav, xref graph");
    println!("  read   <file.tmd> [--run]  project the document to plain text (agent-readable;");
    println!(
        "                             --run executes cells + reports produced figures/output)"
    );
    println!("  render <file.tmd>          render a full HTML page to stdout");
    println!("                             (static; does NOT execute code cells)");
    println!("  blocks <file.tmd>          list block ids + sourcepos (debug)");
    println!("  symbols <file.tmd> [--format human|json]  list the doc's cross-reference targets");
    println!();
    println!("Editor & agent");
    println!(
        "  schema [--out <dir>]       emit JSON Schemas for _site.yml + front matter (editor autocomplete)"
    );
    println!(
        "  vocab                      emit editor autocomplete vocabulary as JSON (companion)"
    );
    println!(
        "  mcp                        stdio MCP server (check/read/symbols/map/vocab/build tools)"
    );
    println!("  lsp                        stdio LSP server: live .tmd diagnostics in any editor");
    println!("  completions <shell> [--install]  print (or --install) a shell completion script");
    println!(
        "                             (subcommand + flag + .tmd-aware path completion; --install writes it for you)"
    );
    println!();
    println!("  help, --version            show this help / the version");
    println!();
    print!("{ENV_HELP}");
}

/// Focused help for one subcommand (synopsis + its flags + a one-line example), or
/// `None` for a name with no dedicated page (the caller falls back to `usage()`).
/// Aliases (`dev`/`serve`) resolve to their canonical command's help. Kept as a flat
/// match over the canonical name to mirror the hand-rolled `usage()` style; printed by
/// `main()` when `--help`/`-h` follows a known subcommand.
fn subcommand_help(cmd: &str) -> Option<&'static str> {
    let text = match cmd {
        "preview" | "dev" | "serve" => {
            "taliesin preview <file.tmd | dir> [port] [--host] [--open] [--no-exec]\n\
             \n\
             Live preview server (aliases: dev, serve). A file previews one document; a\n\
             directory previews the whole SITE with cross-page nav + per-page hot reload.\n\
             Default port 4321. Re-previewing a project replaces its own running\n\
             preview, so there is only ever one; a port held by anything else falls\n\
             back to the next free one.\n\
             \n\
             Flags:\n\
             \x20 --host      bind your LAN + print a QR code for phones (token-gated)\n\
             \x20 --open      launch the default browser at the preview URL\n\
             \x20 --no-exec   render code cells as source, never executing them\n\
             \n\
             Example:\n\
             \x20 taliesin preview index.tmd --open\n"
        }
        "build" => {
            "taliesin build <file.tmd | dir> [out.html] [--out <dir>] [--strict] [--bare] [--jobs <N>] [--format json]\n\
             \n\
             Render a self-contained HTML file. A directory builds the whole SITE to\n\
             _site/. Default output is <name>.html beside the source.\n\
             \n\
             Flags:\n\
             \x20 --out <dir>  write a portable folder (<dir>/index.html + copied assets)\n\
             \x20 --strict     exit non-zero on a cell error or located warning (CI gate)\n\
             \x20 --bare       single-doc only: zero-<script>, CSS-only-theme HTML\n\
             \x20 --jobs <N>   max parallel pages (default: auto, memory- and core-capped;\n\
             \x20              --jobs 1 forces sequential; --jobs 0 same as auto)\n\
             \x20 --format json  emit {diagnostics:[…]} to stdout (agent/CI) instead of only the log\n\
             \n\
             Example:\n\
             \x20 taliesin build post.tmd --strict\n\
             \x20 taliesin build . --jobs 4\n"
        }
        "check" => {
            "taliesin check <file.tmd | dir> [--format human|json] [--errors-only|--strict]\n\
             \x20                            [--require-kernel] [--stdin] [--explain <CODE>]\n\
             \n\
             Render in memory and list every located diagnostic; exits non-zero if any\n\
             ERROR or WARNING is found (a CI / pre-publish gate). A SUGGESTION is advice:\n\
             it is printed and never fails the run unless you ask with --strict. Does NOT\n\
             execute code cells.\n\
             \n\
             Flags:\n\
             \x20 --format human   path:line: message lines to stderr (default)\n\
             \x20 --format json    {diagnostics:[{code,docs_url,severity,file,line,message,\n\
             \x20                     suggestion?}], environment:[...]} object to stdout (jq)\n\
             \x20 --errors-only    report + gate on errors only; warnings no longer fail\n\
             \x20 --strict         also fail on suggestions (the strictest gate)\n\
             \x20 --require-kernel also fail if a used language's Jupyter kernel isn't ready\n\
             \x20                  (interpreter + ipykernel/IRkernel); off by default\n\
             \x20 --stdin          lint the buffer piped on stdin as if it were <file.tmd>,\n\
             \x20                  not the last-saved file (unsaved edits; the editor on-type\n\
             \x20                  path). The path gives the base dir + reported location;\n\
             \x20                  the interpreter probe is skipped (environment: []).\n\
             \x20 --explain <CODE> expand a diagnostic code (e.g. TAL-XREF-UNREF) into its\n\
             \x20                  cause + canonical fix, rustc-style; bare lists every code.\n\
             \x20                  Honours --format json. Needs no file.\n\
             \n\
             Example:\n\
             \x20 taliesin check . --format json | jq\n\
             \x20 taliesin check src/ --errors-only --require-kernel\n\
             \x20 taliesin check post.tmd --stdin --format json < buffer.tmd\n\
             \x20 taliesin check --explain TAL-FM-KEY\n"
        }
        "render" => {
            "taliesin render <file.tmd>\n\
             \n\
             Render a full HTML page to stdout (one-shot). Static: it does NOT execute\n\
             code cells, so kernel cells emit as source with empty outputs. Use build or\n\
             preview to run them.\n\
             \n\
             Example:\n\
             \x20 taliesin render post.tmd > post.html\n"
        }
        "mcp" => {
            "taliesin mcp\n\
             \n\
             Run a local, offline stdio MCP (Model Context Protocol) server so an MCP host\n\
             drives Taliesin's read/validate/build loop without shelling out. Exposes six\n\
             tools — check, read, symbols, map, vocab, build — and NO write/edit/preview\n\
             tool: the .tmd stays your direct edit surface. JSON-RPC on stdout, logs on\n\
             stderr.\n\
             \n\
             NOT a sandbox, so do not allowlist it as one. That edit-surface guarantee is\n\
             about the .tmd, not containment: there is no project root, so every tool reads\n\
             any path you hand it (including outside the project), and build writes HTML\n\
             beside that path and executes the document's code cells. Contain it with the\n\
             host's own sandbox and working directory.\n\
             \n\
             Example (in an MCP host's config):\n\
             \x20 { \"command\": \"taliesin\", \"args\": [\"mcp\"] }\n"
        }
        "lsp" => {
            "taliesin lsp\n\
             \n\
             Run a local, offline LSP (Language Server Protocol) server over stdio so any\n\
             LSP editor (Neovim, Helix, Zed, VS Code) gets live .tmd diagnostics as you\n\
             type — the same validators as `check`, on the unsaved buffer. Parse-only: no\n\
             kernel, no code execution, read-only (it never edits your source). JSON-RPC on\n\
             stdout, logs on stderr.\n\
             \n\
             Example (Neovim, via nvim-lspconfig or vim.lsp.start):\n\
             \x20 cmd = { \"taliesin\", \"lsp\" }\n"
        }
        "map" => {
            "taliesin map <dir> [--format human|json]\n\
             \n\
             The whole-project outline in one read-only call: the page list in nav /\n\
             chapter order (rel, url, title, date, categories, layout), nav + mounts, the\n\
             cross-reference graph (each anchor → where it's defined + its backlinks), and\n\
             embedded decks. Reuses site discovery; no kernel, no code execution.\n\
             \n\
             Example:\n\
             \x20 taliesin map . --format json | jq '.pages[].url'\n"
        }
        "read" => {
            "taliesin read <file.tmd>\n\
             \n\
             Project the rendered document to structured plain text (headings, resolved\n\
             \"Figure N\"/cross-reference numbers, callouts, fenced code, math as raw TeX),\n\
             so an agent can read what it made with no browser and no HTML. A VIEW, not an\n\
             output format. Static: like render it does NOT execute code cells.\n\
             \n\
             Example:\n\
             \x20 taliesin read post.tmd\n"
        }
        "schema" => {
            "taliesin schema [--out <dir>]\n\
             \n\
             Emit the bundled JSON Schemas for taliesin's YAML config (document front\n\
             matter + _site.yml) so an editor's YAML language server can validate them.\n\
             Prints both to stdout, or writes two files with --out <dir>.\n\
             \n\
             Example:\n\
             \x20 taliesin schema --out .schemas\n"
        }
        "vocab" => {
            "taliesin vocab\n\
             \n\
             Emit taliesin's editor vocabulary (front-matter keys, cell options, callout\n\
             and theorem kinds, div classes, cross-reference prefixes) as one JSON blob,\n\
             for the VS Code companion's autocomplete. Generated from the validator's own\n\
             lists, so it never drifts from what `check` enforces.\n\
             \n\
             Example:\n\
             \x20 taliesin vocab | jq .cellOptions\n"
        }
        "new" => {
            "taliesin new [post|page|deck|paper] [slug] [--dir <root>] [--draft] [--tour] [--json] [-y]\n\
             \n\
             Scaffold one document that is correct on its first save: it renders, and\n\
             `taliesin check` passes on it with no diagnostics. A post lands in\n\
             posts/<slug>/index.tmd and is dated today; a page and a deck land in\n\
             <slug>.tmd; a paper lands in posts/<slug>/ with a ready-to-cite\n\
             references.bib beside it. Refuses to overwrite an existing file.\n\
             \n\
             Run at a terminal with the kind or slug omitted and it prompts for them\n\
             (arrow keys to pick the kind); pass -y to never prompt.\n\
             \n\
             Flags:\n\
             \x20 --dir <root>   scaffold under <root> instead of the current directory\n\
             \x20 --draft        mark the scaffold `draft: true`, held out of the published build\n\
             \x20 --tour         (deck only) scaffold a guided deck: one slide per feature, explained\n\
             \x20 --json         print a {kind, slug, created, preview} receipt (agent-friendly)\n\
             \x20 -y, --yes      skip the interactive prompt (for scripts run at a terminal)\n\
             \n\
             Example:\n\
             \x20 taliesin new post my-first-post --draft\n"
        }
        "symbols" => {
            "taliesin symbols <file.tmd> [--format human|json]\n\
             \n\
             List the document's cross-reference targets: every anchor you can name after\n\
             `@`, whether it was written as a brace anchor (`{#sec-why}`) or as a cell\n\
             label (`#| label: fig-scree`), with the number Taliesin resolved for it.\n\
             Static: renders in memory and never runs a code cell, so an editor can call\n\
             it while you type.\n\
             \n\
             Flags:\n\
             \x20 --format human|json   json feeds the companion's @-completion\n\
             \n\
             Example:\n\
             \x20 taliesin symbols post.tmd --format json | jq '.[].id'\n"
        }
        "blocks" => {
            "taliesin blocks <file.tmd>\n\
             \n\
             List the document's block ids + sourcepos + source file + a short preview\n\
             (a debugging aid for the block model). Does NOT execute code cells.\n\
             \n\
             Example:\n\
             \x20 taliesin blocks post.tmd\n"
        }
        "init" => {
            "taliesin init [dir] [--template basic|site|book] [-y]\n\
             \n\
             Scaffold a starter project into dir (default the current directory) and print\n\
             the preview hint. Refuses to overwrite existing files.\n\
             \n\
             Templates:\n\
             \x20 basic   a one-page site (the default): _site.yml + index.tmd\n\
             \x20 site    a multi-page site: a nav linking a Home and an About page\n\
             \x20 book    a chapters: project: a landing page + two starter chapters\n\
             \n\
             Every template also writes AGENTS.md (the agent onramp) and the .taliesin/\n\
             config schemas that drive editor autocomplete.\n\
             \n\
             Run at a terminal with no --template and it prompts for one (arrow keys);\n\
             pass -y to take the basic default without prompting.\n\
             \n\
             Example:\n\
             \x20 taliesin init my-book --template book\n"
        }
        "publish" => {
            "taliesin publish <dir> [--project-name <name>] [--out <dir>] [--public] [--no-strict] [--dry-run] [--format json]\n\
             \n\
             Build a site or book and deploy it to Cloudflare Pages (Wrangler direct\n\
             upload). Strict by default (a cell error or broken ref fails the deploy) and\n\
             gated behind a shared passcode by default. One-way: it never writes to your\n\
             source. The passcode lives only as a Cloudflare secret, never in your repo.\n\
             \n\
             Flags:\n\
             \x20 --project-name <name>  Cloudflare Pages project (default: the dir-name slug)\n\
             \x20 --out <dir>            build output dir (default: the project's _site/_book)\n\
             \x20 --public               deploy a public, un-gated site (default: passcode-gated;\n\
             \x20                        also settable as publish.gate: false in _site.yml)\n\
             \x20 --no-strict            deploy even if the build has warnings (default: strict)\n\
             \x20 --dry-run              build + gate, print the deploy command, do not deploy\n\
             \x20 --format json         emit {diagnostics:[…]} from the build to stdout (agent/CI)\n\
             \n\
             One-time setup (per repo):\n\
             \x20 export CLOUDFLARE_API_TOKEN=...   (also CLOUDFLARE_ACCOUNT_ID)\n\
             \x20 wrangler pages project create <name> --production-branch production\n\
             \x20 wrangler pages secret put PASSWORD --project-name <name>\n\
             \n\
             Example:\n\
             \x20 taliesin publish . --dry-run\n"
        }
        "completions" => {
            "taliesin completions <bash|zsh|fish|powershell> [--install]\n\
             \n\
             Print a shell completion script to stdout. The script is a thin shim that\n\
             asks the running binary for candidates, so Tab offers subcommands, flags, and\n\
             only .tmd files plus directories that contain one (site/book roots first).\n\
             \n\
             --install writes the script into your shell's completion dir instead (the\n\
             shell is detected from $SHELL when omitted); completion works after a restart:\n\
             \x20 taliesin completions --install         # detect $SHELL and install\n\
             \x20 taliesin completions zsh --install     # install for a named shell\n\
             \n\
             Or install by hand:\n\
             \x20 bash        taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin\n\
             \x20 zsh         taliesin completions zsh  > \"${fpath[1]}/_taliesin\"   # then: compinit\n\
             \x20 fish        taliesin completions fish > ~/.config/fish/completions/taliesin.fish\n\
             \x20 powershell  taliesin completions powershell >> $PROFILE\n\
             \n\
             Example:\n\
             \x20 taliesin completions zsh\n"
        }
        "doctor" => {
            "taliesin doctor [dir] [--format human|json]\n\
             \n\
             Audit whether the environment can run code cells: the Python and R interpreter\n\
             (resolved as a build would: _site.yml python:/r:, a .venv, TALIESIN_PYTHON/R, then\n\
             the PATH default), whether ipykernel/IRkernel import, the active conda/virtualenv,\n\
             and _site.yml validity. Prints a status line per item with a fix command; exits\n\
             non-zero only if a configured interpreter is broken.\n\
             \n\
             Example:\n\
             \x20 taliesin doctor\n\
             \x20 taliesin doctor myproject --format json\n"
        }
        _ => return None,
    };
    Some(text)
}

/// The one-line synopsis for `cmd` — the first line of [`subcommand_help`] (`taliesin <cmd> …`),
/// the single source of truth a subcommand's missing-positional `usage:` error derives from, so
/// it can't drift from the `--help` block. `None` for a command with no focused help.
pub(crate) fn command_synopsis(cmd: &str) -> Option<&'static str> {
    subcommand_help(cmd).and_then(|h| h.lines().next())
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;

    #[test]
    fn closest_command_suggests_nearest() {
        // A near-miss typo resolves to the intended command.
        assert_eq!(taliesin_core::closest("biuld", COMMANDS), Some("build"));
        assert_eq!(taliesin_core::closest("previw", COMMANDS), Some("preview"));
        assert_eq!(taliesin_core::closest("innit", COMMANDS), Some("init"));
        // Something far from every command yields no suggestion (not a wild guess).
        assert_eq!(taliesin_core::closest("frobnicate", COMMANDS), None);
    }
}

#[cfg(test)]
mod cli_microcopy_tests {
    use super::*;

    /// Every `TALIESIN_*` variable the code reads at runtime is documented in the `ENV:`
    /// block, and every documented one is really read. `TALIESIN_MERMAID_URL` was
    /// user-facing but absent from `usage()`; nothing connected the two.
    #[test]
    fn env_help_lists_every_runtime_env_var() {
        // Not runtime knobs, so not the CLI's business to document: `TALIESIN_GIT_SHA` is
        // stamped by `build.rs` and read with `env!`; `TALIESIN_BLESS` (snapshot blessing)
        // and `TALIESIN_REQUIRE_KERNEL` (the CI kernel job) are read only under
        // `#[cfg(test)]`. Everything else a user can set must appear in the ENV block.
        const NOT_RUNTIME_KNOBS: &[&str] = &[
            "TALIESIN_GIT_SHA",
            "TALIESIN_BLESS",
            "TALIESIN_REQUIRE_KERNEL",
            // Like TALIESIN_REQUIRE_KERNEL: a CI-only gate that turns a "tool missing, so
            // skip" into a hard failure (here, Node for the JS-equivalence guard). Not a
            // knob a user of the binary ever sets.
            "TALIESIN_REQUIRE_NODE",
        ];

        fn walk(dir: &std::path::Path, out: &mut std::collections::BTreeSet<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    // Only real lookups, never prose: `var("…")`, `var_os("…")`,
                    // `set_var("…", …)`. Stop at the identifier boundary, since some
                    // literals open with the name and continue into a message.
                    for call in ["var(\"", "var_os(\"", "set_var(\""] {
                        for (i, _) in src.match_indices(call) {
                            let rest = &src[i + call.len()..];
                            if !rest.starts_with("TALIESIN_") {
                                continue;
                            }
                            out.insert(
                                rest.chars()
                                    .take_while(|c| {
                                        c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_'
                                    })
                                    .collect(),
                            );
                        }
                    }
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let mut read = std::collections::BTreeSet::new();
        walk(&root.join("server/src"), &mut read);
        walk(&root.join("core/src"), &mut read);
        read.retain(|v| !NOT_RUNTIME_KNOBS.contains(&v.as_str()));

        for v in &read {
            assert!(
                ENV_HELP.contains(v.as_str()),
                "`{v}` is read at runtime but missing from usage()'s ENV block"
            );
        }
        for (i, _) in ENV_HELP.match_indices("TALIESIN_") {
            let name: String = ENV_HELP[i..]
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || *c == '_')
                .collect();
            assert!(
                read.contains(&name),
                "usage() documents `{name}`, which nothing reads"
            );
        }
    }

    /// `taliesin help <cmd>` is the same request as `taliesin <cmd> --help`. It used to
    /// print top-level usage, because the `help` verb matched before the subcommand was
    /// looked at.
    #[test]
    fn help_verb_with_a_subcommand_resolves_to_that_subcommands_help() {
        for cmd in ["build", "preview", "check"] {
            assert!(
                subcommand_help(cmd).is_some(),
                "`help {cmd}` needs a focused page to resolve to"
            );
        }
        // A bare `help`, or `help <unknown>`, still falls back to top-level usage.
        assert!(subcommand_help("frobnicate").is_none());
    }

    /// The source of `main()`'s dispatch `match`, sliced out of this very file. Panics
    /// rather than returning an empty region, so a rename of either marker fails loudly
    /// instead of turning the gate below into a vacuous pass.
    fn dispatch_region(src: &str) -> &str {
        const START: &str = "match args.get(1).map(String::as_str) {";
        const END: &str = "/// Every subcommand name";
        let s = src.find(START).expect("main() dispatches on args.get(1)");
        let e = src[s..]
            .find(END)
            .expect("the COMMANDS const follows main()")
            + s;
        &src[s..e]
    }

    /// Every command name a dispatch region matches on, from the string literals inside
    /// each `Some(…)` pattern.
    ///
    /// Deliberately NOT line-based. rustfmt wraps a long or-pattern onto its own lines,
    /// which splits `Some(` from its `=>`, and a line-based scan then silently collects
    /// nothing: a gate that cannot fail is worse than no gate. Reading `Some(` up to the
    /// `)` that closes it survives any wrapping, and stopping at that `)` keeps a string
    /// inside a match guard from being mistaken for a command.
    fn commands_in_dispatch(region: &str) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        let mut rest = region;
        while let Some(i) = rest.find("Some(") {
            rest = &rest[i + "Some(".len()..];
            let Some(end) = rest.find(')') else { break };
            // Flags (`--version`, `-h`) and hidden internal subcommands (`__complete`,
            // underscore-prefixed) are not user-facing commands: never suggested, never in
            // `COMMANDS`. A binding pattern (`Some(other)`) has no literal.
            for lit in rest[..end].split('"').skip(1).step_by(2) {
                if !lit.starts_with('-') && !lit.starts_with('_') {
                    out.insert(lit.to_string());
                }
            }
            rest = &rest[end..];
        }
        out
    }

    #[test]
    fn the_dispatch_scan_survives_rustfmt_wrapping_and_guards() {
        let set = |names: &[&str]| -> std::collections::BTreeSet<String> {
            names.iter().map(|s| s.to_string()).collect()
        };
        // A long or-pattern, wrapped: `Some(` and `=>` land on different lines.
        assert_eq!(
            commands_in_dispatch("Some(\n    \"alpha\" | \"beta\",\n) => cmd(),"),
            set(&["alpha", "beta"])
        );
        // A long guard, wrapped off the pattern's line.
        assert_eq!(
            commands_in_dispatch("Some(\"gamma\")\n    if x.is_some() =>\n{ }"),
            set(&["gamma"])
        );
        // A string *inside* a guard is not a command.
        assert_eq!(
            commands_in_dispatch("Some(\"help\") if a.map(|s| s == \"delta\").is_some() => u(),"),
            set(&["help"])
        );
        // Flags and binding patterns contribute nothing.
        assert!(commands_in_dispatch("Some(\"--version\" | \"-V\") => v(),").is_empty());
        assert!(commands_in_dispatch("Some(other) => fail(other),").is_empty());
        // Hidden underscore-prefixed subcommands are excluded (like flags).
        assert!(commands_in_dispatch("Some(\"__complete\") => c(),").is_empty());
    }

    /// Every name `main()` dispatches on is in `COMMANDS`, and vice versa. `COMMANDS` is
    /// what the unknown-command did-you-mean searches, so a subcommand missing from it is
    /// invisible: `taliesin symbol` would suggest nothing instead of `symbols`. Nothing
    /// tied the two together, exactly as nothing tied `usage()`'s `ENV:` block to the
    /// variables the code reads (see `env_help_lists_every_runtime_env_var`).
    #[test]
    fn every_dispatched_command_is_listed_in_commands() {
        let dispatched = commands_in_dispatch(dispatch_region(include_str!("main.rs")));
        let listed: std::collections::BTreeSet<String> =
            COMMANDS.iter().map(|c| c.to_string()).collect();
        assert_eq!(
            dispatched, listed,
            "the dispatch and COMMANDS disagree (left: dispatched, right: COMMANDS)"
        );
    }

    /// Names in `COMMANDS` that are an alias of another command, or not a command at all.
    /// `subcommand_help_covers_documented_commands` skips them: `dev`/`serve` resolve to
    /// `preview`'s help, and `help` is the help system rather than an entry in it.
    const ALIASES_AND_META: &[&str] = &["dev", "serve", "help"];

    /// Each covered subcommand has a focused help that names itself and shows an
    /// example; an unknown command has none.
    #[test]
    fn subcommand_help_covers_documented_commands() {
        // Driven by COMMANDS, so a new subcommand cannot ship without focused help.
        for cmd in COMMANDS.iter().filter(|c| !ALIASES_AND_META.contains(c)) {
            let help = subcommand_help(cmd).unwrap_or_else(|| panic!("help for `{cmd}`"));
            assert!(
                help.contains(cmd),
                "`{cmd}` help should name the subcommand: {help}"
            );
            assert!(
                help.contains("taliesin"),
                "`{cmd}` help should show a `taliesin …` example: {help}"
            );
        }
        // Aliases resolve to the canonical (preview) help.
        assert_eq!(subcommand_help("dev"), subcommand_help("preview"));
        assert_eq!(subcommand_help("serve"), subcommand_help("preview"));
        // An unrecognized command has no focused help (falls back to top-level usage).
        assert!(subcommand_help("frobnicate").is_none());
        // `--jobs` is documented in build help.
        let build_help = subcommand_help("build").unwrap();
        assert!(
            build_help.contains("--jobs"),
            "build help must document --jobs: {build_help}"
        );
        // PL15: `new` help documents `--draft`/`--tour` (the scaffold advertises `--draft`).
        let new_help = subcommand_help("new").unwrap();
        assert!(
            new_help.contains("--draft") && new_help.contains("--tour"),
            "new help must document --draft + --tour: {new_help}"
        );
        // PL15: the missing-positional `usage:` one-liners derive from the `--help` synopsis, so
        // they can't drift — the derived `new` synopsis carries the new flags, and the `build`
        // synopsis carries `--format json` (the flag its old hand-written one-liner had dropped).
        assert!(
            command_synopsis("new").is_some_and(|s| s.contains("--draft") && s.contains("--tour")),
            "the derived `new` usage synopsis must carry --draft/--tour"
        );
        assert!(
            command_synopsis("build").is_some_and(|s| s.contains("--format json")),
            "the derived `build` usage synopsis must carry --format json"
        );
    }
}
