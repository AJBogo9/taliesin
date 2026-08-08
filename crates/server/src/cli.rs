//! Front-door subcommands: `init` (scaffold a starter site), `new` (scaffold one document)
//! and `preview` (launch the live preview server).
//!
//! **What:** `init` writes a minimal previewable site (`_site.yml` + `index.tmd` +
//! the `.taliesin/` editor onramp); `cmd_serve` parses the preview flags
//! (`--open`/`--host`/`--no-exec`/port) and starts the dev server.
//!
//! **How to use:** `main()` dispatches `init`, `new` and `preview` to `cmd_init` /
//! `cmd_new` / `cmd_serve` here. The `serve`/`dev` spellings of `preview` were retired in
//! Wave 5, which is why `cmd_serve` keeps a name its verb no longer has.
//!
//! **Depends on:** [`crate::serve_site`] (the dev server), [`crate::serve`] (its shared
//! plumbing + CLI error helpers) and [`crate::log`].

use crate::{interactive, log, serve, serve_site};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// `_site.yml` for the scaffold: the schema modeline (so an editor's YAML language server
/// validates + autocompletes config keys with zero manual step — the schema is emitted into
/// `.taliesin/` beside it) followed by the minimal flat-native config (just a title).
const INIT_SITE_YML: &str =
    "# yaml-language-server: $schema=.taliesin/tali-site.schema.json\ntitle: My site\n";

/// `index.tmd` for the scaffold: a hello-world page that previews immediately and
/// points the new user at the next steps. `.tmd` is the native extension.
const INIT_INDEX_TMD: &str = "---\ntitle: Hello, Taliesin\n---\n\n\
    Welcome to your new [Taliesin](https://github.com/AJBogo9/taliesin) site.\n\n\
    Edit `index.tmd` and the preview reloads as you save.\n\n\
    ## Next steps\n\n\
    - Scaffold a post, paper, or deck with `taliesin new` (e.g. `taliesin new post my-first-post`; add `--draft` to hold it back).\n\
    - Add more `.tmd` pages beside this one: each becomes its own page.\n\
    - Configure navigation and the title in `_site.yml`.\n\
    - Drop in a `{python}` or `{r}` code cell to run live output.\n";

/// `_site.yml` for the `site` template: the schema modeline + a title and a two-item
/// top nav wiring the two starter pages. Byte-pinned by `corpus/scaffold-site/`.
const SITE_SITE_YML: &str = r#"# yaml-language-server: $schema=.taliesin/tali-site.schema.json
title: My site
nav:
  left:
  - text: Home
    href: index.tmd
  - text: About
    href: about.tmd
"#;

/// The `site` template's home page: explains the multi-page/nav model and points at the
/// next moves. Byte-pinned by `corpus/scaffold-site/index.tmd`.
const SITE_INDEX_TMD: &str = r#"---
title: Home
---

Welcome to your new [Taliesin](https://github.com/AJBogo9/taliesin) site. This is a
multi-page site: each `.tmd` file beside this one becomes its own page, and the `nav:`
in `_site.yml` links them across the top.

Edit `index.tmd` and the preview reloads as you save.

## Next steps

- Add a page beside this one and link it from `nav:` in `_site.yml`.
- Scaffold a blog post with `taliesin new post my-first-post` (add `--draft` to hold it back).
- Drop in a `{python}` or `{r}` code cell to run live output.
"#;

/// The `site` template's About stub. Byte-pinned by `corpus/scaffold-site/about.tmd`.
const SITE_ABOUT_TMD: &str = r#"---
title: About
---

Say who you are and what this site is about. Edit `about.tmd`, or delete it and remove
its link from `nav:` in `_site.yml`.
"#;

/// `_site.yml` for the `book` template: `chapters:` (which makes it a book) plus title and
/// author. No `toc:` — it is inert in a book (item 76), and scaffolding a key the tool then
/// warns about is the worst of both. Byte-pinned by `corpus/scaffold-book/`.
const BOOK_SITE_YML: &str = r#"# yaml-language-server: $schema=.taliesin/tali-site.schema.json
title: My book
author: Your Name
chapters:
  - index.tmd
  - intro.tmd
  - methods.tmd
"#;

/// The `book` template's landing page: a preface whose auto-generated table of contents
/// (the book-landing TOC) lists the chapters. Byte-pinned by `corpus/scaffold-book/index.tmd`.
const BOOK_INDEX_TMD: &str = r#"# Preface {.unnumbered}

This is your book's landing page. Write a short preface here; Taliesin generates the
table of contents below from the chapters listed in `_site.yml`.
"#;

/// The `book` template's first chapter. Byte-pinned by `corpus/scaffold-book/intro.tmd`.
const BOOK_INTRO_TMD: &str = r#"# Introduction

Open your book here. Each chapter is a `.tmd` file listed under `chapters:` in
`_site.yml`; Taliesin numbers them and builds the sidebar and previous/next
navigation for you.
"#;

/// The `book` template's second chapter, showing a cross-referenceable section anchor.
/// Byte-pinned by `corpus/scaffold-book/methods.tmd`.
const BOOK_METHODS_TMD: &str = r#"# Methods {#sec-methods}

Write one chapter per `.tmd` file. This heading has an id, so you can cross-reference
it as @sec-methods from any chapter. Drop in a `{python}` or `{r}` cell to compute a
result inline.
"#;

/// Which starter `init` scaffolds. `Basic` is the frozen one-page site (the historical
/// default); `Site` and `Book` add the two multi-page project shapes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum InitTemplate {
    Basic,
    Site,
    Book,
}

/// The template names, for the unknown-template did-you-mean and `--template` help.
const INIT_TEMPLATES: &[&str] = &["basic", "site", "book"];

