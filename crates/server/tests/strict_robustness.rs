//! Server-robustness CLI behaviors, exercised end-to-end through the real binary (the
//! exit codes are `std::process::ExitCode`, opaque to a unit test, so these go through
//! `CARGO_BIN_EXE_taliesin`):
//!
//! - a malformed `_site.yml` is a `--strict` build problem (a silently-degraded site must
//!   not ship green); a *missing* `_site.yml` is a harder, unconditional failure (a
//!   directory is refused as not a project regardless of `--strict` -- see
//!   `project_required.rs`);
//! - a page the site build could not read or write fails it unconditionally (the deploy
//!   still holds the previous body, so exit 0 said "current" about a stale page);
//! - an unknown `--flag` is a hard error with a did-you-mean (not silently dropped);
//! - a value-less `--out` is a hard error (not a silent `<stem>.html` write).

use std::fs;
use std::process::Command;

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tali-robust-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn taliesin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_taliesin"))
}

#[test]
fn malformed_site_yml_fails_strict_build() {
    let dir = tmp_dir("malformed");
    // Unterminated double-quoted scalar -> serde_yaml parse error -> degraded default site.
    fs::write(dir.join("_site.yml"), "title: \"unterminated\nfoo: bar\n").unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
    let out = dir.join("_site");

    // Without --strict the build still WRITES the degraded site, but no longer exits 0:
    // since 2026-08-13 an unparseable YAML block fails unconditionally, because nothing in
    // it was read (no title, no nav, no `url:` and so no feed or sitemap) and the old exit
    // 0 was the one outcome CI reads as "fine". `--strict` is what escalates every *other*
    // problem; it is not what decides this one.
    let lenient = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run lenient build");
    let lenient_err = String::from_utf8_lossy(&lenient.stderr);
    assert!(
        !lenient.status.success(),
        "an unparseable _site.yml fails with no --strict: {lenient_err}"
    );
    assert!(
        lenient_err.contains("not valid YAML"),
        "the malformed config is still reported: {lenient_err}"
    );
    assert!(
        out.join("index.html").exists(),
        "the degraded site is still written; only the exit code changes"
    );

    // With --strict the same malformed config must fail the build (non-zero exit).
    let strict = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--strict")
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run strict build");
    let strict_err = String::from_utf8_lossy(&strict.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !strict.status.success(),
        "a malformed _site.yml must fail --strict, stderr was:\n{strict_err}"
    );
    // Deliberately NOT the `--strict` tally: the unparseable-YAML failure takes precedence
    // and does not name the flag, because `--strict` neither caused this failure nor can
    // suppress it. Pointing at it here would read as "turn this off to make it pass".
    assert!(
        strict_err.contains("unparseable YAML block"),
        "the failure names the unparseable block, not --strict: {strict_err}"
    );
}

#[test]
fn a_page_the_site_build_cannot_write_fails_regardless_of_strict() {
    // A read-only output file is the ordinary shape of this: a deploy directory whose
    // permissions changed, a page checked out read-only, an ACL. The write fails, the
    // error prints -- and the build used to close with `built … 1 page` and exit 0 while
    // the sweep kept the page's URL, so the deploy still held the PREVIOUS body. That is
    // the one outcome CI reads as "the site is current". The single-doc path has always
    // returned FAILURE here; the two verbs disagreed.
    let dir = tmp_dir("readonly-out");
    let out = dir.join("_site");
    fs::write(dir.join("_site.yml"), "title: Probe\n").unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nFirst.\n").unwrap();

    let first = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .arg("--no-exec")
        .output()
        .expect("first build");
    assert!(first.status.success(), "the first build is clean");

    let page = out.join("index.html");
    let mut perms = fs::metadata(&page).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o444);
    }
    perms.set_readonly(true);
    fs::set_permissions(&page, perms).unwrap();
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nSecond.\n").unwrap();

    for extra in [&[][..], &["--strict"][..]] {
        let res = taliesin()
            .arg("build")
            .arg(&dir)
            .arg("--out")
            .arg(&out)
            .arg("--no-exec")
            .args(extra)
            .output()
            .expect("rebuild over a read-only page");
        let err = String::from_utf8_lossy(&res.stderr);
        assert!(
            !res.status.success(),
            "a page that could not be written must fail the build ({extra:?}): {err}"
        );
        assert!(
            err.contains("could not be read or written"),
            "the closing verdict names what happened ({extra:?}): {err}"
        );
    }
    assert!(
        fs::read_to_string(&page).unwrap().contains("First."),
        "and the stale body is what is still deployed, which is why exit 0 was a lie"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_site_yml_fails_regardless_of_strict() {
    // A bare directory of `.tmd` pages (no `_site.yml`) is no longer a legitimate build
    // target at all (see `project_required.rs`): the failure is unconditional, so
    // `--strict` is not what decides it -- a lenient build of the same directory fails
    // exactly the same way.
    let dir = tmp_dir("nofile");
    fs::write(dir.join("index.tmd"), "---\ntitle: Home\n---\n\nWelcome.\n").unwrap();
    let out = dir.join("_site");
    let res = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--strict")
        .arg("--out")
        .arg(&out)
        .output()
        .expect("run strict build");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "a missing _site.yml must fail even without --strict, stderr was:\n{err}"
    );
    assert!(err.contains("no _site.yml"), "names the reason: {err}");
}

