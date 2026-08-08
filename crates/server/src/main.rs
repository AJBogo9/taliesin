//! taliesin — dev server & CLI entry point.
//!
//!   - `taliesin preview <file.tmd> [port]`  live preview server
//!   - `taliesin build  <file.tmd> [out]`    render a self-contained HTML file
//!   - `taliesin build  <file.tmd> --stdout` the same page, to stdout

mod build;
mod build_budget;
mod check;
mod cli;
mod complete;
mod doctor;
mod exec;
mod freeze;
mod headless_js;
mod http1;
mod image_opt;
mod interactive;
mod interpreter;
mod kernel;
mod log;
mod lsp;
mod lsp_cells;
mod lsp_complete;
mod lsp_diag;
mod lsp_edits;
mod lsp_fold;
mod lsp_format;
mod lsp_hints;
mod lsp_insert;
mod lsp_lens;
mod lsp_links;
mod lsp_memo;
mod lsp_nav;
mod lsp_outline;
mod lsp_pos;
mod lsp_project;
mod lsp_refs;
mod lsp_rename_file;
mod lsp_select;
mod lsp_trace;
mod minify;
mod packages;
mod pdf;
mod preview_diag;
mod protocol;
mod publish;
mod run_cmd;
mod run_control;
mod run_print;
mod runspec;
mod runtime_dirs;
mod serve;
mod serve_site;
mod session;
#[cfg(test)]
mod testutil;
mod trace_py;
mod warm_pool;

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
        Some("build") => {
            runtime_dirs::sweep_stale_runtime_dirs();
            build::cmd_build(&args)
        }
        Some("pdf") => pdf::cmd_pdf(&args),
        Some("publish") => publish::cmd_publish(&args),
        // `run` needs the same stale-runtime-dir sweep `build`/`preview` do: it may be
        // the thing that starts the session that owns the kernels.
        Some("run") => {
            runtime_dirs::sweep_stale_runtime_dirs();
            run_cmd::cmd_run(&args)
        }
        Some("check") => check::cmd_check(&args),
        Some("doctor") => doctor::cmd_doctor(&args),
        Some("lsp") => lsp::cmd_lsp(&args),
        Some("init") => cli::cmd_init(&args),
        Some("new") => cli::cmd_new(&args),
        Some("preview") => {
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
            log::error(&unknown_command_message(other));
            usage();
            ExitCode::FAILURE
        }
    }
}

/// Every subcommand name, for the unknown-command did-you-mean.
const COMMANDS: &[&str] = &[
    "build",
    "run",
    "pdf",
    "check",
    "doctor",
    "lsp",
    "init",
    "new",
    "preview",
    "publish",
    "help",
    "completions",
];

/// Subcommands that used to exist, and the one line that says what replaced them.
///
/// The same job [`taliesin_core::RETIRED_KEYS`] does for front matter, for the same reason:
/// a did-you-mean over the *surviving* names answers a retired verb with either silence or
/// a wrong command, and both are worse than nothing when the person typing it is following
/// an older page. Measured on the Wave 5 cuts: `render`, `blocks`, `symbols` and `serve`
/// are all further than edit distance 2 from every survivor, so they got silence — while
/// **`dev` is two edits from `new`**, so `taliesin dev .` answered a request for the preview
/// server by suggesting the command that *scaffolds files*.
///
/// Retired names are deliberately NOT in [`COMMANDS`]: they must not be suggested for a
/// typo of something else, only recognized when typed exactly.
///
/// **A note is ONE sentence naming the replacement, or saying there is none.** Adding an
/// entry is the entire cost of retiring a verb; `a_retired_command_names_its_replacement_
/// instead_of_guessing` below covers every entry in the table, so no per-verb test is owed.
const RETIRED_COMMANDS: &[(&str, &str)] = &[
    (
        "render",
        "`build <file.tmd> --stdout --no-exec` writes the same page to stdout",
    ),
    ("blocks", "`taliesin lsp` publishes the block model now"),
    (
        "symbols",
        "`taliesin lsp` completes cross-reference targets after `@`",
    ),
    ("serve", "use `preview`"),
    ("dev", "use `preview`"),
    ("skim", "nothing; read the `.tmd` source"),
    ("read", "nothing; read the `.tmd` source"),
    (
        "map",
        "nothing on the CLI; `taliesin lsp` answers the project outline in your editor",
    ),
    ("features", "`check --format json` is the machine surface"),
    (
        "vocab",
        "`taliesin lsp` serves the same vocabulary as completions",
    ),
    (
        "schema",
        "`init` writes `.taliesin/tali-site.schema.json` for you",
    ),
    ("mcp", "`check --format json`, run from your agent"),
];