/// The `init` wizard's template picker: a friendly one-line label per template, index-aligned
/// with the value it selects.
const INIT_TEMPLATE_MENU: [(&str, InitTemplate); 3] = [
    ("basic  — a one-page site", InitTemplate::Basic),
    (
        "site   — a multi-page site with a top nav",
        InitTemplate::Site,
    ),
    (
        "book   — chapters with a sidebar and TOC",
        InitTemplate::Book,
    ),
];

impl InitTemplate {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "basic" => Ok(Self::Basic),
            "site" => Ok(Self::Site),
            "book" => Ok(Self::Book),
            other => Err(match taliesin_core::closest(other, INIT_TEMPLATES) {
                Some(t) => format!("unknown template `{other}` (did you mean `{t}`?)"),
                None => format!("unknown template `{other}` (expected basic, site, or book)"),
            }),
        }
    }
}

/// The authored files a `taliesin init --template <t>` writes (config + pages), as
/// `(project-relative path, contents)`. Pure, so the corpus pins can compare the bytes
/// exactly (`corpus/scaffold-{site,book}/`) and the CLI stays a thin wrapper. The shared
/// onramp (the `.taliesin/` schema) is appended by [`scaffold_init`], not
/// here, since it is a generated constant already golden-locked in core.
pub(crate) fn init_files(template: InitTemplate) -> Vec<(PathBuf, String)> {
    let files: &[(&str, &str)] = match template {
        InitTemplate::Basic => &[("_site.yml", INIT_SITE_YML), ("index.tmd", INIT_INDEX_TMD)],
        InitTemplate::Site => &[
            ("_site.yml", SITE_SITE_YML),
            ("index.tmd", SITE_INDEX_TMD),
            ("about.tmd", SITE_ABOUT_TMD),
        ],
        InitTemplate::Book => &[
            ("_site.yml", BOOK_SITE_YML),
            ("index.tmd", BOOK_INDEX_TMD),
            ("intro.tmd", BOOK_INTRO_TMD),
            ("methods.tmd", BOOK_METHODS_TMD),
        ],
    };
    files
        .iter()
        .map(|(name, contents)| (PathBuf::from(name), contents.to_string()))
        .collect()
}

/// Every long flag `init` accepts (drives the unknown-flag did-you-mean).
const INIT_FLAGS: &[&str] = &["--json", "--format", "--template", "--yes"];

/// `taliesin init [dir] [--json]`: scaffold a minimal previewable site into `dir` (default
/// the current directory). Writes `_site.yml` + `index.tmd` + `.taliesin/` (the editor
/// onramp), then prints the preview hint (or, with `--json`, a `{created, preview}` receipt).
pub(crate) fn cmd_init(args: &[String]) -> ExitCode {
    let mut dir_arg: Option<&str> = None;
    let mut json = false;
    let mut yes = false;
    // `None` = no `--template` given, so a human at a TTY is asked which starter to scaffold.
    let mut template: Option<InitTemplate> = None;
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json = true,
            // `--format json` / `--format human`: accept the long spelling too (json is the
            // shorthand), so `init --format json` doesn't dead-end.
            "--format" => match it.next().map(|s| s.as_str()) {
                Some("json") => json = true,
                Some("human") => json = false,
                other => {
                    log::error(&serve::bad_format_error(other));
                    return ExitCode::FAILURE;
                }
            },
            // `--template basic|site|book`: which starter to scaffold. An unknown value gets a
            // did-you-mean; omitted, a human at a TTY is prompted (else it defaults to basic).
            "--template" => match it.next() {
                Some(v) => match InitTemplate::parse(v) {
                    Ok(t) => template = Some(t),
                    Err(e) => {
                        log::error(&e);
                        return ExitCode::FAILURE;
                    }
                },
                None => {
                    log::error("--template needs a value (basic, site, or book)");
                    return ExitCode::FAILURE;
                }
            },
            // `-y`/`--yes` skips the interactive wizard (basic into the given/current dir).
            "-y" | "--yes" => yes = true,
            s if s.starts_with("--") => {
                log::error(&serve::unknown_flag_error(s, INIT_FLAGS));
                return ExitCode::FAILURE;
            }
            s if dir_arg.is_none() => dir_arg = Some(s),
            _ => {}
        }
    }

    // With a piece missing, a human at a TTY is prompted; a pipe/CI/agent (or `-y`/`--json`)
    // takes the historical defaults (basic, into the given or current dir) with no prompt.
    let interactive = interactive::is_interactive(yes, json);
    let template = match template {
        Some(t) => t,
        None if interactive => {
            let labels: Vec<&str> = INIT_TEMPLATE_MENU.iter().map(|(l, _)| *l).collect();
            match interactive::select("What kind of project?", &labels, 0) {
                Ok(i) => INIT_TEMPLATE_MENU[i].1,
                Err(e) => {
                    log::error(&e.to_string());
                    return ExitCode::FAILURE;
                }
            }
        }
        None => InitTemplate::Basic,
    };
    let dir_owned: String = match dir_arg {
        Some(d) => d.to_string(),
        None if interactive => match interactive::input("Directory", Some("."), |_| Ok(())) {
            Ok(d) => d,
            Err(e) => {
                log::error(&e.to_string());
                return ExitCode::FAILURE;
            }
        },
        None => ".".to_string(),
    };

    let dir = Path::new(&dir_owned);
    let where_ = if dir == Path::new(".") {
        ".".to_string()
    } else {
        dir.display().to_string()
    };
    match scaffold_init(dir, template) {
        Ok(written) => {
            if json {
                let created: Vec<String> =
                    written.iter().map(|p| p.display().to_string()).collect();
                let payload = serde_json::json!({
                    "created": created,
                    "preview": format!("taliesin preview {where_}"),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
                );
            } else {
                for f in &written {
                    log::built(&f.display().to_string());
                }
                println!("Scaffolded a Taliesin site. Preview it:\n  taliesin preview {where_}");
                // The onramp file is the one nobody asked for. It was listed above as a
                // bare path like the rest, which is how a dot-directory arrives in a new
                // project with nothing anywhere saying what it is or that it can go.
                // Naming it is the whole fix — it is a good file, just unexplained.
                println!("\n{ONRAMP_NOTE}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&e);
            ExitCode::FAILURE
        }
    }
}

/// The editor onramp every scaffolded project gets regardless of template: the bundled
/// `_site.yml` schema, wired via the config's own modeline. Kept out of [`init_files`] (and
/// the corpus pins) because it is a generated constant already locked in core, not authored
/// template bytes.
/// The one line that names the file `init` writes which the user did not ask for.
///
/// Item 123: it was listed as a bare path beside `_site.yml` and `index.tmd`, so a
/// scaffolded project acquired a dot-directory with nothing stating what it was for, or
/// that deleting it costs nothing. It is genuinely useful, which is exactly why the fix is
/// a sentence rather than a flag: an `--onramp` knob would be a configuration answer to a
/// documentation problem.
const ONRAMP_NOTE: &str = "\
.taliesin/ holds the schema your editor reads for `_site.yml` completion. It is not
required — delete it and everything still builds.";

fn onramp_files() -> [(&'static str, &'static str); 1] {
    [
        // The bundled config schema, wired into `_site.yml` via the modeline. In a walker-
        // skipped dot-dir so it never becomes a page or ships into `_site/`. `_site.yml` is
        // the one YAML surface `taliesin lsp` does not serve, so this is the only editor
        // intelligence an author gets while editing it.
        (
            ".taliesin/tali-site.schema.json",
            taliesin_core::schema::SITE_SCHEMA,
        ),
    ]
}

/// Scaffold the `template` starter into `dir`, creating it if needed: the template's authored
/// files (config + pages) plus the shared onramp. Refuses to overwrite an existing file (so
/// re-running `init` never clobbers the user's work) and returns the paths written.
fn scaffold_init(dir: &Path, template: InitTemplate) -> Result<Vec<PathBuf>, String> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Err(format!("cannot create {}: {e}", dir.display()));
    }
    let mut files = init_files(template);
    files.extend(
        onramp_files()
            .into_iter()
            .map(|(name, contents)| (PathBuf::from(name), contents.to_string())),
    );
    write_scaffold(dir, &files)
}

