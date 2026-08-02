//! `{{< dataset path-or-url >}}` — a provenance card for the data a document was computed
//! from (backlog item 176).
//!
//! **Why provenance and not a bundle.** A `data/train.csv` referenced only inside a
//! `{python}` string is invisible to the build: `copy_local_assets` follows `src=`/`href=`
//! in the emitted HTML and nothing else, so the file is not copied, not validated and not
//! mentioned. The obvious fix — ship the data with the book — is wrong in the direction
//! that matters: a multi-GB parquet does not belong in a folder of HTML. The failure worth
//! fixing is that a **reader cannot re-run the document**, and the web-native answer to
//! that is a citation, not a copy.
//!
//! So a card states what the file is, how big it is, what it hashes to, and where it came
//! from. An in-tree file gets a download link, which also hands `copy_local_assets` the
//! `href=` it needs to actually ship a small file. A remote one gets its URL and a
//! verify-after-download snippet instead.
//!
//! **Zero configuration is the default.** For a file in the tree, size and digest are read
//! off the file itself, so `{{< dataset data/penguins.csv >}}` alone renders a complete
//! card. The arguments are only for what a file cannot tell you (licence, where it came
//! from, a human title) and for a remote file, which has nothing local to interrogate.
//!
//! **Those annotations ride the shortcode, not front matter** (backlog item 204, 2026-08-02).
//! They used to be a `datasets:` front-matter block addressing each entry by `path:`/`url:`.
//! Measured across every `.tmd` in the repo, the shortcode had 11 uses and the block had
//! **zero** outside its own pin and the page documenting it. The block's filed problem was
//! that it re-declared what the card already derives, and that turned out to be false: the
//! derivable half was already derived and everything left really is underivable. The real
//! cost was the shape. A key costs six drift gates, and addressing an entry back to an
//! invocation by repeating its path is an index to keep in step for no gain, when the
//! invocation is right there. `{{< video >}}` already carries `poster=`/`caption=` inline,
//! so this is the house idiom rather than a new one, and the argument vocabulary is closed
//! and typo-linted like every other.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::render::{Warning, escape_attr};

/// What one `{{< dataset >}}` invocation declared. Every field is optional because the
/// point is to add only what the file cannot say about itself.
///
/// **There is no `path`/`url` field.** The shortcode's positional argument already names
/// the data, so an addressing field would be a second copy of it that can fall out of step.
#[derive(Debug, Clone, Default)]
pub(crate) struct Declared {
    pub title: Option<String>,
    pub licence: Option<String>,
    pub source: Option<String>,
    pub description: Option<String>,
    /// A digest the author recorded. When the file is in the tree this is **checked**
    /// against it, which is what turns "my figure changed and I do not know why" into a
    /// diagnostic. For a remote file it is the only thing a reader can verify with.
    pub sha256: Option<String>,
    /// Declared size, used only for a remote file (an in-tree one is measured).
    pub bytes: Option<u64>,
}

/// The `key=value` arguments `{{< dataset >}}` accepts, read by `SHORTCODE_SPECS` so the
/// same did-you-mean that catches `postr=` on a video catches `souce=` here. Closed, like
/// every other vocabulary in the tool.
pub(crate) const DATASET_KEYS: &[&str] = &[
    "title",
    "licence",
    "license",
    "source",
    "description",
    "sha256",
    "bytes",
];

/// Read the invocation's `key=value` arguments. An unrecognized or malformed one yields
/// nothing here rather than an error: `validate_shortcode_args` is what reports it (with a
/// did-you-mean), and a card that falls back to what the file itself says is better than a
/// document that fails to render.
pub(crate) fn from_args(args: &[String]) -> Declared {
    let get = |k: &str| -> Option<String> {
        args.iter().find_map(|a| {
            a.strip_prefix(k)
                .and_then(|rest| rest.strip_prefix('='))
                .map(|v| v.trim_matches(['"', '\'']).to_owned())
                .filter(|v| !v.is_empty())
        })
    };
    Declared {
        title: get("title"),
        // Both spellings, because both are correct English and an author should not have
        // to guess which one this tool picked.
        licence: get("licence").or_else(|| get("license")),
        source: get("source"),
        description: get("description"),
        sha256: get("sha256"),
        bytes: get("bytes").and_then(|b| b.parse().ok()),
    }
}

/// Whether `target` names a remote resource rather than a file in the tree.
fn is_remote(target: &str) -> bool {
    super::url_scheme(target).is_some()
}

