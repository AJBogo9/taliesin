//! The `publish` subcommand: build a site/book and deploy it to Cloudflare Pages
//! (Wrangler direct upload) behind a shared passcode.
//!
//! **What:** `publish <dir>` builds the project (reusing the site build), writes a
//! bundled `functions/_middleware.js` HTTP Basic-Auth gate into the output tree, then
//! runs `wrangler pages deploy` from that tree. One-way: it never writes to the source.
//!
//! **How to use:** `main()` dispatches `publish` to [`cmd_publish`].
//!
//! **One-time setup (per repo):** `publish <dir> --init` runs it — see [`init_commands`].
//! It needs `CLOUDFLARE_API_TOKEN` (and `CLOUDFLARE_ACCOUNT_ID`) in the environment, the
//! same as a deploy.

use crate::log;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The Cloudflare Pages Function that gates the deployed site behind a shared passcode.
/// Written into `<out>/functions/_middleware.js` at publish time.
const MIDDLEWARE_JS: &str = include_str!("assets/_middleware.js");

/// Fixed production-branch label used at project-create time and at every deploy, so a
/// deploy is always a *production* deploy (stable `<name>.pages.dev`) regardless of the
/// source repo's current git branch.
const PRODUCTION_BRANCH: &str = "production";

/// The Cloudflare Pages environment variable the passcode gate reads. Named once here
/// because `--init` puts it in place and [`MIDDLEWARE_JS`] consumes it, and a rename in
/// one of those without the other yields a site nobody can open with a secret nothing
/// reads. `the_gate_secret_is_the_one_the_middleware_reads` pins the pair.
const GATE_SECRET: &str = "PASSWORD";

/// Long flags `publish` accepts (drives the unknown-flag did-you-mean).
const PUBLISH_FLAGS: &[&str] = &[
    "--project-name",
    "--out",
    "--strict",
    "--no-strict",
    "--public",
    "--dry-run",
    "--init",
    "--format",
    "--json",
];

/// Parsed `publish` argv (pure; no I/O), so the positional/flag rules are unit-testable.
#[derive(Debug)]
struct PublishArgs<'a> {
    path: &'a str,
    project_name: Option<&'a str>,
    out_dir: Option<&'a str>,
    strict: bool,
    dry_run: bool,
    public: bool,
    /// `--init`: run the one-time Cloudflare setup instead of building + deploying.
    init: bool,
    /// `--format json` emits the build's structured diagnostics to stdout. Default `human`.
    format: &'a str,
}

/// Parse `publish` argv (`args[2..]`). The first positional is the project dir.
fn parse_publish_args(args: &[String]) -> Result<PublishArgs<'_>, String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut project_name: Option<&str> = None;
    let mut out_dir: Option<&str> = None;
    let mut strict = true; // publish is strict by default; --no-strict opts out
    let mut dry_run = false;
    let mut public = false;
    let mut init = false;
    let mut format: &str = "human";
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--format" => match it.next().map(|s| s.as_str()) {
                Some(v) if v == "human" || v == "json" => format = v,
                other => return Err(format!("error: {}", crate::serve::bad_format_error(other))),
            },
            // `--json`: clig.dev shorthand for `--format json`.
            "--json" => format = "json",
            "--project-name" => match it.next().map(|s| s.as_str()) {
                Some(v) if !v.starts_with("--") => project_name = Some(v),
                _ => {
                    return Err(
                        "error: --project-name requires a value (e.g. --project-name my-book)"
                            .to_string(),
                    );
                }
            },
            // `--out` = the output dir. The undocumented `--dir` alias was dropped
            // (`--dir` is reserved for the scaffold-input flag on `init`/`new`).
            "--out" => match it.next().map(|s| s.as_str()) {
                Some(v) if !v.starts_with("--") => out_dir = Some(v),
                _ => {
                    return Err(format!(
                        "error: {a} requires a directory value (e.g. {a} out)"
                    ));
                }
            },
            "--strict" => strict = true,
            "--no-strict" => strict = false,
            "--public" => public = true,
            "--dry-run" => dry_run = true,
            "--init" => init = true,
            s if s.starts_with("--") => {
                return Err(format!(
                    "error: {}",
                    crate::serve::unknown_flag_error(s, PUBLISH_FLAGS)
                ));
            }
            s => positionals.push(s),
        }
    }
    let path = positionals
        .first()
        .copied()
        .ok_or_else(|| crate::usage_line("publish"))?;
    Ok(PublishArgs {
        path,
        project_name,
        out_dir,
        strict,
        dry_run,
        public,
        init,
        format,
    })
}

