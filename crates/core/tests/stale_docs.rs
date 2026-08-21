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
/// `notes/` is deliberately excluded and must stay excluded. It is a dated record — a
/// 2026-06 audit correctly describes the tree as it was that day, and five of the six
/// stale paths the path gate first reported were of exactly that kind. Rewriting a dated
/// document to match today's tree destroys the record; this is the difference between
/// prose that *claims* and prose that *remembers*.
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
        // The two licence documents joined on 2026-08-10; see the gate below.
        "THIRD_PARTY.md",
        "LICENSE-OUTPUT-EXCEPTION.md",
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

/// [`backticked`], but keeping each span's 1-based line number so a gate can point at it.
fn backticked_located(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (n, line) in text.lines().enumerate() {
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
                    out.push((n + 1, rest[..close].to_string()));
                    rest = &rest[close + 1..];
                }
                None => break,
            }
        }
    }
    out
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
    backticked_located(text)
        .into_iter()
        .map(|(_, s)| s)
        .collect()
}

/// These phrases describe machinery deleted in the native rewrite and must not return.
///
/// Not path-bound: it used to read `docs/guide/reference/configuration.tmd` by name, but
/// that page's `_site.yml` content (and the two needles below) merged into
/// `docs/guide/reference/frontmatter.tmd` on 2026-08-14, and a test that re-pointed at the
/// new path would just as soon go stale again on the next rename. Scanning
/// [`reader_facing_docs`] instead makes the assertion survive the move and any future one,
/// and reports which file and line a hit landed in, so a regression is still located.
/// This is the opposite case from the `internals_do_not_describe_the_deleted_shim` needle
/// below: there the subject content was deleted outright, so re-pointing would have been
/// vacuous; here the subject content survives and merely changed file, so the assertion
/// stays live by widening its source rather than by naming a new path.
#[test]
fn docs_do_not_claim_quarto_config_still_works() {
    // Scoped to a line naming Quarto: "still works" alone is ordinary English (a
    // block-model page saying cross-file source mapping "still works", an execution
    // page saying an uncached build "still works") and matched two of those on the
    // first run of the widened gate.
    let mut hits = Vec::new();
    for (rel, text) in reader_facing_docs() {
        for (line, l) in text.lines().enumerate() {
            if l.contains("Quarto") && l.contains("still works")
                || l.contains("Coming from a Quarto config?")
            {
                hits.push(format!("{rel}:{}: {}", line + 1, l.trim()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "a reader-facing doc still claims a Quarto config works, or still carries the \
         stale Quarto-config callout:\n{}",
        hits.join("\n")
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

/// Every token in `text` shaped like a relative file path: a run of path characters
/// holding a `/`, whose last segment carries an extension.
///
/// The extension anchor is deliberate, and it is why this does not see
/// `crates/core/assets/katex/LICENSE` in the release workflow's `cp`. Dropping the anchor
/// would first have to tell that file apart from `actions/checkout@v4`,
/// `dtolnay/rust-toolchain@stable` and every other `owner/repo@ref` action reference,
/// which are exactly the same shape and name nothing in this repo. The three verbatim
/// notices are guarded by name from the other side instead, in
/// `crates/core/tests/third_party.rs`, which also fails if the `Package` step disappears.
fn path_like_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for run in text.split(|c: char| !(c.is_ascii_alphanumeric() || "_./-".contains(c))) {
        // A leading `/` is not part of the claim (`//host/path` in a URL, an absolute
        // path in prose); a trailing `.` or `-` is punctuation, not the name.
        let tok = run
            .trim_start_matches('/')
            .trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        let Some((dir, file)) = tok.rsplit_once('/') else {
            continue;
        };
        if dir.is_empty() || !file.contains('.') {
            continue;
        }
        out.push(tok.to_string());
    }
    out
}

/// The `target: <triple>` entries of the release workflow's build matrix.
fn released_targets() -> Vec<String> {
    read(".github/workflows/release.yml")
        .lines()
        .filter_map(|l| l.trim().strip_prefix("target: "))
        // `targets: ${{ matrix.target }}` on the toolchain step is an expression, not a
        // triple; the matrix entries are literal.
        .filter(|t| !t.contains("${{"))
        .map(|t| t.trim().to_string())
        .collect()
}

/// Neither workflow is run by anything today — every job carries the visibility guard the
/// test above pins — so a path that stops resolving inside one costs nothing until the
/// repo is made public, and then costs a red X on the launch commit, in front of every
/// first visitor and every would-be contributor. `cargo test --workspace` was green on
/// these two files with three dead paths in them: a `run:` target under
/// `crates/server/src/` for a node test deleted with the `publish` verb, the passcode
/// middleware that step claimed to guard, and `crates/core/tests/release_targets.rs`,
/// named by the release workflow as the assertion binding its build matrix to the README
/// and deleted in wave 1 of the cut.
///
/// **This is the narrow exception to "grep, do not gate".** For a shipped doc a stale path
/// costs a reader one confusing sentence, and the author is looking at the file while
/// editing it. For a workflow nobody runs, nobody is looking at all — not on any push, not
/// on any green suite — until publication, when it is too late to be quiet about. There is
/// direct precedent one file over: `gate_script.rs`'s
/// `every_canary_the_gate_script_names_still_exists` guards the identical failure, a
/// script asserting on a name nothing emits.
///
/// Both halves are derived, not listed: the walk finds the workflow files (so a third one
/// cannot arrive unguarded) and the matrix is read out of the workflow that actually
/// produces the artifacts. The README half is the substance of the deleted
/// `release_targets.rs`, restored here rather than as a file of its own, and its two
/// directions fail differently: a target built but not documented is invisible, while a
/// target documented but not built is a promise — the reader downloads nothing and
/// concludes the project is abandoned.
#[test]
fn workflows_do_not_name_a_path_that_does_not_exist() {
    let root = repo();
    let dir = root.join(".github/workflows");
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

    let mut checked = 0usize;
    let mut missing = Vec::new();
    for f in &files {
        let name = f
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let text = std::fs::read_to_string(f).unwrap();
        // The repo root, plus every `working-directory:` this file declares. The allowance
        // is load-bearing, not defensive: the companion job runs
        // `node scripts/ensure-vscode.cjs` under `working-directory: editor/vscode`, and a
        // root-only check reports that real file as missing.
        let bases: Vec<PathBuf> = std::iter::once(root.clone())
            .chain(
                text.lines()
                    .filter_map(|l| l.trim().strip_prefix("working-directory:"))
                    .map(|d| root.join(d.trim())),
            )
            .collect();
        for tok in path_like_tokens(&text) {
            checked += 1;
            if !bases.iter().any(|b| b.join(&tok).exists()) {
                missing.push(format!("{name}: {tok}"));
            }
        }
    }
    // Anti-vacuity, the same reasoning as the shipped-docs path gate below: a scanner that
    // stopped matching yields ~0, and an empty gate passes forever. Measured at 11 on
    // 2026-08-10, of which FOUR sit in an executable `run:`/`cp` line and the other seven
    // are comments citing the test or config that justifies a step. Floored at the
    // executable four: the comment half is prose and may legitimately be rewritten away,
    // and a floor just under the live count would fail the next comment edit for the
    // wrong reason.
    assert!(
        checked >= 4,
        "only {checked} workflow path(s) examined: the scanner stopped matching, so this \
         gate is now green for the wrong reason"
    );
    assert!(
        missing.is_empty(),
        "{} path(s) named in .github/workflows/ do not exist. A `run:` target is a hard \
         failure on the first public run; a comment naming a deleted file is a lie told to \
         the next reader:\n{}",
        missing.len(),
        missing.join("\n")
    );

    // The README's platform matrix and the build matrix are one claim written twice.
    let targets = released_targets();
    assert!(
        targets.len() >= 2,
        "parsed {targets:?} from the release workflow's matrix — the parser broke, or the \
         release workflow stopped shipping binaries"
    );
    let readme = read("README.md");
    for t in &targets {
        assert!(
            readme.contains(&format!("`{t}`")),
            "the release workflow builds `{t}` but README.md's platform matrix never \
             names it, so nobody knows the binary exists (workflow builds {targets:?})"
        );
    }
    for line in readme.lines() {
        for word in line.split('`') {
            let looks_like_a_triple = word.matches('-').count() >= 2
                && (word.ends_with("-gnu")
                    || word.ends_with("-musl")
                    || word.ends_with("-darwin")
                    || word.ends_with("-msvc"));
            if looks_like_a_triple {
                assert!(
                    targets.contains(&word.to_string()),
                    "README.md advertises the target `{word}` but the release workflow \
                     builds only {targets:?} — that is a promise of a download that will \
                     not be there"
                );
            }
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
/// `extensions.rs`, which was by then `theme_css.rs` (itself deleted with the `theme:` key
/// on 2026-08-17).
///
/// `THIRD_PARTY.md` and `LICENSE-OUTPUT-EXCEPTION.md` joined the scanned set on 2026-08-10:
/// they were outside every gate in the tree, and a stale path in a licence document is a
/// claim about bytes this repository redistributes, read by a downstream party before
/// anything else. Note what this still does NOT catch there — `is_repo_path_claim` needs a
/// top-level root or a `.rs` suffix, so a bare unanchored script name in prose slips past.
/// That half is guarded from the other side, in `crates/core/tests/third_party.rs`, which
/// requires every name it exempts from attribution to still be a file.
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

/// The subset of [`shipped_docs`] a *reader* downloads or browses.
///
/// `CLAUDE.md` and `LICENSE-OUTPUT-EXCEPTION.md` are deliberately out of scope for the two
/// gates below, and only for those two. Both legitimately name cut verbs in order to
/// explain their removal: CLAUDE.md records that the `check` verb never had a gate wired
/// and that a retired `run` row survived several edits, and the licence file carries an
/// editorial correction whose whole point is that `render` and `publish` are **not**
/// Taliesin commands. Measured on 2026-08-14: six such mentions, every one correct prose.
/// Gating them would force this project's two most careful documents to stop naming what
/// they removed.
fn reader_facing_docs() -> Vec<(String, String)> {
    let docs: Vec<(String, String)> = shipped_docs()
        .into_iter()
        .filter(|(rel, _)| {
            rel.starts_with("docs/guide")
                || rel.starts_with("docs/internals")
                || rel.starts_with("site")
                || rel == "README.md"
        })
        .collect();
    // Anti-vacuity, the same reason `shipped_docs` carries one: a filter that silently
    // matches nothing makes every gate below pass forever.
    assert!(
        docs.len() > 20,
        "only {} reader-facing docs matched: the path prefixes drifted",
        docs.len()
    );
    docs
}

/// The LIVE verb names, read out of `main.rs`'s `COMMANDS` rather than restated here, so
/// cutting or adding a verb extends this gate automatically.
///
/// Read as text because `taliesin-server` is a binary-only crate with no `lib.rs`, so no
/// test crate can import the const. `gate_script.rs` solves the same problem the same way,
/// and it needs no production change.
fn live_verbs() -> Vec<String> {
    let src = read("crates/server/src/main.rs");
    let (_, table) = src
        .split_once("const COMMANDS: &[Command] = &[")
        .expect("COMMANDS moved or changed shape: update this gate, do not delete it");
    // One `name: "<verb>",` per row, ending at the const's own `\n];`. Every string literal
    // used to be a verb; since FA20 the rows carry their blurb and focused help too, so the
    // field name is the anchor.
    let table = table.split("\n];").next().unwrap_or_default();
    table
        .lines()
        .filter_map(|l| l.trim().strip_prefix("name: \""))
        .filter_map(|l| l.split_once('"').map(|(name, _)| name.to_string()))
        .collect()
}

/// A reader-facing doc must not tell a reader to type a command the binary does not answer.
///
/// The register this used to read (`RETIRED_COMMANDS`) went on 2026-08-17 with the author's
/// FD2 ruling, so the check is INVERTED: instead of matching a closed list of dead verbs,
/// every `` `taliesin <verb>` `` token in the manual must name a verb that is in `COMMANDS`
/// today. That is the stronger direction — it catches an invented verb as well as a cut one
/// — but it is narrower in one way the old spelling was not, and the difference is recorded
/// rather than papered over: a **bare** backticked token (`` `run` `` with no prefix, which
/// is the form wave 13's stale `cli.tmd` row actually took) can no longer be judged, since
/// nothing distinguishes a dead verb from an ordinary word. That exact surface is covered
/// twice over by `main.rs`'s `every_subcommand_has_a_row_in_the_cli_reference`, which walks
/// the reference table in both directions and fails on a row naming a verb the binary does
/// not answer.
///
/// The prefix is anchored to the literal word `taliesin `, so `` `cargo run` `` does not
/// collide with it, and the first token after it is what must be a verb — `` `taliesin
/// build --check-only` `` is fine.
#[test]
fn reader_facing_docs_only_name_subcommands_the_binary_answers() {
    let live = live_verbs();
    assert!(
        live.iter().any(|v| v == "build") && live.iter().any(|v| v == "preview"),
        "COMMANDS parsed as {live:?}: the shape changed, update this gate rather than \
         deleting it"
    );
    assert!(live.len() >= 5, "only {} live verbs parsed", live.len());

    let mut hits = Vec::new();
    for (rel, text) in reader_facing_docs() {
        for (line, tok) in backticked_located(&text) {
            let Some(rest) = tok.strip_prefix("taliesin ") else {
                continue;
            };
            let Some(verb) = rest.split_whitespace().next() else {
                continue;
            };
            // Flags and paths are arguments to a verb, never a verb: `taliesin --version`
            // and `taliesin <file.tmd>` are both legitimate spellings in prose.
            if verb.starts_with('-') || verb.starts_with('<') {
                continue;
            }
            if !live.iter().any(|v| v == verb) {
                hits.push(format!("{rel}:{line}: `{tok}`"));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "reader-facing doc(s) name a subcommand the binary does not answer. Each is either \
         teaching a dead command or inventing one; rewrite the sentence rather than \
         widening this gate:\n{}",
        hits.join("\n")
    );
}

/// README.md's install command hands the reader a literal URL, and a URL with a version in
/// it is a promise that a file exists at that name. `VERSION=v1.0.0` survived the tag that
/// moved past it would 404 for every visitor, which is the same failure
/// `workflows_do_not_name_a_path_that_does_not_exist` guards one level down: a download the
/// project advertises and does not have. The difference is that a triple is checked against
/// the build matrix, while a version can only be checked against the version.
///
/// This is deliberately NOT the "grep, do not trust" case. A stale path in prose costs a
/// reader one confusing sentence; a stale version in a copy-pasteable `curl` costs them the
/// install, silently, at the exact moment they are deciding whether the tool works at all.
#[test]
fn readme_install_command_names_the_current_version() {
    let readme = read("README.md");
    let want = format!("VERSION=v{}", env!("CARGO_PKG_VERSION"));
    let found: Vec<&str> = readme
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("VERSION=v"))
        .collect();
    assert!(
        !found.is_empty(),
        "README.md's install command no longer sets `{want}`. If the download block moved \
         or was reworded, move this gate with it rather than deleting it: the version in a \
         copy-pasteable URL is the one number here that 404s instead of merely reading oddly."
    );
    for line in &found {
        assert!(
            line.starts_with(&want),
            "README.md's install command says `{line}` but the workspace is at version {}. \
             That URL resolves to nothing, so the first thing a new reader runs fails.",
            env!("CARGO_PKG_VERSION")
        );
    }
}
