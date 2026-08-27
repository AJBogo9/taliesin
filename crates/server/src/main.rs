//! taliesin — dev server & CLI entry point.
//!
//!   - `taliesin preview <file.tmd> [port]`  live preview server
//!   - `taliesin build  <file.tmd> [out]`    render a self-contained HTML file
//!   - `taliesin build  <file.tmd> --stdout` the same page, to stdout

mod build;
mod build_budget;
mod cli;
mod doctor;
mod exec;
mod freeze;
mod interpreter;
mod kernel;
mod lint;
mod log;
mod lsp;
mod lsp_cells;
mod lsp_complete;
mod lsp_diag;
mod lsp_fold;
mod lsp_nav;
mod lsp_outline;
mod lsp_pos;
mod lsp_project;
mod packages;
mod preview_diag;
mod protocol;
mod runtime_dirs;
mod serve;
mod serve_site;
#[cfg(test)]
mod testutil;

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
        Some("--version" | "-V") => {
            println!(
                "taliesin {} ({})",
                taliesin_core::VERSION,
                env!("TALIESIN_GIT_SHA")
            );
            ExitCode::SUCCESS
        }
        // No command, or an explicit help request: print usage and succeed. (`help` itself
        // is a row in the table below, because it is a verb an author types and a verb the
        // did-you-mean must be able to suggest.)
        Some("--help" | "-h") | None => {
            usage();
            ExitCode::SUCCESS
        }
        Some(name) => match command(name) {
            Some(c) => {
                // Unconditionally, and before the command's own work: the syntax sets and
                // the KaTeX JS context are ~176 ms of process-wide setup that every
                // rendering verb pays on its critical path. Not gated on which verb this
                // is, because a second list of verb names next to `COMMANDS` is exactly the
                // drift the table exists to prevent — and the cost of being wrong is
                // nothing. `prewarm` is fire-and-forget, so `init` and `help` return long
                // before its threads matter and the process exit takes them with it.
                taliesin_core::prewarm();
                if c.sweeps_runtime_dirs {
                    runtime_dirs::sweep_stale_runtime_dirs();
                }
                (c.run)(&args)
            }
            // An unrecognized command is an error (non-zero), not a silent success.
            // Suggest the nearest valid command (reusing core's Levenshtein helper).
            //
            // **A pointer, not the whole help, and on stderr.** This arm used to call
            // `usage()`, which is built from `println!` — so an error printed one line to
            // stderr and 56 to stdout, and `taliesin buidl . 2>/dev/null` showed a wall of
            // help with the error gone. The did-you-mean plus a pointer is the whole useful
            // content on an error path; the other 55 lines are noise, and they are one
            // `taliesin help` away. The unknown-*flag* path next door already worked this way.
            None => {
                log::error(&unknown_command_message(name));
                eprintln!("          run `taliesin help` for the full list of commands");
                ExitCode::FAILURE
            }
        },
    }
}

/// One subcommand: **everything** the CLI knows about it, in one place.
///
/// The point of the struct is that there is no second list. This table used to exist four
/// times over — the dispatch `match`, a `COMMANDS` name const, the `--help` command block
/// and the `subcommand_help` match — and main.rs then spent more of its length policing
/// that alignment than dispatching, including a test that read `include_str!("main.rs")`
/// back and diffed the match arms against the const. Those gates went with their subject
/// (FA20): a copy that cannot be made is a copy that needs no gate.
///
/// One copy still lives outside this binary — the row in
/// `docs/guide/reference/cli.tmd`'s table — and that one keeps its gate, because nothing
/// structural can reach it.
struct Command {
    /// The verb as typed.
    name: &'static str,
    /// The `COMMANDS:` group this prints under, or `None` for a verb the grouped list does
    /// not carry as an entry (`help` is the help system, not a line inside it).
    group: Option<&'static str>,
    /// The description printed beside it. Wrapped by hand — the first line sits in the
    /// description column and the rest are indented to meet it, so the prose can break
    /// where it reads best rather than where a width counter says.
    blurb: &'static str,
    /// The focused `taliesin <cmd> --help` page: synopsis, flags, one example. `None` only
    /// for `help`, which has no page of its own to print.
    help: Option<&'static str>,
    /// Sweep abandoned runtime dirs before running. True for the two verbs that can leave
    /// one behind, so a crashed previous run is cleaned up by the next real one.
    sweeps_runtime_dirs: bool,
    /// What actually runs it.
    run: fn(&[String]) -> ExitCode,
}