/// The error for a command that is not one of [`COMMANDS`]: the retired-verb note when the
/// name is one this tool used to have, otherwise a did-you-mean within edit distance 2.
fn unknown_command_message(other: &str) -> String {
    if let Some((_, note)) = RETIRED_COMMANDS.iter().find(|(name, _)| *name == other) {
        return format!("`{other}` was removed: {note}");
    }
    match taliesin_core::closest(other, COMMANDS).or_else(|| extended_command(other)) {
        Some(c) => format!("unknown command: `{other}` (did you mean `{c}`?)"),
        None => format!("unknown command: `{other}`"),
    }
}

/// The command a typed name extends or abbreviates, for the cases edit distance cannot see.
/// `taliesin preview-site .` is **five** edits from `preview`, so the distance-2 rule that
/// catches `preveiw` answered the likelier mistake — a plausible longer spelling, the shape
/// every other tool's `<verb>-<noun>` habit teaches — with silence.
///
/// Consulted only after [`taliesin_core::closest`] has declined, so no suggestion that
/// already worked changes. Candidates are [`COMMANDS`], so a retired verb is never the
/// answer here either (`serve-site` must not resolve to the `serve` that was cut).
///
/// Ambiguity yields nothing rather than a coin flip: `c` opens both `check` and
/// `completions`, and picking one would teach a rule that is not real.
fn extended_command(other: &str) -> Option<&'static str> {
    if other.len() < 2 {
        return None;
    }
    let mut hits = COMMANDS
        .iter()
        .copied()
        .filter(|c| other.starts_with(c) || c.starts_with(other));
    let first = hits.next()?;
    hits.next().is_none().then_some(first)
}