/// Slugify a directory name into a Cloudflare Pages project name: lowercase, runs of
/// non-alphanumerics collapse to one `-`, trimmed of leading/trailing `-`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// The facts shown in the [`preflight_summary`] block printed right before a deploy.
struct Preflight<'a> {
    /// Cloudflare Pages project name (also the `<project>.pages.dev` subdomain).
    project: &'a str,
    /// Display path of the freshly built tree that will be uploaded.
    out: &'a str,
    /// Whether the deploy carries the shared-passcode gate (the safe default).
    gated: bool,
    /// Whether `--strict` is in force (informs the checks-line wording).
    strict: bool,
    /// Located problems in the built tree — always `0` when a strict build reached here.
    problems: usize,
    /// `true` for `--dry-run` (nothing is uploaded), which only changes the verb.
    dry_run: bool,
}

/// The human pre-flight summary: one line per fact (target, source, access, checks), in
/// print order. Pure — so the gate/flip wording is unit-testable without a real (or dry-run)
/// deploy. Its whole job is to make an accidental PUBLIC (or accidental gate) impossible to
/// miss *before* the irreversible upload: the reported incident was a public blog shipped
/// passcode-gated and only noticed once it was live, because the default (gated) path prints
/// no confirmation at all.
fn preflight_summary(p: &Preflight) -> Vec<String> {
    let verb = if p.dry_run {
        "would deploy"
    } else {
        "deploying"
    };
    let access = if p.gated {
        "GATED — visitors need the shared passcode \
         (publish publicly with --public, or set publish.gate: false in _site.yml)"
    } else {
        "PUBLIC — no passcode; anyone with the URL can read it \
         (re-gate by dropping --public, or set publish.gate: true in _site.yml)"
    };
    let checks = if p.problems == 0 {
        "checks:  passed with no problems".to_string()
    } else {
        // Only reachable without --strict (a strict build with problems aborts before here).
        let _ = p.strict;
        format!(
            "checks:  {} problem{} shipped without --strict (--strict would fail the deploy instead)",
            p.problems,
            if p.problems == 1 { "" } else { "s" }
        )
    };
    vec![
        format!("pre-flight — {verb} to Cloudflare Pages:"),
        format!("  target:  https://{}.pages.dev", p.project),
        format!("  source:  {}", p.out),
        format!("  access:  {access}"),
        format!("  {checks}"),
    ]
}

/// The one-time Cloudflare Pages setup as `wrangler` argv, in run order: create the Pages
/// project, then — only for a **gated** site — put the shared passcode in place.
///
/// Pure, so the command set is unit-testable with no wrangler and no network. The
/// `project` it is given is resolved by [`cmd_publish`]'s ordinary rules, which is the
/// whole point of the flag: the setup and the deploy that follows it must name one
/// project, and a hand-typed setup line is exactly where that drifts.
///
/// **The gated case is a real branch, not a formality.** A `--public` site has no
/// passcode, so offering `secret put` there would teach the author to set a secret
/// nothing reads.
fn init_commands(project: &str, gated: bool) -> Vec<Vec<String>> {
    let s = str::to_string;
    let mut cmds = vec![vec![
        s("pages"),
        s("project"),
        s("create"),
        s(project),
        s("--production-branch"),
        s(PRODUCTION_BRANCH),
    ]];
    if gated {
        cmds.push(vec![
            s("pages"),
            s("secret"),
            s("put"),
            s(GATE_SECRET),
            s("--project-name"),
            s(project),
        ]);
    }
    cmds
}

