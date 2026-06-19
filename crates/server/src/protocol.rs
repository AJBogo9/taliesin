//! The websocket wire protocol shared by both dev servers: the single-document
//! [`crate::serve`] server and the multi-page [`crate::serve_site`] server push
//! the same JSON messages to the preview client, and `web-client/` is the other
//! end of this contract. Keeping the message shapes here (rather than copied in
//! each server) means the two servers can't drift apart from each other or from
//! the client.

use qmd_fast_core::BlockOp;

/// A non-fatal issue surfaced in the preview (an unresolved include, the kernel
/// state, a front-matter typo). Held in each document's state and serialized
/// into the `full_render` / `diagnostics` messages.
#[derive(Clone, PartialEq)]
pub struct Diagnostic {
    pub level: &'static str, // "warning" | "error"
    pub message: String,
}

fn diags_array(diags: &[Diagnostic]) -> Vec<serde_json::Value> {
    diags
        .iter()
        .map(|d| serde_json::json!({ "level": d.level, "message": d.message }))
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

/// Tell the client to do a full page reload (used after a kernel restart, so OJS
/// cells re-bind to freshly-defined values).
pub fn reload() -> String {
    serde_json::json!({ "type": "reload" }).to_string()
}

/// A fatal render/read error, shown in the preview banner.
pub fn error(message: &str) -> String {
    serde_json::json!({ "type": "error", "message": message }).to_string()
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
    }
    .to_string()
}
