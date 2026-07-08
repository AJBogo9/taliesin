//! The websocket wire protocol shared by both dev servers: the single-document
//! [`crate::serve`] server and the multi-page [`crate::serve_site`] server push
//! the same JSON messages to the preview client, and `web-client/` is the other
//! end of this contract. Keeping the message shapes here (rather than copied in
//! each server) means the two servers can't drift apart from each other or from
//! the client.

use taliesin_core::BlockOp;

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
///
/// `generation` (wire key `gen`) is a monotonic counter bumped whenever the
/// rendered body changes. The server-rendered page stamps the generation its SSR
/// body was built at into `window.TALIESIN_SSR_GEN`; the client compares it to the
/// first `full_render`'s `gen` to decide whether the SSR content is already current
/// (skip the re-mount) or stale because a rebuild landed between the HTTP render and
/// the websocket connect (mount for real). Without this the client blindly skips the
/// first `full_render`, so a doc whose initial code-exec pass finishes in that window
/// loses its cell outputs until a manual reload.
pub fn full_render(
    title: Option<&str>,
    body_html: &str,
    generation: u64,
    diags: &[Diagnostic],
) -> String {
    serde_json::json!({
        "type": "full_render",
        "title": title,
        "gen": generation,
        "boot": boot_id(),
        "body_html": body_html,
        "diagnostics": diags_array(diags),
    })
    .to_string()
}

/// A per-process boot id (nanoseconds at first call, mixed with the pid), stamped into
/// the SSR page (`window.TALIESIN_BOOT`) and every `full_render`. The client uses it to
/// tell a same-process reconnect (safe to skip the re-mount when the gen also matches)
/// from a reconnect to a RESTARTED server: `generation` is process-local (`DocState`/
/// `PageDoc` reset it to 0 on start), so after a `taliesin preview` restart the reset
/// counter can re-hit a value a long-lived tab already mounted, and a bare gen match
/// would wrongly suppress the re-mount and leave the preview showing stale source. A
/// changed boot id always forces a fresh mount, so a restart can never show stale content.
pub fn boot_id() -> u64 {
    use std::sync::OnceLock;
    static BOOT: OnceLock<u64> = OnceLock::new();
    *BOOT.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        nanos ^ ((std::process::id() as u64) << 40)
    })
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
/// server, and `.tmd`→`.html` link rewriting for the site server.
///
/// Every op carries `generation` (wire key `gen`): the render generation the document
/// reaches AFTER this op's burst is applied (all ops in one rebuild share it). The
/// client tracks it so a websocket reconnect on a byte-identical doc (gen unchanged)
/// can skip the wholesale re-mount that would otherwise destroy live block state
/// (WebGL/`{js}` widgets, playing video, open `<details>`). See [`full_render`].
pub fn op(op: &BlockOp, generation: u64, rewrite_html: impl Fn(&str) -> String) -> String {
    match op {
        BlockOp::Update { target_id, html } => serde_json::json!({
            "type": "update", "gen": generation, "target_id": target_id, "html": rewrite_html(html)
        }),
        BlockOp::Insert { after_id, html } => serde_json::json!({
            "type": "insert", "gen": generation, "after_id": after_id, "html": rewrite_html(html)
        }),
        BlockOp::Remove { target_id } => {
            serde_json::json!({ "type": "remove", "gen": generation, "target_id": target_id })
        }
        BlockOp::SetMeta {
            target_id,
            sourcepos,
            source_file,
        } => serde_json::json!({
            "type": "set_meta", "gen": generation, "target_id": target_id,
            "sourcepos": sourcepos, "source_file": source_file
        }),
    }
    .to_string()
}

/// One rebuild's diff outcome: the block ops plus which whole-message updates the
/// rebuild also needs. Both dev servers build one of these and turn it into the
/// ordered broadcast burst via [`Broadcast::messages`], so the single-doc
/// [`crate::serve`] and multi-page [`crate::serve_site`] servers can't drift on the
/// block-level incremental invariant. The remount trigger and the gen-bump stay
/// caller-side: only the single-doc server folds deck restructure/title flags into
/// `remount`, and the bump must land before the lazy `full_render` reads it.
pub struct Broadcast<'a> {
    /// The block ops from `diff_blocks`, applied one message each on the incremental path.
    pub ops: &'a [BlockOp],
    /// Send a whole `full_render` (a re-mount) instead of the ops: error recovery, or a
    /// deck restructure/title change whose slides can't be expressed as flat block ops.
    pub remount: bool,
    /// The theme CSS changed — hot-swap the `<style>` after the body.
    pub theme_changed: bool,
    /// The diagnostics list changed — push it after the body.
    pub diags_changed: bool,
}

