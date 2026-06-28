//! The websocket wire protocol shared by both dev servers: the single-document
//! [`crate::serve`] server and the multi-page [`crate::serve_site`] server push
//! the same JSON messages to the preview client, and `web-client/` is the other
//! end of this contract. Keeping the message shapes here (rather than copied in
//! each server) means the two servers can't drift apart from each other or from
//! the client.

use qmd_fast_core::BlockOp;

/// A non-fatal issue surfaced in the preview (an unresolved include, the kernel
/// state, a front-matter typo). Held in each document's state and serialized
/// into the `full_render` / `diagnostics` messages. When `line` is set the client
/// makes the row click-to-source; `frame` is an optional small code frame shown
/// inline (a few source lines around `line`, the offending one marked).
#[derive(Clone, PartialEq, Default)]
pub struct Diagnostic {
    pub level: &'static str, // "warning" | "error"
    pub message: String,
    /// Source file for click-to-source, relative to the doc's base dir; `None`
    /// means "the document being previewed" (the client falls back to its path).
    pub file: Option<String>,
    pub line: Option<u32>, // 1-based
    pub frame: Option<String>,
}

impl Diagnostic {
    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            level: "warning",
            message: message.into(),
            ..Self::default()
        }
    }
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: "error",
            message: message.into(),
            ..Self::default()
        }
    }
    /// Attach a click-to-source location (and so make the diagnostic clickable).
    pub fn at(mut self, file: Option<String>, line: u32) -> Self {
        self.file = file;
        self.line = Some(line);
        self
    }
    pub fn with_frame(mut self, frame: String) -> Self {
        self.frame = Some(frame);
        self
    }
}

fn diags_array(diags: &[Diagnostic]) -> Vec<serde_json::Value> {
    diags
        .iter()
        .map(|d| {
            let mut o = serde_json::json!({ "level": d.level, "message": d.message });
            if let Some(f) = &d.file {
                o["file"] = serde_json::json!(f);
            }
            if let Some(l) = d.line {
                o["line"] = serde_json::json!(l);
            }
            if let Some(fr) = &d.frame {
                o["frame"] = serde_json::json!(fr);
            }
            o
        })
        .collect()
}

/// `full_render`: replace the whole document body + the diagnostics list in one
/// message (the initial paint, and after a change too large to express as ops).
pub fn full_render(title: Option<&str>, body_html: &str, diags: &[Diagnostic]) -> String {
    serde_json::json!({
        "type": "full_render",
        "title": title,
        "body_html": body_html,
        "diagnostics": diags_array(diags),
    })
    .to_string()
}

/// Standalone diagnostics update: the document is unchanged, only the issue list
/// moved (e.g. the kernel came back).
pub fn diagnostics(diags: &[Diagnostic]) -> String {
    serde_json::json!({ "type": "diagnostics", "messages": diags_array(diags) }).to_string()
}

/// Tell the client to do a full page reload (used after a kernel restart, so
/// `{js}` cells re-bind to freshly-defined `qmd-define` values).
pub fn reload() -> String {
    serde_json::json!({ "type": "reload" }).to_string()
}

/// Hot-swap the theme CSS in place (no reload): the client replaces the contents
/// of `<style id="qmd-theme">` (creating it if absent). Sent when only the theme
/// CSS changed, so scroll position, the current deck slide, and open callouts all
/// survive a theme edit.
pub fn style(css: &str) -> String {
    serde_json::json!({ "type": "style", "css": css }).to_string()
}

/// A fatal render/read error, shown in the preview banner.
pub fn error(message: &str) -> String {
    serde_json::json!({ "type": "error", "message": message }).to_string()
}

/// `build-state`: document-level execution phase + a deterministic k-of-N count.
/// `phase` is one of "warming-kernel" | "executing" | "idle" | "error". `page` is
/// the source rel-path for the multi-page server, `None` for the single-doc server.
pub fn build_state(page: Option<&str>, phase: &str, ran: u32, total: u32, lang: &str) -> String {
    serde_json::json!({
        "type": "build-state", "page": page,
        "phase": phase, "ran": ran, "total": total, "lang": lang
    })
    .to_string()
}

/// `cell-state`: per-cell execution state. `state` is one of
/// "queued" | "running" | "done" | "error". `started_ms`/`duration_ms` are epoch
/// millis / elapsed millis when known; the client ticks the live timer itself.
/// `cell_id` is the cell's own id (the same id the output block is built from as
/// `{cell_id}-out`), so the client can target that block. `page` is the source
/// rel-path for the multi-page server, `None` for the single-doc server.
pub fn cell_state(
    page: Option<&str>,
    cell_id: &str,
    state: &str,
    started_ms: Option<u64>,
    duration_ms: Option<u64>,
) -> String {
    serde_json::json!({
        "type": "cell-state", "page": page, "cell_id": cell_id,
        "state": state, "started_ms": started_ms, "duration_ms": duration_ms
    })
    .to_string()
}

/// A single incremental block op. `rewrite_html` is applied to the block HTML of
/// `Update`/`Insert` before it goes over the wire: identity for the single-doc
/// server, and `.qmd`→`.html` link rewriting for the site server.
pub fn op(op: &BlockOp, rewrite_html: impl Fn(&str) -> String) -> String {
    match op {
        BlockOp::Update { target_id, html } => serde_json::json!({
            "type": "update", "target_id": target_id, "html": rewrite_html(html)
        }),
        BlockOp::Insert { after_id, html } => serde_json::json!({
            "type": "insert", "after_id": after_id, "html": rewrite_html(html)
        }),
        BlockOp::Remove { target_id } => {
            serde_json::json!({ "type": "remove", "target_id": target_id })
        }
        BlockOp::SetMeta {
            target_id,
            sourcepos,
            source_file,
        } => serde_json::json!({
            "type": "set_meta", "target_id": target_id,
            "sourcepos": sourcepos, "source_file": source_file
        }),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn build_state_serializes_phase_and_counts() {
        let s = super::build_state(Some("ch1.qmd"), "executing", 3, 8, "python");
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "build-state");
        assert_eq!(v["page"], "ch1.qmd");
        assert_eq!(v["phase"], "executing");
        assert_eq!(v["ran"], 3);
        assert_eq!(v["total"], 8);
        assert_eq!(v["lang"], "python");
    }

    #[test]
    fn cell_state_includes_state_and_optional_timing() {
        let s = super::cell_state(Some("p.qmd"), "abc", "running", Some(1000), None);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "cell-state");
        assert_eq!(v["cell_id"], "abc");
        assert_eq!(v["state"], "running");
        assert_eq!(v["started_ms"], 1000);
        assert!(v.get("duration_ms").is_none_or(|d| d.is_null()));
    }
}
