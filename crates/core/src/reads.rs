//! The files a render reads, or looks for and does not find, recorded as it runs.
//!
//! The dev server rebuilds an open page when a file the page depends on changes, and the one
//! complete answer to "which files does this page depend on" is the set its render touched:
//! its source, every `{{< include >}}` it tried, every `.bib` it loaded, every image it
//! measured or checked. A file it looked for and did not find is a dependency too, since
//! creating it changes the page. Re-deriving that set from the source (a second parse of the
//! includes, a second read of the front matter) had to agree with the readers and did not:
//! a shared `bibliography:` declared before its file existed, an image added or re-exported
//! at a new size, each was a file the page read that no second reader named (audit
//! 2026-09-24 C4, C7).
//!
//! So each read site calls [`note`] or [`probe`], and a caller that wants the set runs the
//! render inside [`record`]. Recording is per thread and off unless asked for, so a build,
//! the editor and a whole-project pass pay one thread-local check per read. A render that
//! runs on a worker thread carries its caller's recording there ([`current`], [`within`]).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// How a render used a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Access {
    /// Looked for or measured, not read as the page's text: an image, a linked file. A
    /// page's own cells can write such a file (`savefig("gen.png")`, shown as
    /// `![…](gen.png)`), which makes it the page's output as much as its input.
    Probed,
    /// Read as the page's text: its source, an `{{< include >}}`, a `.bib`.
    Read,
}

/// Every file a recording saw, each with the most it did with it.
pub type Reads = BTreeMap<PathBuf, Access>;

/// One recording in progress: the paths noted so far, shared with the threads it was handed
/// to.
#[derive(Clone, Default)]
pub(crate) struct Recording(Arc<Mutex<Reads>>);

thread_local! {
    static CURRENT: RefCell<Option<Recording>> = const { RefCell::new(None) };
}

/// Note that the work being recorded on this thread read `path` as text, or tried to and
/// found nothing there. Does nothing when no recording is running here.
pub fn note(path: &Path) {
    add(path, Access::Read);
}

/// Note that the work being recorded on this thread looked for `path` or measured it
/// without reading it as text. Does nothing when no recording is running here.
pub fn probe(path: &Path) {
    add(path, Access::Probed);
}

/// Kept absolute and lexically normalized, since a file that does not exist cannot be
/// canonicalized.
fn add(path: &Path, access: Access) {
    CURRENT.with(|current| {
        if let Some(recording) = &*current.borrow() {
            let mut reads = recording.0.lock().unwrap_or_else(|e| e.into_inner());
            merge_one(&mut reads, crate::includes::absolutize(path), access);
        }
    });
}

fn merge_one(reads: &mut Reads, path: PathBuf, access: Access) {
    let seen = reads.entry(path).or_insert(access);
    *seen = (*seen).max(access);
}

/// Add `more` to `reads`, keeping the most each saw of a file.
pub fn merge(reads: &mut Reads, more: Reads) {
    for (path, access) in more {
        merge_one(reads, path, access);
    }
}

/// Run `f`, returning its result and every path it noted, a render it ran on a worker
/// thread included. A recording started inside another collects the inner work's notes on
/// its own; the outer one does not see them.
pub fn record<T>(f: impl FnOnce() -> T) -> (T, Reads) {
    let recording = Recording::default();
    let out = within(Some(recording.clone()), f);
    let reads = std::mem::take(&mut *recording.0.lock().unwrap_or_else(|e| e.into_inner()));
    (out, reads)
}

/// The recording running on this thread, for a thread that works on its behalf.
pub(crate) fn current() -> Option<Recording> {
    CURRENT.with(|current| current.borrow().clone())
}

/// Run `f` with `recording` as this thread's, putting back whatever was there before, a
/// panic included.
pub(crate) fn within<T>(recording: Option<Recording>, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Recording>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            CURRENT.with(|current| *current.borrow_mut() = previous);
        }
    }
    let _restore = Restore(CURRENT.with(|current| current.replace(recording)));
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_collects_notes_on_its_thread_and_on_threads_it_is_handed_to() {
        let ((), reads) = record(|| {
            note(Path::new("/p/a.tmd"));
            probe(Path::new("/p/a.tmd"));
            probe(Path::new("/p/pic.png"));
            let handed = current();
            std::thread::spawn(move || within(handed, || note(Path::new("/p/sub/../b.bib"))))
                .join()
                .unwrap();
        });
        let want: Reads = [
            (PathBuf::from("/p/a.tmd"), Access::Read),
            (PathBuf::from("/p/b.bib"), Access::Read),
            (PathBuf::from("/p/pic.png"), Access::Probed),
        ]
        .into_iter()
        .collect();
        assert_eq!(reads, want, "a file read and probed was read");
        // Outside a recording a note goes nowhere, and the thread holds no recording after.
        note(Path::new("/p/c.png"));
        assert!(current().is_none());
    }
}