/// Every subcommand, **in the order `--help` prints them**.
const COMMANDS: &[Command] = &[
    Command {
        name: "init",
        group: Some("Author"),
        blurb: "scaffold a starter site you can preview right away\n\
                (writes _site.yml + index.tmd + one dated example\n\
                post; default: current dir)",
        help: Some(
            "taliesin init [dir]\n\
             \n\
             Scaffold a starter project into dir (default the current directory) and print\n\
             the preview hint: a `_site.yml` holding the title, an `index.tmd` you can\n\
             preview immediately, and one dated post under posts/ that its listing shows.\n\
             Nothing else. Refuses to overwrite existing files.\n\
             \n\
             Add pages by dropping more .tmd files beside index.tmd; add posts by copying\n\
             posts/my-first-post/; make it a book by listing pages under chapters: in\n\
             _site.yml.\n\
             \n\
             Example:\n\
             \x20 taliesin init my-site\n",
        ),
        sweeps_runtime_dirs: false,
        run: cli::cmd_init,
    },
    Command {
        name: "preview",
        group: Some("Preview & build"),
        blurb: "live preview server, on loopback only (a dir previews\n\
                the whole SITE with nav + hot reload;\n\
                default port 4321 (or [port] / --port <N>), replacing\n\
                this project's own running preview and stepping past\n\
                anyone else's;\n\
                --open launches a browser;\n\
                --no-exec renders code cells as source,\n\
                kernel and {js} alike, but does not strip raw\n\
                HTML: see `Documents you did not write`)",
        help: Some(
            "taliesin preview <file.tmd | dir> [port] [--port <N>] [--open] [--no-exec]\n\
             \n\
             Live preview server, bound to loopback only. A file previews one document; a\n\
             directory previews the whole SITE with cross-page nav + per-page hot reload.\n\
             Default port 4321. Re-previewing a project replaces its own running\n\
             preview, so there is only ever one; a port held by anything else falls\n\
             back to the next free one.\n\
             \n\
             Flags:\n\
             \x20 --port <N>  serve on port N (same as the [port] positional, which it wins\n\
             \x20             over when both are given)\n\
             \x20 --open      launch the default browser at the preview URL\n\
             \x20 --no-exec   render code cells as source ({python} and {js} alike),\n\
             \x20             never executing them. Not an HTML sanitizer\n\
             \n\
             Example:\n\
             \x20 taliesin preview index.tmd --open\n\
             \x20 taliesin preview . --port 4400\n",
        ),
        sweeps_runtime_dirs: true,
        run: cli::cmd_serve,
    },
    Command {
        name: "build",
        group: Some("Preview & build"),
        blurb: "render a self-contained HTML file (a dir builds the\n\
                whole SITE to _site/); default <name>.html beside\n\
                the source; --out <dir> writes a portable folder;\n\
                --stdout writes the page to stdout instead of a file;\n\
                --check-only lints and writes nothing (the\n\
                pre-publish gate); --strict exits non-zero on a\n\
                located warning, and on a cell error when cells\n\
                actually run (they never do under --check-only,\n\
                which is static); --jobs <N> caps\n\
                parallel page renders (site build); --no-exec\n\
                renders code cells as source\n\
                (executable cells with no kernel otherwise FAIL)",
        help: Some(
            "taliesin build <file.tmd | dir> [out.html] [--out <dir>] [--stdout] [--check-only]\n\
             \x20                            [--strict] [--jobs <N>] [--no-exec] [--format json]\n\
             \n\
             Render a self-contained HTML file. A directory builds the whole SITE to\n\
             _site/. Default output is <name>.html beside the source.\n\
             \n\
             Flags:\n\
             \x20 --out <dir>  write a portable folder (<dir>/index.html + copied assets)\n\
             \x20 --stdout     write the page to stdout instead of to a file (single\n\
             \x20              document only). With --no-exec this is the one-shot,\n\
             \x20              kernel-free HTML dump the `render` verb used to be\n\
             \x20 --check-only lint and write NOTHING: render in memory, print every located\n\
             \x20              diagnostic, exit non-zero if any of them gates. The pre-publish\n\
             \x20              gate. Never starts a kernel, so --no-exec is implied, and it\n\
             \x20              refuses --out/--stdout/--jobs (nothing is written)\n\
             \x20 --strict     exit non-zero on any located warning, and on a cell error\n\
             \x20              (CI gate). A --check-only run is STATIC and starts no kernel,\n\
             \x20              so there it can only be the warning half — and it additionally\n\
             \x20              fails on advice (suggestions)\n\
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
             \x20 taliesin build post.tmd --stdout --no-exec > post.html\n\
             \x20 taliesin build docs/guide --check-only --strict\n\
             \x20 taliesin build . --check-only --format json | jq\n",
        ),
        sweeps_runtime_dirs: true,
        run: build::cmd_build,
    },
    Command {
        name: "doctor",
        group: Some("Inspect"),
        blurb: "audit the environment for running code cells\n\
                (the Python interpreter, ipykernel, _site.yml)",
        help: Some(
            "taliesin doctor [dir] [--format human|json]\n\
             \n\
             Audit whether the environment can run code cells: the Python interpreter\n\
             (resolved as a build would: _site.yml python:, a .venv, TALIESIN_PYTHON, then\n\
             the PATH default), whether ipykernel imports, and _site.yml validity. Prints a\n\
             status line per item with a fix command; exits non-zero only if a configured\n\
             interpreter is broken.\n\
             \n\
             Example:\n\
             \x20 taliesin doctor\n\
             \x20 taliesin doctor myproject --format json\n",
        ),
        sweeps_runtime_dirs: false,
        run: doctor::cmd_doctor,
    },
    Command {
        name: "lsp",
        group: Some("Editor"),
        blurb: "stdio LSP server: live .tmd diagnostics in any editor",
        help: Some(
            "taliesin lsp\n\
             \n\
             Run a local, offline LSP (Language Server Protocol) server over stdio so any\n\
             LSP editor (Neovim, Helix, Zed, VS Code) gets live .tmd diagnostics as you\n\
             type — the same validators the build gate runs, on the unsaved buffer. Parse-only: no\n\
             kernel, no code execution, read-only (it never edits your source). JSON-RPC on\n\
             stdout, logs on stderr.\n\
             \n\
             Example (Neovim, via nvim-lspconfig or vim.lsp.start):\n\
             \x20 cmd = { \"taliesin\", \"lsp\" }\n",
        ),
        sweeps_runtime_dirs: false,
        run: lsp::cmd_lsp,
    },
    Command {
        name: "help",
        group: None,
        blurb: "show this help",
        help: None,
        sweeps_runtime_dirs: false,
        run: cmd_help,
    },
];