/// Write `files` (project-relative path → contents) under `root`, refusing to overwrite any
/// existing target before writing any of them and creating parent dirs as needed. Shared by
/// `init` (project scaffold) and `new` (document scaffold); returns the paths written.
fn write_scaffold(root: &Path, files: &[(PathBuf, String)]) -> Result<Vec<PathBuf>, String> {
    // Refuse to overwrite *any* target before writing *any*, so a partial scaffold never
    // lands on top of an existing project.
    for (rel, _) in files {
        let path = root.join(rel);
        if path.exists() {
            return Err(format!(
                "{} already exists; refusing to overwrite",
                path.display()
            ));
        }
    }
    let mut written = Vec::new();
    for (rel, contents) in files {
        let path = root.join(rel);
        // Nested targets (`.taliesin/…`, `posts/<slug>/…`) need their parent created first.
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return Err(format!("cannot create {}: {e}", parent.display()));
        }
        if let Err(e) = std::fs::write(&path, contents) {
            return Err(format!("cannot write {}: {e}", path.display()));
        }
        written.push(path);
    }
    Ok(written)
}

/// What `taliesin new` can scaffold. Each maps to a front-matter shape; most write one
/// file, `Paper` writes two (its `index.tmd` + a matching `references.bib`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum NewKind {
    Post,
    Page,
    Deck,
    Paper,
}

/// The kind names, for the unknown-kind did-you-mean.
pub(crate) const NEW_KINDS: &[&str] = &["post", "page", "deck", "paper"];

impl NewKind {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "post" => Ok(Self::Post),
            "page" => Ok(Self::Page),
            "deck" => Ok(Self::Deck),
            "paper" => Ok(Self::Paper),
            other => Err(match taliesin_core::closest(other, NEW_KINDS) {
                Some(k) => format!("unknown kind `{other}` (did you mean `{k}`?)"),
                None => format!("unknown kind `{other}` (expected post, page, deck, or paper)"),
            }),
        }
    }

    /// The canonical kind name (for `--json` output).
    fn name(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::Page => "page",
            Self::Deck => "deck",
            Self::Paper => "paper",
        }
    }
}

/// A slug names a file inside the project, so it may not climb out of it or reach into a
/// subdirectory. Kept to the characters a URL wants anyway, which is what a page's path
/// becomes.
fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.is_empty() {
        return Err("the slug is empty (try `taliesin new post my-first-post`)".to_string());
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(format!(
            "invalid slug `{slug}`: use lowercase letters, digits and hyphens \
             (it becomes the page's URL)"
        ));
    }
    Ok(())
}

/// `my-first-post` -> `My First Post`, the title an author would have typed anyway.
fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(c) => c.to_ascii_uppercase().to_string() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Today's date as `YYYY-MM-DD`, **UTC**. Taliesin has no date dependency and does not
/// want one for this (see the backlog's library-outsourcing ruling), so the civil date is
/// derived from the Unix day number directly. Near midnight this can name yesterday or
/// tomorrow in the author's local zone; the date is front matter they can edit.
fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 -> `(year, month, day)`. Howard Hinnant's `civil_from_days`,
/// exact for every date this program can see. Unit-tested against known days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Per-invocation options for `taliesin new` (beyond kind + slug). `Default` is today's
/// behavior, so an unflagged scaffold is byte-identical to before (the corpus pin holds).
#[derive(Clone, Copy, Default)]
pub(crate) struct NewOpts {
    /// `--draft`: mark the scaffold `draft: true`, holding it out of the published build.
    pub(crate) draft: bool,
}

