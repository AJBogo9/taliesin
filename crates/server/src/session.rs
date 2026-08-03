//! Finding (and starting) the **session** that owns a project's kernels.
//!
//! A session is not a new program: it is the ordinary dev server, which already owns
//! the warm kernels ([`crate::exec::Executor`]), the warm pool, the per-page executor
//! LRU, and the `_freeze/` cache. `taliesin run` is a thin client against it, so a
//! terminal run and a browser preview share one kernel set and one cache writer. That
//! sharing is the whole point: two owners of `_freeze/<page>.json` is a lost-update bug
//! that publishes stale output.
//!
//! # Discovery: a hint file, proven by handshake
//!
//! The server writes `<runtime>/taliesin/<hash-of-root>.json` on bind, holding just the
//! port. The file is a **hint, never evidence** — it survives SIGKILL, a reboot can
//! recycle the port, and any local user can bind loopback. So a client always proves it
//! with the identity handshake the preview already serves ([`crate::serve::IDENTITY_PATH`]):
//! the responder must name the same canonical root *and* pass
//! [`crate::serve::is_sibling_preview`], which confirms against `/proc/<pid>/exe` that the
//! pid is really another instance of this binary rather than a number someone typed. Both
//! checks already existed for "is this port already previewing my project"; this reuses
//! them rather than inventing a second, weaker identity.
//!
//! A stale hint is therefore self-correcting: it fails the handshake and we start a
//! session, overwriting it.
//!
//! # Why not a pid liveness check
//!
//! Because pids are reused. A file saying `pid 4711` proves nothing once 4711 has been
//! recycled, and the failure mode is signalling or trusting an unrelated process. The
//! handshake has no such window.

use std::path::{Path, PathBuf};

/// Where session hints live: `$XDG_RUNTIME_DIR/taliesin/`, else `$TMPDIR/taliesin/`.
///
/// `XDG_RUNTIME_DIR` is the right home (user-private, 0700, cleared at logout), but it is
/// absent on macOS and in bare containers, so the temp dir is the fallback rather than a
/// hard failure: a hint file is recoverable state, and losing it costs one extra session
/// start, never correctness.
fn hint_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("taliesin")
}

/// The hint file for `root`, named by a digest of the canonical root path.
///
/// Digested rather than path-mangled so the name is fixed-length and filesystem-safe
/// whatever the project is called, and so two projects cannot collide by having names
/// that flatten to the same string. This is a lookup key, not a security boundary, which
/// is why a short hex digest is enough.
pub(crate) fn hint_path(root: &Path) -> PathBuf {
    use sha2::{Digest, Sha256};
    let canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let digest = Sha256::digest(canon.to_string_lossy().as_bytes());
    hint_dir().join(format!("{:.16x}.json", HexPrefix(&digest)))
}

/// `{:.16x}` over the first bytes of a digest, so `hint_path` can format inline.
struct HexPrefix<'a>(&'a [u8]);

impl std::fmt::LowerHex for HexPrefix<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `{:.16x}` sets `precision`, which the derive-free path must honour itself;
        // default to the full digest when no precision is given.
        let want = f.precision().unwrap_or(self.0.len() * 2);
        let mut out = String::with_capacity(want);
        for b in self.0 {
            if out.len() >= want {
                break;
            }
            out.push_str(&format!("{b:02x}"));
        }
        out.truncate(want);
        f.write_str(&out)
    }
}

/// Record that a session for `root` is listening on `port`.
///
/// Best-effort: a hint that cannot be written costs a later client one extra session
/// start, so a failure here must never take the server down with it.
pub(crate) fn write_hint(root: &Path, port: u16) {
    let path = hint_path(root);
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let body = serde_json::json!({ "port": port }).to_string();
    let _ = std::fs::write(&path, body);
}

/// Drop the hint for `root` (clean shutdown). Absent file is success.
pub(crate) fn clear_hint(root: &Path) {
    let _ = std::fs::remove_file(hint_path(root));
}

