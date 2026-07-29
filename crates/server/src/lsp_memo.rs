//! A text-keyed memo of the last rendered buffer.

use std::sync::Arc;

/// A one-entry memo of the last rendered buffer, keyed on `(uri, text)`.
///
/// Keying on the text itself is the whole design: a different buffer is a different
/// key, so there is no invalidation to write and no staleness class to get wrong. The
/// URI is part of the key because [`crate::lsp::render_buffer`] derives the render base
/// directory from it, so the same text in two directories is two different renders.
///
/// One entry, not an LRU: the access pattern is many reads of the buffer the author is
/// currently typing in, and a second entry would only help when two documents are being
/// edited in strict alternation.
#[derive(Default)]
pub(crate) struct RenderMemo {
    last: Option<(lsp_types::Url, String, Arc<taliesin_core::RenderedDoc>)>,
}

impl RenderMemo {
    /// The render for `text` at `uri`, from cache when the key is unchanged. `None` when
    /// the buffer cannot be rendered (`render_buffer` is panic-guarded and returns `None`).
    pub(crate) fn get(
        &mut self,
        uri: &lsp_types::Url,
        text: &str,
    ) -> Option<Arc<taliesin_core::RenderedDoc>> {
        if let Some((u, t, doc)) = &self.last
            && u == uri
            && t == text
        {
            return Some(doc.clone());
        }
        let doc = Arc::new(crate::lsp::render_buffer(uri, text)?);
        self.last = Some((uri.clone(), text.to_owned(), doc.clone()));
        Some(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::RenderMemo;
    use lsp_types::Url;

    fn uri(name: &str) -> Url {
        Url::parse(&format!("file:///tmp/{name}")).unwrap()
    }

    #[test]
    fn repeated_identical_text_reuses_one_render() {
        let mut memo = RenderMemo::default();
        let u = uri("a.tmd");
        let a = memo.get(&u, "# Hi\n").unwrap();
        let b = memo.get(&u, "# Hi\n").unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&a, &b),
            "identical text must return the very same render, not an equal one"
        );
    }

    #[test]
    fn changed_text_renders_again() {
        let mut memo = RenderMemo::default();
        let u = uri("a.tmd");
        let a = memo.get(&u, "# Hi\n").unwrap();
        let b = memo.get(&u, "# Bye\n").unwrap();
        assert!(
            !std::sync::Arc::ptr_eq(&a, &b),
            "changed text must re-render"
        );
    }

    #[test]
    fn same_text_in_a_different_file_renders_again() {
        // The render base directory comes from the URI, so the URI is part of the key.
        // Keying on text alone would serve one directory's render for another's buffer.
        let mut memo = RenderMemo::default();
        let a = memo.get(&uri("a.tmd"), "# Hi\n").unwrap();
        let b = memo.get(&uri("sub/b.tmd"), "# Hi\n").unwrap();
        assert!(
            !std::sync::Arc::ptr_eq(&a, &b),
            "a different URI must re-render"
        );
    }
}