/// The files a `taliesin new <kind> <slug>` writes, as `(project-relative path, contents)`.
///
/// Pure, so the corpus pin can compare the bytes exactly (`corpus/scaffold/`) and the CLI
/// can stay a thin wrapper. Every front-matter key here is one the validator knows; a
/// `check`-clean scaffold is asserted by `crates/server/tests/new_cli.rs`, and the emitted
/// documents are rendered and linted by the corpus regression net like any other document.
pub(crate) fn new_files(
    kind: NewKind,
    slug: &str,
    today: &str,
    opts: NewOpts,
) -> Vec<(PathBuf, String)> {
    let title = title_from_slug(slug);
    // `--draft` splices a `draft: true` line into the front matter (right after `title:`);
    // default off emits nothing, keeping the unflagged scaffold byte-identical.
    let draft = if opts.draft { "draft: true\n" } else { "" };
    // A research paper scaffolds TWO files: a citation-wired doc plus the `.bib` its one
    // `[@key]` resolves against, so `check` is clean on the first save (a declared-but-
    // missing bibliography, or a citation with no bibliography, would both warn).
    if kind == NewKind::Paper {
        let index = format!(
            "---\n\
             title: \"{title}\"\n{draft}\
             date: {today}\n\
             description: \"One sentence: the claim this paper makes.\"\n\
             categories: [research]\n\
             bibliography: [references.bib]\n\
             ---\n\
             \n\
             State your claim in the first paragraph, then support it. Cite prior work with\n\
             `[@key]` syntax, which resolves against `references.bib` — for example the\n\
             literate-programming idea [@knuth1984literate]. @sec-methods works a figure end\n\
             to end.\n\
             \n\
             ## Methods {{#sec-methods}}\n\
             \n\
             A `{{python}}` cell runs when you preview (with a kernel) and renders its figure\n\
             inline. Quarto's cell options work verbatim: `#| label:` names it — a `fig-` prefix\n\
             makes it a figure — and `#| fig-cap:` is its caption, so `@fig-demo` cross-references\n\
             resolve automatically.\n\
             \n\
             ```{{python}}\n\
             #| label: fig-demo\n\
             #| fig-cap: \"A worked figure — replace it with your result.\"\n\
             import matplotlib.pyplot as plt\n\
             \n\
             fig, ax = plt.subplots()\n\
             ax.plot([0, 1, 2, 3], [0, 1, 4, 9])\n\
             ax.set_xlabel(\"x\")\n\
             ax.set_ylabel(\"y\")\n\
             ```\n\
             \n\
             @fig-demo shows the result. Display math uses `$$`:\n\
             \n\
             $$\n\
             y = x^2\n\
             $$\n\
             \n\
             ## References\n"
        );
        let bib = "@article{knuth1984literate,\n\
             \x20 author  = {Knuth, Donald E.},\n\
             \x20 title   = {Literate Programming},\n\
             \x20 journal = {The Computer Journal},\n\
             \x20 volume  = {27},\n\
             \x20 number  = {2},\n\
             \x20 pages   = {97--111},\n\
             \x20 year    = {1984}\n\
             }\n"
        .to_string();
        return vec![
            (PathBuf::from("posts").join(slug).join("index.tmd"), index),
            (
                PathBuf::from("posts").join(slug).join("references.bib"),
                bib,
            ),
        ];
    }
    let (path, body) = match kind {
        NewKind::Post => (
            PathBuf::from("posts").join(slug).join("index.tmd"),
            format!(
                "---\n\
                 title: \"{title}\"\n{draft}\
                 date: {today}\n\
                 description: \"One sentence: what a reader will understand by the end.\"\n\
                 categories: [writing]\n\
                 ---\n\
                 \n\
                 Open with the question this post answers.\n\
                 \n\
                 ## The first idea\n\
                 \n\
                 Save the file and the preview re-renders only the block you changed.\n"
            ),
        ),
        NewKind::Page => (
            PathBuf::from(format!("{slug}.tmd")),
            format!(
                "---\n\
                 title: \"{title}\"\n{draft}\
                 ---\n\
                 \n\
                 Save the file and the preview re-renders only the block you changed.\n"
            ),
        ),
        NewKind::Deck => {
            let body = format!(
                "---\n\
                     title: \"{title}\"\n{draft}\
                     subtitle: \"A subtitle\"\n\
                     format: deck\n\
                     ---\n\
                     \n\
                     ## The first slide\n\
                     \n\
                     - A point worth making\n\
                     - Another one\n\
                     \n\
                     ## The second slide\n\
                     \n\
                     Each `##` heading starts a new slide.\n"
            );
            (PathBuf::from(format!("{slug}.tmd")), body)
        }
        // Paper is handled by the early return above (it writes two files).
        NewKind::Paper => unreachable!("Paper scaffold is built before this match"),
    };
    vec![(path, body)]
}