impl Broadcast<'_> {
    /// The ordered burst both dev servers push after a rebuild diff. The two servers
    /// MUST sequence these identically or the incremental invariant drifts between the
    /// previews, so the ordering lives here once:
    ///
    ///   1. body — one `full_render` when `remount`, otherwise one `op` per block op,
    ///      in diff order;
    ///   2. then `style` if `theme_changed`;
    ///   3. then `diagnostics` if `diags_changed`.
    ///
    /// The theme/diagnostics messages ride *after* the body — including on the `remount`
    /// path — so a save that both re-mounts and changes the theme still applies the new
    /// theme: the re-mounted HTML carries the stale `<style>` body, and a `diagnostics`
    /// update would be lost under a fresh `full_render` otherwise. This after-the-body
    /// ordering is the load-bearing contract that was previously copy-pasted in both
    /// servers.
    ///
    /// The message builders are closures because each server reads its own just-updated
    /// state and the `op`/`full_render` bodies differ by link-rewrite (identity for the
    /// single doc, `.tmd`→`.html` for the site). `full_render` stays lazy so the hot
    /// incremental path never serializes the whole body just to discard it.
    pub fn messages(
        &self,
        full_render: impl FnOnce() -> String,
        op: impl Fn(&BlockOp) -> String,
        style: impl FnOnce() -> String,
        diagnostics: impl FnOnce() -> String,
    ) -> Vec<String> {
        let mut msgs = Vec::new();
        if self.remount {
            msgs.push(full_render());
        } else {
            msgs.extend(self.ops.iter().map(op));
        }
        if self.theme_changed {
            msgs.push(style());
        }
        if self.diags_changed {
            msgs.push(diagnostics());
        }
        msgs
    }
}

#[cfg(test)]
mod tests {
    use taliesin_core::BlockOp;

    fn label(op: &BlockOp) -> String {
        match op {
            BlockOp::Update { target_id, .. } => format!("OP:update:{target_id}"),
            BlockOp::Remove { target_id } => format!("OP:remove:{target_id}"),
            _ => "OP:other".into(),
        }
    }

    fn messages(b: super::Broadcast) -> Vec<String> {
        b.messages(|| "FULL".into(), label, || "STYLE".into(), || "DIAG".into())
    }

    #[test]
    fn broadcast_incremental_is_one_op_per_block_in_order() {
        let ops = vec![
            BlockOp::Update {
                target_id: "a".into(),
                html: "<p>a</p>".into(),
            },
            BlockOp::Remove {
                target_id: "b".into(),
            },
        ];
        let msgs = messages(super::Broadcast {
            ops: &ops,
            remount: false,
            theme_changed: false,
            diags_changed: false,
        });
        assert_eq!(msgs, vec!["OP:update:a", "OP:remove:b"]);
    }

    #[test]
    fn broadcast_remount_replaces_ops_and_keeps_theme_after_body() {
        // The load-bearing case: a full re-mount that also changed the theme and the
        // diagnostics. `full_render` replaces the ops, and `style`/`diagnostics` still
        // ride AFTER it (the re-mounted HTML carries the stale <style>).
        let ops = vec![BlockOp::Remove {
            target_id: "a".into(),
        }];
        let msgs = messages(super::Broadcast {
            ops: &ops,
            remount: true,
            theme_changed: true,
            diags_changed: true,
        });
        assert_eq!(msgs, vec!["FULL", "STYLE", "DIAG"]);
    }

    #[test]
    fn broadcast_diagnostics_only_change_sends_just_diagnostics() {
        // A rebuild that produced no block ops and no theme change but a new diagnostic
        // (e.g. the kernel came back) sends only the diagnostics message.
        let msgs = messages(super::Broadcast {
            ops: &[],
            remount: false,
            theme_changed: false,
            diags_changed: true,
        });
        assert_eq!(msgs, vec!["DIAG"]);
    }

    #[test]
    fn broadcast_no_change_sends_nothing() {
        let msgs = messages(super::Broadcast {
            ops: &[],
            remount: false,
            theme_changed: false,
            diags_changed: false,
        });
        assert!(msgs.is_empty());
    }

    #[test]
    fn build_state_serializes_phase_and_counts() {
        let s = super::build_state(Some("ch1.tmd"), "executing", 3, 8, "python");
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "build-state");
        assert_eq!(v["page"], "ch1.tmd");
        assert_eq!(v["phase"], "executing");
        assert_eq!(v["ran"], 3);
        assert_eq!(v["total"], 8);
        assert_eq!(v["lang"], "python");
    }

    #[test]
    fn cell_state_includes_state_and_optional_timing() {
        let s = super::cell_state(Some("p.tmd"), "abc", "running", Some(1000), None);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "cell-state");
        assert_eq!(v["cell_id"], "abc");
        assert_eq!(v["state"], "running");
        assert_eq!(v["started_ms"], 1000);
        assert!(v.get("duration_ms").is_none_or(|d| d.is_null()));
    }
}
