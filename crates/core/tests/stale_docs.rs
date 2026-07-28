//! Gates that compare shipped prose against shipped behaviour.
//!
//! The tests below the hand-written needle sets are the *widening* (item 146): each
//! derives what it checks from the tree rather than from a list someone remembered to
//! update, so they cover claims nobody has thought to look for yet. Every stale-string
//! defect this repo has found was a symptom of the same gap — `fmt`, `clippy`, the suite
//! and `check` all pass over a false sentence.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// The docs a *reader* gets: the two books, the marketing site, the README, the samples.
///
/// `notes/` and `docs/superpowers/` are deliberately excluded and must stay excluded.
/// They are dated records — a 2026-06 spec correctly describes the tree as it was that
/// day, and five of the six stale paths the path gate first reported were of exactly that
/// kind. Rewriting a dated document to match today's tree destroys the record; this is
/// the difference between prose that *claims* and prose that *remembers*.
fn shipped_docs() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in [
        "README.md",
        "SECURITY.md",
        "CONTRIBUTING.md",
        "samples/README.md",
    ] {
        out.push((rel.to_string(), read(rel)));
    }
    for dir in ["docs/guide", "docs/internals", "site"] {
        collect_docs(&repo().join(dir), &repo(), &mut out);
    }
    assert!(
        out.len() > 40,
        "only {} shipped docs found — the walk broke, and an empty gate passes forever",
        out.len()
    );
    out
}

fn collect_docs(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "_site" || name == "_freeze" || name == "_extensions" {
                continue;
            }
            collect_docs(&p, root, out);
        } else if p.extension().is_some_and(|x| x == "tmd" || x == "md") {
            let rel = p.strip_prefix(root).unwrap_or(&p).display().to_string();
            out.push((rel, std::fs::read_to_string(&p).unwrap()));
        }
    }
}

/// Every backticked token in `text` (what the docs use to name a file, a flag or a key).
fn backticked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        match rest.find('`') {
            Some(close) => {
                out.push(rest[..close].to_string());
                rest = &rest[close + 1..];
            }
            None => break,
        }
    }
    out
}

/// These phrases describe machinery deleted in the native rewrite and must not return.
#[test]
fn docs_do_not_claim_quarto_config_still_works() {
    let cfg = read("docs/guide/reference/configuration.tmd");
    assert!(
        !cfg.contains("still works"),
        "configuration.tmd still claims a Quarto config works"
    );
    assert!(
        !cfg.contains("Coming from a Quarto config?"),
        "configuration.tmd still has the stale Quarto-config callout"
    );
}

#[test]
fn internals_do_not_describe_the_deleted_shim() {
    let sites = read("docs/internals/sites.tmd");
    assert!(
        !sites.contains("site/config/quarto.rs"),
        "sites.tmd still describes the deleted quarto.rs shim"
    );
}

/// The workflow was restored on 2026-07-28, but **every job is guarded on repository
/// visibility** so it stays inert until this repo is public. That means the false claim
/// this test was built to catch is still false: nothing in CI checks a push today. A doc
/// that credits "CI" for a gate is worse than silence — it tells the next reader (or
/// agent) a push is checked for them in ways it is not.
///
/// The two halves are asserted together on purpose. Making the workflow live is one
/// deletion (the guard) and it must not be possible to do that half without noticing the
/// prose it makes stale, in either direction.
#[test]
fn docs_do_not_promise_a_ci_that_enforces_gates() {
    // Walk the directory rather than naming ci.yml, so a workflow added later cannot
    // start billing a private repo just by not being on a hand-written list.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect(".github/workflows is missing: the workflow was restored on 2026-07-28")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no workflows found under {}",
        dir.display()
    );

    let mut total_jobs = 0;
    for f in &files {
        let workflow = std::fs::read_to_string(f).unwrap();
        // Everything below `jobs:`, so the `on:` keys above it are not counted as jobs
        // and the guard named in a header comment is not counted as a guard.
        let (_, body) = workflow
            .split_once("\njobs:\n")
            .unwrap_or_else(|| panic!("{} has no jobs: block", f.display()));
        let jobs = body
            .lines()
            .filter(|l| {
                l.strip_prefix("  ").is_some_and(|k| {
                    !k.starts_with(' ')
                        && k.trim_end().ends_with(':')
                        && k.starts_with(|c: char| c.is_ascii_lowercase())
                })
            })
            .count();
        let guards = body
            .matches("if: github.event.repository.private != true")
            .count();
        assert!(
            jobs > 0 && guards == jobs,
            "{guards} of {jobs} jobs in {} carry the repository-visibility guard. If the \
             repo is public now, dropping the guard is right — but then these docs have to \
             start crediting CI, so update them (and this test) rather than only the YAML.",
            f.display()
        );
        total_jobs += jobs;
    }
    assert!(
        total_jobs >= 7,
        "only {total_jobs} guarded jobs across {} workflow file(s): the restored gate set \
         had seven, so something was deleted rather than un-guarded",
        files.len()
    );
    // THIRD_PARTY.md and deny.toml were the two that actually carried a false claim past
    // this gate: both asserted "CI enforces" the licence policy while `cargo deny` runs
    // nowhere but by hand. A gate whose file list omits the files that drift is not a gate.
    for rel in [
        "CLAUDE.md",
        "README.md",
        "THIRD_PARTY.md",
        "deny.toml",
        ".claude/hooks/cargo-fmt.sh",
        ".claude/agents/corpus-verifier.md",
        "docs/internals/extending.tmd",
        "docs/internals/repository.tmd",
    ] {
        let text = read(rel);
        // Match the shapes that actually shipped, not one canonical phrasing. `deny.toml`
        // carried TWO independent claims and the first pass at this gate caught only one:
        // the header said "wired into CI" and a comment twelve lines below still called
        // cargo-audit "the other CI job". A gate that knows one spelling of a false claim
        // leaves its siblings in the same file.
        for needle in ["CI enforces", "CI-gated", "wired into CI", "CI job"] {
            assert!(
                !text.contains(needle),
                "{rel} still promises a CI gate ({needle:?}), but the workflow is gone \
                 and the check is manual"
            );
        }
    }
}