/// `taliesin new <post|page|deck> <slug> [--dir <root>]`: scaffold one document, correct
/// on its first save. Refuses to overwrite, exactly as `init` does.
pub(crate) fn cmd_new(args: &[String]) -> ExitCode {
    let mut positional: Vec<&str> = Vec::new();
    let mut root = ".".to_string();
    let mut json = false;
    let mut yes = false;
    let mut opts = NewOpts::default();
    let mut it = args[2..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // `--dir` = the scaffold-input root (where the project lives). The undocumented
            // `--out` alias was dropped — `--out` is the output-dir flag on build/publish.
            "--dir" => {
                if let Some(v) = it.next() {
                    root = v.clone();
                }
            }
            // `--json` prints `{kind, slug, created, preview}` (pure JSON to stdout), so an
            // agent knows exactly what it made and where. Suppresses the human hints.
            "--json" => json = true,
            // `--format json` / `--format human`: accept the long spelling too (json is the
            // shorthand), so `new --format json` doesn't dead-end.
            "--format" => match it.next().map(|s| s.as_str()) {
                Some("json") => json = true,
                Some("human") => json = false,
                other => {
                    log::error(&serve::bad_format_error(other));
                    return ExitCode::FAILURE;
                }
            },
            // `--draft` marks the scaffold `draft: true` (held out of the published build).
            "--draft" => opts.draft = true,
            // `-y`/`--yes` skips the interactive wizard (use it for scripts at a TTY).
            "-y" | "--yes" => yes = true,
            s if s.starts_with("--") => {
                log::error(&serve::unknown_flag_error(s, NEW_FLAGS));
                return ExitCode::FAILURE;
            }
            s => positional.push(s),
        }
    }

    // Missing kind/slug prompts a human at a TTY (the wizard); a pipe/CI/agent (or `-y`/`--json`)
    // gets the historical usage error instead of ever blocking on a prompt.
    let interactive = interactive::is_interactive(yes, json);

    let kind = match positional.first() {
        Some(k) => match NewKind::parse(k) {
            Ok(k) => k,
            Err(e) => {
                log::error(&e);
                return ExitCode::FAILURE;
            }
        },
        None if interactive => {
            const KINDS: [NewKind; 4] =
                [NewKind::Post, NewKind::Page, NewKind::Deck, NewKind::Paper];
            match interactive::select("What do you want to create?", NEW_KINDS, 0) {
                Ok(i) => KINDS[i],
                Err(e) => {
                    log::error(&e.to_string());
                    return ExitCode::FAILURE;
                }
            }
        }
        None => return new_usage(),
    };

    let slug: String = match positional.get(1) {
        Some(s) => (*s).to_string(),
        // The prompt's validator re-asks on a bad slug rather than aborting.
        None if interactive => {
            match interactive::input("Slug (lowercase, used in the URL)", None, validate_slug) {
                Ok(s) => s,
                Err(e) => {
                    log::error(&e.to_string());
                    return ExitCode::FAILURE;
                }
            }
        }
        None => return new_usage(),
    };
    if let Err(e) = validate_slug(&slug) {
        log::error(&e);
        return ExitCode::FAILURE;
    }
    let root = Path::new(&root);
    match write_new(root, kind, &slug, opts) {
        Ok(written) => {
            if json {
                println!("{}", new_json(kind.name(), &slug, &written));
            } else {
                for f in &written {
                    log::built(&f.display().to_string());
                }
                println!("{}", new_next_steps(root, kind, &written[0]));
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error(&e);
            ExitCode::FAILURE
        }
    }
}

/// What to do with a freshly scaffolded document.
///
/// A deck inside a site needs different words from every other kind, because the obvious next
/// command is the wrong one (item 120). `init` prints "Preview it: `taliesin preview .`" and
/// the scaffold's own Next steps point at `taliesin new deck`; following both in order used to
/// warn that the deck will flatten, and it did — browser-verified, the scaffolded slide reading
/// "Each `##` heading starts a new slide" rendered as one stacked article. A deck is a
/// *component* of a site page, not a page of it, so the site path only builds it when a page
/// references it with `{{< embed >}}`.
///
/// Words, not a write: `new` must not edit an existing `index.tmd` to insert the embed. The
/// `.tmd` is the author's editing surface and a scaffolder that rewrites their prose is the
/// same mistake as a preview that writes back.
fn new_next_steps(root: &Path, kind: NewKind, written: &Path) -> String {
    let in_site = root.join("_site.yml").is_file();
    if kind == NewKind::Deck && in_site {
        let name = written
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| written.display().to_string());
        return format!(
            "A deck in a site is a component of a page, not a page of its own.\n\
             Either embed it in a page (add this line to `index.tmd`):\n\
             \x20 {{{{< embed {name} >}}}}\n\
             or preview the deck on its own:\n\
             \x20 taliesin preview {}\n\
             `taliesin preview .` renders the site, where an unreferenced deck flattens \
             into an article.",
            written.display()
        );
    }
    format!("Preview it:\n  taliesin preview {}", written.display())
}

/// The `new` usage line, printed when a kind/slug is missing and there's no TTY to prompt at.
/// Derived from `new`'s `--help` synopsis so the two can't drift.
fn new_usage() -> ExitCode {
    crate::usage_error("new")
}

