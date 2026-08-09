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
///
/// `CLAUDE.md` is in the list even though no reader downloads it: every session and every
/// agent reads it first, so a false claim there is copied forward before anyone checks.
/// **What this does NOT cover, measured rather than assumed:** the path gate below reads
/// *backticked* tokens, and CLAUDE.md's "Where things are" map is one fenced block, which
/// the extractor sees as a single token full of spaces and discards. That is where the
/// stale `sentences.rs`/`backlinks.rs` claim lived (deleted in `3a2f197a`, still described
/// here on 2026-08-08), so adding this file catches its eleven backticked claims and would
/// NOT have caught that one. Widening the extractor to parse the map was considered and
/// declined: it is machinery, and this wave removes machinery.
fn shipped_docs() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in [
        "README.md",
        "SECURITY.md",
        "CONTRIBUTING.md",
        "CLAUDE.md",
        "samples/README.md",
    ] {
        out.push((rel.to_string(), read(rel)));
    }
    for dir in ["docs/guide", "docs/internals", "site"] {
        collect_docs(&repo().join(dir), &repo(), &mut out);
    }
    // An anti-vacuity guard against a broken walk, not a content floor: it exists so a
    // `read_dir` that silently returns nothing cannot make every gate below pass forever.
    // Lowered from 40 on 2026-08-09, when the Internals book went from thirteen chapters
    // to six; the walk finds ~38 today.
    assert!(
        out.len() > 25,
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

/// Every inline-code token in `text` (what the docs use to name a file, a flag or a key).
///
/// **Fenced blocks are removed first, and that is load-bearing, not tidiness.** Pairing
/// backticks across a whole document makes every span downstream of a fence depend on the
/// document's running backtick *parity*, so an edit anywhere above shifts which half of the
/// file the caller reads. Measured when Wave 13 rewrote a section of the CLI reference: the
/// flag gate went from 55 mentions to 69 and started reporting `rsync -a --delete` as a
/// Taliesin flag the CLI does not accept, purely because the fence count above it changed.
/// Its own `checked >= 50` floor passed on both sides, so nothing announced it.
///
/// The distinction is also the right one on the merits. An inline span is a *claim* the
/// author is making about this tool; a fenced block is an example, and an example may
/// legitimately invoke `rsync` or `cargo`, or name a path the reader is about to create.
/// (Fenced content was already out of reach in practice — Wave 1 recorded that CLAUDE.md's
/// fenced tree map reads as one token full of spaces and is discarded — so this makes an
/// accident into a rule. Measured across the walked docs: the path-claim population is
/// unchanged at 112.)
fn backticked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
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

// `internals_do_not_describe_the_deleted_shim` lived here and read
// `docs/internals/sites.tmd` by name to assert it no longer described the deleted
// `site/config/quarto.rs`. That chapter was deleted on 2026-08-09, and the assertion is
// not re-pointed, because `shipped_docs_do_not_name_a_file_that_does_not_exist` below
// already subsumes it: `site/config/quarto.rs` is a backticked `.rs` path that resolves
// to nothing, so any doc that names it fails the derived gate. A needle test whose
// subject file is gone is the vacuous shape this file exists to prevent.

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

/// The file a `path:line:` or `path:line:col` citation names.
///
/// That is the shape every diagnostic in this tool prints, so it is the natural way for a doc
/// to point at a line — and without this the trailing `:12:` was part of the token, so citing
/// a location was indistinguishable from naming a file that does not exist. Stripping it makes
/// the gate check the real path instead of rejecting the citation, which can only find more:
/// a cited path that is stale still fails, it just fails for the right reason.
fn without_location_suffix(tok: &str) -> &str {
    let head = tok.trim_end_matches(':');
    match head.rsplit_once(':') {
        Some((path, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            without_location_suffix(path)
        }
        _ => head,
    }
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
    // The location strip is part of the resolution, so it is part of the vacuity control: a
    // strip that took too much would silently turn every cited path into a shorter one that
    // still resolves, and a strip that took nothing would fail every citation.
    for (cited, file) in [
        (
            "crates/core/src/render/emit.rs:12:",
            "crates/core/src/render/emit.rs",
        ),
        (
            "crates/core/src/render/emit.rs:12:5",
            "crates/core/src/render/emit.rs",
        ),
        (
            "crates/core/src/render/emit.rs",
            "crates/core/src/render/emit.rs",
        ),
    ] {
        assert_eq!(
            without_location_suffix(cited),
            file,
            "location strip: {cited}"
        );
    }

    // A doc may name a dead path when the dead path IS the subject of the sentence. There
    // is no such exemption today: the one that existed (`site/README.md` explaining why the
    // guide's demo deck and the marketing copy had drifted) went with the deck engine on
    // 2026-08-08. An exemption must be deleted with the sentence that earned it, never left
    // behind to shadow the next stale path.

    let mut checked = 0usize;
    let mut stale = Vec::new();
    for (rel, text) in shipped_docs() {
        for tok in backticked(&text) {
            let tok = without_location_suffix(tok.trim_end_matches('/'));
            if !is_repo_path_claim(tok) {
                continue;
            }
            checked += 1;
            if !known.contains(tok) {
                stale.push(format!("{rel}: `{tok}`"));
            }
        }
    }
    // Anti-vacuity, like the walk floor above: an extractor that stopped matching yields
    // ~0, not 60. Lowered from 120 on 2026-08-09 with the seven Internals chapters, which
    // were the densest path-claim prose in the tree. 122 survive, and a floor two below
    // the live count would fail the next docs edit for the wrong reason.
    assert!(
        checked >= 60,
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

/// The body of a `const NAME: … = &[ … ];` block, as raw source text.
fn const_block(src: &str, name: &str) -> String {
    let (_, rest) = src
        .split_once(&format!("{name}: &[&str] = &["))
        .unwrap_or_else(|| panic!("{name} moved or changed shape — update this gate"));
    rest.split("\n];").next().unwrap_or_default().to_string()
}

/// Every double-quoted string literal in `block`, in order.
fn string_literals(block: String) -> Vec<String> {
    block
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// The retired keys as `(scope, key)` pairs, read out of `frontmatter.rs` rather than
/// restated here, so retiring another key extends this gate automatically.
///
/// **The scope is load-bearing and must not be flattened away.** `RETIRED_KEYS` is scoped
/// because the same word can be retired in one place and live in another: `toc:` is gone
/// from `_site.yml` but is still a front-matter key, `theorems:` likewise, `image:` is gone
/// from `hero:` while the top-level `image:` is live, and `echo:` is gone from `execute:`
/// while `#| echo:` is a live cell option. A gate that matched on the bare key would fire on
/// every one of those legitimate examples.
fn retired_keys() -> Vec<(String, String)> {
    let src = read("crates/core/src/frontmatter.rs");
    let (_, table) = src
        .split_once("RETIRED_KEYS: &[(&str, &str, &str)] = &[")
        .expect("RETIRED_KEYS moved or changed shape — update this gate, do not delete it");
    let table = table.split("\n];").next().unwrap_or_default();
    // Each entry is (scope, key, note); the first two string literals are what we want.
    let mut keys = Vec::new();
    for entry in table.split("    (").skip(1) {
        let mut lits = entry.split('"').skip(1).step_by(2);
        if let (Some(scope), Some(key)) = (lits.next(), lits.next()) {
            keys.push((scope.to_string(), key.to_string()));
        }
    }
    keys
}

/// Item 146. A key the validator *rejects* must not appear in a shipped example.
///
/// A front-matter example inside the manual is read by no validator: `check` lints the
/// corpus and the books' own front matter, never a YAML block quoted in prose. So the one
/// surface where a reader learns the vocabulary is the one surface nothing checks.
///
/// Gated on the keys **both** validators reject: a top-level retirement (`front-matter key`
/// or `config key`) that is not in `KNOWN_KEYS` and not in `NATIVE_KEYS`, matched at column
/// 0. Anything narrower would be wrong, because a shipped example is prose — nothing tells
/// this gate whether a given YAML block is front matter or a `_site.yml`. `toc:` and
/// `theorems:` are exactly that case: retired from `_site.yml`, live in front matter, so a
/// `toc: true` line in the manual is legitimate and must not fail.
///
/// Two residual gaps, both covered elsewhere rather than papered over here. A `toc:` shown
/// inside a `_site.yml` example is not caught (`configuration.tmd` is the page that would
/// carry one, and the `_site.yml` schema golden pins the real vocabulary). And a nested
/// retirement (`hero key`, `execute key`, …) is not caught, since its key is indented and
/// collides with live spellings elsewhere. Neither gap is covered by prose any more: the
/// migration page that used to enumerate every retirement was deleted on 2026-08-08 with
/// `quarto_migration_page.rs`, because the register IS the enumeration and a leftover key
/// draws its own note with a file and a line — a reader who hits one is told what to do at
/// the moment they need it, which no page can beat.
#[test]
fn shipped_docs_do_not_use_a_retired_front_matter_key() {
    let keys = retired_keys();
    assert!(
        keys.iter()
            .any(|(s, k)| s == "front-matter key" && k == "about"),
        "expected the retired `about:` key in RETIRED_KEYS, got {keys:?} — if it was \
         un-retired, this gate needs rewriting rather than deleting"
    );
    assert!(keys.len() >= 2, "only {} retired keys parsed", keys.len());

    // The two live top-level vocabularies, read from the same consts the validators use.
    let live_fm = string_literals(const_block(
        &read("crates/core/src/frontmatter.rs"),
        "KNOWN_KEYS",
    ));
    let live_cfg = string_literals(const_block(
        &read("crates/core/src/site/config/mod.rs"),
        "NATIVE_KEYS",
    ));
    assert!(
        live_fm.contains(&"toc".to_string()) && !live_cfg.contains(&"toc".to_string()),
        "this gate's premise is that a key can be live in one vocabulary and retired in the \
         other; `toc` was the case it was written for (live_fm={live_fm:?})"
    );

    let top_level: Vec<&String> = keys
        .iter()
        .filter(|(scope, _)| scope == "front-matter key" || scope == "config key")
        .map(|(_, key)| key)
        .filter(|k| !live_fm.contains(k) && !live_cfg.contains(k))
        .collect();
    assert!(
        top_level.len() >= 2,
        "no unambiguously-retired top-level keys parsed out of {keys:?}"
    );

    let mut hits = Vec::new();
    for (rel, text) in shipped_docs() {
        for (n, line) in text.lines().enumerate() {
            // Column 0 only: an indented `toc:` is a sub-key of something else, and a
            // top-level retirement says nothing about it.
            if line.starts_with(char::is_whitespace) {
                continue;
            }
            for k in &top_level {
                if line.starts_with(&format!("{k}:")) {
                    hits.push(format!("{rel}:{}: {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "shipped doc(s) show a retired top-level key that the validator now rejects \
         ({top_level:?}):\n{}",
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
    // An ANTI-VACUITY floor, not a content floor: a broken extractor yields ~0, not 25.
    // Measured at 49 after Wave 13 cut `taliesin run` and its six flag mentions from the
    // reference (55 before). It sat at 50 with one of headroom, so the cut tripped it, and a
    // floor that close fails the next docs edit for the wrong reason.
    assert!(
        checked >= 25,
        "only {checked} flag mentions examined — the extractor broke"
    );
    assert!(
        missing.is_empty(),
        "the CLI reference documents flag(s) the CLI does not accept: {}",
        missing.join(", ")
    );
}
