//! Dataset provenance (backlog item 176): `{{< dataset >}}` cites the data a document was
//! computed from, so a reader can re-run it, without shipping the data itself.
//!
//! `corpus/datasets.tmd` is the pin. The unit tests in `render/extension/dataset.rs` cover
//! the card's fields on synthetic input; what this file adds is the two claims the pin
//! document actually makes to a reader — that a small in-tree file **travels with the
//! build**, and that a large remote one **does not** — plus the drift diagnostic, which is
//! the reason the feature exists rather than a nicety of it.

use std::fs;

mod common;
use common::corpus_dir;

const PIN: &str = "datasets.tmd";
const CSV: &str = "data/measurements.csv";

fn rendered() -> taliesin_core::RenderedDoc {
    let src = fs::read_to_string(corpus_dir().join(PIN)).unwrap();
    taliesin_core::render_single_doc(&src, &corpus_dir())
}

/// The card states what the file is, and states it from the FILE rather than from the
/// declaration. The size and digest are the two fields nobody should have to type, and
/// getting them from front matter would defeat the point: they could then be wrong.
#[test]
fn an_in_tree_dataset_card_reports_the_files_own_size_and_digest() {
    let body = rendered().body_html();
    let bytes = fs::metadata(corpus_dir().join(CSV)).unwrap().len();

    assert!(
        body.contains(&format!("<dd>{bytes} B</dd>")),
        "the card must state the file's measured size ({bytes} B)"
    );
    // Read back off the pin's own invocation the way a reader would, so this fails if the
    // fixture is edited without its declaration being updated — the drift case, checked
    // below. The annotations ride the shortcode since item 204, so this scrapes an
    // argument rather than a front-matter line.
    let pin_src = fs::read_to_string(corpus_dir().join(PIN)).unwrap();
    let declared = pin_src
        .split("sha256=")
        .nth(1)
        .map(|rest| {
            rest.split([' ', '\n', '>'])
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .filter(|d| d.len() == 64)
        .expect("the pin declares a sha256 for the in-tree file");
    assert!(
        body.contains(&format!("title=\"sha256:{declared}\"")),
        "the rendered digest must match the declared one; if it does not, the fixture \
         changed and `corpus/datasets.tmd` was not updated with it"
    );
    assert!(
        body.contains("CC0-1.0") && body.contains("https://example.org/tarn-survey"),
        "licence and origin come from the declaration, since a file cannot state them"
    );
}

/// The whole design in one assertion pair: small and local means a download link (which is
/// also the `href=` the build follows to copy it); large and remote means a fetch command
/// and never a copy. Reversing either would be a different, worse feature.
#[test]
fn a_local_dataset_is_offered_for_download_and_a_remote_one_is_not() {
    let body = rendered().body_html();

    assert!(
        body.contains(&format!("href=\"{CSV}\" download")),
        "the in-tree file is a download link, which is what makes the build copy it"
    );
    assert!(
        body.contains("curl -LO https://example.org/tarn-survey/full-season.parquet"),
        "the remote file is fetched by the reader, not bundled: {body}"
    );
    assert!(
        body.contains("sha256sum -c"),
        "and comes with the means to check what arrived"
    );
    // The negative half. A remote target must never be rendered as a local download, which
    // is the mistake that would silently 404 for every reader.
    assert!(
        !body.contains("full-season.parquet\" download"),
        "a remote dataset must not be offered as a local download"
    );
    assert!(
        !body.contains("curl -LO data/measurements.csv"),
        "and an in-tree file must not be handed a fetch command"
    );
}

/// The build ships a small in-tree dataset, because the card gives `copy_local_assets` an
/// `href=` to follow. This is the concrete answer to the item's premise: a `data/x.csv`
/// named only inside a `{python}` string is invisible to the build, so it is not copied,
/// not validated and not mentioned.
#[test]
fn the_built_page_carries_the_local_dataset_beside_it() {
    let doc = rendered();
    let refs = doc.body_html();
    assert!(
        refs.contains(&format!("href=\"{CSV}\"")),
        "the emitted href is what the asset copier follows; without it the build has no \
         way to know the file exists"
    );
    // And the path is document-relative, not absolute: an absolute one would name the
    // author's machine and copy nothing anywhere else.
    assert!(
        !refs.contains("href=\"/"),
        "a dataset href must be relative to the page"
    );
}

/// The diagnostic the item was filed for. The corpus document is clean, so this proves the
/// check fires by making the data drift in a scratch copy — the corpus itself must stay
/// warning-free, and a test that only ever renders the clean case cannot see the check
/// inverted.
#[test]
fn data_that_moved_under_the_document_is_reported() {
    let dir = std::env::temp_dir().join(format!("tali-ds-drift-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("data")).unwrap();
    fs::write(dir.join("data/m.csv"), b"a,b\n1,2\n").unwrap();

    let declare =
        |sha: &str| format!("---\ntitle: T\n---\n\n{{{{< dataset data/m.csv sha256={sha} >}}}}\n");

    // Known-positive first: with the right digest the document is silent. Without this
    // control, "declaring a digest warns" would pass the test below just as well.
    let good = taliesin_core::render_single_doc(
        &declare("f3e1a80e9ca9e0e17b1d1d0e4a9c8fa61a7e0b2a3d9c1e8f7b6a5c4d3e2f1a09"),
        &dir,
    );
    let drift_msgs = |d: &taliesin_core::RenderedDoc| {
        d.warnings
            .iter()
            .filter(|w| w.message.contains("changed since it was recorded"))
            .count()
    };
    // (That literal is deliberately wrong, so the assertion is the other way round.)
    assert_eq!(drift_msgs(&good), 1, "a wrong digest must be reported");

    // The real digest of `a,b\n1,2\n`, as a literal rather than shelled out to
    // `sha256sum`: that binary does not exist on macOS, which is two of the three targets
    // the release workflow builds. Reproduce with `printf 'a,b\n1,2\n' | sha256sum`.
    let real = "492d5ea496056f1a6a6592241032fab764c321596317930b4fa0e1e8bc3b7470";
    let clean = taliesin_core::render_single_doc(&declare(real), &dir);
    assert_eq!(
        drift_msgs(&clean),
        0,
        "a digest that matches must not warn: {:?}",
        clean
            .warnings
            .iter()
            .map(|w| &w.message)
            .collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&dir);
}

/// The pin document renders clean. A corpus document that warns would be caught by the
/// corpus walker anyway, but this states the *reason* it matters here: every warning this
/// feature emits is one a real author would have to act on, so the fixture has to be a
/// document that is actually correct.
#[test]
fn the_pin_document_is_warning_free() {
    let doc = rendered();
    let msgs: Vec<&String> = doc.warnings.iter().map(|w| &w.message).collect();
    assert!(msgs.is_empty(), "corpus/datasets.tmd warns: {msgs:?}");
}

/// The annotations moved onto the invocation (item 204), and joining `SHORTCODE_SPECS` is
/// what buys the closed-vocabulary did-you-mean. Worth its own pin because `dataset` is
/// dispatched AHEAD of that table, so it carried no argument linting at all while its
/// annotations lived in front matter: a typo'd sub-key was caught by the front-matter
/// validator, and moving them without this would have traded a diagnostic for silence.
#[test]
fn a_typod_dataset_argument_earns_a_did_you_mean() {
    let dir = std::env::temp_dir().join(format!("tali-ds-typo-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("d.csv"), b"a,b\n1,2\n").unwrap();

    let doc = taliesin_core::render_single_doc(
        "---\ntitle: T\n---\n\n{{< dataset d.csv souce=x licence=CC0-1.0 >}}\n",
        &dir,
    );
    let msgs: Vec<&String> = doc.warnings.iter().map(|w| &w.message).collect();
    assert!(
        msgs.iter()
            .any(|m| m.contains("`souce=`") && m.contains("did you mean `source=`")),
        "a typo'd argument must be named with its correction: {msgs:?}"
    );
    // The control: the correctly-spelled sibling on the SAME invocation stays silent, so
    // this is a vocabulary check rather than "any argument warns".
    assert!(
        !msgs.iter().any(|m| m.contains("licence")),
        "a known argument must not warn: {msgs:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}