/// The `--json` receipt for a scaffold: `{kind?, slug?, created:[...], preview}` as pretty
/// JSON. `kind`/`slug` are `None` for `init` (which scaffolds a whole site, not a document).
fn new_json(kind: &str, slug: &str, written: &[PathBuf]) -> String {
    let created: Vec<String> = written.iter().map(|p| p.display().to_string()).collect();
    let preview = written
        .first()
        .map(|p| format!("taliesin preview {}", p.display()))
        .unwrap_or_default();
    let payload = serde_json::json!({
        "kind": kind,
        "slug": slug,
        "created": created,
        "preview": preview,
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// Every long flag `new` accepts (drives the unknown-flag did-you-mean).
const NEW_FLAGS: &[&str] = &["--dir", "--json", "--format", "--draft", "--yes"];

/// Write the scaffold under `root`, refusing to overwrite any existing target before
/// writing any of them (so a partial scaffold never lands on the author's work).
fn write_new(
    root: &Path,
    kind: NewKind,
    slug: &str,
    opts: NewOpts,
) -> Result<Vec<PathBuf>, String> {
    write_scaffold(root, &new_files(kind, slug, &today_utc(), opts))
}

/// Parse the optional `[port]` positional: absent -> the 4321 default; a present but
/// unparseable value is an error (not a silent fall-back to the default). Pure/unit-tested.
fn parse_port(raw: Option<&str>) -> Result<u16, String> {
    match raw {
        None => Ok(4321),
        Some(p) => p
            .parse()
            .map_err(|_| format!("invalid port: `{p}` (expected 0-65535)")),
    }
}

/// Every long flag `preview` accepts (drives the unknown-flag did-you-mean).
/// `--help`/`-h` are intercepted by `main()` before this parser runs, so they aren't here.
///
/// [`SESSION_FLAG`] is deliberately absent: it is internal, not offered and not documented.
const SERVE_FLAGS: &[&str] = &["--open", "--host", "--no-exec", "--port"];

/// The hidden flag `taliesin run` passes when it starts a session for itself. Underscore-
/// prefixed like the `__complete` subcommand, and for the same reason: it is a seam between
/// two of this binary's own commands, never something a person types. It was `--headless`,
/// a documented `preview` flag, until Wave 5 — where its whole user-facing life was one
/// line in one `--help` page and zero invocations.
pub(crate) const SESSION_FLAG: &str = "--__session";

/// What `preview` parsed out of argv, before any environment or IO.
#[derive(Debug, PartialEq)]
pub(crate) struct ServeArgs<'a> {
    pub path: &'a str,
    pub port: u16,
    pub open: bool,
    pub expose: bool,
    pub no_exec: bool,
    /// Run as a background SESSION: no console chrome, never opens a browser. The server
    /// is otherwise identical, which is the point — `taliesin run` must not get a
    /// different executor from the one a preview would use.
    pub headless: bool,
}

/// Parse `preview <file.tmd|dir> [port] [--port <N>] [--host] [--open] [--no-exec]`.
///
/// The port may be the second positional (the original spelling) or `--port <N>` /
/// `--port=<N>`. Without the flag, `--port 4400` tripped the unknown-flag did-you-mean and
/// suggested `--host`, which is two edits away and does something else entirely.
/// Pure + unit-tested: no environment reads, no filesystem.
pub(crate) fn parse_serve_args(args: &[String]) -> Result<ServeArgs<'_>, String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut flag_port: Option<&str> = None;
    let (mut open, mut expose, mut no_exec) = (false, false, false);
    let mut headless = false;

    let mut it = args[2..].iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--open" => open = true,
            "--host" => expose = true,
            "--no-exec" => no_exec = true,
            // A SESSION rather than a preview you are watching. Same server, same kernels,
            // same `_freeze/` — it just skips the console chrome (screen clear, banner, QR)
            // that a background process has nobody to show. Passed by `taliesin run` when
            // no session is up; see [`SESSION_FLAG`].
            s if s == SESSION_FLAG => headless = true,
            "--port" => {
                flag_port = Some(
                    it.next()
                        .map(String::as_str)
                        .ok_or_else(|| "--port needs a value (e.g. --port 4400)".to_string())?,
                );
            }
            s if s.starts_with("--port=") => flag_port = Some(&s["--port=".len()..]),
            // An unrecognized `--flag` is a hard error with a did-you-mean (never silently
            // dropped: a typo'd `--hots` would otherwise preview without exposing).
            s if s.starts_with("--") => return Err(serve::unknown_flag_error(s, SERVE_FLAGS)),
            s => positionals.push(s),
        }
    }

    let path = *positionals
        .first()
        .ok_or_else(|| crate::usage_line("preview"))?;
    // `--port` wins over the positional when both are given (the explicit flag is the
    // more deliberate spelling); a present-but-unparseable value is always an error.
    let port = parse_port(flag_port.or_else(|| positionals.get(1).copied()))?;
    Ok(ServeArgs {
        path,
        port,
        open,
        expose,
        no_exec,
        headless,
    })
}