#[test]
fn single_doc_malformed_front_matter_fails_strict() {
    // Batch 5: a single-doc `build` used to skip yaml_error(), so a typo'd `---` block
    // built clean and passed --strict. It must now be a --strict problem.
    let dir = tmp_dir("singleyaml");
    let doc = dir.join("post.tmd");
    // `bad: : x` is a YAML syntax error (a mapping value that is itself a bare colon).
    fs::write(&doc, "---\ntitle: OK\nbad: : x\n---\n\nProse.\n").unwrap();

    // Lenient: still WRITES the degraded page, but fails since 2026-08-13. Every key in an
    // unparseable block is dropped, so this page shipped without the `title:` it declares
    // -- and with a `bibliography:`/`listing:` silently gone -- having printed `built` and
    // exited 0. An author who never learns `--check-only` publishes that.
    let lenient = taliesin()
        .arg("build")
        .arg(&doc)
        .output()
        .expect("lenient build");
    let lenient_err = String::from_utf8_lossy(&lenient.stderr);
    assert!(
        !lenient.status.success(),
        "unparseable front matter fails with no --strict: {lenient_err}"
    );
    assert!(
        dir.join("post.html").exists(),
        "the degraded page is still written; only the exit code changes"
    );

    let strict = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--strict")
        .output()
        .expect("strict build");
    let err = String::from_utf8_lossy(&strict.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !strict.status.success(),
        "malformed single-doc front-matter must fail --strict, stderr:\n{err}"
    );
}

#[test]
fn single_doc_embed_counts_toward_strict() {
    // Batch 7: an unresolved `{{< embed >}}` in a single-doc build warns (its target
    // isn't built beside the page), but the warning never counted toward `problems`, so
    // `--strict` passed green despite shipping a dead iframe. It must now fail --strict.
    let dir = tmp_dir("embedstrict");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\n{{< embed talk.tmd >}}\n").unwrap();

    // Lenient: still builds (the warning is non-fatal), exit 0.
    let lenient = taliesin()
        .arg("build")
        .arg(&doc)
        .output()
        .expect("lenient build");
    assert!(
        lenient.status.success(),
        "an embed warning without --strict still builds"
    );

    let strict = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--strict")
        .output()
        .expect("strict build");
    let err = String::from_utf8_lossy(&strict.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !strict.status.success(),
        "an unresolved single-doc embed must fail --strict, stderr:\n{err}"
    );
    assert!(
        err.contains("--strict") && err.contains("problem"),
        "the strict failure names the problem count: {err}"
    );
}

#[test]
fn build_rejects_unknown_flag_with_suggestion() {
    let dir = tmp_dir("badflag");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    let res = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--stict") // typo for --strict
        .output()
        .expect("run build");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "an unknown flag must fail the build, stderr was:\n{err}"
    );
    assert!(
        err.contains("--stict") && err.contains("--strict"),
        "the error names the bad flag and suggests --strict: {err}"
    );
}

