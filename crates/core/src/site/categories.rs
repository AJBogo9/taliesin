//! Category-vocabulary linting: catch the typo that silently forks a listing filter.
//!
//! Categories are free text. Nothing normalizes them, so `statistics`, `Statistics` and
//! `statstics` on one post render three separate chips, each with a count of 1, and the
//! reader's filter quietly splits the archive. The author never sees an error because
//! there is nothing to error on: every value is "valid".
//!
//! This validator reads the *whole site's* vocabulary and flags a value that looks like a
//! misspelling of a more-common sibling. It is read-only and `check`-level: a category is
//! not a build defect.
//!
//! Two rules, deliberately different in strictness:
//!
//! - **case-only forks** (`Statistics` vs `statistics`) are always a fork, whatever the
//!   length, because no listing shows both on purpose.
//! - **near misses** (`statstics` vs `statistics`) reuse the edit-distance-2 ceiling the
//!   front-matter did-you-mean uses, but only for values of [`MIN_FUZZY_LEN`]+ characters
//!   and only when the neighbour is *strictly more common*. Short tags legitimately sit
//!   within two edits of each other (`R`/`C`, `ML`/`UI`, `algorithm`/`algorithms`), so a
//!   naive `closest()` over the vocabulary would cry wolf on a correct site.

use super::*;
use std::collections::BTreeMap;

/// Below this length, an edit-distance-2 neighbour is much more likely to be a distinct
/// tag than a typo (`R` and `C`; `ML` and `UI` are two edits apart).
const MIN_FUZZY_LEN: usize = 5;

impl Site {
    /// Every category value that looks like a misspelling of a more-common sibling,
    /// as `(page rel-path, warning)` pairs located to the offending front-matter line.
    ///
    /// Read-only: it never rewrites a category, because the `.tmd` file is the only
    /// editing surface.
    pub fn validate_categories(&self) -> Vec<(String, Warning)> {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for page in &self.pages {
            for c in &page.categories {
                *counts.entry(c.as_str()).or_default() += 1;
            }
        }
        if counts.len() < 2 {
            return Vec::new(); // nothing to be confused with
        }

        let mut out = Vec::new();
        for page in &self.pages {
            for cat in &page.categories {
                let Some(suggestion) = suspicious(cat, &counts) else {
                    continue;
                };
                let mut w = Warning::new(format!(
                    "category `{cat}` looks like `{suggestion}`, used elsewhere in this \
                     site; they filter as two separate categories"
                ));
                w.file = Some(page.rel.clone());
                w.line = category_line(&page.input, cat);
                out.push((page.rel.clone(), w));
            }
        }
        out
    }
}

/// The vocabulary entry `cat` was probably meant to be, or `None` when it looks intentional.
fn suspicious<'a>(cat: &str, counts: &BTreeMap<&'a str, usize>) -> Option<&'a str> {
    let mine = *counts.get(cat)?;
    let mut best: Option<(&str, usize)> = None;
    for (&other, &n) in counts {
        if other == cat {
            continue;
        }
        // A case-only difference is always a fork: nobody lists `Statistics` and
        // `statistics` as two categories on purpose. Report the more common spelling,
        // breaking a tie toward the one that is already lowercase.
        let case_only = other.eq_ignore_ascii_case(cat);
        let near = cat.chars().count() >= MIN_FUZZY_LEN
            && other.chars().count() >= MIN_FUZZY_LEN
            && crate::frontmatter::levenshtein(cat, other) <= 2;
        if !case_only && !near {
            continue;
        }
        // Only a *more common* sibling is evidence of a typo; otherwise the rarer value
        // would accuse the more common one and both would warn.
        let wins = if case_only {
            n > mine || (n == mine && other.chars().all(|c| !c.is_uppercase()))
        } else {
            n > mine
        };
        if wins && best.is_none_or(|(_, bn)| n > bn) {
            best = Some((other, n));
        }
    }
    best.map(|(s, _)| s)
}

/// The 1-based line of `cat` inside `input`'s front matter, so the diagnostic is clickable.
/// `None` when the value cannot be located (e.g. it came from a flow-style list).
fn category_line(input: &Path, cat: &str) -> Option<u32> {
    let src = std::fs::read_to_string(input).ok()?;
    let mut lines = src.lines().enumerate();
    // Front matter only: the opening `---`, then up to the closing one.
    if !lines.next()?.1.trim_end().starts_with("---") {
        return None;
    }
    for (i, line) in lines {
        if line.trim_end() == "---" {
            return None;
        }
        if line.contains(cat) {
            return Some(i as u32 + 1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab(pairs: &[(&'static str, usize)]) -> BTreeMap<&'static str, usize> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn a_near_miss_of_a_more_common_category_is_flagged() {
        let v = vocab(&[("statistics", 4), ("statstics", 1)]);
        assert_eq!(suspicious("statstics", &v), Some("statistics"));
        // The correct, more common spelling is never accused of being the typo.
        assert_eq!(suspicious("statistics", &v), None);
    }

    #[test]
    fn a_case_only_fork_is_always_flagged_regardless_of_length() {
        let v = vocab(&[("statistics", 3), ("Statistics", 1)]);
        assert_eq!(suspicious("Statistics", &v), Some("statistics"));
        assert_eq!(suspicious("statistics", &v), None);

        // Even for a short tag, where the fuzzy rule stays silent.
        let v = vocab(&[("ml", 2), ("ML", 1)]);
        assert_eq!(suspicious("ML", &v), Some("ml"));

        // On a tie, the lowercase spelling is the canonical one.
        let v = vocab(&[("rust", 1), ("Rust", 1)]);
        assert_eq!(suspicious("Rust", &v), Some("rust"));
        assert_eq!(suspicious("rust", &v), None);
    }

    #[test]
    fn short_distinct_tags_within_two_edits_are_not_flagged() {
        // The false positive a naive `closest()` over the vocabulary would produce.
        let v = vocab(&[("R", 3), ("C", 2), ("ML", 4), ("UI", 1), ("Go", 1)]);
        for tag in ["R", "C", "ML", "UI", "Go"] {
            assert_eq!(
                suspicious(tag, &v),
                None,
                "`{tag}` is a real tag, not a typo"
            );
        }
    }

    #[test]
    fn a_rarer_but_legitimate_neighbour_is_not_accused() {
        // `algorithms` is two edits from `algorithm`, but both are common enough that
        // neither is evidence of a typo in the other... except the rarer one loses.
        // Pin the asymmetry: only the *less* common value can be the misspelling.
        let v = vocab(&[("algorithm", 1), ("algorithms", 1)]);
        assert_eq!(
            suspicious("algorithm", &v),
            None,
            "equal counts: no accusation"
        );
        assert_eq!(suspicious("algorithms", &v), None);
    }

    #[test]
    fn a_single_category_vocabulary_never_warns() {
        let v = vocab(&[("statistics", 1)]);
        assert_eq!(suspicious("statistics", &v), None);
    }
}