pub(crate) fn cmd_serve(args: &[String]) -> ExitCode {
    let parsed = match parse_serve_args(args) {
        Ok(p) => p,
        Err(msg) if msg.starts_with("usage:") => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
        Err(msg) => {
            log::error(&msg);
            return ExitCode::FAILURE;
        }
    };
    // `--open` and `--host` are flags and only flags. The `TALIESIN_OPEN`/`TALIESIN_HOST`
    // env vars that also set them were retired in Wave 5: a second spelling for a flag is
    // not worth its documentation, and an exported `TALIESIN_HOST` silently changed the
    // network exposure of every preview in that shell.
    let (open, expose) = (parsed.open, parsed.expose);
    // `--no-exec` is sugar for `TALIESIN_NO_EXEC=1`. Two readers, one owner
    // (`taliesin_core::render::no_exec_in_force`): `exec::Executor` skips the kernel, and the
    // render pass leaves a `{js}` cell as source, since a `{js}` cell is a code cell whose
    // runtime is the browser (item 79). It does NOT sanitize raw HTML — see the CLI
    // reference's "Documents you did not write".
    if parsed.no_exec {
        // SAFETY: set once at CLI startup, before the tokio runtime / kernel
        // threads spawn, so no other thread is touching the environment.
        unsafe { std::env::set_var("TALIESIN_NO_EXEC", "1") };
    }
    // A directory is a project; a `.tmd` is one document, served as the project it belongs
    // to (or as a project of its own). One server handles both — there is no separate
    // single-document server to dispatch to.
    let result = serve_site::run(
        serve_site::Target::at(PathBuf::from(parsed.path)),
        parsed.port,
        open && !parsed.headless,
        expose,
        parsed.headless,
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error(&format!("serve: {e}"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn preview_port_defaults_parses_and_rejects() {
        // No port positional -> the 4321 default; a valid number parses; a present-but-
        // unparseable value is an error (not a silent fall-back); > u16::MAX is rejected.
        assert_eq!(parse_port(None).unwrap(), 4321);
        assert_eq!(parse_port(Some("8080")).unwrap(), 8080);
        assert_eq!(parse_port(Some("0")).unwrap(), 0);
        assert!(parse_port(Some("not-a-port")).is_err());
        assert!(parse_port(Some("70000")).is_err());
    }

    fn argv(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn serve_accepts_port_as_a_flag_or_a_positional() {
        // The original spelling: the second positional.
        let a = argv(&["taliesin", "preview", "doc.tmd", "4400"]);
        assert_eq!(parse_serve_args(&a).unwrap().port, 4400);

        // `--port 4400` used to error with "unknown flag `--port` (did you mean `--host`?)".
        let a = argv(&["taliesin", "preview", "doc.tmd", "--port", "4400"]);
        assert_eq!(parse_serve_args(&a).unwrap().port, 4400);

        // `--port=4400` too, and the flag may precede the path.
        let a = argv(&["taliesin", "preview", "--port=4400", "doc.tmd"]);
        let p = parse_serve_args(&a).unwrap();
        assert_eq!((p.port, p.path), (4400, "doc.tmd"));

        // Default when neither is given.
        let a = argv(&["taliesin", "preview", "doc.tmd"]);
        assert_eq!(parse_serve_args(&a).unwrap().port, 4321);

        // The explicit flag wins over the positional.
        let a = argv(&["taliesin", "preview", "doc.tmd", "1111", "--port", "2222"]);
        assert_eq!(parse_serve_args(&a).unwrap().port, 2222);
    }

    #[test]
    fn serve_flag_errors_stay_loud() {
        // A bad port value is an error, never a silent fall-back to the default.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--port", "not-a-port"]);
        assert!(parse_serve_args(&a).unwrap_err().contains("invalid port"));

        // `--port` with nothing after it names the fix.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--port"]);
        assert!(parse_serve_args(&a).unwrap_err().contains("needs a value"));

        // An unknown flag still gets a did-you-mean, and `--prot` now resolves to
        // `--port` rather than to the unrelated `--host`.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--prot", "4400"]);
        let err = parse_serve_args(&a).unwrap_err();
        assert!(err.contains("--prot"), "{err}");
        assert!(err.contains("--port"), "{err}");

        // Flags are not swallowed as positionals.
        let a = argv(&["taliesin", "preview", "doc.tmd", "--host", "--open"]);
        let p = parse_serve_args(&a).unwrap();
        assert!(p.expose && p.open && p.port == 4321);
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tali-init-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn the_onramp_note_names_every_file_init_writes_unasked() {
        // Item 123. `.taliesin/` is written by every template regardless of what the user
        // asked for, so the note is the only place that says what it is. Derived from
        // `onramp_files()` rather than hand-listed: add a second onramp file and this fails
        // until the note mentions it, which is the drift that made the item worth filing in
        // the first place.
        for (name, _) in onramp_files() {
            // `.taliesin/x.schema.json` is named by its directory, which is what a reader
            // sees in the file list and what the note can meaningfully talk about.
            let named = name.split('/').next().unwrap_or(name);
            assert!(
                ONRAMP_NOTE.contains(named),
                "the onramp note must name `{named}`, or it arrives unexplained: \
                 {ONRAMP_NOTE}"
            );
        }
        // And it must say the files are optional — "what is this?" and "can I delete it?"
        // are the two questions an unasked-for file raises, and the second is the one a
        // bare filename can never answer.
        assert!(
            ONRAMP_NOTE.contains("delete"),
            "the note must say they can be removed: {ONRAMP_NOTE}"
        );
    }

    #[test]
    fn init_scaffolds_a_previewable_site() {
        let dir = tmp("scaffold");
        // The dir doesn't exist yet — `scaffold_init` must create it.
        let written =
            scaffold_init(&dir, InitTemplate::Basic).expect("scaffold succeeds into a fresh dir");

        let site_yml = dir.join("_site.yml");
        let index = dir.join("index.tmd");
        let site_schema = dir.join(".taliesin").join("tali-site.schema.json");
        assert!(site_yml.exists(), "_site.yml written");
        assert!(index.exists(), "index.tmd written");
        assert!(
            site_schema.exists(),
            ".taliesin/tali-site.schema.json written"
        );
        assert_eq!(
            written,
            vec![site_yml.clone(), index.clone(), site_schema.clone()]
        );

        // The scaffold is a real, parseable site whose one page previews.
        let cfg = fs::read_to_string(&site_yml).unwrap();
        assert!(cfg.contains("title:"), "config has a title: {cfg}");

        // Load-bearing: the modeline points at a real schema whose body is the bundled one, so
        // the referenced path and the emitted file can never silently drift.
        let first = cfg.lines().next().unwrap_or("");
        assert!(
            first.starts_with("# yaml-language-server: $schema="),
            "first line is the schema modeline: {first}"
        );
        let rel = first.trim_end().rsplit('=').next().unwrap();
        let pointed = dir.join(rel);
        assert!(
            pointed.exists(),
            "modeline path resolves to a real file: {rel}"
        );
        assert_eq!(
            fs::read_to_string(&pointed).unwrap(),
            taliesin_core::schema::SITE_SCHEMA,
            "the wired schema is the bundled SITE_SCHEMA"
        );
        let page = fs::read_to_string(&index).unwrap();
        assert!(
            page.starts_with("---") && page.contains("title:"),
            "index has front matter: {page}"
        );

        // Re-running refuses to overwrite (never clobbers existing work).
        let err =
            scaffold_init(&dir, InitTemplate::Basic).expect_err("second init refuses to overwrite");
        assert!(err.contains("already exists"), "overwrite refused: {err}");

        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod init_template_tests {
    use super::*;

    #[test]
    fn an_unknown_template_suggests_the_nearest() {
        assert_eq!(InitTemplate::parse("basic").unwrap(), InitTemplate::Basic);
        assert_eq!(InitTemplate::parse("site").unwrap(), InitTemplate::Site);
        assert_eq!(InitTemplate::parse("book").unwrap(), InitTemplate::Book);
        let e = InitTemplate::parse("sit").unwrap_err();
        assert!(e.contains("did you mean `site`?"), "got: {e}");
        let e = InitTemplate::parse("zzzzzz").unwrap_err();
        assert!(e.contains("expected basic, site, or book"), "got: {e}");
    }

    /// The default `init` must not drift: its two authored files are exactly the constants
    /// that shipped before templates existed, so an existing `taliesin init` is byte-identical.
    #[test]
    fn basic_template_is_byte_identical_to_the_frozen_scaffold() {
        assert_eq!(
            init_files(InitTemplate::Basic),
            vec![
                (PathBuf::from("_site.yml"), INIT_SITE_YML.to_string()),
                (PathBuf::from("index.tmd"), INIT_INDEX_TMD.to_string()),
            ]
        );
    }

    /// The `site` and `book` templates are pinned byte-for-byte by real, buildable projects
    /// under `corpus/`, which the corpus regression net renders and lints like any other
    /// document — so a scaffold that stops being `check`-clean fails the suite.
    #[test]
    fn site_and_book_templates_match_their_corpus_pins() {
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
        for (template, dir) in [
            (InitTemplate::Site, "scaffold-site"),
            (InitTemplate::Book, "scaffold-book"),
        ] {
            for (rel, contents) in init_files(template) {
                let pinned = std::fs::read_to_string(corpus.join(dir).join(&rel))
                    .unwrap_or_else(|e| panic!("corpus pin for {template:?} at {rel:?}: {e}"));
                assert_eq!(
                    contents,
                    pinned,
                    "`init --template {template:?}` drifted from corpus/{dir}/{}",
                    rel.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod new_tests {
    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1)); // the epoch
        assert_eq!(civil_from_days(-1), (1969, 12, 31)); // before it
        assert_eq!(civil_from_days(59), (1970, 3, 1)); // 1970 is not a leap year
        assert_eq!(civil_from_days(11_016), (2000, 2, 29)); // 2000 is (a 400-year leap)
        assert_eq!(civil_from_days(20_581), (2026, 5, 8)); // a real post's date
        assert_eq!(civil_from_days(20_644), (2026, 7, 10));
    }

    #[test]
    fn today_is_a_well_formed_iso_date() {
        let t = today_utc();
        assert_eq!(t.len(), 10, "got {t}");
        let (y, rest) = t.split_at(4);
        assert!(y.parse::<u32>().unwrap() >= 2024, "got {t}");
        assert!(rest.starts_with('-') && rest[3..4] == *"-", "got {t}");
    }

    #[test]
    fn a_slug_becomes_the_title_an_author_would_have_typed() {
        assert_eq!(title_from_slug("my-first-post"), "My First Post");
        assert_eq!(title_from_slug("about"), "About");
        assert_eq!(title_from_slug("pca-2d"), "Pca 2d");
    }

    #[test]
    fn a_slug_may_not_escape_the_project_or_carry_a_path() {
        assert!(validate_slug("my-first-post").is_ok());
        for bad in ["", "../evil", "a/b", "Upper", "has space", "dot.tmd"] {
            assert!(validate_slug(bad).is_err(), "`{bad}` must be rejected");
        }
    }

    #[test]
    fn an_unknown_kind_suggests_the_nearest() {
        assert!(NewKind::parse("post").is_ok());
        let e = NewKind::parse("pots").unwrap_err();
        assert!(e.contains("did you mean `post`?"), "got: {e}");
        let e = NewKind::parse("zzzzzz").unwrap_err();
        assert!(
            e.contains("expected post, page, deck, or paper"),
            "got: {e}"
        );
    }

    /// The scaffold's bytes are pinned by `corpus/scaffold/`, which the corpus regression
    /// net renders and lints like any other document. If `new` ever emits a front-matter
    /// key the validator rejects, `cargo test -p taliesin-core` fails; if it emits
    /// something else entirely, this fails.
    #[test]
    fn every_scaffold_matches_its_corpus_pin() {
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/scaffold");
        for kind in [NewKind::Post, NewKind::Page, NewKind::Deck, NewKind::Paper] {
            let slug = match kind {
                NewKind::Post => "my-first-post",
                NewKind::Page => "about",
                NewKind::Deck => "my-talk",
                NewKind::Paper => "my-paper",
            };
            for (rel, contents) in new_files(kind, slug, "2026-07-10", NewOpts::default()) {
                let pinned = std::fs::read_to_string(corpus.join(&rel))
                    .unwrap_or_else(|e| panic!("corpus pin for {kind:?} at {rel:?}: {e}"));
                assert_eq!(
                    contents,
                    pinned,
                    "`taliesin new {slug}` drifted from corpus/scaffold/{}",
                    rel.display()
                );
            }
        }
    }
}
