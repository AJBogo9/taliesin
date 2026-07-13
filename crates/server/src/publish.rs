//! The `publish` subcommand: build a site/book and deploy it to Cloudflare Pages
//! (Wrangler direct upload) behind a shared passcode.
//!
//! **What:** `publish <dir>` builds the project (reusing the site build), writes a
//! bundled `functions/_middleware.js` HTTP Basic-Auth gate into the output tree, then
//! runs `wrangler pages deploy` from that tree. One-way: it never writes to the source.
//!
//! **How to use:** `main()` dispatches `publish` to [`cmd_publish`].
//!
//! **One-time setup (per repo, documented, not automated here):**
//! `wrangler pages project create <name> --production-branch production` and
//! `wrangler pages secret put PASSWORD --project-name <name>`, with `CLOUDFLARE_API_TOKEN`
//! (and `CLOUDFLARE_ACCOUNT_ID`) in the environment.

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

/// Long flags `publish` accepts (drives the unknown-flag did-you-mean).
const PUBLISH_FLAGS: &[&str] = &[
    "--project-name",
    "--out",
    "--strict",
    "--no-strict",
    "--public",
    "--dry-run",
    "--format",
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
    let mut format: &str = "human";
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--format" => match it.next().map(|s| s.as_str()) {
                Some(v) if v == "human" || v == "json" => format = v,
                other => {
                    return Err(format!(
                        "error: --format expects human or json (got {})",
                        other.unwrap_or("nothing")
                    ));
                }
            },
            "--project-name" => match it.next().map(|s| s.as_str()) {
                Some(v) if !v.starts_with("--") => project_name = Some(v),
                _ => {
                    return Err(
                        "error: --project-name requires a value (e.g. --project-name my-book)"
                            .to_string(),
                    );
                }
            },
            "--out" | "--dir" => match it.next().map(|s| s.as_str()) {
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
            s if s.starts_with("--") => {
                return Err(format!(
                    "error: {}",
                    crate::serve::unknown_flag_error(s, PUBLISH_FLAGS)
                ));
            }
            s => positionals.push(s),
        }
    }
    let path = positionals.first().copied().ok_or_else(|| {
        "usage: taliesin publish <dir> [--project-name <name>] [--out <dir>] [--public] [--no-strict] [--dry-run]"
            .to_string()
    })?;
    Ok(PublishArgs {
        path,
        project_name,
        out_dir,
        strict,
        dry_run,
        public,
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

    // Resolve the output dir the same way the build does, so we can inject + deploy from it.
    let out: PathBuf = out_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(site.output_dir()));

    // Build (reuses the full site build, including its own discover + strict handling).
    let json = format == "json";
    let outcome = crate::build::run_site_build(root, out.to_str(), strict, None);
    if json {
        // Structured diagnostics to stdout (human log stays on stderr).
        println!("{}", crate::check::diagnostics_json(&outcome.diagnostics));
    }
    if !outcome.ok {
        return ExitCode::FAILURE;
    }
    let out = out.canonicalize().unwrap_or(out);

    // Warn only once the build succeeded and we are actually about to deploy, so an
    // aborted run (strict failure, bad provider) does not print a false PUBLIC alarm.
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
}
