//! Static document-lint validators for `qmd-fast check` (the "check-superset").
//!
//! Each takes the rendered block model (and, where needed, the doc base dir) and
//! returns located [`Warning`]s on the same click-to-source channel as the other
//! diagnostics, so `check` becomes a true preflight superset of build/preview and a
//! green `check` means the document is publishable. Read-only static analysis only.

use crate::render::{Block, Warning};
use std::path::Path;

/// 1-based start line from a block's `sourcepos` (`"startLine:col-..."`), if positive.
fn start_line(sourcepos: &str) -> Option<u32> {
    sourcepos
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()
        .filter(|&l| l > 0)
}

/// The `id="..."` attribute of a heading block (`<h1>`..`<h6>`), or None for a
/// non-heading block or a heading with no id. Reads only the opening tag and matches
/// the ` id="` attribute specifically (so `data-block-id="..."` does not false-match).
fn heading_id(html: &str) -> Option<&str> {
    let level_ok = html.as_bytes().get(2).is_some_and(|c| c.is_ascii_digit());
    if !(html.starts_with("<h") && level_ok) {
        return None;
    }
    let tag_end = html.find('>')?;
    let head = &html[..tag_end];
    let i = head.find(" id=\"")? + 5;
    let rest = &head[i..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Two headings that emit the same `id` (e.g. a repeated explicit `{#id}`) produce an
/// invalid duplicate DOM id, so anchors, the TOC, and cross-references silently jump to
/// the first. Auto-slugged ids are already deduped, so a duplicate here is an explicit-id
/// collision the renderer does not catch.
pub fn validate_duplicate_heading_ids(blocks: &[Block]) -> Vec<Warning> {
    use std::collections::HashSet;
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for b in blocks {
        let Some(id) = heading_id(&b.html) else {
            continue;
        };
        if !seen.insert(id) {
            let w = Warning::new(format!(
                "duplicate heading id `{id}`: an earlier heading already uses it, so anchors, the TOC, and cross-references jump to the first"
            ));
            out.push(match start_line(&b.sourcepos) {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// Every value of attribute `attr` (e.g. `"id=\""`) found in `html`, appended to `out`.
fn collect_attr_values<'a>(
    html: &'a str,
    attr: &str,
    out: &mut std::collections::HashSet<&'a str>,
) {
    let mut i = 0;
    while let Some(pos) = html[i..].find(attr) {
        let start = i + pos + attr.len();
        let Some(len) = html[start..].find('"') else {
            break;
        };
        out.insert(&html[start..start + len]);
        i = start + len;
    }
}

/// Same-page `href="#fragment"` values (without `#`) from MANUAL `<a>` links only.
/// `@fig-`/`@sec-`/`@tbl-` cross-references (anchors carrying `qmd-xref`) are skipped:
/// they are validated by `validate_xrefs`, resolved cross-page by the site layer, and may
/// target an id emitted only by code-cell execution (which static `check` does not run).
/// Cross-page `href="page.html#x"` and empty `href="#"` are also skipped.
fn same_page_manual_fragments(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find("<a ") {
        let tag_start = i + pos;
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + rel_end];
        i = tag_start + rel_end + 1;
        if tag.contains("qmd-xref") {
            continue; // a cross-reference, not a manual in-page link
        }
        let Some(hpos) = tag.find("href=\"") else {
            continue;
        };
        let vstart = hpos + "href=\"".len();
        let Some(vlen) = tag[vstart..].find('"') else {
            continue;
        };
        if let Some(frag) = tag[vstart..vstart + vlen].strip_prefix('#')
            && !frag.is_empty()
        {
            out.push(frag);
        }
    }
    out
}

/// In-page anchor links (`[text](#anchor)`) whose `#fragment` matches no element id on
/// the page — a broken jump that silently lands nowhere (or scrolls to the top). The
/// valid-target set is every `id="..."` the page emits, so it never false-flags a real
/// anchor. (`@fig-`/`@sec-` cross-references are covered separately by `validate_xrefs`.)
pub fn validate_internal_anchors(blocks: &[Block]) -> Vec<Warning> {
    // Static check never executes cells; a {python}/{r}/{js} cell can emit the target id at
    // runtime (e.g. `HTML('<div id="x">')`). Conservatively skip the manual-anchor check for
    // any doc with executable cells, so a green check stays a no-false-positive promise.
    if blocks.iter().any(|b| b.cell.is_some()) {
        return Vec::new();
    }
    let mut ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for b in blocks {
        collect_attr_values(&b.html, "id=\"", &mut ids);
    }
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for frag in same_page_manual_fragments(&b.html) {
            if ids.contains(frag) {
                continue;
            }
            let w = Warning::new(format!(
                "broken in-page link: #{frag} (no element with that id on this page)"
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// Whether `v` is a local file reference, i.e. not external, an in-page anchor, a data
/// URI, or a non-file scheme. (Mirrors the asset-bundling heuristic in the build path.)
fn is_local_ref(v: &str) -> bool {
    !v.is_empty()
        && !v.starts_with('#')
        && !v.starts_with("//")
        && !v.contains("://")
        && !v.starts_with("data:")
        && !v.starts_with("mailto:")
        && !v.starts_with("tel:")
        && !v.starts_with("vscode:")
        && !v.starts_with("javascript:")
}

/// Unique local `src="..."` values from `<img>` tags only. Restricted to images on
/// purpose: `<audio>`/`<video>`/`<source>` refs are frequently generated by code
/// execution or are streamed/unvendored heavy media, which a static (no-execution) check
/// cannot resolve — checking them would false-flag. Links (`href=`) are out of scope.
fn local_img_refs(html: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find("<img ") {
        let tag_start = i + pos;
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + rel_end];
        i = tag_start + rel_end + 1;
        let Some(spos) = tag.find("src=\"") else {
            continue;
        };
        let vstart = spos + "src=\"".len();
        let Some(vlen) = tag[vstart..].find('"') else {
            continue;
        };
        let val = &tag[vstart..vstart + vlen];
        if is_local_ref(val) && !out.contains(&val) {
            out.push(val);
        }
    }
    out
}

/// Citations are present (`cite::process` appended the `qmd-references` section) but the
/// front matter declares no `bibliography:`, so every reference renders as a raw key with
/// no diagnostic today. (A declared-but-missing bibliography file is a separate warning.)
pub fn citations_without_bibliography(src: &str, blocks: &[Block]) -> Vec<Warning> {
    let has_citations = blocks.iter().any(|b| b.id == "qmd-references");
    if !has_citations {
        return Vec::new();
    }
    let declares_bib = crate::frontmatter::front_matter_block(src)
        .and_then(|fm| serde_yaml::from_str::<serde_yaml::Value>(fm).ok())
        .and_then(|v| v.as_mapping().map(|m| m.get("bibliography").is_some()))
        .unwrap_or(false);
    if declares_bib {
        return Vec::new();
    }
    vec![Warning::new(
        "citations are present but no `bibliography:` is declared, so every reference renders as a raw key",
    )]
}

/// Local `<img src>` references (`![](img.png)`, raw `<img>`) whose target file does not
/// exist under the doc base dir — a broken image that ships silently today. Absolute
/// (`/...`) and external refs are out of scope; audio/video are skipped (see
/// [`local_img_refs`]: a static check cannot resolve generated/streamed media).
pub fn validate_local_assets(blocks: &[Block], base: &Path) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for val in local_img_refs(&b.html) {
            let path = &val[..val.find(['?', '#']).unwrap_or(val.len())];
            if path.is_empty() || path.starts_with('/') || base.join(path).is_file() {
                continue;
            }
            let w = Warning::new(format!(
                "local asset not found: `{path}` (no such file under the document directory)"
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// The value of attribute `attr` (e.g. `"src=\""`) on the tag opened at the start of
/// `tag` (everything before the first `>`), if present. Used to read `src`/`poster` off
/// a `<video>`/`<source>` tag.
fn tag_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let pos = tag.find(attr)? + attr.len();
    let rest = &tag[pos..];
    let len = rest.find('"')?;
    Some(&rest[..len])
}

/// Unique local **video** refs (`src=`/`poster=`) on every `<video …>` tag plus any
/// `<source …>` *nested inside a `<video>` element*. Mirrors [`local_img_refs`] for the
/// one media case a static check *can* resolve: `{{< video clip.mp4 >}}` (and hand-written
/// `<video src>`/`<video><source></video>`) emit a literal local path, so a missing file is
/// catchable. `<audio>` (and its `<source>` children, often streamed/generated) and `<img>`
/// are handled elsewhere — so we track `<video>`…`</video>` nesting and skip a `<source>`
/// that belongs to an `<audio>` element.
fn local_media_refs(html: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    let mut in_video = false;
    while let Some(pos) = html[i..].find('<') {
        let tag_start = i + pos;
        let after = &html[tag_start..];
        let Some(rel_end) = after.find('>') else {
            break;
        };
        i = tag_start + rel_end + 1;
        let tag = &after[..rel_end];
        // Track <video>…</video> nesting so a <source> is only checked when it belongs to
        // a <video> (not an <audio>, whose sources are often streamed/generated).
        if after.starts_with("</video") {
            in_video = false;
            continue;
        }
        if after.starts_with("<audio") {
            in_video = false; // an <audio> opens a non-video media context
            continue;
        }
        let scan = if after.starts_with("<video") {
            in_video = true;
            true
        } else {
            after.starts_with("<source") && in_video
        };
        if !scan {
            continue;
        }
        for attr in ["src=\"", "poster=\""] {
            if let Some(val) = tag_attr(tag, attr)
                && is_local_ref(val)
                && !out.contains(&val)
            {
                out.push(val);
            }
        }
    }
    out
}

/// Local `<video src>`/`<source src>`/`poster=` references (from `{{< video clip.mp4 >}}`
/// or raw `<video>`) whose target file does not exist under the doc base dir — a broken
/// clip that ships silently. The video sibling of [`validate_local_assets`]: absolute
/// (`/...`) and external refs are out of scope. (`{{< video >}}` renders to raw
/// `<video src>`, so scanning the emitted HTML catches the shortcode and hand-written
/// `<video>` alike.)
pub fn validate_local_media(blocks: &[Block], base: &Path) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for val in local_media_refs(&b.html) {
            let path = &val[..val.find(['?', '#']).unwrap_or(val.len())];
            if path.is_empty() || path.starts_with('/') || base.join(path).is_file() {
                continue;
            }
            let w = Warning::new(format!(
                "local video not found: `{path}` (no such file under the document directory)"
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// Unique local link targets from MANUAL `<a href>` tags only, paired with their tag
/// span so the caller can locate each. Cross-reference links (`qmd-xref`) are skipped
/// (validated by `validate_xrefs`); bare in-page `#fragment` links are skipped (validated
/// by [`validate_internal_anchors`]). Returns each `href` value verbatim (path + optional
/// `#frag`), so a caller can split the path from the fragment.
fn local_link_refs(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = html[i..].find("<a ") {
        let tag_start = i + pos;
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let tag = &html[tag_start..tag_start + rel_end];
        i = tag_start + rel_end + 1;
        if tag.contains("qmd-xref") {
            continue; // a cross-reference, validated separately
        }
        let Some(val) = tag_attr(tag, "href=\"") else {
            continue;
        };
        // A bare in-page anchor (`#frag`) is `validate_internal_anchors`'s job.
        if val.starts_with('#') {
            continue;
        }
        if is_local_ref(val) && !out.contains(&val) {
            out.push(val);
        }
    }
    out
}

/// Whether `path` (relative to a doc at `base`) is backed by a file on disk, accepting the
/// forms a `.qmd`→`.html` site build produces: the literal file, a `.html` link whose
/// `.qmd` source exists, and a directory link (`x/` or `x`) whose `x/index.qmd`/`.html`
/// exists. So a single-doc check doesn't false-flag an intra-project `.html` link whose
/// source page is present (the site build will emit the `.html`).
fn link_target_exists(base: &Path, path: &str) -> bool {
    let join = |p: &str| base.join(p).is_file();
    if join(path) {
        return true;
    }
    // `x.html` → its `x.qmd` source (the built page is produced from it).
    if let Some(stem) = path.strip_suffix(".html")
        && join(&format!("{stem}.qmd"))
    {
        return true;
    }
    // A directory link (`dir/` or `dir`) → that dir's index page (`.qmd`/`.html`).
    let dir = path.trim_end_matches('/');
    base.join(format!("{dir}/index.qmd")).is_file()
        || base.join(format!("{dir}/index.html")).is_file()
}

/// Manual relative links (`[text](other.qmd)`, `[text](sub/page.html#x)`) whose local
/// **target file** does not exist under the doc base dir — a broken cross-file jump that
/// ships silently. External (`http(s)://`, `mailto:`, …) and absolute (`/…`) links are
/// out of scope (external links are never fetched — `check` stays offline + deterministic);
/// bare `#anchor` links and the in-page fragment are handled by
/// [`validate_internal_anchors`]. Cross-page `#fragment` resolution is a site-registry job
/// (the server's site path resolves anchors). A `.html` link whose `.qmd` source exists on
/// disk is accepted (see [`link_target_exists`]) so an intra-project link to a yet-to-be-
/// built page is not false-flagged — only a target with no file *and* no source is broken.
pub fn validate_local_links(blocks: &[Block], base: &Path) -> Vec<Warning> {
    let mut out = Vec::new();
    for b in blocks {
        let line = start_line(&b.sourcepos);
        for val in local_link_refs(&b.html) {
            let path = &val[..val.find(['?', '#']).unwrap_or(val.len())];
            if path.is_empty() || path.starts_with('/') || link_target_exists(base, path) {
                continue;
            }
            let w = Warning::new(format!(
                "broken link: `{path}` (no such file under the document directory)"
            ));
            out.push(match line {
                Some(l) => w.at(b.source_file.clone(), l),
                None => w,
            });
        }
    }
    out
}

/// One `{js}` cell's reactive wiring, distilled from the block model for the static graph
/// check: the names it `defines` (its `//| name` and/or `//| viewof`), the names it
/// `inputs` (its `//| input:` list), and where it lives (for the located warning).
struct JsNode {
    defines: Vec<String>,
    inputs: Vec<String>,
    file: Option<String>,
    line: Option<u32>,
    /// A human label for cycle diagnostics: the first define name, else "(unnamed cell)".
    label: String,
}

/// Static mirror of `qmd-js.js`'s `buildGraph`: flag (a) a `//| input: x` referencing a
/// name that no cell/`{{< input >}}` *defines*, and (b) a dependency cycle among `{js}`
/// cells (Kahn's topo-sort over `define -> consumer` edges; any cell left undrained is in
/// a cycle). Read-only — never touches the reactive runtime.
///
/// Conservative, matching [`validate_internal_anchors`]: a Python `ojs_define` publishes
/// names at *runtime* via a blob a static pass can't enumerate, so when the doc has any
/// non-`{js}` executable cell the *dangling-input* half is suppressed (a name could be
/// defined at runtime). The *cycle* half is a structural fact among `{js}` cells, so it
/// always runs.
pub fn validate_js_reactive_graph(blocks: &[Block]) -> Vec<Warning> {
    let nodes: Vec<JsNode> = blocks
        .iter()
        .filter_map(|b| {
            let cell = b.cell.as_ref()?;
            if cell.lang != "js" {
                return None;
            }
            let mut defines = Vec::new();
            if let Some(n) = cell.js.name.as_deref() {
                defines.push(n.to_string());
            }
            if let Some(v) = cell.js.viewof.as_deref() {
                defines.push(v.to_string());
            }
            let label = defines
                .first()
                .cloned()
                .unwrap_or_else(|| "(unnamed cell)".to_string());
            Some(JsNode {
                defines,
                inputs: cell.js.inputs.clone(),
                file: b.source_file.clone(),
                line: start_line(&b.sourcepos),
                label,
            })
        })
        .collect();
    if nodes.is_empty() {
        return Vec::new();
    }

    // Every statically-known define name: js-cell names/viewofs plus declarative
    // `{{< input name="k" >}}` controls (which emit `data-qmd-input="k"`).
    let mut defined: std::collections::HashSet<String> = std::collections::HashSet::new();
    for n in &nodes {
        for d in &n.defines {
            defined.insert(d.clone());
        }
    }
    for b in blocks {
        let mut vals = std::collections::HashSet::new();
        collect_attr_values(&b.html, "data-qmd-input=\"", &mut vals);
        for v in vals {
            defined.insert(v.to_string());
        }
    }

    let mut out = Vec::new();

    // (a) Dangling inputs — suppressed if a non-js executable cell could define names at
    // runtime (Python/R `ojs_define`).
    let runtime_defines = blocks
        .iter()
        .any(|b| b.cell.as_ref().is_some_and(|c| c.lang != "js"));
    if !runtime_defines {
        let candidates: Vec<String> = defined.iter().cloned().collect();
        for n in &nodes {
            for inp in &n.inputs {
                if defined.contains(inp) {
                    continue;
                }
                let suggestion = closest_owned(inp, &candidates);
                let msg = match suggestion {
                    Some(s) => format!(
                        "unknown reactive input `{inp}`: no `{{js}}` cell or `{{{{< input >}}}}` defines it (did you mean `{s}`?)"
                    ),
                    None => format!(
                        "unknown reactive input `{inp}`: no `{{js}}` cell or `{{{{< input >}}}}` defines it"
                    ),
                };
                let w = Warning::new(msg);
                out.push(match n.line {
                    Some(l) => w.at(n.file.clone(), l),
                    None => w,
                });
            }
        }
    }

    // (b) Cycle detection — Kahn's topological sort over `define -> consumer` edges, the
    // same model `buildGraph` uses. Any node never drained is part of a cycle.
    // consumers[name] = indices of nodes listing `name` in their inputs.
    let mut consumers: std::collections::HashMap<&str, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        for inp in &n.inputs {
            consumers.entry(inp.as_str()).or_default().push(i);
        }
    }
    let mut indeg = vec![0usize; nodes.len()];
    for n in &nodes {
        for d in &n.defines {
            if let Some(cs) = consumers.get(d.as_str()) {
                for &c in cs {
                    indeg[c] += 1;
                }
            }
        }
    }
    let mut queue: std::collections::VecDeque<usize> =
        (0..nodes.len()).filter(|&i| indeg[i] == 0).collect();
    let mut drained = vec![false; nodes.len()];
    while let Some(i) = queue.pop_front() {
        drained[i] = true;
        for d in &nodes[i].defines {
            if let Some(cs) = consumers.get(d.as_str()) {
                for &c in cs {
                    indeg[c] -= 1;
                    if indeg[c] == 0 {
                        queue.push_back(c);
                    }
                }
            }
        }
    }
    for (i, n) in nodes.iter().enumerate() {
        if drained[i] {
            continue;
        }
        let w = Warning::new(format!(
            "reactive dependency cycle involving `{}`: `{{js}}` cells form a loop, so none can run",
            n.label
        ));
        out.push(match n.line {
            Some(l) => w.at(n.file.clone(), l),
            None => w,
        });
    }

    out
}

/// `frontmatter::closest` over an owned candidate list (the reactive-graph define names
/// are dynamic, so they can't be the `&'static` slice that helper wants). Same edit-
/// distance-≤2 "did you mean" rule.
fn closest_owned(key: &str, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .map(|k| (crate::frontmatter::levenshtein(key, k), k))
        .filter(|&(d, _)| d > 0 && d <= 2)
        .min_by_key(|&(d, _)| d)
        .map(|(_, k)| k.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{render_document, render_document_with_includes};

    /// A throwaway directory under the system temp dir, removed on drop.
    struct Tmp(std::path::PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "qmd-diag-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn msgs(ws: &[Warning]) -> Vec<String> {
        ws.iter().map(|w| w.message.clone()).collect()
    }

    #[test]
    fn local_links_flag_missing_relative_target_only() {
        let dir = Tmp::new("links");
        std::fs::write(dir.0.join("exists.qmd"), "x").unwrap();
        let doc = render_document(
            "[gone](missing.qmd) [here](exists.qmd) [ext](https://example.com) \
             [page](sub/page.html#frag) [anchor](#top) [abs](/root.html)\n",
        );
        let ws = validate_local_links(&doc.blocks, &dir.0);
        let m = msgs(&ws);
        assert_eq!(m.len(), 2, "only the two missing local files: {m:?}");
        assert!(m.iter().any(|s| s.contains("`missing.qmd`")), "{m:?}");
        assert!(m.iter().any(|s| s.contains("`sub/page.html`")), "{m:?}");
        // The existing sibling, external, in-page anchor, and absolute links are clean.
        assert!(!m.iter().any(|s| s.contains("exists.qmd")), "{m:?}");
        assert!(!m.iter().any(|s| s.contains("example.com")), "{m:?}");
        assert!(!m.iter().any(|s| s.contains("/root.html")), "{m:?}");
        // Located to a line.
        assert!(ws.iter().all(|w| w.line.is_some()), "located: {ws:?}");
    }

    #[test]
    fn local_links_skip_xref_links() {
        // A `@sec-`/`@fig-` cross-reference renders an `<a … data-qmd-xref>`; it is
        // validated by `validate_xrefs`, so the link checker must not double-flag it.
        let doc = render_document("## Sec {#sec-a}\n\nSee @sec-a.\n");
        let ws = validate_local_links(&doc.blocks, Path::new("."));
        assert!(msgs(&ws).is_empty(), "xref link must be skipped: {ws:?}");
    }

    #[test]
    fn local_links_accept_html_link_with_qmd_source() {
        // A `.html` link whose `.qmd` source exists on disk (an intra-project page link the
        // site build will emit) must NOT be flagged; only a target with no file *and* no
        // source is broken. Mirrors the docs/ cross-book links checked standalone.
        let dir = Tmp::new("links-html");
        std::fs::write(dir.0.join("page.qmd"), "x").unwrap();
        std::fs::create_dir_all(dir.0.join("guide")).unwrap();
        std::fs::write(dir.0.join("guide/index.qmd"), "x").unwrap();
        let doc = render_document(
            "[built page](page.html) [dir link](guide/) [really gone](ghost.html)\n",
        );
        let m = msgs(&validate_local_links(&doc.blocks, &dir.0));
        assert_eq!(m.len(), 1, "only the truly missing target: {m:?}");
        assert!(m[0].contains("`ghost.html`"), "{m:?}");
    }

    #[test]
    fn local_media_flags_missing_video() {
        let dir = Tmp::new("video");
        std::fs::write(dir.0.join("there.mp4"), "x").unwrap();
        let doc = render_document_with_includes(
            "{{< video gone.mp4 >}}\n\n{{< video there.mp4 >}}\n\n\
             <video src=\"https://cdn.example/clip.mp4\"></video>\n",
            &dir.0,
        );
        let ws = validate_local_media(&doc.blocks, &dir.0);
        let m = msgs(&ws);
        assert_eq!(m.len(), 1, "only the missing local clip: {m:?}");
        assert!(m[0].contains("local video not found"), "{m:?}");
        assert!(m[0].contains("`gone.mp4`"), "{m:?}");
        assert!(ws[0].line.is_some(), "located: {ws:?}");
    }

    #[test]
    fn media_dark_and_poster_sources_checked() {
        let dir = Tmp::new("video-dark");
        std::fs::write(dir.0.join("light.mp4"), "x").unwrap();
        // dark= source missing; poster missing.
        let doc = render_document_with_includes(
            "{{< video light.mp4 dark=dark.mp4 poster=cover.png >}}\n",
            &dir.0,
        );
        let m = msgs(&validate_local_media(&doc.blocks, &dir.0));
        assert!(m.iter().any(|s| s.contains("`dark.mp4`")), "{m:?}");
        assert!(m.iter().any(|s| s.contains("`cover.png`")), "{m:?}");
        assert!(!m.iter().any(|s| s.contains("light.mp4")), "{m:?}");
    }

    #[test]
    fn audio_source_is_not_a_video_false_positive() {
        // An `<audio><source>` is NOT a video; its (often streamed/generated) source must
        // not be flagged. A real `<video><source>` next to it IS checked.
        let doc = render_document(
            "<audio controls><source src=\"tone.wav\" type=\"audio/wav\"></audio>\n\n\
             <video><source src=\"clip.mp4\" type=\"video/mp4\"></video>\n",
        );
        let m = msgs(&validate_local_media(&doc.blocks, Path::new(".")));
        assert!(
            !m.iter().any(|s| s.contains("tone.wav")),
            "audio src skipped: {m:?}"
        );
        assert!(
            m.iter().any(|s| s.contains("clip.mp4")),
            "video <source> checked: {m:?}"
        );
    }

    #[test]
    fn js_reactive_graph_flags_dangling_input() {
        let doc = render_document(
            "```{js}\n//| viewof: n\nreturn html`<input type=range>`;\n```\n\n\
             ```{js}\n//| input: n, missing\nreturn n;\n```\n",
        );
        let m = msgs(&validate_js_reactive_graph(&doc.blocks));
        assert_eq!(
            m.len(),
            1,
            "only `missing` is dangling (`n` is defined): {m:?}"
        );
        assert!(m[0].contains("unknown reactive input `missing`"), "{m:?}");
    }

    #[test]
    fn js_reactive_graph_did_you_mean_over_defines() {
        let doc = render_document(
            "```{js}\n//| name: count\nreturn 1;\n```\n\n\
             ```{js}\n//| input: cont\nreturn cont;\n```\n",
        );
        let m = msgs(&validate_js_reactive_graph(&doc.blocks));
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(m[0].contains("did you mean `count`?"), "{m:?}");
    }

    #[test]
    fn js_reactive_graph_input_shortcode_define_clears_dangling() {
        // A declarative `{{< input name="k" >}}` defines `k`, so a cell consuming it is clean.
        let dir = Tmp::new("js-input");
        let doc = render_document_with_includes(
            "{{< input name=\"k\" type=\"slider\" min=\"0\" max=\"10\" >}}\n\n\
             ```{js}\n//| input: k\nreturn k;\n```\n",
            &dir.0,
        );
        let m = msgs(&validate_js_reactive_graph(&doc.blocks));
        assert!(m.is_empty(), "shortcode-defined input must resolve: {m:?}");
    }

    #[test]
    fn js_reactive_graph_detects_cycle() {
        let doc = render_document(
            "```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
             ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
        );
        let m = msgs(&validate_js_reactive_graph(&doc.blocks));
        // Both cells are undrained -> one cycle warning each; `a` and `b` are mutually defined.
        assert_eq!(m.len(), 2, "both cycle members flagged: {m:?}");
        assert!(
            m.iter().all(|s| s.contains("reactive dependency cycle")),
            "{m:?}"
        );
    }

    #[test]
    fn js_dangling_input_suppressed_when_python_cell_present() {
        // A Python `ojs_define` can publish `runtime_name` at runtime, which a static pass
        // can't see; so the presence of any non-js cell suppresses the dangling-input half.
        let doc = render_document(
            "```{python}\nojs_define(runtime_name=5)\n```\n\n\
             ```{js}\n//| input: runtime_name\nreturn runtime_name;\n```\n",
        );
        let m = msgs(&validate_js_reactive_graph(&doc.blocks));
        assert!(m.is_empty(), "dangling-input must be suppressed: {m:?}");
    }

    #[test]
    fn js_cycle_still_flagged_with_python_cell_present() {
        // The cycle half is a structural fact among js cells; it survives a python cell.
        let doc = render_document(
            "```{python}\nx = 1\n```\n\n\
             ```{js}\n//| name: a\n//| input: b\nreturn b;\n```\n\n\
             ```{js}\n//| name: b\n//| input: a\nreturn a;\n```\n",
        );
        let m = msgs(&validate_js_reactive_graph(&doc.blocks));
        assert!(
            m.iter().filter(|s| s.contains("cycle")).count() == 2,
            "cycle still flagged: {m:?}"
        );
    }

    #[test]
    fn js_reactive_graph_clean_chain_is_silent() {
        // n -> squared -> consumer, no cycle, every input defined.
        let doc = render_document(
            "```{js}\n//| viewof: n\nreturn html`<input type=range>`;\n```\n\n\
             ```{js}\n//| name: squared\n//| input: n\nreturn n*n;\n```\n\n\
             ```{js}\n//| input: squared\nreturn squared;\n```\n",
        );
        assert!(
            validate_js_reactive_graph(&doc.blocks).is_empty(),
            "a clean reactive chain must be silent"
        );
    }
}