#[test]
fn build_rejects_value_less_out_flag() {
    let dir = tmp_dir("noout");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    // `--out` at the end of args (no directory value): a hard error, not a silent
    // `<stem>.html` write.
    let res = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--out")
        .output()
        .expect("run build");
    let err = String::from_utf8_lossy(&res.stderr);
    let html = dir.join("post.html");
    let wrote_default = html.exists();
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "a value-less --out must fail, stderr was:\n{err}"
    );
    assert!(
        err.contains("--out") && err.contains("requires"),
        "the error explains --out needs a value: {err}"
    );
    assert!(
        !wrote_default,
        "a value-less --out must not silently write the default <stem>.html"
    );
}

#[test]
fn build_into_pdf_is_rejected() {
    // DX11: `build doc.tmd doc.pdf` used to write HTML bytes into `doc.pdf`, log `built
    // doc.pdf`, and exit 0 (the academic persona opens a "PDF" full of `<!DOCTYPE html>`).
    // It must now be a hard error that writes nothing and explains HTML-only + the fix.
    let dir = tmp_dir("pdftarget");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    let pdf = dir.join("post.pdf");
    let res = taliesin()
        .arg("build")
        .arg(&doc)
        .arg(&pdf)
        .output()
        .expect("run build");
    let err = String::from_utf8_lossy(&res.stderr);
    let wrote_pdf = pdf.exists();
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "a .pdf output target must fail the build, stderr was:\n{err}"
    );
    assert!(
        !wrote_pdf,
        "the rejected build must not write HTML bytes into the .pdf file"
    );
    assert!(
        err.contains(".pdf") && err.contains("HTML only"),
        "the error names the extension and states HTML-only: {err}"
    );
    assert!(
        err.contains("ROADMAP") && err.contains("Print"),
        "the error points at the planned print track + the browser-Print escape hatch: {err}"
    );
}

#[test]
fn nonstrict_build_summarizes_problems() {
    // DX12: without `--strict`, a single-doc build with a problem (here a broken
    // cross-reference: a located warning that needs no kernel) still writes the page
    // and exits 0 — but the per-warning lines have already scrolled past. It must print
    // a closing tally naming the count + the flag that would have failed it, so the
    // silent degradation isn't shipped with a green, wordless exit.
    let dir = tmp_dir("nonstrict-summary");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, "---\ntitle: Doc\n---\n\nSee @fig-nope for details.\n").unwrap();
    let res = taliesin()
        .arg("build")
        .arg(&doc)
        .output()
        .expect("lenient build");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        res.status.success(),
        "a non-strict build with problems still writes (exit 0): {err}"
    );
    // The per-warning line proves the problem was detected...
    assert!(
        err.contains("broken cross-reference"),
        "the broken cross-ref is reported: {err}"
    );
    // ...and DX12 adds the closing tally naming the count + `--strict`.
    assert!(
        err.contains("1 problem") && err.contains("--strict"),
        "a non-strict build names its problem count + --strict: {err}"
    );
}

#[test]
fn nonstrict_site_build_summarizes_problems() {
    // DX12, site path: a build that ships with problems must print the same closing tally
    // as the single-doc path, pointing at `--strict`, rather than a wordless green exit
    // after pages of scrolled-past warnings.
    //
    // The fixture is a *warning*-severity problem (a missing local image) and not the
    // malformed `_site.yml` it used to be: since 2026-08-13 an unparseable YAML block
    // fails unconditionally, so that fixture would exercise the failure path and this
    // assertion -- that a problem can ship green with a tally -- would test nothing.
    // `malformed_site_yml_fails_strict_build` covers the unparseable case.
    let dir = tmp_dir("nonstrict-site-summary");
    fs::write(dir.join("_site.yml"), "title: Fine\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n![a chart](missing.png)\n",
    )
    .unwrap();
    let out = dir.join("_site");
    let res = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("lenient site build");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        res.status.success(),
        "a non-strict site build with a degraded config still ships (exit 0): {err}"
    );
    assert!(
        err.contains("problem") && err.contains("--strict"),
        "a non-strict site build names its problem count + --strict: {err}"
    );
}