/// `13.8 kB`, `2.4 GB` — SI units, because that is what a data portal quotes and what a
/// reader will compare against. Deliberately not KiB: matching the page the file came from
/// matters more here than matching `ls`.
fn human_bytes(n: u64) -> String {
    const UNITS: [(&str, u64); 4] = [
        ("GB", 1_000_000_000),
        ("MB", 1_000_000),
        ("kB", 1_000),
        ("B", 1),
    ];
    for (unit, scale) in UNITS {
        if n >= scale {
            if scale == 1 {
                return format!("{n} B");
            }
            let whole = n / scale;
            let frac = (n % scale) * 10 / scale;
            return if whole >= 100 || frac == 0 {
                format!("{whole} {unit}")
            } else {
                format!("{whole}.{frac} {unit}")
            };
        }
    }
    "0 B".to_owned()
}

/// The SHA-256 of a file, lowercase hex, or `None` when it cannot be read.
fn file_sha256(p: &Path) -> Option<String> {
    let bytes = std::fs::read(p).ok()?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Some(format!("{:x}", h.finalize()))
}

/// A digest shortened for display, with the full value kept in `title=` so it can still be
/// copied. A 64-hex-character string in the middle of a card is noise; the first twelve
/// are enough to compare at a glance, which is what a reader actually does.
fn short_digest(hex: &str) -> String {
    match hex.len() > 16 {
        true => format!("{}…{}", &hex[..8], &hex[hex.len() - 4..]),
        false => hex.to_owned(),
    }
}

/// Render one `{{< dataset … >}}`, plus any warnings it earned.
///
/// `decl` is what THIS invocation declared, so there is no lookup and nothing to mismatch.
/// `base_dir` is the document's directory; `None` (a render with no filesystem context)
/// means an in-tree file cannot be measured, so the card states only what was declared.
pub(crate) fn render(
    target: &str,
    decl: &Declared,
    base_dir: Option<&Path>,
    line_no: usize,
) -> (String, Vec<Warning>) {
    let mut warnings = Vec::new();
    let remote = is_remote(target);

    // What the file itself says, when there is a file. This is the half that needs no
    // configuration, and the half that can contradict the declaration.
    let local = (!remote)
        .then(|| base_dir.map(|d| d.join(target)))
        .flatten()
        .filter(|p| p.is_file());

    let measured_bytes = local
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len());
    let measured_sha = local.as_ref().and_then(|p| file_sha256(p));

    if !remote && local.is_none() {
        warnings.push(
            Warning::new(format!(
                "`{{{{< dataset >}}}}` at line {line_no}: `{target}` is not a file in this \
                 project — the card cannot state a size or a checksum, so a reader has no \
                 way to check what you computed from"
            ))
            .at(None, line_no as u32),
        );
    }

    // The diagnostic the item was filed for: the data moved under the document.
    if let (Some(want), Some(got)) = (decl.sha256.as_deref(), measured_sha.as_deref())
        && !want.eq_ignore_ascii_case(got)
    {
        warnings.push(
            Warning::new(format!(
                "`{{{{< dataset >}}}}` at line {line_no}: `{target}` hashes to \
                 `{got}`, but the card declares `sha256={want}` — the file changed since it \
                 was recorded, so any figure computed from it is stale"
            ))
            .at(None, line_no as u32),
        );
    }

    // A remote file a reader cannot verify is the case provenance exists to prevent.
    if remote && decl.sha256.is_none() {
        warnings.push(
            Warning::new(format!(
                "`{{{{< dataset >}}}}` at line {line_no}: remote dataset `{target}` \
                 declares no `sha256=`, so a reader who downloads it cannot tell whether \
                 they got the same bytes you used"
            ))
            .at(None, line_no as u32),
        );
    }

    let digest = measured_sha.clone().or_else(|| decl.sha256.clone());
    let size = measured_bytes.or(decl.bytes);
    let name = decl.title.clone().unwrap_or_else(|| {
        target
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(target)
            .to_owned()
    });

    let mut rows = String::new();
    let mut row = |label: &str, value: String| {
        rows.push_str(&format!("<div><dt>{label}</dt><dd>{value}</dd></div>"));
    };
    if let Some(n) = size {
        row("Size", escape_attr(&human_bytes(n)));
    }
    if let Some(d) = &digest {
        row(
            "Checksum",
            format!(
                "<code class=\"tali-dataset-sum\" title=\"sha256:{full}\">{short}</code>",
                full = escape_attr(d),
                short = escape_attr(&short_digest(d))
            ),
        );
    }
    if let Some(l) = &decl.licence {
        row("Licence", escape_attr(l));
    }
    if let Some(s) = &decl.source {
        let esc = escape_attr(s);
        row(
            "Source",
            match is_remote(s) {
                true => format!("<a href=\"{esc}\" rel=\"noopener noreferrer\">{esc}</a>"),
                false => esc,
            },
        );
    }

    // The affordance differs by where the data lives, and that is the whole design: an
    // in-tree file is offered for download (which is also the `href=` the build follows to
    // copy it), a remote one is offered a command that fetches AND verifies.
    let esc_target = escape_attr(target);
    let action = if remote {
        let verify = digest
            .as_deref()
            .map(|d| {
                format!(
                    "\necho \"{d}  {name}\" | sha256sum -c",
                    name = file_name(target)
                )
            })
            .unwrap_or_default();
        format!(
            "<a class=\"tali-dataset-link\" href=\"{esc_target}\" rel=\"noopener noreferrer\">\
             {esc_target}</a><pre class=\"tali-dataset-fetch\"><code>curl -LO {esc_target}\
             {verify}</code></pre>",
            verify = escape_attr(&verify)
        )
    } else {
        format!("<a class=\"tali-dataset-link\" href=\"{esc_target}\" download>{esc_target}</a>")
    };

    let caption = decl
        .description
        .as_deref()
        .map(|d| format!("<figcaption>{}</figcaption>", escape_attr(d)))
        .unwrap_or_default();

    let html = format!(
        "<figure class=\"tali-dataset\">\
         <div class=\"tali-dataset-head\">\
         <span class=\"tali-dataset-kind\">{kind}</span>\
         <span class=\"tali-dataset-name\">{name}</span></div>\
         {action}\
         <dl class=\"tali-dataset-meta\">{rows}</dl>{caption}</figure>",
        kind = if remote { "Remote dataset" } else { "Dataset" },
        name = escape_attr(&name),
    );
    (html, warnings)
}

