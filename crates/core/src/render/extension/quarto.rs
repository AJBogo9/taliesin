//! Quarto-`_extension.yml`-shape compatibility: read the nested
//! `contributes: → formats: → <base>:` (+ `contributes: shortcodes:`) layout into
//! a native [`Contribution`].
//!
//! ISOLATED ON PURPOSE. To drop Quarto-extension support: delete this file and the
//! `quarto::contribution` dispatch branch in `mod.rs`. The native flat path is
//! unaffected.

use super::*;

/// Build a [`Contribution`] from the Quarto `contributes:` shape. Shortcodes live
/// under `contributes.shortcodes` (format-agnostic); the includes/theme/resources
/// under `contributes.formats.<base>`. Warns only when the manifest contributes
/// *nothing* the active base can use (a common copy/paste mistake).
pub(super) fn contribution(
    m: &serde_yaml::Value,
    base: &str,
    ext: &str,
    warnings: &mut Vec<String>,
) -> Contribution {
    let contributes = m.get("contributes");
    let shortcodes = contributes.and_then(|c| c.get("shortcodes")).cloned();

    let Some(fmt) = contributes
        .and_then(|c| c.get("formats"))
        .and_then(|f| f.get(base))
    else {
        if shortcodes.is_none() {
            warnings.push(format!(
                "extension '{ext}' declares no `contributes.formats.{base}` block"
            ));
        }
        return Contribution {
            shortcodes,
            ..Contribution::default()
        };
    };

    Contribution {
        head: fmt.get("include-in-header").cloned(),
        body_start: fmt.get("include-before-body").cloned(),
        body_end: fmt.get("include-after-body").cloned(),
        css: fmt.get("css").cloned(),
        theme: fmt.get("theme").cloned(),
        resources: fmt.get("format-resources").cloned(),
        shortcodes,
    }
}
