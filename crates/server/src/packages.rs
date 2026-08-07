//! What is actually installed in an interpreter, and one number standing for it.
//!
//! **The hole this closes is named in `freeze.rs`'s own doc comment.** The cumulative cache
//! key folds in *code and interpreter identity only*, so `pip install --upgrade pandas` is the
//! same interpreter reporting the same `--version` and every key is unchanged: the next build
//! restores output computed by a library that is gone. That is CHI 2020's **Reproduce and
//! Reuse** pain point exactly ("the only way another person can run the notebook is if they're
//! able to match all the environment settings"), and `doctor` could not see it either — it
//! audits interpreter *presence*, reporting "ipykernel MISSING" but never "which pandas".
//!
//! **This does not make the cache correct; it makes it honest.** The digest is *recorded*
//! beside a page's cached outputs and compared on restore, so a replay that crosses an
//! environment change says so. Folding it into the key itself is the strong version and the
//! disruptive one — every `pip install`, however unrelated, would bust the whole cache — so it
//! is deliberately not done. The escape hatches for a cell that really depends on the
//! environment are unchanged and both predate this: `#| cache: false` and `TALIESIN_NO_CACHE`.
//!
//! One subprocess per interpreter per process, memoized. Not `pip list`: `importlib.metadata`
//! is stdlib, so it answers for an interpreter with no pip, and it does not pay pip's import.

use crate::interpreter::Lang;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// The packages one interpreter has, and the digest that stands for the set.
#[derive(Clone, Debug, Default)]
pub(crate) struct Manifest {
    /// Name → version, sorted by name so two runs of the same environment agree.
    pub packages: BTreeMap<String, String>,
    /// 16 hex digits over `name==version` lines. The same FNV-1a the block ids and the
    /// freeze keys use, for the same reason: one hash definition in the tree.
    pub digest: String,
}

/// `import importlib.metadata` rather than `pip list`: stdlib since 3.8, so it answers for an
/// interpreter that has no pip at all, and it skips pip's own import cost. `sorted` on the
/// Python side is belt-and-braces — the `BTreeMap` below orders it either way — but it keeps
/// the raw output stable, which is what makes the probe debuggable by hand.
const PYTHON_PROBE: &str = "\
import importlib.metadata as m
seen = {}
for d in m.distributions():
    n = d.metadata['Name']
    if n:
        seen[n] = d.version or ''
for n in sorted(seen):
    print(n + '\\t' + seen[n])
";

/// `installed.packages()` is the R equivalent and needs no extra package. `--vanilla` so a
/// user's `.Rprofile` cannot print into the answer, `--slave` so R does not echo the code.
const R_PROBE: &str = "\
ip <- installed.packages()[, c('Package', 'Version'), drop = FALSE]
ip <- ip[order(rownames(ip)), , drop = FALSE]
cat(paste(ip[, 'Package'], ip[, 'Version'], sep = '\\t'), sep = '\\n')
";

/// The manifest for `program`, memoized process-wide.
///
/// `None` means *we could not ask* — the interpreter is missing, or the probe failed — and it
/// is deliberately not cached, for the same reason `exec::probe_interp_id` does not cache a
/// failed version probe: a transient failure must not become this process's permanent answer.
/// A `None` here costs the environment warning and nothing else; it never changes what runs.
pub(crate) fn manifest(lang: Lang, program: &Path) -> Option<Manifest> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Manifest>>> = OnceLock::new();
    let key = format!("{lang:?}\u{0}{}", program.display());
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(m) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return Some(m);
    }
    let out = match lang {
        Lang::Python => std::process::Command::new(program)
            .args(["-c", PYTHON_PROBE])
            .stdin(std::process::Stdio::null())
            .output()
            .ok()?,
        Lang::R => std::process::Command::new(program)
            .args(["--vanilla", "--slave", "-e", R_PROBE])
            .stdin(std::process::Stdio::null())
            .output()
            .ok()?,
    };
    if !out.status.success() {
        return None;
    }
    let m = parse(&String::from_utf8_lossy(&out.stdout));
    // An interpreter with genuinely nothing installed is not a thing (Python ships its own
    // distributions; R ships base packages), so an empty parse means the probe printed
    // something we could not read — which must not be memoized as "this environment is empty".
    if m.packages.is_empty() {
        return None;
    }
    if let Ok(mut c) = cache.lock() {
        c.insert(key, m.clone());
    }
    Some(m)
}