/// The last path segment of a target, for the `sha256sum -c` line (which checks a file by
/// the name `curl -O` will have written).
fn file_name(target: &str) -> String {
    target
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("download")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tali-dataset-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The zero-config case the minimal-config convention asks for: a file in the tree and
    /// nothing declared anywhere still produces a complete card.
    #[test]
    fn an_in_tree_file_needs_no_declaration_at_all() {
        let dir = scratch("bare");
        std::fs::write(dir.join("d.csv"), b"a,b\n1,2\n").unwrap();
        let (html, warnings) = render("d.csv", &Declared::default(), Some(&dir), 3);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(html.contains("8 B"), "the measured size: {html}");
        // A checksum row exists, carrying the file's own digest. (That the digest really
        // is SHA-256 is pinned against the FIPS vector in the next test; the claim here is
        // only that measuring happened at all, with nothing declared to read it from.)
        assert!(html.contains("<dt>Checksum</dt>"), "{html}");
        assert!(
            html.contains(&format!(
                "title=\"sha256:{}\"",
                file_sha256(&dir.join("d.csv")).unwrap()
            )),
            "the card states the file's measured digest: {html}"
        );
        assert!(
            html.contains("href=\"d.csv\" download"),
            "an in-tree file is offered for download, which is also the href the build \
             follows to copy it: {html}"
        );
        assert!(!html.contains("curl"), "no fetch snippet for a local file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The measured digest must be a real SHA-256 of the bytes, not any old hash. A reader
    /// checks it with `sha256sum`, so a different algorithm would be worse than none.
    #[test]
    fn the_checksum_is_the_files_real_sha256() {
        let dir = scratch("sha");
        std::fs::write(dir.join("d.txt"), b"abc").unwrap();
        // The canonical SHA-256 of "abc", from FIPS 180-4.
        let want = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(file_sha256(&dir.join("d.txt")).unwrap(), want);
        let (html, _) = render("d.txt", &Declared::default(), Some(&dir), 1);
        assert!(
            html.contains(&format!("title=\"sha256:{want}\"")),
            "the full digest stays copyable in the title: {html}"
        );
        assert!(
            html.contains("ba7816bf…15ad"),
            "and is shortened for display: {html}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The diagnostic item 176 was filed for: the data moved under the document.
    #[test]
    fn a_declared_digest_that_no_longer_matches_is_reported() {
        let dir = scratch("drift");
        std::fs::write(dir.join("d.txt"), b"abc").unwrap();
        let stale = Declared {
            sha256: Some("0".repeat(64)),
            ..Default::default()
        };
        let (_html, warnings) = render("d.txt", &stale, Some(&dir), 7);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0]
                .message
                .contains("changed since it was recorded"),
            "{}",
            warnings[0].message
        );
        assert_eq!(warnings[0].line, Some(7), "located on the shortcode's line");

        // The control: the SAME machinery must stay silent when the digest agrees, or the
        // check is just "declaring a digest warns".
        let good = Declared {
            sha256: file_sha256(&dir.join("d.txt")),
            ..stale.clone()
        };
        let (_h, ok) = render("d.txt", &good, Some(&dir), 7);
        assert!(ok.is_empty(), "a matching digest must not warn: {ok:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_remote_dataset_gets_a_verify_snippet_and_is_never_downloaded() {
        let decl = Declared {
            sha256: Some("dead".repeat(16)),
            licence: Some("ODbL-1.0".into()),
            bytes: Some(2_400_000_000),
            ..Default::default()
        };
        let (html, warnings) = render("https://example.org/big.parquet", &decl, None, 2);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(html.contains("Remote dataset"));
        assert!(html.contains("2.4 GB"), "the declared size: {html}");
        assert!(html.contains("ODbL-1.0"));
        assert!(
            html.contains("curl -LO https://example.org/big.parquet"),
            "a fetch command, not a copy of the data: {html}"
        );
        assert!(
            html.contains("sha256sum -c"),
            "and a way to check what arrived: {html}"
        );
        assert!(
            !html.contains("download>"),
            "a remote file is never offered as a local download: {html}"
        );
    }

    #[test]
    fn a_remote_dataset_without_a_digest_is_reported() {
        let (_html, warnings) = render("https://example.org/x.csv", &Declared::default(), None, 4);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0]
                .message
                .contains("cannot tell whether they got the same bytes"),
            "{}",
            warnings[0].message
        );
    }

    #[test]
    fn a_dataset_that_names_no_file_is_reported() {
        let dir = scratch("missing");
        let (html, warnings) = render("data/gone.csv", &Declared::default(), Some(&dir), 9);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0]
                .message
                .contains("is not a file in this project")
        );
        assert!(
            !html.contains("Checksum"),
            "no digest may be claimed for a file that is not there: {html}"
        );
    }

    /// Arguments are read off the invocation, both spellings of licence accepted, and
    /// anything absent or unparseable falls back to what the file itself says rather than
    /// failing the render (`validate_shortcode_args` is what reports a bad argument).
    #[test]
    fn from_args_reads_the_invocation_and_both_spellings_of_licence() {
        let args = |s: &str| -> Vec<String> { s.split(' ').map(str::to_owned).collect() };
        assert_eq!(
            from_args(&args("licence=CC0-1.0")).licence.as_deref(),
            Some("CC0-1.0")
        );
        assert_eq!(
            from_args(&args("license=CC0-1.0")).licence.as_deref(),
            Some("CC0-1.0")
        );
        let full = from_args(&args(
            "data/x.csv title=Measurements source=https://example.org bytes=2400 sha256=abc",
        ));
        assert_eq!(full.title.as_deref(), Some("Measurements"));
        assert_eq!(full.source.as_deref(), Some("https://example.org"));
        assert_eq!(full.bytes, Some(2400));
        assert_eq!(full.sha256.as_deref(), Some("abc"));
        // The positional source is not swallowed as a value, and an empty or unparseable
        // argument yields nothing rather than an empty-string annotation on the card.
        let bare = from_args(&args("data/x.csv"));
        assert!(bare.title.is_none() && bare.licence.is_none() && bare.bytes.is_none());
        assert!(from_args(&args("title= bytes=many")).title.is_none());
        assert_eq!(from_args(&args("bytes=many")).bytes, None);
    }

    #[test]
    fn human_bytes_reads_like_a_data_portal() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1_000), "1 kB");
        assert_eq!(human_bytes(13_800), "13.8 kB");
        assert_eq!(human_bytes(2_400_000_000), "2.4 GB");
        // Three significant figures is enough; a decimal on a large number is noise.
        assert_eq!(human_bytes(150_500_000), "150 MB");
    }
}