/// The 2026-07-12 deck audit (A1/A2) deleted reader/scroll mode, drawing mode and
/// PDF-export mode, and `render::tests::deck_opens_as_a_deck_without_reader_or_pdf_export`
/// pins the machinery gone at the bundle level. Nothing pinned the *prose*: the two
/// marketing pages went on selling PDF export, and `samples/README.md` listed reader mode
/// and drawing on top of it. That audit's own findings doc even claims the stale claims
/// "were all stale and are now corrected" — the sweep missed all three files. fmt, clippy,
/// the suite and `check` every one of them pass over a false sentence, so gate the prose
/// against the machinery instead of against a memory of it.
#[test]
fn shipped_prose_does_not_advertise_deleted_deck_modes() {
    let deck_js = read("crates/core/assets/js/deck.js");
    for machinery in ["enterPrint", "enterScroll", "drawMode"] {
        assert!(
            !deck_js.contains(machinery),
            "{machinery} is back in deck.js — revive the prose deliberately rather than \
             deleting this test"
        );
    }

    // These sell the deck engine and have no other reason to name a deleted mode, so a
    // bare mention is the defect. (`docs/guide/using/formats.tmd` deliberately says there
    // is *no* PDF export, which is why it is not on this list.)
    //
    // `demo.tmd` is on the list because the first version of this gate omitted it and the
    // omission cost exactly what the gate exists to prevent: `site/demo.tmd` is embedded
    // INTO `site/index.tmd` and `site/formats.tmd` via `{{< embed >}}`, so checking only
    // the two embedding pages proves nothing about what the landing page renders. It went
    // on advertising a one-slide-per-page PDF, a "scrollable reader", and a <kbd>D</kbd>
    // pen tool that never existed in any version of the engine.
    //
    // The needles are the *shapes that actually shipped*, not the vocabulary the deleted
    // features were named after: the stale prose said "one-slide-per-page PDF" and
    // "scrollable **reader**", neither of which contains "PDF export" or "reader mode".
    for rel in [
        "site/index.tmd",
        "site/formats.tmd",
        "site/demo.tmd",
        "docs/guide/demo.tmd",
        "samples/README.md",
    ] {
        let text = read(rel);
        for claim in [
            "PDF export",
            "PDF-export",
            "per-page PDF",
            "reader mode",
            "Reader mode",
            "scrollable **reader**",
            "a **pen**",
            "to annotate",
        ] {
            assert!(
                !text.contains(claim),
                "{rel} advertises {claim:?}, which the deck engine does not do \
                 (reader/PDF modes were deleted 2026-07-12; the pen never existed)"
            );
        }
    }
}