/// `publish <dir> --init`: run [`init_commands`] and stop.
///
/// **It deliberately neither builds nor deploys.** Setup and deploy are separate one-way
/// steps, and folding them together would make a first `publish` do two irreversible
/// things on one command.
fn cmd_init(project: &str, gated: bool, dry_run: bool, json: bool) -> ExitCode {
    let cmds = init_commands(project, gated);
    if dry_run {
        for c in &cmds {
            let line = format!("would run: wrangler {}", c.join(" "));
            // `--format json` owns stdout for a build's diagnostics; `--init` has none to
            // emit, so keep stdout empty rather than non-JSON in that mode.
            if json {
                log::info(&line);
            } else {
                println!("{line}");
            }
        }
        return ExitCode::SUCCESS;
    }
    for c in &cmds {
        log::info(&format!("running: wrangler {}", c.join(" ")));
        // stdio is INHERITED, not captured: `secret put` prompts for the passcode on the
        // terminal, so capturing its output would hang on an invisible prompt.
        match std::process::Command::new("wrangler").args(c).status() {
            Ok(s) if s.success() => {}
            Ok(s) => {
                log::error(&format!(
                    "`wrangler {}` exited with status {s}",
                    c.join(" ")
                ));
                // Re-running setup is the common case (rotating the passcode), and
                // `project create` refuses an existing project — so name that reading
                // rather than leaving the author to guess which step to skip.
                if c.get(2).is_some_and(|v| v == "create") {
                    log::info(&format!(
                        "if the project already exists this is expected; the remaining step is \
                         `wrangler pages secret put {GATE_SECRET} --project-name {project}`"
                    ));
                }
                return ExitCode::FAILURE;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::error(
                    "wrangler was not found on PATH. Install it (npm install -g wrangler), \
                     then re-run this command.",
                );
                return ExitCode::FAILURE;
            }
            Err(e) => {
                log::error(&format!("cannot run wrangler: {e}"));
                return ExitCode::FAILURE;
            }
        }
    }
    log::info("setup complete — deploy with `taliesin publish <dir>`");
    ExitCode::SUCCESS
}

/// Write the passcode gate into `<out>/functions/_middleware.js`. Called AFTER the build
/// (the build's stale-sweep would otherwise delete the `functions/` dir, which is neither
/// dot- nor underscore-prefixed); re-injected on every publish.
fn inject_gate(out: &Path) -> std::io::Result<()> {
    let dir = out.join("functions");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("_middleware.js"), MIDDLEWARE_JS)
}