#[test]
fn the_lint_rejects_an_unknown_flag_with_a_suggestion() {
    let dir = tmp_dir("checkflag");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    let res = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--check-only")
        .arg("--formt") // typo for --format
        .arg("json")
        .output()
        .expect("run the lint");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "an unknown flag must fail, stderr was:\n{err}"
    );
    assert!(
        err.contains("--formt") && err.contains("--format"),
        "the error names the bad flag and suggests --format: {err}"
    );
}

#[test]
fn preview_rejects_unknown_flag_with_suggestion() {
    let dir = tmp_dir("previewflag");
    let doc = dir.join("post.tmd");
    fs::write(&doc, "---\ntitle: Post\n---\n\nProse.\n").unwrap();
    // A typo'd `--noexec` (for --no-exec) must fail fast, before the server binds a port,
    // and this one is not cosmetic: dropped silently, the preview would run every cell.
    let res = taliesin()
        .arg("preview")
        .arg(&doc)
        .arg("--noexec")
        .output()
        .expect("run preview");
    let err = String::from_utf8_lossy(&res.stderr);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        !res.status.success(),
        "an unknown preview flag must fail, stderr was:\n{err}"
    );
    assert!(
        err.contains("--noexec") && err.contains("--no-exec"),
        "the error names the bad flag and suggests --no-exec: {err}"
    );
}

