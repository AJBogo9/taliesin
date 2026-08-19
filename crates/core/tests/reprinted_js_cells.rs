//! A `{js}` cell cannot echo its own source, so a page that wants to SHOW the cell it is
//! running has to reprint it in a display fence. Nothing else makes the two agree.
//!
//! Verified against the binary on 2026-08-19 rather than assumed: a `{js}` cell emits no
//! source listing with `//| echo: true` any more than without it (the only copy of the
//! source in a built page is the executable blob handed to the browser, not a `<pre>`).
//! So the reprint is a real workaround for a real gap, not redundancy that could be cut,
//! and it will stay until a `{js}` cell can list itself.
//!
//! `site/showcase.tmd` carries one such pair today. Wave 3 of the 2026-08 docs audit cut
//! the other two as page weight (showcase.html fell 221,792 -> 79,113 bytes); this one is
//! deliberate, because "here is the cell that draws the thing above it" is the page's
//! point. Through that whole pass the pair stayed byte-identical only because a person
//! remembered to edit both halves, which is exactly the property a test should hold
//! instead.
//!
//! **What establishes a pair, and why it is not the filename.** A plain ` ```js ` fence is
//! not automatically a transcript: `docs/guide/using/interactive.tmd` has five that teach
//! the `//|` option syntax and correspond to no cell on the page, and
//! `docs/internals/extending.tmd` has one that is an enhancer example. Naming the guarded
//! file instead would be a hand-kept list, and this tree has already shipped one of those
//! that undercounted (see `three_scene_theme.rs`). So a pair is established by the
//! **first line**: a display fence and a live cell in the same file that open on the same
//! line are the same cell, and must then match to the byte. Measured against every `.tmd`
//! in the tree, that pairs showcase's transcript and nothing else: interactive's live
//! cell opens `//| input: k` while none of its display fences do.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.tmd` a person wrote. `_freeze/` is generated, `notes/` and `target/` are not
/// shipped, and a dot-directory is either not ours or a tool's scratch space.
fn all_tmd() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if p.is_dir() {
                if name.starts_with('.')
                    || matches!(name, "target" | "_freeze" | "notes" | "node_modules")
                {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "tmd") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root(), &mut out);
    out.sort();
    assert!(!out.is_empty(), "found no .tmd files");
    out
}

/// `(is_live_cell, first_line, body, line_number)` for every ` ```{js} ` cell and every
/// plain ` ```js ` display fence in `src`, in source order. A fence with an empty body is
/// skipped: it pairs with everything and pins nothing.
fn js_fences(src: &str) -> Vec<(bool, String, String, usize)> {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let live = match lines[i].trim() {
            "```{js}" => true,
            "```js" => false,
            _ => {
                i += 1;
                continue;
            }
        };
        let open = i + 1;
        let mut body = Vec::new();
        i += 1;
        while i < lines.len() && !lines[i].starts_with("```") {
            body.push(lines[i]);
            i += 1;
        }
        i += 1; // step over the closing fence
        if let Some(first) = body.first() {
            out.push((live, (*first).to_string(), body.join("\n"), open));
        }
    }
    out
}

#[test]
fn a_reprinted_js_cell_is_byte_identical_to_the_cell_it_reprints() {
    let root = repo_root();
    let mut pinned = 0usize;
    let mut problems: Vec<String> = Vec::new();

    for path in all_tmd() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let fences = js_fences(&src);
        for (live, first, body, at) in fences.iter().filter(|(live, ..)| !live) {
            debug_assert!(!live);
            let same_opening: Vec<&(bool, String, String, usize)> = fences
                .iter()
                .filter(|(is_live, f, ..)| *is_live && f == first)
                .collect();
            if same_opening.is_empty() {
                continue; // a standalone example, not a transcript of anything
            }
            pinned += 1;
            if !same_opening.iter().any(|(.., b, _)| b == body) {
                let cells: Vec<String> = same_opening
                    .iter()
                    .map(|(.., line)| format!("line {line}"))
                    .collect();
                problems.push(format!(
                    "{rel}:{at}: the display fence opening `{first}` no longer matches the \
                     `{{js}}` cell it reprints ({}). A `{{js}}` cell cannot echo itself, so \
                     the two halves are only ever equal by hand: edit both, or delete the \
                     reprint",
                    cells.join(", ")
                ));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "{} reprinted `{{js}}` cell(s) have drifted:\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
    // Anti-vacuity: measured at 1 on 2026-08-19 (site/showcase.tmd's harmonics cell). The
    // pairing rule keys on the opening line, so an edit that rewrites BOTH halves' first
    // line would dissolve the pair rather than fail above, which is what this catches.
    assert!(
        pinned >= 1,
        "no display fence was paired with a `{{js}}` cell, so this gate pinned nothing: \
         either the reprint was deleted (then delete this test) or the pairing rule stopped \
         matching it"
    );
}