/// The port recorded for `root`, if any. Parse failure reads as "no hint": a truncated or
/// hand-edited file must not be louder than a missing one.
pub(crate) fn hinted_port(root: &Path) -> Option<u16> {
    let text = std::fs::read_to_string(hint_path(root)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let raw = v.get("port")?.as_u64()?;
    u16::try_from(raw).ok().filter(|p| *p != 0)
}

/// The project a `.tmd` (or a directory) belongs to: its nearest ancestor `_site.yml`,
/// else the directory itself.
///
/// This is what makes the session *per project* rather than per file. Every page in a
/// book shares one session, so running a cell in chapter 9 reuses the kernel that
/// chapter 8 warmed, and the `_freeze/` writer stays single.
pub(crate) fn project_root_for(target: &Path) -> PathBuf {
    let start = if target.is_dir() {
        target.to_path_buf()
    } else {
        target.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    taliesin_core::site::enclosing_site_root(&start).unwrap_or(start)
}

/// The key the session serving `file` is registered under: its enclosing `_site.yml`
/// project, else the document itself.
///
/// This is the **client half of one contract**, and the server half is
/// `serve_site::Resolved::session_key`. They are two derivations of one key, in two
/// processes, with nothing in the type system holding them together — so when they
/// disagree a run cannot find a session that is running perfectly well, waits out
/// `SESSION_READY_TIMEOUT`, and blames the server. That is exactly what happened once
/// (see the server-side doc comment), which is why this is a named function with a test
/// asserting the two answers match rather than an `if` at the call site: an inline
/// spelling is how the two drifted apart in the first place.
///
/// Note it is *not* [`project_root_for`]: a document with no ancestor `_site.yml` keys on
/// itself, not on its directory. Its session is a project of just that document, and its
/// neighbours in that directory are somebody else's.
pub(crate) fn session_key_for(file: &Path) -> PathBuf {
    taliesin_core::site::enclosing_site_root(file).unwrap_or_else(|| file.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_path_is_stable_and_distinct_per_root() {
        let a = hint_path(Path::new("/tmp/tali-a"));
        let b = hint_path(Path::new("/tmp/tali-b"));
        assert_eq!(a, hint_path(Path::new("/tmp/tali-a")), "must be stable");
        assert_ne!(a, b, "different roots must not share a hint file");
        assert_eq!(
            a.extension().and_then(|e| e.to_str()),
            Some("json"),
            "hint files are json"
        );
        // 16 hex chars of digest, so the name can never contain a path separator or a
        // character the filesystem dislikes, whatever the project is called.
        let stem = a.file_stem().unwrap().to_string_lossy().to_string();
        assert_eq!(stem.len(), 16, "expected a 16-char digest stem, got {stem}");
        assert!(
            stem.chars().all(|c| c.is_ascii_hexdigit()),
            "stem must be hex, got {stem}"
        );
    }

    #[test]
    fn a_project_hostile_name_still_produces_a_safe_hint_file() {
        // The reason the name is digested rather than mangled: these must not escape the
        // hint dir or collide with each other.
        let dir = hint_dir();
        for weird in ["/tmp/a b/../c", "/tmp/a/b", "/tmp/a%2Fb", "/tmp/a\nb"] {
            let p = hint_path(Path::new(weird));
            assert_eq!(p.parent(), Some(dir.as_path()), "{weird} escaped the dir");
        }
        assert_ne!(
            hint_path(Path::new("/tmp/a/b")),
            hint_path(Path::new("/tmp/a%2Fb")),
            "distinct roots must not collide after digesting"
        );
    }

    #[test]
    fn round_trips_a_port_and_reads_absent_or_corrupt_as_none() {
        let root = std::env::temp_dir().join(format!("tali-session-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        clear_hint(&root);
        assert_eq!(hinted_port(&root), None, "no file must read as no hint");

        write_hint(&root, 4388);
        assert_eq!(hinted_port(&root), Some(4388));

        // A corrupt hint must be as quiet as a missing one, not a hard error.
        std::fs::write(hint_path(&root), "{not json").unwrap();
        assert_eq!(hinted_port(&root), None, "corrupt hint must read as none");

        // Port 0 is never a real listener; treat it as no hint rather than dialling it.
        std::fs::write(hint_path(&root), r#"{"port":0}"#).unwrap();
        assert_eq!(hinted_port(&root), None, "port 0 must read as none");

        clear_hint(&root);
        assert_eq!(hinted_port(&root), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_root_is_the_nearest_site_yml_so_a_book_shares_one_session() {
        let base = std::env::temp_dir().join(format!("tali-proj-test-{}", std::process::id()));
        let deep = base.join("chapters/part2");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(base.join("_site.yml"), "title: t\n").unwrap();
        let page = deep.join("ch9.tmd");
        std::fs::write(&page, "# hi\n").unwrap();

        let want = std::fs::canonicalize(&base).unwrap();
        assert_eq!(
            std::fs::canonicalize(project_root_for(&page)).unwrap(),
            want,
            "a nested page must resolve to the project root, not its own directory"
        );
        assert_eq!(
            std::fs::canonicalize(project_root_for(&deep)).unwrap(),
            want,
            "a nested directory must resolve to the project root too"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_loose_file_outside_any_project_roots_at_its_own_directory() {
        let base = std::env::temp_dir().join(format!("tali-loose-test-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let page = base.join("scratch.tmd");
        std::fs::write(&page, "# hi\n").unwrap();
        assert_eq!(
            std::fs::canonicalize(project_root_for(&page)).unwrap(),
            std::fs::canonicalize(&base).unwrap(),
            "with no _site.yml the file's own directory is the session scope"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
