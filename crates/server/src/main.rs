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
mod exec;
mod freeze;
mod interpreter;
mod kernel;
mod log;
mod protocol;
mod publish;
mod query;
mod serve;
mod serve_site;
#[cfg(test)]
mod testutil;
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
        Some("render") => query::cmd_render(args.get(2)),
        Some("read") => query::cmd_read(args.get(2)),
        Some("build") => build::cmd_build(&args),
        Some("publish") => publish::cmd_publish(&args),
        Some("blocks") => query::cmd_blocks(args.get(2)),
        Some("schema") => query::cmd_schema(&args),
        Some("vocab") => query::cmd_vocab(),
        Some("symbols") => query::cmd_symbols(&args),
        Some("check") => check::cmd_check(&args),
        Some("init") => cli::cmd_init(args.get(2).map(String::as_str)),
        Some("new") => cli::cmd_new(&args),
        // `preview`/`dev` are vite-style aliases for the live server.
        Some("serve" | "preview" | "dev") => cli::cmd_serve(&args),
        Some("completions") => cli::cmd_completions(&args),
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
    println!("COMMANDS:");
    println!("  init   [dir]               scaffold a starter site you can preview right away");
    println!("                             (writes _site.yml + index.tmd; default: current dir)");
    println!("  new <post|page|deck> <slug> [--dir <root>]");
    println!("                             scaffold one document, correct on its first save");
    println!("  preview <file.tmd | dir> [port] [--host] [--open] [--no-exec]");
    println!("                             live preview server (aliases: dev, serve;");
    println!("                             a dir previews the whole SITE with nav + hot reload;");
    println!("                             default port 4321, auto-picks a free one;");
    println!("                             --host exposes it on your LAN with a QR code");
    println!("                             to open on a phone; --open launches a browser;");
    println!("                             --no-exec previews untrusted docs as source,");
    println!("                             never running their code cells)");
    println!("  build  <file.tmd | dir> [out.html] [--out <dir>] [--strict] [--bare] [--jobs <N>]");
    println!("                             render a self-contained HTML file (a dir builds the");
    println!("                             whole SITE to _site/); default <name>.html beside");
    println!("                             the source; --out <dir> writes a portable folder;");
    println!("                             --strict exits non-zero on a cell error or located");
    println!("                             warning; --bare emits zero-JS, CSS-only single-doc");
    println!(
        "                             HTML; --jobs <N> caps parallel page renders (site build)"
    );
    println!("  publish <dir> [--project-name <name>] [--out <dir>] [--strict] [--dry-run]");
    println!("                             build a site/book + deploy it to Cloudflare Pages");
    println!("                             behind a shared passcode (Wrangler direct upload);");
    println!("                             --dry-run builds + gates + prints the deploy command");
    println!("  render <file.tmd>          render a full HTML page to stdout");
    println!("                             (static; does NOT execute code cells)");
    println!("  read   <file.tmd>          project the document to plain text (agent-readable;");
    println!("                             static, does NOT execute code cells)");
    println!("  blocks <file.tmd>          list block ids + sourcepos (debug)");
    println!(
        "  schema [--out <dir>]       emit JSON Schemas for _site.yml + front matter (editor autocomplete)"
    );
    println!(
        "  vocab                      emit editor autocomplete vocabulary as JSON (companion)"
    );
    println!("  symbols <file.tmd> [--format human|json]  list the doc's cross-reference targets");
    println!(
        "  check <file|dir> [--format human|json]  list located diagnostics; exits non-zero if any"
    );
    println!(
        "  completions <bash|zsh|fish>  print a shell completion script to stdout (install hint in --help)"
    );
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
             Default port 4321 (auto-picks the next free one if it's taken).\n\
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
            "taliesin build <file.tmd | dir> [out.html] [--out <dir>] [--strict] [--bare] [--jobs <N>]\n\
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
             \n\
             Example:\n\
             \x20 taliesin build post.tmd --strict\n\
             \x20 taliesin build . --jobs 4\n"
        }
        "check" => {
            "taliesin check <file.tmd | dir> [--format human|json]\n\
             \n\
             Render in memory and list every located diagnostic; exits non-zero if any\n\
             are found (a CI / pre-publish gate). Does NOT execute code cells.\n\
             \n\
             Flags:\n\
             \x20 --format human  path:line: message lines to stderr (default)\n\
             \x20 --format json   {diagnostics:[{code,severity,file,line,message,suggestion?}],\n\
             \x20                    environment:[...]} object to stdout (pipes to jq)\n\
             \n\
             Example:\n\
             \x20 taliesin check . --format json | jq\n"
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
            "taliesin new <post|page|deck> <slug> [--dir <root>]\n\
             \n\
             Scaffold one document that is correct on its first save: it renders, and\n\
             `taliesin check` passes on it with no diagnostics. A post lands in\n\
             posts/<slug>/index.tmd and is dated today; a page and a deck land in\n\
             <slug>.tmd. Refuses to overwrite an existing file.\n\
             \n\
             Flags:\n\
             \x20 --dir <root>   scaffold under <root> instead of the current directory\n\
             \n\
             Example:\n\
             \x20 taliesin new post my-first-post\n"
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
            "taliesin init [dir]\n\
             \n\
             Scaffold a minimal previewable site into dir (default the current\n\
             directory): writes _site.yml + index.tmd, then prints the preview hint.\n\
             Refuses to overwrite existing files.\n\
             \n\
             Example:\n\
             \x20 taliesin init my-site\n"
        }
        "publish" => {
            "taliesin publish <dir> [--project-name <name>] [--out <dir>] [--strict] [--dry-run]\n\
             \n\
             Build a site or book and deploy it to Cloudflare Pages (Wrangler direct\n\
             upload) behind a shared passcode. One-way: it never writes to your source.\n\
             The passcode lives only as a Cloudflare secret, never in your repo.\n\
             \n\
             Flags:\n\
             \x20 --project-name <name>  Cloudflare Pages project (default: the dir-name slug)\n\
             \x20 --out <dir>            build output dir (default: the project's _site/_book)\n\
             \x20 --strict               fail before deploying if the build has warnings\n\
             \x20 --dry-run              build + inject the gate, print the deploy command,\n\
             \x20                        do not deploy\n\
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
            "taliesin completions <bash|zsh|fish>\n\
             \n\
             Print a shell completion script to stdout (command names + file arguments).\n\
             The offered commands are generated from the CLI itself, so they never drift.\n\
             \n\
             Install:\n\
             \x20 bash  taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin\n\
             \x20 zsh   taliesin completions zsh  > \"${fpath[1]}/_taliesin\"   # then: compinit\n\
             \x20 fish  taliesin completions fish > ~/.config/fish/completions/taliesin.fish\n\
             \n\
             Example:\n\
             \x20 taliesin completions bash\n"
        }
        _ => return None,
    };
    Some(text)
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
            // Flags (`--version`, `-h`) are not commands and are never suggested, so they
            // are not in `COMMANDS`. A binding pattern (`Some(other)`) has no literal.
            for lit in rest[..end].split('"').skip(1).step_by(2) {
                if !lit.starts_with('-') {
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
    }
}