/// The `ENV:` block of `usage()`. A const so `env_help_lists_every_runtime_env_var` can
/// diff it against the variables the code actually reads: `TALIESIN_MERMAID_URL` shipped
/// user-facing but undocumented because nothing tied the two together.
const ENV_HELP: &str = "\
ENV: TALIESIN_PYTHON (python kernel), TALIESIN_R (r kernel),
     TALIESIN_CELL_SILENCE (per-cell seconds with NO output; default 600, 0 disables),
     TALIESIN_CELL_TIMEOUT (per-cell wall-clock seconds; off by default, 0 disables),
     TALIESIN_RENDER_TIMEOUT (per-render seconds; default 30, 0 disables),
     TALIESIN_JS_TIMEOUT (pdf's headless-Chrome page-layout seconds; default 10),
     TALIESIN_NO_CLEAR,
     TALIESIN_NO_CACHE (skip the _freeze/ execution cache),
     TALIESIN_NO_EXEC (=--no-exec, never run code cells),
     TALIESIN_MERMAID_URL (override the url the live preview lazy-loads mermaid from)
";

/// The `USAGE:` + `COMMANDS:` block of [`usage`]. A const for the same reason [`ENV_HELP`]
/// is one: `commands_help_lists_every_subcommand` diffs it against `COMMANDS`, and nothing
/// else could. `skim` shipped with a focused `--help` page and a dispatch arm but was
/// absent from this list for its whole life, so the only way to find it was to already
/// know it existed.
const COMMANDS_HELP: &str = "\
USAGE:
  taliesin <command> <file.tmd | dir> [args]
  (a directory argument is a multi-page SITE project: an _site.yml + .tmd pages)

COMMANDS:

Author
  init   [dir]               scaffold a starter site you can preview right away
                             (writes _site.yml + index.tmd; default: current dir)
  new <post|page|deck|paper> <slug> [--dir <root>] [--draft] [--json]
                             scaffold one document, correct on its first save

Preview & build
  run <file.tmd> [--cell N | --line L | --all] [--quiet] [--interrupt]
                             execute code cells in the terminal against this
                             project's warm session; no browser, outputs cached
                             so a later build re-executes nothing
  preview <file.tmd | dir> [port] [--port <N>] [--host] [--open] [--no-exec]
                             live preview server (a dir previews the whole SITE
                             with nav + hot reload;
                             default port 4321 (or [port] / --port <N>), replacing
                             this project's own running preview and stepping past
                             anyone else's;
                             --host exposes it on your LAN with a QR code
                             to open on a phone; --open launches a browser;
                             --no-exec renders code cells as source,
                             kernel and {js} alike, but does not strip raw
                             HTML: see `Documents you did not write`)
  build  <file.tmd | dir> [out.html] [--out <dir>] [--stdout] [--strict] [--bare] [--jobs <N>] [--no-exec] [--format json]
                             render a self-contained HTML file (a dir builds the
                             whole SITE to _site/); default <name>.html beside
                             the source; --out <dir> writes a portable folder;
                             --stdout writes the page to stdout instead of a file;
                             --strict exits non-zero on a cell error or located
                             warning; --bare emits zero-JS, CSS-only single-doc
                             HTML; --jobs <N> caps parallel page renders (site
                             build); --no-exec renders code cells as source
                             (executable cells with no kernel otherwise FAIL)
  pdf    <file.tmd> [-o out.pdf] [--paper a4|letter|a5]
                             a typeset, paginated PDF rendered FROM the built
                             HTML: running heads, folios, cross-refs that name
                             their page (\"Figure 3 (p. 12)\") and an automatic
                             list of figures; default <name>.pdf beside the
                             source; needs a local Chrome for page layout
  publish <dir> [--project-name <name>] [--out <dir>] [--public] [--no-strict] [--dry-run] [--init] [--format json]
                             build a site/book + deploy it to Cloudflare Pages
                             behind a shared passcode (strict by default);
                             --public deploys un-gated; --dry-run skips the deploy;
                             --init runs the one-time Cloudflare setup instead

Inspect
  check <file|dir> [--format human|json] [--errors-only|--strict] [--require-kernel] [--explain <CODE>]
                             list located diagnostics; exits non-zero if any
                             (--explain <CODE> prints a diagnostic code's cause + fix)
  doctor [dir] [--format human|json]  audit the environment for running code cells
                             (interpreters, ipykernel/IRkernel, active conda/venv)

Editor
  lsp                        stdio LSP server: live .tmd diagnostics in any editor
  completions <shell> [--install]  print (or --install) a shell completion script
                             (subcommand + flag + .tmd-aware path completion; --install writes it for you)

  help, --version            show this help / the version

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
    // Grouped by purpose (git/cargo/gh style; clig.dev): the everyday three sit apart from the
    // ten an author rarely types. Flush-left section headers keep each command line unindented.
    print!("{COMMANDS_HELP}");
    print!("{ENV_HELP}");
}

/// Focused help for one subcommand (synopsis + its flags + a one-line example), or
/// `None` for a name with no dedicated page (the caller falls back to `usage()`). Kept as
/// a flat match to mirror the hand-rolled `usage()` style; printed by `main()` when
/// `--help`/`-h` follows a known subcommand.
fn subcommand_help(cmd: &str) -> Option<&'static str> {
    let text = match cmd {
        "preview" => {
            "taliesin preview <file.tmd | dir> [port] [--port <N>] [--host] [--open] [--no-exec]\n\
             \n\
             Live preview server. A file previews one document; a\n\
             directory previews the whole SITE with cross-page nav + per-page hot reload.\n\
             Default port 4321. Re-previewing a project replaces its own running\n\
             preview, so there is only ever one; a port held by anything else falls\n\
             back to the next free one.\n\
             \n\
             Flags:\n\
             \x20 --port <N>  serve on port N (same as the [port] positional, which it wins\n\
             \x20             over when both are given)\n\
             \x20 --host      bind your LAN + print a QR code for phones (token-gated)\n\
             \x20 --open      launch the default browser at the preview URL\n\
             \x20 --no-exec   render code cells as source ({python}/{r} and {js} alike),\n\
             \x20             never executing them. Not an HTML sanitizer\n\
             \n\
             Example:\n\
             \x20 taliesin preview index.tmd --open\n\
             \x20 taliesin preview . --port 4400\n"
        }
        "run" => {
            "taliesin run <file.tmd> [--cell N | --line L | --all] [--quiet] [--interrupt]\n\
             \n\
             Execute the document's code cells and print what they produced, in the\n\
             terminal, with no browser in the loop. Attaches to this project's warm\n\
             session (starting one headlessly if none is up), so the kernel and its\n\
             variables survive between runs: re-running one cell does not re-run the\n\
             expensive ones above it.\n\
             \n\
             Outputs land in `_freeze/` exactly as a preview would write them, so a\n\
             later `taliesin build` replays them and re-executes nothing.\n\
             \n\
             Runs are inclusive and top-down: `--cell 3` means \"make the document true\n\
             THROUGH cell 3\", running whatever earlier cells the kernel is missing.\n\
             \n\
             Flags:\n\
             \x20 --cell N    run through the Nth executable cell (1-based)\n\
             \x20 --line L    run through the cell at source line L (what editors send)\n\
             \x20 --all       run the whole document (the default)\n\
             \x20 --quiet     only errors and the summary, for scripts\n\
             \x20 --interrupt stop this document's run, keeping the warm kernel\n\
             \n\
             Ctrl-C stops a run: it interrupts the running cell and abandons the rest,\n\
             leaving the kernel and every earlier cell's variables intact. `--interrupt`\n\
             is the same thing from another terminal, and says so when nothing is running.\n\
             \n\
             Figures are written to `_freeze/figs/` and their paths printed, since a\n\
             terminal cannot show an image; ctrl-click one to open it.\n\
             \n\
             Example:\n\
             \x20 taliesin run analysis.tmd --cell 5\n\
             \x20 taliesin run analysis.tmd --all --quiet\n\
             \x20 taliesin run analysis.tmd --interrupt\n"
        }
        "build" => {
            "taliesin build <file.tmd | dir> [out.html] [--out <dir>] [--stdout] [--strict] [--bare] [--jobs <N>] [--no-exec] [--format json]\n\
             \n\
             Render a self-contained HTML file. A directory builds the whole SITE to\n\
             _site/. Default output is <name>.html beside the source.\n\
             \n\
             Flags:\n\
             \x20 --out <dir>  write a portable folder (<dir>/index.html + copied assets)\n\
             \x20 --stdout     write the page to stdout instead of to a file (single\n\
             \x20              document only). With --no-exec this is the one-shot,\n\
             \x20              kernel-free HTML dump the `render` verb used to be\n\
             \x20 --strict     exit non-zero on a cell error or located warning (CI gate)\n\
             \x20 --bare       single-doc only: zero-<script>, CSS-only-theme HTML\n\
             \x20 --jobs <N>   max parallel pages (default: auto, memory- and core-capped;\n\
             \x20              --jobs 1 forces sequential; --jobs 0 same as auto)\n\
             \x20 --no-exec    render code cells as source instead of running them. A build\n\
             \x20              whose document HAS executable cells but no usable kernel fails;\n\
             \x20              this is how to ask for source-only output on purpose\n\
             \x20 --format json  emit {diagnostics:[…]} to stdout (agent/CI) instead of only the log\n\
             \n\
             Example:\n\
             \x20 taliesin build post.tmd --strict\n\
             \x20 taliesin build . --jobs 4\n\
             \x20 taliesin build post.tmd --stdout --no-exec > post.html\n"
        }
        "pdf" => {
            "taliesin pdf <file.tmd> [-o out.pdf] [--paper a4|letter|a5]\n\
             \n\
             Render a typeset, paginated PDF *from the built HTML* — the same HTML the\n\
             preview serves, laid out into pages. Running heads, folios, cross-references\n\
             that name their page (\"Figure 3 (p. 12)\") and an automatic list of figures.\n\
             Default output is <name>.pdf beside the source.\n\
             \n\
             Code cells run first (replaying from _freeze when unchanged), so figures are\n\
             real rather than empty. A local Chrome does the page layout.\n\
             \n\
             Flags:\n\
             \x20 -o, --out <path>  write the PDF here (default: <name>.pdf)\n\
             \x20 --paper <size>    a4 (default), letter, or a5\n\
             \n\
             Example:\n\
             \x20 taliesin pdf paper.tmd --paper letter\n"
        }
        "check" => {
            "taliesin check <file.tmd | dir> [--format human|json] [--errors-only|--strict]\n\
             \x20                            [--require-kernel] [--explain <CODE>]\n\
             \n\
             Render in memory and list every located diagnostic; exits non-zero if any\n\
             ERROR or WARNING is found (a CI / pre-publish gate). A SUGGESTION is advice:\n\
             it is printed and never fails the run unless you ask with --strict. Does NOT\n\
             execute code cells.\n\
             \n\
             If the target contains {python}/{r} cells, an Environment footer names the\n\
             interpreter each language WOULD run on. It is not spawned, so the footer says\n\
             nothing about whether it works -- ask `taliesin doctor` or --require-kernel.\n\
             \n\
             Flags:\n\
             \x20 --format human   path:line: message lines to stderr (default). Each path is\n\
             \x20                  rooted on the target as you typed it, so it opens from where\n\
             \x20                  you are (`check docs/guide` -> `docs/guide/sub/page.tmd:5:`)\n\
             \x20 --format json    {diagnostics:[{code,docs_url,severity,file,line,message,\n\
             \x20                     suggestion?}], environment:[...]} object to stdout (jq).\n\
             \x20                  `file` is relative to the TARGET, which the caller passed\n\
             \x20 --errors-only    report + gate on errors only; warnings no longer fail\n\
             \x20 --strict         also fail on suggestions (the strictest gate)\n\
             \x20 --require-kernel also fail if a used language's Jupyter kernel isn't ready\n\
             \x20                  (interpreter + ipykernel/IRkernel); off by default\n\
             \x20 --explain <CODE> expand a diagnostic code (e.g. TAL-XREF-UNREF) into its\n\
             \x20                  cause + canonical fix, rustc-style; bare lists every code.\n\
             \x20                  Honours --format json. Needs no file.\n\
             \n\
             Example:\n\
             \x20 taliesin check . --format json | jq\n\
             \x20 taliesin check src/ --errors-only --require-kernel\n\
             \x20 taliesin check --explain TAL-FM-KEY\n"
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
        "new" => {
            "taliesin new [post|page|deck|paper] [slug] [--dir <root>] [--draft] [--json] [-y]\n\
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
             \x20 --json         print a {kind, slug, created, preview} receipt (agent-friendly)\n\
             \x20 --format human|json  the long spelling of --json\n\
             \x20 -y, --yes      skip the interactive prompt (for scripts run at a terminal)\n\
             \n\
             Example:\n\
             \x20 taliesin new post my-first-post --draft\n"
        }
        "init" => {
            "taliesin init [dir] [--template basic|site|book] [--json] [-y]\n\
             \n\
             Scaffold a starter project into dir (default the current directory) and print\n\
             the preview hint. Refuses to overwrite existing files.\n\
             \n\
             Templates:\n\
             \x20 basic   a one-page site (the default): _site.yml + index.tmd\n\
             \x20 site    a multi-page site: a nav linking a Home and an About page\n\
             \x20 book    a chapters: project: a landing page + two starter chapters\n\
             \n\
             Flags:\n\
             \x20 --template basic|site|book  which starter to scaffold (prompted without it)\n\
             \x20 --json         print a {created, preview} receipt instead of the hint\n\
             \x20 --format human|json  the long spelling of --json\n\
             \x20 -y, --yes      take the basic default without prompting\n\
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
            "taliesin publish <dir> [--project-name <name>] [--out <dir>] [--public] [--no-strict] [--dry-run] [--init] [--format json]\n\
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
             \x20 --strict               ask for the default explicitly (with --no-strict, the\n\
             \x20                        last one given wins)\n\
             \x20 --dry-run              build + gate, print the deploy command, do not deploy\n\
             \x20 --init                 run the one-time Cloudflare setup for this project and\n\
             \x20                        stop (creates the Pages project, then prompts for the\n\
             \x20                        passcode unless --public); neither builds nor deploys\n\
             \x20 --format json         emit {diagnostics:[…]} from the build to stdout (agent/CI)\n\
             \n\
             One-time setup (per repo):\n\
             \x20 export CLOUDFLARE_API_TOKEN=...   (also CLOUDFLARE_ACCOUNT_ID)\n\
             \x20 taliesin publish <dir> --init     (--dry-run first to see the wrangler commands)\n\
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

/// The synopsis for `cmd` — everything in [`subcommand_help`] before its first blank line
/// (`taliesin <cmd> …`), the single source of truth a subcommand's missing-positional `usage:`
/// error derives from, so it can't drift from the `--help` block. `None` for a command with no
/// focused help.
///
/// Not `lines().next()`: a synopsis too long for one line wraps onto an indented continuation,
/// and reading only the first line silently truncated it. `check`'s wrapped continuation is
/// where `--require-kernel`/`--explain` lived.
pub(crate) fn command_synopsis(cmd: &str) -> Option<String> {
    let help = subcommand_help(cmd)?;
    let joined = help
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    (!joined.is_empty()).then_some(joined)
}

/// `usage: <synopsis>` for `cmd`: what a subcommand prints when a required positional is
/// missing. Derived from that command's `--help` synopsis so the two cannot drift — every
/// subcommand goes through here rather than spelling its own line out, which is gated by
/// `no_subcommand_hand_writes_its_own_usage_line`.
pub(crate) fn usage_line(cmd: &str) -> String {
    // Unreachable for a real subcommand: `subcommand_help_covers_documented_commands` pins
    // every name in `COMMANDS` to a focused help page. The fallback keeps a typo'd caller
    // pointing somewhere useful instead of printing a bare `usage:`.
    match command_synopsis(cmd) {
        Some(s) => format!("usage: {s}"),
        None => format!("run `taliesin help` for usage ({cmd} has no focused help)"),
    }
}

/// Print [`usage_line`] to stderr and fail — the whole body of a missing-positional arm.
pub(crate) fn usage_error(cmd: &str) -> std::process::ExitCode {
    eprintln!("{}", usage_line(cmd));
    std::process::ExitCode::FAILURE
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

    /// A retired verb answers with what replaced it, and is never itself suggested.
    ///
    /// The measurement that made this a register rather than a comment: with `dev` merely
    /// deleted, `taliesin dev .` fell through to the did-you-mean, and `dev` is exactly two
    /// edits from `new` — so a request to *preview* a project was answered with the command
    /// that *scaffolds files into it*. The other four cuts got silence, which is only
    /// marginally better for someone following an older page.
    #[test]
    fn a_retired_command_names_its_replacement_instead_of_guessing() {
        // The bad suggestion this register exists to prevent is still one edit-distance
        // lookup away, so assert the register wins rather than trusting it to.
        assert_eq!(taliesin_core::closest("dev", COMMANDS), Some("new"));
        assert!(
            unknown_command_message("dev").contains("preview"),
            "`dev` must point at preview, not at `new`: {}",
            unknown_command_message("dev")
        );
        assert!(
            !unknown_command_message("dev").contains("did you mean"),
            "the retired note replaces the did-you-mean, it does not follow it"
        );
        assert!(unknown_command_message("render").contains("--stdout"));
        assert!(unknown_command_message("symbols").contains("lsp"));
        // A retired name is not a live command, or `main()` would dispatch it and the
        // `--help`/COMMANDS gates would demand a page for it.
        for (name, note) in RETIRED_COMMANDS {
            assert!(
                !COMMANDS.contains(name),
                "`{name}` is retired but still listed in COMMANDS"
            );
            assert!(
                !note.is_empty(),
                "`{name}` retired with no replacement note"
            );
        }
        // And an ordinary typo still gets its did-you-mean.
        assert!(unknown_command_message("biuld").contains("did you mean `build`?"));
    }

    /// A name that EXTENDS or ABBREVIATES a command gets its did-you-mean. Measured:
    /// `taliesin preview-site .` is **five** edits from `preview`, so the distance-2 rule
    /// answered the likelier of the two mistakes with silence while `preveiw` (two edits)
    /// was answered fine. A prefix relation is the stronger signal here, not the weaker one.
    #[test]
    fn a_name_that_extends_or_abbreviates_a_command_suggests_it() {
        for (typed, want) in [
            ("preview-site", "preview"),
            ("build-site", "build"),
            ("publish-site", "publish"),
            ("prev", "preview"),
            ("com", "completions"),
        ] {
            // The premise, measured rather than assumed: edit distance cannot see any of
            // these. (`pre` and `co` are NOT in this list — both are two edits from `pdf`
            // and `mcp` respectively, so `closest` answers them first and this rule never
            // runs. That is deliberate: it only fills silence, it never overrides.)
            assert_eq!(
                taliesin_core::closest(typed, COMMANDS),
                None,
                "`{typed}` is supposed to be out of distance-2 reach"
            );
            let msg = unknown_command_message(typed);
            assert!(
                msg.contains(&format!("did you mean `{want}`?")),
                "`{typed}` should suggest `{want}`: {msg}"
            );
        }
        // One letter is not a signal, even though `read` and `run` both open with it.
        assert_eq!(extended_command("r"), None);
        // A name related to nothing still gets no suggestion.
        assert!(!unknown_command_message("frobnicate").contains("did you mean"));
        // A retired verb keeps its replacement note; this rule never overrides it.
        assert!(unknown_command_message("dev").contains("preview"));
        assert!(!unknown_command_message("dev").contains("did you mean"));
        // A retired name is never itself the suggestion: candidates are COMMANDS, which
        // excludes the cut verbs, so `serve-site` resolves to nothing rather than `serve`.
        assert_eq!(extended_command("serve-site"), None);
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
            // Same shape again, for the browser. The `{js}` observation canary is gated
            // from `crates/server/tests/`, which this walk never sees; the math hover's
            // canary has to live in `src/` (a bin crate cannot reach `pub(crate)` from an
            // integration test), so the scanner meets this one and must be told it is a
            // test gate rather than a user-facing knob.
            "TALIESIN_REQUIRE_CHROME",
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

    /// Every variable in [`ENV_HELP`] is also in the user guide's Environment table. The
    /// third link in the chain: `env_help_lists_every_runtime_env_var` ties the code to
    /// `--help`, and this ties `--help` to the docs, so a knob cannot ship half-documented.
    ///
    /// DOCS-1 measured the gap it closes: `TALIESIN_RENDER_TIMEOUT` and `TALIESIN_JS_TIMEOUT`
    /// both reached `--help` with the feature work that added them (AP2 hardening, DX17b) and
    /// neither reached the guide, which is the surface a user actually reads. Same drift
    /// shape as the CLI flags, which is why that gate exists — it just did not extend here.
    #[test]
    fn every_documented_env_var_is_in_the_user_guide() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/guide/reference/cli.tmd");
        let guide = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        // Anchor on the table, not the whole page: a passing mention in prose elsewhere is
        // not a reference entry, and this is exactly where a reader looks the knob up.
        let table = guide
            .split_once("## Environment")
            .expect("cli.tmd has an Environment section")
            .1;
        let mut missing: Vec<String> = Vec::new();
        for (i, _) in ENV_HELP.match_indices("TALIESIN_") {
            let name: String = ENV_HELP[i..]
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || *c == '_')
                .collect();
            if !table.contains(&name) {
                missing.push(name);
            }
        }
        assert!(
            missing.is_empty(),
            "documented in `taliesin --help` but missing from the guide's Environment table \
             ({}): {missing:?}",
            path.display()
        );
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

    /// Every `.rs` source under this crate, as `(path, text)`, sorted so a failure names the
    /// same file every run. The two gates below read the sources rather than importing the
    /// constants, so a *new* parser is covered without anyone remembering to list it.
    fn server_sources() -> Vec<(std::path::PathBuf, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    out.push((p, src));
                }
            }
        }
        let mut out = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut out,
        );
        out.sort();
        out
    }

    /// The command a `<PREFIX>_FLAGS` const belongs to. Only `SERVE_FLAGS` needs a mapping:
    /// `preview`'s parser is still named `cmd_serve` because the `serve`/`dev` spellings came
    /// first, but `preview` is the canonical name and the one with focused help.
    fn command_for_flag_const(prefix: &str) -> String {
        match prefix {
            "SERVE" => "preview".to_string(),
            p => p.to_ascii_lowercase(),
        }
    }

    /// Each argv parser's accepted long-flag set, read out of the `<PREFIX>_FLAGS` const that
    /// already drives its unknown-flag did-you-mean, paired with its subcommand.
    fn parser_flag_lists() -> Vec<(String, Vec<String>)> {
        const MARK: &str = "_FLAGS: &[&str] = &[";
        let mut out = Vec::new();
        for (_, src) in server_sources() {
            for (i, _) in src.match_indices(MARK) {
                // A top-level `const` only. A flag list declared *inside* a function is a
                // test fixture or a local carve-out, not a parser's real accepted set, and
                // it is indented — so requiring `const` at column 0 excludes it.
                let line_start = src[..i].rfind('\n').map_or(0, |n| n + 1);
                let Some(prefix) = src[line_start..i].strip_prefix("const ") else {
                    continue;
                };
                let Some(end) = src[i + MARK.len()..].find("];") else {
                    continue;
                };
                let flags: Vec<String> = src[i + MARK.len()..i + MARK.len() + end]
                    .split('"')
                    .skip(1)
                    .step_by(2)
                    .filter(|s| s.starts_with("--"))
                    .map(str::to_string)
                    .collect();
                out.push((command_for_flag_const(prefix), flags));
            }
        }
        out
    }

    /// Every long flag a subcommand's parser accepts appears in that subcommand's focused
    /// `--help`. `preview --port <N>` was parsed, unit-tested, shell-completed and printed in
    /// its own error line while appearing in **no** help text at all (PA-CLI1), and the
    /// since-retired `read` had two flags in the same state (PA-CLI2) — on the least
    /// discoverable surface there is, the agent-facing JSON mode. One `--jobs`-shaped
    /// assertion per flag is what let that happen; this compares the two lists mechanically.
    #[test]
    fn every_parsed_flag_is_documented_in_its_subcommand_help() {
        let lists = parser_flag_lists();
        // A scan that finds nothing would pass every assertion below it.
        assert!(
            lists.len() >= 7,
            "the flag-const scan collected only {} lists; the declaration shape moved",
            lists.len()
        );
        // Collected, not asserted per flag: the first miss would otherwise hide the rest, and
        // the whole point is to see the drift as one list.
        let mut undocumented: Vec<String> = Vec::new();
        for (cmd, flags) in lists {
            assert!(!flags.is_empty(), "`{cmd}`'s flag const parsed as empty");
            let help =
                subcommand_help(&cmd).unwrap_or_else(|| panic!("`{cmd}` has no focused help"));
            for f in flags {
                // `--json` is clig.dev shorthand for `--format json` (every parser treats it
                // that way), so documenting `--format` documents both spellings.
                let documented = help.contains(&f) || (f == "--json" && help.contains("--format"));
                if !documented {
                    undocumented.push(format!("{cmd} {f}"));
                }
            }
        }
        assert!(
            undocumented.is_empty(),
            "parsed but documented in no `--help` block: {undocumented:#?}"
        );
    }

    /// No subcommand hand-writes its missing-positional `usage:` line: every one derives from
    /// the `--help` synopsis via [`usage_line`], so the two cannot drift. They already had,
    /// in both directions — the hand-written `preview` line advertised a `--port` its help
    /// omitted, while the hand-written `check` line dropped five flags its help documented.
    #[test]
    fn no_subcommand_hand_writes_its_own_usage_line() {
        // Assembled by `concat!`, so this test's own needle does not appear contiguously in
        // this file and the gate cannot flag itself.
        let needle = concat!("usage: ", "taliesin ");
        for (path, src) in server_sources() {
            if let Some(i) = src.find(needle) {
                let line = src[..i].matches('\n').count() + 1;
                panic!(
                    "{}:{line} spells out a literal usage line; print `crate::usage_line(cmd)` \
                     instead, so it derives from that command's `--help` synopsis",
                    path.display()
                );
            }
        }
    }

    /// Names in `COMMANDS` that are not a subcommand with a page of its own.
    /// `subcommand_help_covers_documented_commands` skips them: `help` is the help system
    /// rather than an entry in it. (The `dev`/`serve` aliases that used to live here were
    /// retired in Wave 5 — one spelling, so there is nothing to resolve.)
    const ALIASES_AND_META: &[&str] = &["help"];

    /// Every subcommand is listed in `taliesin --help`, and everything listed there is a
    /// real subcommand. The command half of the same link `env_help_lists_every_runtime_env_var`
    /// makes for environment variables — and the one that was missing.
    ///
    /// The since-retired `skim` shipped with a dispatch arm, an entry in `COMMANDS`, a
    /// focused `--help` page and an integration test, and was absent from the one list a user
    /// reads to find out what the tool can do. Every other gate passed:
    /// `subcommand_help_covers_documented_commands` only asks whether a *focused* page
    /// exists, which it did.
    #[test]
    fn commands_help_lists_every_subcommand() {
        // `help` is documented on the trailing `help, --version` line: listed, just not as
        // a `  <name> ` entry of its own.
        const LISTED_IN_PROSE: &[&str] = &["help"];
        // A command's entry opens its line: two spaces, the name, then a space. Matching the
        // bare name anywhere would pass on a mention inside another command's description
        // (`preview`'s text names `--no-exec`, `build`'s names `--stdout`), which is exactly
        // the false pass that let `skim` through.
        let listed = |name: &str| COMMANDS_HELP.contains(&format!("\n  {name} "));

        for cmd in COMMANDS.iter().filter(|c| !LISTED_IN_PROSE.contains(c)) {
            assert!(
                listed(cmd),
                "`{cmd}` dispatches but is not listed in `taliesin --help`"
            );
        }
        for name in LISTED_IN_PROSE {
            assert!(
                COMMANDS_HELP.contains(name),
                "`{name}` must at least be mentioned in --help"
            );
        }

        // And the other direction: every entry in the list is a command that exists. An
        // entry lines up under the flush-left group headers, so it is a line starting with
        // exactly two spaces whose first token is not a flag or a continuation.
        let mut found = 0usize;
        // Only the COMMANDS: section — the USAGE: synopsis above it is indented the same way
        // and its first token is the binary's own name.
        let (_, list) = COMMANDS_HELP
            .split_once("COMMANDS:")
            .expect("--help has a COMMANDS: section");
        for line in list.lines() {
            let Some(rest) = line.strip_prefix("  ") else {
                continue;
            };
            let Some(name) = rest.split_whitespace().next() else {
                continue;
            };
            if rest.starts_with(' ') || name.starts_with('-') || name.starts_with('(') {
                continue; // a wrapped description line, a flag, or a parenthetical
            }
            // `help, --version` is the one entry naming two things on one line.
            let name = name.trim_end_matches(',');
            assert!(
                COMMANDS.contains(&name),
                "--help lists `{name}`, which is not a subcommand"
            );
            found += 1;
        }
        // A floor: an extractor that stops matching is a gate that passes forever.
        assert!(
            found >= COMMANDS.len() - LISTED_IN_PROSE.len(),
            "only {found} entries were extracted from --help; the parser has drifted"
        );
    }

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
        // An unrecognized command has no focused help (falls back to top-level usage), and
        // a *retired* one has none either: `dev`/`serve` are not spellings of `preview` any
        // more, they are names the tool no longer answers to.
        assert!(subcommand_help("frobnicate").is_none());
        for (retired, _) in RETIRED_COMMANDS {
            assert!(
                subcommand_help(retired).is_none(),
                "`{retired}` was retired but still has a focused --help page"
            );
        }
        // `--jobs` is documented in build help.
        let build_help = subcommand_help("build").unwrap();
        assert!(
            build_help.contains("--jobs"),
            "build help must document --jobs: {build_help}"
        );
        // PL15: `new` help documents `--draft` (the scaffold advertises it).
        let new_help = subcommand_help("new").unwrap();
        assert!(
            new_help.contains("--draft"),
            "new help must document --draft: {new_help}"
        );
        // PL15: the missing-positional `usage:` one-liners derive from the `--help` synopsis, so
        // they can't drift — the derived `new` synopsis carries the new flags, and the `build`
        // synopsis carries `--format json` (the flag its old hand-written one-liner had dropped).
        assert!(
            command_synopsis("new").is_some_and(|s| s.contains("--draft")),
            "the derived `new` usage synopsis must carry --draft"
        );
        assert!(
            command_synopsis("build").is_some_and(|s| s.contains("--format json")),
            "the derived `build` usage synopsis must carry --format json"
        );
        // A synopsis too long for one line wraps onto an indented continuation, and reading
        // only the first line dropped it: `check`'s `--require-kernel`/`--explain` live
        // there, so `check` with no path advertised two of its flags.
        let check_synopsis = command_synopsis("check").expect("check synopsis");
        for flag in ["--errors-only", "--strict", "--require-kernel", "--explain"] {
            assert!(
                check_synopsis.contains(flag),
                "the derived `check` synopsis must carry {flag} from its wrapped continuation: \
                 {check_synopsis}"
            );
        }
        assert!(
            !check_synopsis.contains('\n'),
            "a synopsis is one line once joined: {check_synopsis}"
        );
    }
}