/// The confidence gap: the static validators once ran only inside the `check` verb, so a
/// `build --strict` exited 0 while shipping a broken `<img>`. A green `--strict` reads as
/// "safe to ship", so it must fail on exactly what `--check-only` fails on. Both go through
/// `lint::page_static_diagnostics` now, and this is what keeps that true end to end.
#[test]
fn strict_build_fails_on_everything_the_lint_fails_on() {
    let dir = tmp_dir("strict-superset");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("about.tmd"),
        "---\ntitle: About\n---\n\n## Team {#team}\n\nHi.\n",
    )
    .unwrap();
    // Five defects `build --strict` used to miss entirely or count without locating:
    // a duplicate heading id, a broken in-page anchor, a missing image, a link to a page
    // that does not exist, and an anchor that does not exist on a page that does.
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n## A {#dup}\n\n## B {#dup}\n\n\
         See [anchor](#nope) and ![a missing chart](missing.png).\n\
         A [cross-page](ghost.tmd) link and a [bad anchor](about.tmd#nope).\n",
    )
    .unwrap();

    let out = dir.join("_out");
    let check = taliesin()
        .arg("build")
        .arg(&dir)
        .arg("--check-only")
        .output()
        .unwrap();
    assert!(!check.status.success(), "the lint must fail on this site");

    let strict = taliesin()
        .args(["build"])
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .arg("--strict")
        .output()
        .unwrap();
    assert!(
        !strict.status.success(),
        "build --strict must fail on what check fails on; stderr:\n{}",
        String::from_utf8_lossy(&strict.stderr)
    );

    // Every diagnostic the lint reports is reported by the build, located. The two surfaces
    // decorate the SAME located defect differently, in two ways:
    //   * the lint's linter line is `file:line: severity: message`, while `build --strict`
    //     logs it via `log::warn` as `warn  file:line: message` (the log level already
    //     conveys severity), so the `severity: ` insertion is stripped below;
    //   * the lint roots each path on the target as typed (here an absolute temp dir) so the
    //     path opens from wherever the command ran, while `build`'s log label is still
    //     project-relative. That divergence is why the target prefix comes off below.
    // The identity under test is the SET OF LOCATED DEFECTS, not how either spells a path.
    let prefix = format!("{}/", dir.display());
    let check_msgs: Vec<String> = String::from_utf8_lossy(&check.stderr)
        .lines()
        .filter(|l| l.contains(": error: ") || l.contains(": warning: "))
        .map(|l| {
            let l = l.trim();
            let (loc, rest) = l.split_once(": ").expect("a located finding line");
            let loc = loc.strip_prefix(&prefix).unwrap_or(loc);
            let msg = rest
                .strip_prefix("error: ")
                .or_else(|| rest.strip_prefix("warning: "))
                .unwrap_or(rest);
            format!("{loc}: {msg}")
        })
        .collect();
    assert_eq!(
        check_msgs.len(),
        5,
        "expected 5 findings, got {check_msgs:?}"
    );
    let build_err = String::from_utf8_lossy(&strict.stderr).to_string();
    for m in &check_msgs {
        assert!(
            build_err.contains(m.as_str()),
            "build --strict omitted `{m}`\nbuild stderr:\n{build_err}"
        );
    }

    // Without `--strict` the page is still written and the build succeeds: the lints warn,
    // they do not gate an ordinary build.
    let plain = taliesin()
        .args(["build"])
        .arg(&dir)
        .arg("--out")
        .arg(dir.join("_out2"))
        .output()
        .unwrap();
    assert!(plain.status.success(), "a plain build still succeeds");
    assert!(
        dir.join("_out2/index.html").is_file(),
        "and still writes the page"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// The same wiring on the single-document path, whose validator set differs by one rule
/// (`validate_local_links` runs standalone but not in a site, where a `.tmd` link rewrites
/// to `.html` and only the page registry knows the real url).
#[test]
fn strict_single_doc_build_fails_on_a_missing_image() {
    let dir = tmp_dir("strict-single");
    let doc = dir.join("doc.tmd");
    fs::write(&doc, "---\ntitle: T\n---\n\n![img](missing.png)\n").unwrap();

    let check = taliesin()
        .arg("build")
        .arg(&doc)
        .arg("--check-only")
        .output()
        .unwrap();
    assert!(!check.status.success());

    let strict = taliesin()
        .args(["build"])
        .arg(&doc)
        .arg(dir.join("out.html"))
        .arg("--strict")
        .output()
        .unwrap();
    assert!(
        !strict.status.success(),
        "single-doc --strict must fail on a missing image"
    );
    let err = String::from_utf8_lossy(&strict.stderr);
    assert!(err.contains("missing.png"), "names the asset: {err}");
    // Located to its line AND to a path a tool can open. This asserted `doc:5:` — the bare
    // `file_stem()` — which is what a single-doc build used to print and what no editor,
    // `vim +N`, or CI annotation resolves. The path the user passed is the label now.
    assert!(
        err.contains(&format!("{}:5:", doc.display())),
        "located to an openable path + line: {err}"
    );
    assert!(
        !err.contains("doc:5:"),
        "the bare-stem label must not come back: {err}"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// The `Scope::InSite` carve-out. An intra-site `[x](other.tmd)` link is legitimate: it
/// rewrites to `other.html` at build time. Running the single-doc link rule on site pages
/// would report every internal link as broken, so a correct site must build green.
#[test]
fn strict_site_build_does_not_flag_a_working_intra_site_link() {
    let dir = tmp_dir("strict-intrasite");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n[About](about.tmd)\n",
    )
    .unwrap();
    fs::write(dir.join("about.tmd"), "---\ntitle: About\n---\n\nHi.\n").unwrap();

    let strict = taliesin()
        .args(["build"])
        .arg(&dir)
        .arg("--out")
        .arg(dir.join("_out"))
        .arg("--strict")
        .output()
        .unwrap();
    assert!(
        strict.status.success(),
        "a correct site must build green under --strict; stderr:\n{}",
        String::from_utf8_lossy(&strict.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The audit's original reproduction, isolated: a site whose *only* defect is a missing
/// image. Nothing else fails it, so this is the test that actually pins the per-page
/// static-validator wiring (a broken cross-page link would fail the build by itself).
#[test]
fn strict_site_build_fails_on_a_missing_image_alone() {
    let dir = tmp_dir("strict-missing-img");
    fs::write(dir.join("_site.yml"), "title: S\n").unwrap();
    fs::write(
        dir.join("index.tmd"),
        "---\ntitle: Home\n---\n\n![missing](does-not-exist.png)\n",
    )
    .unwrap();

    let out = dir.join("_out");
    let strict = taliesin()
        .args(["build"])
        .arg(&dir)
        .arg("--out")
        .arg(&out)
        .arg("--strict")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&strict.stderr);
    assert!(
        !strict.status.success(),
        "build --strict shipped a broken <img> with exit 0; stderr:\n{err}"
    );
    assert!(err.contains("does-not-exist.png"), "names the asset: {err}");
    assert!(err.contains("index.tmd:5:"), "located to its line: {err}");
    let _ = fs::remove_dir_all(&dir);
}