/// Parse `name<TAB>version` lines into a manifest, computing the digest.
fn parse(text: &str) -> Manifest {
    let mut packages = BTreeMap::new();
    for line in text.lines() {
        if let Some((name, version)) = line.split_once('\t') {
            let name = name.trim();
            if !name.is_empty() {
                packages.insert(name.to_string(), version.trim().to_string());
            }
        }
    }
    Manifest {
        digest: digest_of(&packages),
        packages,
    }
}

/// The digest for a package set. Separate from [`parse`] so a test can compute one from a
/// literal map without going through the wire format.
fn digest_of(packages: &BTreeMap<String, String>) -> String {
    let joined: String = packages
        .iter()
        .map(|(n, v)| format!("{n}=={v}\n"))
        .collect();
    format!("{:016x}", taliesin_core::hash::fnv1a(&joined))
}

/// Should a restore be announced?
///
/// Only when we know **both** what the outputs were produced under and what is installed now,
/// and the two differ. The two `None` cases are not near-misses to be reported cautiously:
/// "we could not ask" and "this cache predates the record" are both *ignorance*, and a warning
/// built on ignorance is the kind that teaches an author to ignore warnings.
pub(crate) fn crossed(recorded: Option<&str>, now: Option<&str>) -> bool {
    matches!((recorded, now), (Some(was), Some(is)) if was != is)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restore_is_announced_only_when_both_halves_are_known_and_differ() {
        assert!(
            crossed(Some("aaa"), Some("bbb")),
            "the case this exists for"
        );
        assert!(
            !crossed(Some("aaa"), Some("aaa")),
            "same environment, silence"
        );
        // Ignorance is not evidence of change. A cache written before the digest was
        // recorded, and an interpreter that could not be probed, must both stay quiet.
        assert!(!crossed(None, Some("bbb")), "nothing was recorded");
        assert!(
            !crossed(Some("aaa"), None),
            "we could not ask what is installed"
        );
        assert!(!crossed(None, None));
    }

    #[test]
    fn the_digest_moves_when_a_version_does_and_not_otherwise() {
        let a = parse("pandas\t2.1.0\nnumpy\t1.26.0\n");
        // The same set in the other order is the same environment.
        let same = parse("numpy\t1.26.0\npandas\t2.1.0\n");
        assert_eq!(
            a.digest, same.digest,
            "order must not be part of the answer"
        );
        // An in-place upgrade is exactly the case the interpreter's own `--version` cannot
        // see, and it is the whole reason this exists.
        let upgraded = parse("pandas\t2.2.0\nnumpy\t1.26.0\n");
        assert_ne!(a.digest, upgraded.digest);
        // So is an addition or a removal.
        assert_ne!(a.digest, parse("pandas\t2.1.0\n").digest);
        assert_ne!(
            a.digest,
            parse("pandas\t2.1.0\nnumpy\t1.26.0\nscipy\t1.11\n").digest
        );
    }

    #[test]
    fn the_manifest_carries_the_versions_not_only_the_digest() {
        // `doctor` reports "which pandas", which is the half a digest alone cannot answer.
        let m = parse("pandas\t2.1.0\nnumpy\t1.26.0\n");
        assert_eq!(m.packages.get("pandas").map(String::as_str), Some("2.1.0"));
        assert_eq!(m.packages.len(), 2);
    }

    #[test]
    fn a_malformed_line_is_skipped_rather_than_stored_as_a_package() {
        // R's `--slave` still emits a stray blank line, and a warning can reach stdout.
        let m = parse("\nWARNING: something\npandas\t2.1.0\n\t1.0\n");
        assert_eq!(m.packages.len(), 1, "{:?}", m.packages);
    }

    #[test]
    fn a_package_with_no_version_still_counts() {
        // It is part of the environment; dropping it would make two different environments
        // hash the same.
        let m = parse("weird\t\nnumpy\t1.26.0\n");
        assert_eq!(m.packages.len(), 2);
        assert_ne!(m.digest, parse("numpy\t1.26.0\n").digest);
    }
}