/// `publish <dir>`: build the site, inject the passcode gate, deploy to Cloudflare Pages.
pub(crate) fn cmd_publish(args: &[String]) -> ExitCode {
    let PublishArgs {
        path,
        project_name,
        out_dir,
        strict,
        dry_run,
        public,
        init,
        format,
    } = match parse_publish_args(args) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };

    let root = Path::new(path);
    if !root.is_dir() {
        log::error(&format!(
            "publish builds a site or book (a directory with _site.yml); `{path}` is not a directory. \
             For a single document, use `taliesin build {path}` and host the output yourself."
        ));
        return ExitCode::FAILURE;
    }

    // Fail fast (before the build) when a real deploy is missing its credential.
    if !dry_run && std::env::var_os("CLOUDFLARE_API_TOKEN").is_none() {
        log::error(
            "CLOUDFLARE_API_TOKEN is not set (a non-interactive deploy needs it). \
             Create a token with the Cloudflare Pages:Edit permission, then export \
             CLOUDFLARE_API_TOKEN (and CLOUDFLARE_ACCOUNT_ID). Use --dry-run to build without deploying.",
        );
        return ExitCode::FAILURE;
    }

    // Discover the site once to resolve the project name + the output dir.
    let site = taliesin_core::Site::discover(root);

    // Precedence: --public wins, else publish.gate: in _site.yml, else gated (safe default).
    let gated = if public {
        false
    } else {
        site.config
            .publish
            .as_ref()
            .and_then(|p| p.gate)
            .unwrap_or(true)
    };
    if let Some(publish) = &site.config.publish
        && let Some(provider) = &publish.provider
        && provider != "cloudflare"
    {
        log::error(&format!(
            "publish provider `{provider}` is not supported (only `cloudflare`)."
        ));
        return ExitCode::FAILURE;
    }

    let dir_name = root
        .canonicalize()
        .ok()
        .and_then(|c| c.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let project = project_name
        .map(str::to_string)
        .or_else(|| site.config.publish.as_ref().and_then(|p| p.project.clone()))
        .unwrap_or_else(|| slug(&dir_name));
    if project.is_empty() {
        log::error(
            "cannot derive a Cloudflare project name from the directory; \
             pass --project-name <name> or set publish.project in _site.yml.",
        );
        return ExitCode::FAILURE;
    }

    // `--init` is the one-time setup, and it stops here: BEFORE the output dir is resolved
    // and before anything is built. Placed after the project/gate resolution above so it
    // creates exactly the project a later deploy will target, under exactly that deploy's
    // access model.
    let json = format == "json";
    if init {
        return cmd_init(&project, gated, dry_run, json);
    }

    // Resolve the output dir the same way the build does, so we can inject + deploy from it.
    let out: PathBuf = out_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(site.output_dir()));

    // Build (reuses the full site build, including its own discover + strict handling).
    let outcome = crate::build::run_site_build(root, out.to_str(), strict, None);
    if json {
        // Structured diagnostics to stdout (human log stays on stderr).
        println!("{}", crate::check::diagnostics_json(&outcome.diagnostics));
    }
    if !outcome.ok {
        return ExitCode::FAILURE;
    }
    let out = out.canonicalize().unwrap_or(out);

    // Pre-flight summary (DX15): once the build has succeeded and we are actually about to
    // deploy, print target + source + access + checks, so an accidental gate/target is caught
    // *before* the irreversible upload. Always printed (both gated and public, dry-run too);
    // log::info goes to stderr so a `--format json` stream on stdout stays pure.
    for line in preflight_summary(&Preflight {
        project: &project,
        out: &out.display().to_string(),
        gated,
        strict,
        problems: outcome.diagnostics.len(),
        dry_run,
    }) {
        log::info(&line);
    }

    // Keep a loud WARN for the dangerous case: the summary already names PUBLIC, but a
    // public deploy is the one that leaks, so it earns a second, unmissable line.
    if !gated {
        log::warn("publishing WITHOUT a passcode gate: this site will be PUBLIC");
    }

    // Inject the passcode gate into the freshly built tree (unless deploying public).
    if gated && let Err(e) = inject_gate(&out) {
        log::error(&format!(
            "cannot write the passcode gate to {}: {e}",
            out.join("functions/_middleware.js").display()
        ));
        return ExitCode::FAILURE;
    }

    let cmd = format!(
        "wrangler pages deploy . --project-name {project} --branch {PRODUCTION_BRANCH} --commit-dirty=true"
    );
    if dry_run {
        let gate_note = if gated { "gated" } else { "PUBLIC (no gate)" };
        log::info(&format!(
            "built + {gate_note} {} (not deployed)",
            out.display()
        ));
        // In `--format json` the diagnostics were already printed to stdout; keep the
        // "would run" line on stderr so the JSON stream stays pure.
        if json {
            log::info(&format!("would run (cwd {}): {cmd}", out.display()));
        } else {
            println!("would run (cwd {}): {cmd}", out.display());
        }
        return ExitCode::SUCCESS;
    }

    log::info(&format!(
        "deploying {} to Cloudflare Pages ({project})",
        out.display()
    ));
    let status = std::process::Command::new("wrangler")
        .current_dir(&out)
        .args([
            "pages",
            "deploy",
            ".",
            "--project-name",
            &project,
            "--branch",
            PRODUCTION_BRANCH,
            "--commit-dirty=true",
        ])
        .status();
    match status {
        Ok(s) if s.success() => {
            // In `--format json` the diagnostics own stdout; keep the human "published" line
            // on stderr so a `publish … --format json | jq` stream stays pure JSON.
            let published = format!("published: https://{project}.pages.dev");
            if json {
                log::info(&published);
            } else {
                println!("{published}");
            }
            ExitCode::SUCCESS
        }
        Ok(s) => {
            log::error(&format!("wrangler exited with status {s}"));
            ExitCode::FAILURE
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::error(
                "wrangler was not found on PATH. Install it (npm install -g wrangler) and \
                 run the one-time setup: `wrangler pages project create <name> \
                 --production-branch production` then `wrangler pages secret put PASSWORD \
                 --project-name <name>`.",
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            log::error(&format!("cannot run wrangler: {e}"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_lowercases_and_dashes_non_alphanumerics() {
        assert_eq!(slug("FL-Weather"), "fl-weather");
        assert_eq!(slug("invertible speech"), "invertible-speech");
        assert_eq!(slug("My_Book!!"), "my-book");
        assert_eq!(slug("a---b"), "a-b");
        assert_eq!(slug("...."), "");
    }

    fn argv(rest: &[&str]) -> Vec<String> {
        let mut v = vec!["taliesin".to_string(), "publish".to_string()];
        v.extend(rest.iter().map(|s| s.to_string()));
        v
    }

    #[test]
    fn parses_path_and_flags() {
        let a = argv(&["book", "--project-name", "my-book", "--dry-run", "--strict"]);
        let p = parse_publish_args(&a).expect("parse");
        assert_eq!(p.path, "book");
        assert_eq!(p.project_name, Some("my-book"));
        assert!(p.dry_run);
        assert!(p.strict);
        assert_eq!(p.out_dir, None);
    }

    #[test]
    fn missing_path_is_an_error() {
        assert!(parse_publish_args(&argv(&["--dry-run"])).is_err());
    }

    /// PL5: `--json` is the clig.dev shorthand for `--format json`, so neither spelling
    /// dead-ends on a machine-output command.
    #[test]
    fn json_is_an_alias_for_format_json() {
        assert_eq!(
            parse_publish_args(&argv(&["book", "--json"]))
                .unwrap()
                .format,
            "json"
        );
        assert_eq!(
            parse_publish_args(&argv(&["book", "--format", "json"]))
                .unwrap()
                .format,
            "json"
        );
        assert_eq!(
            parse_publish_args(&argv(&["book"])).unwrap().format,
            "human"
        );
    }

    /// PL18: a bad `--format` value uses the one shared wording (`serve::bad_format_error`),
    /// so the same mistake reads identically on every subcommand.
    #[test]
    fn bad_format_value_uses_the_shared_wording() {
        let err = parse_publish_args(&argv(&["book", "--format", "xml"])).unwrap_err();
        assert_eq!(err, "error: --format expects human or json (got xml)");
    }

    #[test]
    fn unknown_flag_is_an_error() {
        let err = parse_publish_args(&argv(&["book", "--projct-name", "x"])).unwrap_err();
        assert!(err.contains("--projct-name"), "{err}");
    }

    #[test]
    fn project_name_flag_requires_a_value() {
        assert!(parse_publish_args(&argv(&["book", "--project-name"])).is_err());
    }

    #[test]
    fn strict_is_the_default() {
        let a = argv(&["book"]);
        let p = parse_publish_args(&a).expect("parse");
        assert!(p.strict, "publish must be strict by default");
        assert!(!p.public);
        assert!(
            !p.init,
            "publish builds + deploys unless --init asks for setup"
        );
    }

    #[test]
    fn init_is_opt_in() {
        let a = argv(&["book", "--init"]);
        assert!(parse_publish_args(&a).expect("parse").init);
    }

    /// Order matters as much as membership: `project create` has to precede
    /// `secret put`, since the secret is put *on* the project.
    #[test]
    fn init_creates_the_project_then_sets_the_passcode() {
        let cmds = init_commands("my-book", true);
        assert_eq!(
            cmds,
            vec![
                vec![
                    "pages",
                    "project",
                    "create",
                    "my-book",
                    "--production-branch",
                    "production"
                ],
                vec![
                    "pages",
                    "secret",
                    "put",
                    "PASSWORD",
                    "--project-name",
                    "my-book"
                ],
            ]
        );
    }

    #[test]
    fn init_for_a_public_project_creates_it_and_sets_no_secret() {
        let cmds = init_commands("my-book", false);
        assert_eq!(
            cmds.len(),
            1,
            "no passcode step for an ungated site: {cmds:?}"
        );
        assert!(cmds[0].contains(&"create".to_string()));
    }

    /// The deploy's `--branch` and the setup's `--production-branch` must be one label, or
    /// every deploy lands on a preview branch of a project whose production branch is
    /// something else — a live site that never updates.
    #[test]
    fn init_creates_the_branch_the_deploy_pushes_to() {
        let create = &init_commands("b", false)[0];
        let at = create
            .iter()
            .position(|a| a == "--production-branch")
            .unwrap();
        assert_eq!(create[at + 1], PRODUCTION_BRANCH);
    }

    /// A two-way pin on the secret's name: `--init` puts it in place and the bundled
    /// middleware reads it, so renaming either alone yields a gate nobody can open.
    #[test]
    fn the_gate_secret_is_the_one_the_middleware_reads() {
        assert!(
            MIDDLEWARE_JS.contains(&format!("env.{GATE_SECRET}")),
            "the middleware reads a different env var than `--init` sets"
        );
        assert!(
            init_commands("b", true)[1].contains(&GATE_SECRET.to_string()),
            "the setup sets a different secret than the middleware reads"
        );
    }

    #[test]
    fn no_strict_opts_out_and_public_opts_in() {
        let a = argv(&["book", "--no-strict", "--public"]);
        let p = parse_publish_args(&a).expect("parse");
        assert!(!p.strict);
        assert!(p.public);
    }

    #[test]
    fn strict_flags_are_last_wins() {
        let av = argv(&["book", "--no-strict", "--strict"]);
        let a = parse_publish_args(&av).expect("parse");
        assert!(a.strict, "--strict after --no-strict wins");
        let bv = argv(&["book", "--strict", "--no-strict"]);
        let b = parse_publish_args(&bv).expect("parse");
        assert!(!b.strict, "--no-strict after --strict wins");
    }

    #[test]
    fn public_typo_still_did_you_means() {
        let err = parse_publish_args(&argv(&["book", "--publik"])).unwrap_err();
        assert!(err.contains("--publik"), "{err}");
    }

    // DX15: a pre-flight summary printed before the (irreversible) deploy, so an accidental
    // gate/target is caught before the upload rather than after (the reported incident was a
    // public blog shipped passcode-gated, only noticed once it was live).

    fn preflight(gated: bool, strict: bool, problems: usize, dry_run: bool) -> Vec<String> {
        preflight_summary(&Preflight {
            project: "my-blog",
            out: "/tmp/site/_site",
            gated,
            strict,
            problems,
            dry_run,
        })
    }

    #[test]
    fn preflight_names_the_target_and_source() {
        let s = preflight(true, true, 0, false).join("\n");
        assert!(s.contains("my-blog.pages.dev"), "target URL missing:\n{s}");
        assert!(s.contains("/tmp/site/_site"), "source dir missing:\n{s}");
    }

    #[test]
    fn preflight_gated_names_the_exact_flip_to_public() {
        // The whole point of DX15: the default (gated) path must say, unmissably, that the
        // site is gated AND how to make it public — that path prints nothing today.
        let s = preflight(true, true, 0, false).join("\n");
        assert!(s.contains("GATED"), "gate status missing:\n{s}");
        assert!(s.contains("--public"), "--public flip missing:\n{s}");
        assert!(
            s.contains("publish.gate: false"),
            "_site.yml flip missing:\n{s}"
        );
        assert!(
            !s.contains("PUBLIC"),
            "gated summary must not say PUBLIC:\n{s}"
        );
    }

    #[test]
    fn preflight_public_names_the_exact_flip_to_gated() {
        let s = preflight(false, true, 0, false).join("\n");
        assert!(s.contains("PUBLIC"), "public status missing:\n{s}");
        // The reverse flip (how to add the gate back) must be spelled out too.
        assert!(
            s.contains("publish.gate: true"),
            "_site.yml re-gate missing:\n{s}"
        );
    }

    #[test]
    fn preflight_reports_shipped_problems_without_strict() {
        let clean = preflight(true, true, 0, false).join("\n");
        assert!(clean.contains("no problems"), "clean checks line:\n{clean}");
        let dirty = preflight(true, false, 3, false).join("\n");
        assert!(dirty.contains('3'), "problem count missing:\n{dirty}");
        assert!(
            dirty.contains("--strict"),
            "should point at --strict:\n{dirty}"
        );
    }

    #[test]
    fn preflight_verb_tracks_dry_run() {
        assert!(
            preflight(true, true, 0, true).join("\n").contains("would"),
            "dry-run should read as a would-deploy"
        );
        assert!(
            !preflight(true, true, 0, false).join("\n").contains("would"),
            "a real deploy is not a would-deploy"
        );
    }
}