/// Every file in the repo, plus every suffix a doc might reasonably name it by.
///
/// A doc writes `serve/mod.rs`, `server/exec.rs` or `tests/regression.rs` — a suffix of
/// the real path, sometimes with the crate's `src/` elided. All of those are legitimate
/// ways to point at a file, so all of them resolve; what must not resolve is a path that
/// names no file at all (`serve.rs` after `serve/` became a module directory).
fn claimable_paths() -> BTreeSet<String> {
    let root = repo();
    let mut out = BTreeSet::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if matches!(
                name,
                "target" | ".git" | "node_modules" | "_site" | "_freeze" | ".vscode-test"
            ) {
                continue;
            }
            let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
            let parts: Vec<&str> = rel.split('/').collect();
            for start in 0..parts.len() {
                let suffix = parts[start..].join("/");
                out.insert(suffix.clone());
                // The same claim with the crate's `src/` elided: `server/exec.rs`.
                let elided: Vec<&str> = parts[start..]
                    .iter()
                    .copied()
                    .filter(|c| *c != "src")
                    .collect();
                out.insert(elided.join("/"));
            }
            if p.is_dir() {
                stack.push(p);
            }
        }
    }
    out
}

/// A token that is a claim about a file in THIS repo, as opposed to an illustrative name
/// (a reader's `custom.css`, an example `_extensions/mytheme/theme.css`) or a pattern.
fn is_repo_path_claim(tok: &str) -> bool {
    const ROOTS: &[&str] = &[
        "crates/",
        "web-client/",
        "tools/",
        "editor/",
        "corpus/",
        "site/",
        "docs/",
        "samples/",
    ];
    // Build output and tool caches: named in the docs (correctly), absent from a fresh
    // clone. `.github/` is not a ROOT for the same reason in reverse — the guide's
    // workflow examples are files the READER creates in their repo, not ours.
    const GENERATED: &[&str] = &[
        "_site",
        "_book",
        "_freeze",
        ".vscode-test",
        "target/",
        "node_modules",
    ];
    if tok.contains(['*', '<', '>', '{', ' ', '…']) || tok.contains("...") {
        return false;
    }
    if tok.starts_with('.') || GENERATED.iter().any(|g| tok.contains(g)) {
        return false;
    }
    // A `.rs` file is always this repo's own source: nobody authoring a document writes
    // Rust for it. Anything else has to be anchored to a real top-level directory.
    tok.ends_with(".rs") || ROOTS.iter().any(|r| tok.starts_with(r))
}

/// Item 146. A shipped doc must not name a source file that does not exist.
///
/// This is the gate the "~20 stale module paths in the Internals book" finding asks for,
/// and it is derived from the filesystem rather than from a needle list, so it also covers
/// the paths nobody has noticed yet. What it caught when it landed: `serve.rs`,
/// `serve_site.rs`, `cite.rs` and `diagnostics.rs` had all become module *directories*,
/// `code-enhance.js` had been split into `code-enhance/` fragments, and a test table named
/// `extensions.rs`, which is `theme_css.rs`.
#[test]
fn shipped_docs_do_not_name_a_file_that_does_not_exist() {
    let known = claimable_paths();
    // Vacuity control: the walk must have found the tree, and a path that is real must
    // resolve while one that is not must not. A resolver that says yes to everything (or
    // a walk that found nothing) passes this gate forever without it.
    assert!(
        known.contains("crates/core/src/render/emit.rs"),
        "walk broke"
    );
    assert!(known.contains("render/emit.rs"), "suffix form must resolve");
    assert!(
        !known.contains("serve.rs"),
        "the resolver is too permissive"
    );

    // A doc may name a dead path when the dead path IS the subject of the sentence.
    // Every exemption must still be present where it claims to be — a rewritten sentence
    // must delete its exemption rather than leave it shadowing a future defect.
    const DEAD_PATH_IS_THE_SUBJECT: &[(&str, &str, &str)] = &[(
        "site/README.md",
        "docs/demo.tmd",
        "the sentence explains that this path is stale and names it as the reason the \
         guide's demo and the marketing copy drifted apart",
    )];
    for (rel, tok, _why) in DEAD_PATH_IS_THE_SUBJECT {
        assert!(
            read(rel).contains(&format!("`{tok}`")),
            "{rel} no longer mentions `{tok}`: delete this exemption instead of leaving \
             it to hide the next stale path"
        );
    }

    let mut checked = 0usize;
    let mut stale = Vec::new();
    for (rel, text) in shipped_docs() {
        for tok in backticked(&text) {
            let tok = tok.trim_end_matches('/');
            if !is_repo_path_claim(tok) {
                continue;
            }
            if DEAD_PATH_IS_THE_SUBJECT
                .iter()
                .any(|(f, t, _)| *f == rel && *t == tok)
            {
                continue;
            }
            checked += 1;
            if !known.contains(tok) {
                stale.push(format!("{rel}: `{tok}`"));
            }
        }
    }
    assert!(
        checked >= 120,
        "only {checked} path claims examined: the extractor stopped matching, so this \
         gate is now green for the wrong reason"
    );
    assert!(
        stale.is_empty(),
        "{} shipped doc(s) name a file that does not exist (a module that became a \
         directory, a file that was split or renamed):\n{}",
        stale.len(),
        stale.join("\n")
    );
}

