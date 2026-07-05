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
mod kernel;
mod log;
mod protocol;
mod query;
mod serve;
mod serve_site;
#[cfg(test)]
mod testutil;
mod warm_pool;

use std::process::ExitCode;

fn main() -> ExitCode {
    bridge_legacy_env();
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
        Some("build") => build::cmd_build(&args),
        Some("blocks") => query::cmd_blocks(args.get(2)),
        Some("schema") => query::cmd_schema(&args),
        Some("check") => check::cmd_check(&args),
        Some("init") => cli::cmd_init(args.get(2).map(String::as_str)),
        // `preview`/`dev` are vite-style aliases for the live server.
        Some("serve" | "preview" | "dev") => cli::cmd_serve(&args),
        Some("--version" | "-V") => {
            println!(
                "taliesin {} ({})",
                taliesin_core::VERSION,
                env!("QMD_FAST_GIT_SHA")
            );
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

/// Accept the native `TALIESIN_*` spelling of every runtime env knob while keeping
/// the historical `QMD_FAST_*` names working. For each knob, if the caller set the
/// `TALIESIN_*` form and left the `QMD_FAST_*` form unset, copy it across so the rest
/// of the code (which still reads the `QMD_FAST_*` names internally) picks it up.
fn bridge_legacy_env() {
    // Runtime knobs only. `QMD_FAST_GIT_SHA` is baked in at compile time (via `env!`),
    // not read here, so it is intentionally absent.
    const KNOBS: &[&str] = &[
        "PYTHON",
        "R",
        "CELL_TIMEOUT",
        "NO_CACHE",
        "NO_EXEC",
        "HOST",
        "OPEN",
        "MERMAID_URL",
        "NO_CLEAR",
    ];
    for knob in KNOBS {
        let new = format!("TALIESIN_{knob}");
        let old = format!("QMD_FAST_{knob}");
        if let Ok(val) = std::env::var(&new)
            && std::env::var_os(&old).is_none()
        {
            // SAFETY: run once at the very top of `main`, before any thread or async
            // runtime spawns, so there is no concurrent access to the environment.
            unsafe { std::env::set_var(&old, val) };
        }
    }
}

/// Every subcommand name (aliases included), for the unknown-command did-you-mean.
const COMMANDS: &[&str] = &[
    "render", "build", "blocks", "schema", "check", "init", "serve", "preview", "dev", "help",
];

fn usage() {
    println!(
        "taliesin {} ({})",
        taliesin_core::VERSION,
        env!("QMD_FAST_GIT_SHA")
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
    println!("  preview <file.tmd | dir> [port] [--host] [--open] [--no-exec]");
    println!("                             live preview server (aliases: dev, serve;");
    println!("                             a dir previews the whole SITE with nav + hot reload;");
    println!("                             default port 4321, auto-picks a free one;");
    println!("                             --host exposes it on your LAN with a QR code");
    println!("                             to open on a phone; --open launches a browser;");
    println!("                             --no-exec previews untrusted docs as source,");
    println!("                             never running their code cells)");
    println!("  build  <file.tmd | dir> [out.html] [--out <dir>] [--strict] [--bare]");
    println!("                             render a self-contained HTML file (a dir builds the");
    println!("                             whole SITE to _site/); default <name>.html beside");
    println!("                             the source; --out <dir> writes a portable folder;");
    println!("                             --strict exits non-zero on a cell error or located");
    println!(
        "                             warning; --bare emits zero-JS, CSS-only single-doc HTML"
    );
    println!("  render <file.tmd>          render a full HTML page to stdout");
    println!("                             (static; does NOT execute code cells)");
    println!("  blocks <file.tmd>          list block ids + sourcepos (debug)");
    println!(
        "  schema [--out <dir>]       emit JSON Schemas for _site.yml + front matter (editor autocomplete)"
    );
    println!(
        "  check <file|dir> [--format human|json]  list located diagnostics; exits non-zero if any"
    );
    println!("  help, --version            show this help / the version");
    println!();
    println!("ENV: QMD_FAST_PYTHON (python kernel), QMD_FAST_R (r kernel),");
    println!("     QMD_FAST_CELL_TIMEOUT (per-cell seconds; 0 disables),");
    println!("     QMD_FAST_OPEN (=--open), QMD_FAST_HOST (=--host), QMD_FAST_NO_CLEAR,");
    println!("     QMD_FAST_NO_CACHE (skip the _freeze/ execution cache),");
    println!("     QMD_FAST_NO_EXEC (=--no-exec, never run code cells)");
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
             \x20 --format json   a [{file,line,message}] array to stdout (pipes to jq)\n\
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
    /// Each covered subcommand has a focused help that names itself and shows an
    /// example; an unknown command has none.
    #[test]
    fn subcommand_help_covers_documented_commands() {
        for cmd in [
            "preview", "build", "check", "render", "schema", "blocks", "init",
        ] {
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