/// The table row for `name`, or `None` when the binary answers no such verb.
fn command(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.name == name)
}

/// `taliesin help [command]`.
///
/// `taliesin help <cmd>` is the same request as `taliesin <cmd> --help`, which the
/// intercept in `main` already serves; this is the other spelling. A bare `help`, or a name
/// with no focused page, prints top-level usage.
fn cmd_help(args: &[String]) -> ExitCode {
    match args.get(2).and_then(|c| subcommand_help(c)) {
        Some(help) => print!("{help}"),
        None => usage(),
    }
    ExitCode::SUCCESS
}

/// The error for a command that is not in [`COMMANDS`]: a did-you-mean within edit
/// distance 2, or the longer-spelling rule below, or nothing.
fn unknown_command_message(other: &str) -> String {
    match taliesin_core::closest_of(other, COMMANDS.iter().map(|c| c.name))
        .or_else(|| extended_command(other))
    {
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
/// Ambiguity yields nothing rather than a coin flip: `b` opens `build` alone today, but `l`
/// opens `lsp` alone only because `lint` is a flag rather than a verb, and picking a winner
/// when two do match would teach a rule that is not real.
fn extended_command(other: &str) -> Option<&'static str> {
    if other.len() < 2 {
        return None;
    }
    let mut hits = COMMANDS
        .iter()
        .map(|c| c.name)
        .filter(|c| other.starts_with(c) || c.starts_with(other));
    let first = hits.next()?;
    hits.next().is_none().then_some(first)
}

/// The `ENV:` block of `usage()`. A const so `env_help_lists_every_runtime_env_var` can
/// diff it against the variables the code actually reads: `TALIESIN_MERMAID_URL` shipped
/// user-facing but undocumented because nothing tied the two together.
const ENV_HELP: &str = "\
ENV: TALIESIN_PYTHON (python kernel),
     TALIESIN_CELL_SILENCE (per-cell seconds with NO output; default 600, 0 disables),
     TALIESIN_CELL_TIMEOUT (per-cell wall-clock seconds; off by default, 0 disables),
     TALIESIN_RENDER_TIMEOUT (per-render seconds; default 30, 0 disables),
     TALIESIN_NO_CLEAR,
     TALIESIN_NO_CACHE (skip the _freeze/ execution cache),
     TALIESIN_NO_EXEC (=--no-exec, never run code cells),
     TALIESIN_MERMAID_URL (override the url the live preview lazy-loads mermaid from)
";

/// Where a command's description starts, in columns. Chosen once so every entry lines up:
/// `  ` + a 6-wide name + ` ` puts the argument sketch at column 9, and anything wider than
/// the column wraps its description onto the next line rather than pushing the whole grid
/// right (which is what the hand-aligned `doctor` row used to do).
const DESC_COL: usize = 29;

/// The fixed head of [`commands_help`].
const USAGE_HEADER: &str = "\
USAGE:
  taliesin <command> <file.tmd | dir> [args]
  (a directory argument is a multi-page SITE project: an _site.yml + .tmd pages)

COMMANDS:
";

/// The argument sketch printed after a verb's name in the grouped list: its own focused
/// help page's synopsis, with the leading `taliesin <verb> ` stripped.
///
/// Derived rather than stored, because the grouped list and the focused page were saying
/// the same thing in two hand-synced places — the shape that let `preview --port <N>` be
/// parsed, unit-tested and shell-completed while appearing in no help text at all. Empty
/// for a verb that takes no arguments (`lsp`), which is also what a verb with no focused
/// page gets.
fn command_args(name: &str) -> String {
    command_synopsis(name)
        .and_then(|s| {
            s.strip_prefix(&format!("taliesin {name} "))
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

/// The `USAGE:` + `COMMANDS:` block, **built from [`COMMANDS`]**.
///
/// This was a hand-maintained const until FA20, and the gate that diffed it against the
/// verb list was the only thing standing between a reader and a verb they could not find:
/// the since-retired `skim` shipped with a dispatch arm, a focused `--help` page and an
/// integration test, and was absent from this list for its whole life. Generated, the list
/// cannot disagree with the binary — so the gate is gone too, and what is left to test is
/// the layout rule rather than the contents.
///
/// Grouped by purpose (git/cargo/gh style; clig.dev), flush-left headers, so the everyday
/// verbs sit apart from the ones an author rarely types.
fn commands_help() -> String {
    let mut out = String::from(USAGE_HEADER);
    let mut current = "";
    for c in COMMANDS {
        let Some(group) = c.group else { continue };
        if group != current {
            out.push_str(&format!("\n{group}\n"));
            current = group;
        }
        let head = format!("  {:<6} {}", c.name, command_args(c.name));
        let head = head.trim_end();
        // Wide entries put their description on the next line; narrow ones share it.
        if head.len() < DESC_COL {
            out.push_str(&format!("{head:<DESC_COL$}"));
        } else {
            out.push_str(&format!("{head}\n{:DESC_COL$}", ""));
        }
        out.push_str(&c.blurb.replace('\n', &format!("\n{:DESC_COL$}", "")));
        out.push('\n');
    }
    // `help` has no entry of its own (it is the page you are reading) and `--version` is a
    // flag rather than a verb, so the pair share one trailing line.
    out.push_str(&format!(
        "\n  {:<DESC_COL_LESS_TWO$}show this help / the version\n\n",
        "help, --version",
        DESC_COL_LESS_TWO = DESC_COL - 2
    ));
    out
}

fn usage() {
    println!(
        "taliesin {} ({})",
        taliesin_core::VERSION,
        env!("TALIESIN_GIT_SHA")
    );
    println!("A fast .tmd -> HTML renderer and live preview server.");
    println!("Docs: https://github.com/AJBogo9/taliesin");
    println!();
    print!("{}", commands_help());
    print!("{ENV_HELP}");
}

/// Focused help for one subcommand (synopsis + its flags + a one-line example), or
/// `None` for a name with no dedicated page (the caller falls back to `usage()`). Printed
/// by `main()` when `--help`/`-h` follows a known subcommand.
fn subcommand_help(cmd: &str) -> Option<&'static str> {
    command(cmd)?.help
}

/// The synopsis for `cmd` — everything in [`subcommand_help`] before its first blank line
/// (`taliesin <cmd> …`), the single source of truth a subcommand's missing-positional `usage:`
/// error derives from, so it can't drift from the `--help` block. `None` for a command with no
/// focused help.
///
/// Not `lines().next()`: a synopsis too long for one line wraps onto an indented continuation,
/// and reading only the first line silently truncated it. The retired `check` verb's wrapped
/// continuation is where two of its flags lived, and reading one line advertised neither.
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

#[cfg(test)]
mod dispatch_tests {
    use super::*;

    /// The verb names, for the did-you-mean assertions below.
    fn names() -> Vec<&'static str> {
        COMMANDS.iter().map(|c| c.name).collect()
    }

    #[test]
    fn closest_command_suggests_nearest() {
        let closest = |typed: &str| taliesin_core::closest(typed, &names());
        // A near-miss typo resolves to the intended command.
        assert_eq!(closest("biuld"), Some("build"));
        assert_eq!(closest("previw"), Some("preview"));
        assert_eq!(closest("innit"), Some("init"));
        // Something far from every command yields no suggestion (not a wild guess).
        assert_eq!(closest("frobnicate"), None);
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
            ("prev", "preview"),
            ("doc", "doctor"),
        ] {
            // The premise, measured rather than assumed: edit distance cannot see any of
            // these, so the prefix rule only ever fills silence and never overrides a
            // did-you-mean. (`pre` and `co` used to be excluded here because `pdf` and
            // `mcp` were two edits away and `closest` answered them first. Both verbs are
            // retired now — wave 2 took `mcp`, wave 4 took `pdf` — so the exclusion is
            // spent; the shorter prefixes would pass too, and the assertion below is what
            // would say so if that ever changed back.)
            assert_eq!(
                taliesin_core::closest(typed, &names()),
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
        // A cut verb is never itself the suggestion: candidates are COMMANDS, which holds
        // only what the binary answers, so `serve-site` resolves to nothing rather than to
        // the `serve` this tool used to have.
        assert_eq!(extended_command("serve-site"), None);
        assert!(!unknown_command_message("dev").contains("did you mean"));
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
            // knob a user of the binary ever sets. Read from `crates/core/tests/`, which
            // this walk never sees, so the exemption is belt-and-braces.
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

    /// Every subcommand has a row in the CLI reference's command table, and every row names
    /// a subcommand the binary answers.
    ///
    /// **This is the fifth registration site, and it was the only ungated one.** A new verb
    /// already trips [`COMMANDS`], `COMMANDS_HELP`, `subcommand_help` and the usage page;
    /// the row in `docs/guide/reference/cli.tmd`'s table was maintained by hand, and
    /// `doctor` — a shipped verb with four VS Code wirings — reached this gate's writing
    /// with no row at all, discoverable only from one subordinate clause 100 lines down.
    /// The reverse direction matters just as much: wave 13 left the retired `run` row
    /// standing through several edits, and what eventually caught it was
    /// `documented_cli_flags_exist_in_the_cli` noticing the *flags* inside the row — so a
    /// retired verb with no flags would have left a documented command the binary refuses,
    /// with every gate green.
    #[test]
    fn every_subcommand_has_a_row_in_the_cli_reference() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/guide/reference/cli.tmd");
        let guide = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let table = guide
            .split_once("| Command | Effect |")
            .expect("cli.tmd has a command table")
            .1;

        let mut documented: Vec<String> = Vec::new();
        for line in table.lines().skip(1) {
            if !line.starts_with('|') {
                break;
            }
            // The `|---|---|` separator is the first row after the header and is not a
            // command; without this it lands in the set as `---|---|` and fails the
            // reverse assertion.
            if line
                .chars()
                .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
            {
                continue;
            }
            let cell = line.trim_start_matches('|').trim();
            let verb: String = cell
                .trim_start_matches('`')
                // Stop at the first space OR backtick: `lsp` ends at the backtick, and
                // `build <file.tmd\|dir> --check-only` must stop at the space rather than
                // at the escaped pipe inside its argument.
                .chars()
                .take_while(|c| *c != ' ' && *c != '`')
                .collect();
            if !verb.is_empty() {
                documented.push(verb);
            }
        }

        assert!(
            documented.len() >= 5,
            "parsed only {} rows out of {}'s command table — the parse broke, not the docs",
            documented.len(),
            path.display()
        );

        // `help` is the usage page itself, not a row in the table it prints.
        let missing: Vec<&str> = COMMANDS
            .iter()
            .map(|c| c.name)
            .filter(|c| *c != "help" && !documented.iter().any(|d| d == c))
            .collect();
        assert!(
            missing.is_empty(),
            "shipped subcommand with no row in {}'s command table: {missing:?}",
            path.display()
        );

        let unknown: Vec<&String> = documented.iter().filter(|d| command(d).is_none()).collect();
        assert!(
            unknown.is_empty(),
            "{} documents a command the binary does not answer: {unknown:?}",
            path.display()
        );
    }

    /// `taliesin help <cmd>` is the same request as `taliesin <cmd> --help`. It used to
    /// print top-level usage, because the `help` verb matched before the subcommand was
    /// looked at.
    #[test]
    fn help_verb_with_a_subcommand_resolves_to_that_subcommands_help() {
        for cmd in ["build", "preview", "init"] {
            assert!(
                subcommand_help(cmd).is_some(),
                "`help {cmd}` needs a focused page to resolve to"
            );
        }
        // A bare `help`, or `help <unknown>`, still falls back to top-level usage.
        assert!(subcommand_help("frobnicate").is_none());
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
                // it is indented, so requiring the declaration at column 0 excludes it.
                // The visibility prefix is optional: `BUILD_FLAGS`/`SERVE_FLAGS` are
                // `pub(crate)` so `serve::retired_flags` can assert a retired flag is
                // offered by no live parser.
                let line_start = src[..i].rfind('\n').map_or(0, |n| n + 1);
                let decl = &src[line_start..i];
                let Some(prefix) = decl
                    .strip_prefix("const ")
                    .or_else(|| decl.strip_prefix("pub(crate) const "))
                else {
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
        // A scan that finds nothing would pass every assertion below it. The floor was 7
        // until wave 4 cut `publish` and `pdf`, each of which owned one const, 6 until
        // wave 9 retired `check` and folded its gate into `build --check-only`, and 5 until
        // `new` was cut on 2026-08-17 with its `NEW_FLAGS`.
        assert!(
            lists.len() >= 4,
            "the flag-const scan collected only {} lists; the declaration shape moved",
            lists.len()
        );
        // Collected, not asserted per flag: the first miss would otherwise hide the rest, and
        // the whole point is to see the drift as one list.
        let mut undocumented: Vec<String> = Vec::new();
        // A verb that genuinely accepts no flags. `init` took `--json`/`--format` until
        // 2026-08-13; it now takes a directory and nothing else, so its const is `&[]` on
        // purpose rather than because the scan lost its contents. The scanner-integrity
        // job this assertion shares is carried by the `lists.len() >= 5` floor above, which
        // is why the const stays declared instead of being deleted.
        const FLAGLESS: &[&str] = &["init"];
        for (cmd, flags) in lists {
            assert!(
                !flags.is_empty() || FLAGLESS.contains(&cmd.as_str()),
                "`{cmd}`'s flag const parsed as empty"
            );
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
    /// omitted, while the retired `check` verb's hand-written line dropped five flags its own
    /// help documented.
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

    /// The generated `--help` list. What used to need a gate — *is every verb listed, and is
    /// every listing a verb* — is now structural: [`commands_help`] walks [`COMMANDS`], so
    /// the two cannot disagree in either direction. What is left to check is the **layout
    /// rule** that replaced the hand-alignment, since a generator can produce a list that is
    /// complete and unreadable.
    #[test]
    fn the_help_list_is_generated_and_lines_its_descriptions_up() {
        let help = commands_help();
        for c in COMMANDS {
            let Some(group) = c.group else {
                // A verb with no group prints no entry of its own; `help` rides the trailing
                // meta line instead, and must still be findable there.
                assert!(
                    help.contains(&format!("  {}, --version", c.name)),
                    "`{}` has no group and no place on the meta line either:\n{help}",
                    c.name
                );
                continue;
            };
            assert!(
                help.contains(&format!("\n{group}\n")),
                "`{}`'s group `{group}` is not a header:\n{help}",
                c.name
            );
            assert!(
                help.contains(&format!("\n  {} ", c.name)),
                "`{}` is in the table but not in the printed list:\n{help}",
                c.name
            );
        }
        // Every description — the first line of a blurb and every wrapped continuation —
        // starts in the same column, whether it shares the entry's row or follows it. The
        // hand-maintained list drifted here (`doctor`'s row sat at 38), which is exactly the
        // kind of detail a generator should own.
        let lines: Vec<&str> = help.lines().collect();
        let indent = format!("{:DESC_COL$}", "");
        for c in COMMANDS.iter().filter(|c| c.group.is_some()) {
            let row = lines
                .iter()
                .position(|l| l.starts_with(&format!("  {} ", c.name)))
                .unwrap_or_else(|| panic!("no entry row for `{}`:\n{help}", c.name));
            let mut want = c.blurb.lines();
            let first = want.next().expect("a blurb is never empty");
            // A narrow entry shares its row with the first description line; a wide one
            // pushes it to the next. Either way the text starts in the same column.
            let mut next = row + 1;
            if lines[row].get(DESC_COL..) != Some(first) {
                assert_eq!(
                    lines[next],
                    format!("{indent}{first}"),
                    "`{}`'s description starts neither at column {DESC_COL} of its own row \
                     nor on the next line",
                    c.name
                );
                next += 1;
            }
            for line in want {
                assert_eq!(
                    lines[next],
                    format!("{indent}{line}"),
                    "`{}`'s wrapped description line is not indented to the column",
                    c.name
                );
                next += 1;
            }
        }
    }

    /// Each covered subcommand has a focused help that names itself and shows an
    /// example; an unknown command has none.
    #[test]
    fn subcommand_help_covers_documented_commands() {
        // Driven by COMMANDS, so a new subcommand cannot ship without focused help. `help`
        // is the one row allowed to carry none: it is the help system, not an entry in it.
        for c in COMMANDS {
            let Some(help) = c.help else {
                assert_eq!(
                    c.name, "help",
                    "`{}` ships with no focused --help page",
                    c.name
                );
                continue;
            };
            assert!(
                help.contains(c.name),
                "`{}` help should name the subcommand: {help}",
                c.name
            );
            assert!(
                help.contains("taliesin"),
                "`{}` help should show a `taliesin …` example: {help}",
                c.name
            );
        }
        // An unrecognized command has no focused help (it falls back to top-level usage).
        assert!(subcommand_help("frobnicate").is_none());
        // `--jobs` is documented in build help.
        let build_help = subcommand_help("build").unwrap();
        assert!(
            build_help.contains("--jobs"),
            "build help must document --jobs: {build_help}"
        );
        // PL15: the missing-positional `usage:` one-liners derive from the `--help` synopsis,
        // so they can't drift — the derived `build` synopsis carries `--format json`, the flag
        // its old hand-written one-liner had dropped.
        assert!(
            command_synopsis("build").is_some_and(|s| s.contains("--format json")),
            "the derived `build` usage synopsis must carry --format json"
        );
        // A synopsis too long for one line wraps onto an indented continuation, and reading
        // only the first line dropped it. `build` is where that lives now: its flags run onto
        // a second line, so `--jobs`/`--no-exec`/`--format json` are only reachable by joining.
        let build_synopsis = command_synopsis("build").expect("build synopsis");
        for flag in [
            "--check-only",
            "--strict",
            "--jobs",
            "--no-exec",
            "--format json",
        ] {
            assert!(
                build_synopsis.contains(flag),
                "the derived `build` synopsis must carry {flag} from its wrapped continuation: \
                 {build_synopsis}"
            );
        }
        assert!(
            !build_synopsis.contains('\n'),
            "a synopsis is one line once joined: {build_synopsis}"
        );
    }
}