/// The retired front-matter keys, read out of `frontmatter.rs` rather than restated here,
/// so retiring another key extends this gate automatically.
fn retired_keys() -> Vec<String> {
    let src = read("crates/core/src/frontmatter.rs");
    let (_, table) = src
        .split_once("RETIRED_KEYS: &[(&str, &str, &str)] = &[")
        .expect("RETIRED_KEYS moved or changed shape — update this gate, do not delete it");
    let table = table.split("\n];").next().unwrap_or_default();
    // Each entry is (scope, key, note); the key is the second string literal.
    let mut keys = Vec::new();
    for entry in table.split("    (").skip(1) {
        let mut lits = entry.split('"').skip(1).step_by(2);
        let _scope = lits.next();
        if let Some(key) = lits.next() {
            keys.push(key.to_string());
        }
    }
    keys
}

/// Item 146. A key the validator *rejects* must not appear in a shipped example.
///
/// A front-matter example inside the manual is read by no validator: `check` lints the
/// corpus and the books' own front matter, never a YAML block quoted in prose. So the one
/// surface where a reader learns the vocabulary is the one surface nothing checks.
#[test]
fn shipped_docs_do_not_use_a_retired_front_matter_key() {
    let keys = retired_keys();
    assert!(
        keys.iter().any(|k| k == "about"),
        "expected the retired `about:` key in RETIRED_KEYS, got {keys:?} — if it was \
         un-retired, this gate needs rewriting rather than deleting"
    );
    assert!(keys.len() >= 2, "only {} retired keys parsed", keys.len());

    let mut hits = Vec::new();
    for (rel, text) in shipped_docs() {
        for (n, line) in text.lines().enumerate() {
            let t = line.trim_start().trim_start_matches("- ");
            for k in &keys {
                if t.starts_with(&format!("{k}:")) {
                    hits.push(format!("{rel}:{}: {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "shipped doc(s) show a retired front-matter key that the validator now rejects \
         ({keys:?}):\n{}",
        hits.join("\n")
    );
}

/// Item 146. Every CLI flag the reference documents must exist in the CLI.
///
/// Scoped to the CLI reference on purpose: the README and CLAUDE.md legitimately name
/// `cargo` and `git` flags (`--test-threads=1`, `--no-verify`), which this repo does not
/// own and must not be asked to implement.
#[test]
fn documented_cli_flags_exist_in_the_cli() {
    let cli_src = {
        let dir = repo().join("crates/server/src");
        let mut all = String::new();
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    all.push_str(&std::fs::read_to_string(&p).unwrap());
                }
            }
        }
        all
    };
    assert!(
        cli_src.contains("--no-exec"),
        "the CLI sources did not load: a flag everyone knows is missing"
    );

    let doc = read("docs/guide/reference/cli.tmd");
    let mut checked = 0usize;
    let mut missing = Vec::new();
    for tok in backticked(&doc) {
        // Scan for `--flag` runs rather than splitting on whitespace: the reference
        // writes alternatives inline (`[--errors-only\|--strict]`), and a whitespace
        // split hands the escaped pipe to the lookup as part of the flag name.
        // char_indices, not byte arithmetic: the reference contains `×` and friends, and
        // slicing mid-codepoint panics the gate rather than reporting anything.
        for (i, _) in tok.char_indices() {
            if !tok[i..].starts_with("--")
                || !tok[i + 2..].starts_with(|c: char| c.is_ascii_alphabetic())
            {
                continue;
            }
            let flag: String = tok[i..]
                .chars()
                .take_while(|c| *c == '-' || c.is_ascii_alphanumeric())
                .collect();
            checked += 1;
            if !cli_src.contains(&flag) {
                missing.push(format!("`{flag}`"));
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        checked >= 50,
        "only {checked} flag mentions examined — the extractor broke"
    );
    assert!(
        missing.is_empty(),
        "the CLI reference documents flag(s) the CLI does not accept: {}",
        missing.join(", ")
    );
}
