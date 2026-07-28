//! The first browser test of `deck.js` (backlog items 112 + 125).
//!
//! **Why this exists.** The repo's browser automation (`chromiumoxide`, added for
//! `read --run`'s `{js}` observation) had never been pointed at the deck engine: every
//! existing deck test asserts what `deck.rs` *emits*, or reads `deck.js` as text. So the
//! 2,690 lines that decide what a reader actually sees when they press a key were covered
//! by nothing that runs them. This walks a built `corpus/deck.tmd` with **real** key
//! events (CDP `Input.dispatchKeyEvent`, not a synthetic `KeyboardEvent` — the Cmd-K
//! lesson: a synthetic event can leave the feature untouched and the assertion vacuous)
//! and asserts two properties the emission tests structurally cannot see:
//!
//! * **112 — the deep link tracks the slide.** At every step the `#/slug` in the address
//!   bar names the slide the reader is on, and re-opening that address in a *fresh page*
//!   lands on the same slide and the same fragment step. The round-trip is the load-bearing
//!   half: a hash writer that is wrong but self-consistent satisfies the first check and
//!   fails this one.
//! * **125 — deck content becomes auditable.** A conformance tool that loads the page sees
//!   one slide: every off-camera slide is `inert` (correct — `inert` is what keeps a screen
//!   reader and the tab order on the current slide), so a Lighthouse-style audit covers a
//!   few percent of the deck. Stepping exposes every slide in turn, and this asserts that
//!   the union over the walk is the whole deck **and** that every slide passes the two
//!   `validate_a11y` rules a live DOM can check. That is the conformance claim about deck
//!   content the project could not previously make.
//!
//! It earned its keep on the first run: the same walk found **two shipped layout defects**
//! that every emission test passed over, both fixed here and pinned below — code blocks
//! clipped off the right edge of 5 of 21 slides, and the browser's own focus ring painted
//! around every slide in a vertical stack.
//!
//! Gated exactly like `read_run_js.rs`: no system Chrome → skip, unless
//! `TALIESIN_REQUIRE_CHROME=1`, which turns the skip into a hard failure so this coverage
//! cannot silently regress to zero. One browser run serves every test here (a `OnceLock`),
//! because launching Chrome and walking 21 slides is the expensive part.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use chromiumoxide::cdp::browser_protocol::input::{DispatchKeyEventParams, DispatchKeyEventType};
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;

/// Hard cap on key presses. The corpus deck is 21 slides plus its fragment steps, so this
/// is several times the real distance: it exists to bound a deck that wraps or refuses to
/// advance, never to bound a normal walk (the walk asserts it reached the last slide).
const MAX_PRESSES: usize = 200;

/// The `ArrowRight` virtual key code (Windows + native), which Chrome needs to synthesise
/// a key event the page cannot tell from a real one.
const VK_ARROW_RIGHT: i64 = 39;

// ---------------------------------------------------------------------------
// Chrome gate (mirrors read_run_js.rs, which mirrors headless_js::chrome_path)
// ---------------------------------------------------------------------------

fn which_chrome() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHROME_PATH") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ] {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// `true` when the live case should run. `false` (skip) unless `TALIESIN_REQUIRE_CHROME=1`,
/// which makes a missing Chrome a hard failure.
fn have_chrome() -> bool {
    if which_chrome().is_some() {
        return true;
    }
    assert!(
        std::env::var_os("TALIESIN_REQUIRE_CHROME").is_none(),
        "TALIESIN_REQUIRE_CHROME=1 but no system Chrome found: the deck engine would go untested"
    );
    eprintln!("skipping: no system Chrome (set CHROME_PATH or install google-chrome/chromium)");
    false
}

// ---------------------------------------------------------------------------
// The observed state of the deck at one step
// ---------------------------------------------------------------------------

/// One settled reading of the deck, taken after every key press. Everything here is a fact
/// about the **live** DOM + the `window.TaliesinDeck` facade, never a re-derivation of what
/// `deck.js` should have done.
#[derive(Debug, Clone, serde::Deserialize)]
struct DeckState {
    ready: bool,
    /// `getSlides().length` — leaf slides, so a vertical stack contributes its children.
    total: usize,
    /// `getSlides().indexOf(getCurrentSlide())`.
    index: i64,
    /// The current slide's `id` (`""` when it has none).
    id: String,
    h: i64,
    v: i64,
    f: i64,
    hash: String,
    /// The position token of the hash: everything before the first `/` or `?`. `""` when
    /// the deck has written no hash yet.
    #[serde(rename = "hashToken")]
    hash_token: String,
    /// Which slide that token resolves to, the way a reader's browser resolves it
    /// (`getElementById` → nearest slide `<section>`): `-1` when it names no slide at all.
    #[serde(rename = "hashIndex")]
    hash_index: i64,
    /// Indices of the slides that are NOT `inert`, i.e. the ones an audit tool, a screen
    /// reader or the tab order can reach right now.
    exposed: Vec<usize>,
    /// Accessibility violations found on the exposed slides: the two `validate_a11y` rules
    /// that are checkable against a live DOM (`<img>` with no `alt`, interactive element
    /// with no accessible name). One string per violation, already naming its slide.
    a11y: Vec<String>,
    /// How many elements the scan above actually looked at. The control on a
    /// zero-violation verdict: a selector that matches nothing reports "clean" forever.
    scanned: usize,
    /// Content on an exposed slide whose right edge lands outside the slide's content box,
    /// i.e. in the region `.tali-deck { overflow: hidden }` clips. One string per offender.
    overflow: Vec<String>,
    /// The `outline-style` computed on the slide that currently holds focus, and whether
    /// the deck really did move focus onto it. `"none"` is the only acceptable style: the
    /// slide is a programmatic `tabindex="-1"` target, not a control.
    #[serde(rename = "focusOutline")]
    focus_outline: String,
    #[serde(rename = "focusOnSlide")]
    focus_on_slide: bool,
}

/// Everything one browser run produced: the stepped walk, plus what a fresh page did when
/// opened at two of the deep links the walk captured.
struct Walk {
    states: Vec<DeckState>,
    /// `(hash re-opened, state after that page settled)`.
    roundtrips: Vec<(String, DeckState)>,
}

// ---------------------------------------------------------------------------
// The walk, computed once
// ---------------------------------------------------------------------------

static WALK: OnceLock<Result<Walk, String>> = OnceLock::new();

fn walk() -> &'static Result<Walk, String> {
    WALK.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("tali-deck-browser-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
        let page = build_corpus_deck(&dir)?;
        let out = tokio::runtime::Runtime::new()
            .map_err(|e| format!("tokio runtime: {e}"))?
            .block_on(drive(&page));
        let _ = std::fs::remove_dir_all(&dir);
        out
    })
}

/// Build `corpus/deck.tmd` into `dir` and return the standalone page.
///
/// `TALIESIN_NO_EXEC=1` deliberately: the subject is the deck **engine**, and executing the
/// deck's `{python}` cell would make this test's page depend on whether the host has a
/// kernel — a different page on a laptop than in CI, for no gain here. It leaves `deck.js`
/// and every slide untouched (no-exec only reaches kernel cells and `{js}`).
fn build_corpus_deck(dir: &Path) -> Result<PathBuf, String> {
    let src = format!("{}/../../corpus/deck.tmd", env!("CARGO_MANIFEST_DIR"));
    let out = dir.join("deck.html");
    let res = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .args(["build", &src])
        .arg(&out)
        .env("TALIESIN_NO_EXEC", "1")
        .output()
        .map_err(|e| format!("run build: {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "build corpus/deck.tmd failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }
    out.exists()
        .then_some(out)
        .ok_or_else(|| "build reported success but wrote no page".to_string())
}

/// Launch a throwaway headless Chrome, walk the deck, then always tear it down.
async fn drive(page_path: &Path) -> Result<Walk, String> {
    let exe = which_chrome().ok_or_else(|| "chrome unavailable".to_string())?;
    let profile = std::env::temp_dir().join(format!("tali-deck-profile-{}", std::process::id()));
    let config = BrowserConfig::builder()
        .chrome_executable(&exe)
        .new_headless_mode()
        // Same reasoning as `headless_js.rs`: Chrome's own sandbox needs unprivileged user
        // namespaces or the setuid helper and is unavailable in containers/CI, where the
        // browser would then fail to start and take this whole gate with it. The page is a
        // `file://` document this repo just rendered from its own corpus.
        .no_sandbox()
        .user_data_dir(&profile)
        // LANDSCAPE, and load-bearing: a deck routes by ASPECT, so a portrait window opens
        // the phone slide-feed instead of the stepped deck and none of this applies.
        .window_size(1280, 900)
        .launch_timeout(Duration::from_secs(20))
        .request_timeout(Duration::from_secs(20))
        .args(vec![
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            "--disable-extensions",
        ])
        .build()
        .map_err(|e| format!("chrome config: {e}"))?;

    let launched = Browser::launch(config)
        .await
        .map_err(|e| format!("chrome launch failed: {e}"));
    let (mut browser, mut handler) = match launched {
        Ok(pair) => pair,
        Err(reason) => {
            let _ = std::fs::remove_dir_all(&profile);
            return Err(reason);
        }
    };
    let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let result = walk_the_deck(&browser, page_path).await;

    let closed = tokio::time::timeout(Duration::from_secs(20), browser.close())
        .await
        .is_ok();
    let exited = closed
        && tokio::time::timeout(Duration::from_secs(20), browser.wait())
            .await
            .is_ok();
    if !exited {
        let _ = tokio::time::timeout(Duration::from_secs(20), browser.kill()).await;
    }
    handler_task.abort();
    let _ = std::fs::remove_dir_all(&profile);
    result
}

async fn walk_the_deck(browser: &Browser, page_path: &Path) -> Result<Walk, String> {
    let url = format!("file://{}", page_path.display());
    let page = open_deck(browser, &url).await?;

    let mut states = vec![settled_state(&page).await?];
    for _ in 0..MAX_PRESSES {
        let before = states.last().expect("seeded above").clone();
        press_arrow_right(&page).await?;
        let now = settled_state(&page).await?;
        let moved = (now.index, now.f) != (before.index, before.f);
        states.push(now);
        if !moved {
            break; // the end of the deck: `next()` at the last step is a no-op
        }
    }

    // Two deep links from the walk, re-opened in a FRESH page: the last one, and the first
    // one that carried a fragment step (`#/slug/2`), because slide and fragment travel in
    // the same token and only a state with `f > 0` exercises both segments.
    let mut roundtrips = Vec::new();
    let mut wanted: Vec<&DeckState> = Vec::new();
    if let Some(fragged) = states.iter().find(|s| s.f > 0) {
        wanted.push(fragged);
    }
    if let Some(last) = states.iter().rev().find(|s| !s.hash.is_empty()) {
        wanted.push(last);
    }
    for want in wanted {
        let deep = open_deck(browser, &format!("{url}{}", want.hash)).await?;
        let landed = settled_state(&deep).await?;
        roundtrips.push((want.hash.clone(), landed));
        let _ = deep.close().await;
    }

    let _ = page.close().await;
    Ok(Walk { states, roundtrips })
}

/// Open `url` and wait for `TaliesinDeck.isReady()`.
async fn open_deck(browser: &Browser, url: &str) -> Result<Page, String> {
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new page: {e}"))?;
    page.goto(url).await.map_err(|e| format!("navigate: {e}"))?;
    for _ in 0..100 {
        if read_state(&page).await?.ready {
            return Ok(page);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(format!("the deck never reported ready at {url}"))
}

/// A real `ArrowRight`, dispatched through CDP so the page receives a **trusted** key event
/// (`rawKeyDown` + `keyUp`, the pair Chrome itself sends for a non-text key).
async fn press_arrow_right(page: &Page) -> Result<(), String> {
    for kind in [
        DispatchKeyEventType::RawKeyDown,
        DispatchKeyEventType::KeyUp,
    ] {
        let params = DispatchKeyEventParams::builder()
            .r#type(kind)
            .key("ArrowRight")
            .code("ArrowRight")
            .windows_virtual_key_code(VK_ARROW_RIGHT)
            .native_virtual_key_code(VK_ARROW_RIGHT)
            .build()
            .map_err(|e| format!("key params: {e}"))?;
        page.execute(params)
            .await
            .map_err(|e| format!("dispatch ArrowRight: {e}"))?;
    }
    Ok(())
}

/// Read the deck's state once it has stopped moving: two consecutive identical readings.
///
/// The settle condition is "nothing moved between two readings", which is deliberately
/// **not** "wait until the assertions pass" — a stable *wrong* state is returned and fails,
/// where a wait-until-green loop would turn a real disagreement into a slow pass.
///
/// It covers geometry as well as position because the `auto-animate` pair morphs elements
/// between two slides: position and hash go stable the moment the move commits, while the
/// morph is still in flight, so a geometry assertion read at that instant would be sampling
/// a transform mid-animation.
async fn settled_state(page: &Page) -> Result<DeckState, String> {
    let fingerprint = |s: &DeckState| {
        (
            s.index,
            s.f,
            s.hash.clone(),
            s.exposed.clone(),
            s.overflow.clone(),
            s.focus_outline.clone(),
        )
    };
    let mut prev = read_state(page).await?;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(25)).await;
        let now = read_state(page).await?;
        if fingerprint(&now) == fingerprint(&prev) {
            return Ok(now);
        }
        prev = now;
    }
    Ok(prev)
}

async fn read_state(page: &Page) -> Result<DeckState, String> {
    let res = tokio::time::timeout(
        Duration::from_secs(15),
        page.evaluate_function(STATE_SCRIPT),
    )
    .await
    .map_err(|_| "reading deck state timed out".to_string())?
    .map_err(|e| format!("evaluate: {e}"))?;
    res.into_value().map_err(|e| format!("decode state: {e}"))
}

/// The in-page probe. Facts only: what the facade reports, what the address bar says, which
/// slides are reachable, and the a11y scan of the reachable ones.
const STATE_SCRIPT: &str = r#"function () {
  var D = window.TaliesinDeck;
  var slides = D ? D.getSlides() : [];
  var cur = D ? D.getCurrentSlide() : null;
  var ix = D ? D.getIndices() : { h: 0, v: 0, f: 0 };

  // Resolve the hash the way a reader's browser does, NOT the way deck.js wrote it: take
  // the position token and ask the document which slide carries that id.
  var raw = location.hash.replace(/^#\/?/, '').split('?')[0];
  var token = raw ? raw.split('/')[0] : '';
  var hashIndex = -1;
  if (token) {
    var el = document.getElementById(token);
    var owner = el && el.closest ? el.closest('.tali-slides section') : null;
    if (owner) hashIndex = slides.indexOf(owner);
  }

  // The two validate_a11y rules a live DOM can check, over the slides that are currently
  // reachable. Scoped to the slide sections, so deck CHROME (menu, arrows, progress bar)
  // is out: the claim is about the author's content.
  var a11y = [];
  var exposed = [];
  var scanned = 0;
  var overflow = [];
  slides.forEach(function (s, i) {
    if (s.hasAttribute('inert')) return;
    exposed.push(i);
    var where = 'slide ' + i + (s.id ? ' (#' + s.id + ')' : '');

    // Does anything sit outside the slide's content box on the right, where
    // `.tali-deck { overflow: hidden }` clips it? `.tali-slide-bg` is EXEMPT and must
    // stay exempt: a per-slide backdrop is deliberately full-bleed, so it covers the
    // slide's padding by design.
    //
    // The stage is a SCALED camera, so `getBoundingClientRect` is in rendered px while
    // `getComputedStyle` padding is in unscaled CSS px. Mixing the two silently shifts the
    // edge by `padding * (scale - 1)` — 13px at the usual 1.333 — which is enough to
    // invent or hide an overflow. `scale` converts.
    var sb = s.getBoundingClientRect();
    var scale = s.offsetWidth ? sb.width / s.offsetWidth : 1;
    var edge = sb.right - parseFloat(getComputedStyle(s).paddingRight || '0') * scale;
    var geom = ' [slide ' + Math.round(sb.width) + 'x' + s.offsetWidth +
      ' @' + scale.toFixed(3) + ']';
    s.querySelectorAll('*').forEach(function (el) {
      var r = el.getBoundingClientRect();
      if (r.width === 0 || el.closest('.tali-slide-bg')) return;
      // 1px of tolerance for sub-pixel rounding at a fractional camera scale.
      var past = Math.round(r.right - edge);
      if (past > 1) {
        overflow.push(where + ': <' + el.tagName.toLowerCase() + '> ends ' + past +
          'px past the content edge (clipped)' + geom);
      }
    });

    s.querySelectorAll('img').forEach(function (img) {
      scanned++;
      if (!img.hasAttribute('alt')) a11y.push(where + ': <img> with no alt: ' + img.outerHTML.slice(0, 90));
    });
    s.querySelectorAll('a[href], button, [role="button"], [role="link"], [role="tab"]').forEach(function (el) {
      scanned++;
      var name = (el.textContent || '').trim() || el.getAttribute('aria-label') || el.getAttribute('title') || '';
      if (!name) {
        var labelled = el.querySelector('img[alt]:not([alt=""]), svg [id], svg title');
        if (!labelled) a11y.push(where + ': interactive element with no accessible name: ' + el.outerHTML.slice(0, 90));
      }
    });
  });

  // Focus follows the current slide (deck.js moves it so a keyboard/AT user is not
  // stranded on the slide that just went inert). What must NOT follow is the browser's
  // default focus ring, which the audience sees.
  var active = document.activeElement;
  var focusOnSlide = !!(active && active.classList && active.classList.contains('tali-slide'));
  var focusOutline = focusOnSlide ? getComputedStyle(active).outlineStyle : '';

  return {
    ready: !!(D && D.isReady && D.isReady()),
    total: slides.length,
    index: cur ? slides.indexOf(cur) : -1,
    id: cur && cur.id ? cur.id : '',
    h: ix.h | 0, v: ix.v | 0, f: ix.f | 0,
    hash: location.hash,
    hashToken: token,
    hashIndex: hashIndex,
    exposed: exposed,
    a11y: a11y,
    scanned: scanned,
    overflow: overflow,
    focusOutline: focusOutline,
    focusOnSlide: focusOnSlide
  };
}"#;

/// The walk, or a skip. `None` means no Chrome (already gated), so the caller returns.
fn walked() -> Option<&'static Walk> {
    if !have_chrome() {
        return None;
    }
    match walk() {
        Ok(w) => Some(w),
        Err(e) => panic!("the deck walk did not run: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Item 112 — the deep link tracks the slide
// ---------------------------------------------------------------------------

#[test]
fn stepping_the_deck_keeps_the_hash_and_the_slide_index_in_agreement() {
    let Some(w) = walked() else { return };

    // The walk really walked: it reached the last slide, and it did so by pressing a key
    // rather than by never moving. Without these two, every assertion below is vacuous on
    // a deck that ignored the keyboard entirely.
    let last = w.states.last().expect("at least the initial state");
    assert!(
        last.total >= 20,
        "the corpus deck should be ~21 leaf slides, got {}",
        last.total
    );
    assert_eq!(
        last.index,
        last.total as i64 - 1,
        "ArrowRight never reached the last slide ({} presses, ended at {} of {})",
        w.states.len() - 1,
        last.index,
        last.total
    );

    // A freshly-loaded deck has written no hash (there was no deep link to preserve and
    // nothing has moved), so the agreement starts at the first move.
    let moves: Vec<&DeckState> = w.states.iter().skip(1).collect();
    assert!(
        moves.len() > w.states[0].total,
        "only {} steps for {} slides — fragments were not stepped through",
        moves.len(),
        w.states[0].total
    );
    for (n, s) in moves.iter().enumerate() {
        assert!(
            s.hash.starts_with("#/"),
            "step {n}: the deck moved to slide {} but the address bar says {:?}",
            s.index,
            s.hash
        );
        assert_eq!(
            s.hash_index, s.index,
            "step {n}: the hash {:?} names slide {} but the reader is on slide {} (#{})",
            s.hash, s.hash_index, s.index, s.id
        );
        // The slug is the slide's own id, which is what makes the link readable and
        // stable across an edit that inserts a slide before it.
        assert_eq!(
            s.hash_token, s.id,
            "step {n}: expected the slide's own id in the hash, got {:?}",
            s.hash
        );
    }

    // `ArrowRight` only ever moves forward, and the walk must have stepped INTO the
    // vertical stack (`# A deeper topic` and its two sub-slides). Without the second
    // assertion the harness could silently skip the one shape that makes the position
    // two-dimensional, and `h`/`v` would never disagree because `v` was always 0.
    let mut prev_h = 0;
    for (n, s) in moves.iter().enumerate() {
        assert!(
            s.h >= prev_h,
            "step {n}: ArrowRight moved backwards, h {prev_h} -> {}",
            s.h
        );
        prev_h = s.h;
    }
    assert!(
        moves.iter().any(|s| s.v > 0),
        "the walk never reached v > 0: it stepped past the vertical stack instead of into it"
    );

    // The fragment step rides in the same hash, and at least one step must have carried
    // one — otherwise the `/f` segment of the writer is untested.
    let fragged: Vec<&&DeckState> = moves.iter().filter(|s| s.f > 0).collect();
    assert!(
        !fragged.is_empty(),
        "no step of the walk revealed a fragment: the `#/slug/N` form is untested"
    );
    for s in fragged {
        assert!(
            s.hash.starts_with(&format!("#/{}/{}", s.id, s.f)),
            "a fragment step must appear in the hash: f={} but hash is {:?}",
            s.f,
            s.hash
        );
    }
}

#[test]
fn a_deep_link_captured_while_stepping_reopens_the_same_slide_and_fragment() {
    let Some(w) = walked() else { return };

    // This is the half a self-consistent-but-wrong writer cannot pass: the address is fed
    // back to a FRESH page, and the deck has to land where the reader was.
    assert_eq!(
        w.roundtrips.len(),
        2,
        "expected a fragment deep link and an end-of-deck deep link, got {:?}",
        w.roundtrips.iter().map(|(h, _)| h).collect::<Vec<_>>()
    );
    for (hash, landed) in &w.roundtrips {
        let want = w
            .states
            .iter()
            .find(|s| &s.hash == hash)
            .expect("every round-tripped hash came from the walk");
        assert_eq!(
            (landed.index, landed.f),
            (want.index, want.f),
            "re-opening {hash:?} landed on slide {} step {}, but it was captured on slide {} step {}",
            landed.index,
            landed.f,
            want.index,
            want.f
        );
        assert_eq!(
            landed.id, want.id,
            "re-opening {hash:?} landed on a different slide id"
        );
    }
}

// ---------------------------------------------------------------------------
// Layout: nothing a slide says gets clipped away
// ---------------------------------------------------------------------------

/// Found by *looking* at the deck this harness had just started stepping, which is the
/// point: every emission test passed while **5 of 21 slides** clipped their own content.
///
/// `.tali-deck pre` set `width: 100%` with `padding: .8em 1em` and a `1px` border, and the
/// global reset is `content-box` — so a code block computed ~32px wider than the slide's
/// content box, ran off the RIGHT edge into `overflow: hidden`, and took the copy button
/// with it. It also made `fitSlide` shrink every code slide to fit an overflow that should
/// not have existed, so the code was rendered smaller than the design calls for.
///
/// The rule is the durable form of that bug: nothing on a slide may end outside the slide's
/// content box. `.tali-slide-bg` is exempt by construction (a per-slide backdrop is
/// full-bleed on purpose) and that exemption is in the probe, not here.
#[test]
fn no_slide_content_is_clipped_by_the_slide_edge() {
    let Some(w) = walked() else { return };

    let clipped: Vec<&String> = w.states.iter().flat_map(|s| s.overflow.iter()).collect();
    assert!(
        clipped.is_empty(),
        "{} element(s) on the corpus deck end past the slide's content edge, where the \
         deck clips them:\n  {}",
        clipped.len(),
        clipped
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// The audience must never see the browser's focus ring, and it did — around every slide in
/// a **vertical stack**, from the first key press that entered one.
///
/// `deck.js` moves focus to the slide that becomes current (correct: the previous slide
/// goes `inert`, so a keyboard/AT user would otherwise be stranded on it), and `deck.css`
/// suppressed the resulting ring with `.tali-slides > section:focus-visible`. That is a
/// **child** combinator, and a vertical sub-slide is a grandchild — inside a `.tali-stack`
/// — so the rule missed it and Chrome's own `outline: auto 1px` painted a light rectangle
/// around the slide on a projected deck. This is why the walk asserts it steps *into* the
/// stack: on top-level slides alone the property already held.
#[test]
fn the_focused_slide_never_shows_the_browsers_focus_ring() {
    let Some(w) = walked() else { return };

    // Control first: the rule below is about a slide that HAS focus, so a walk where focus
    // never landed on one would satisfy it while testing nothing.
    let focused: Vec<&DeckState> = w.states.iter().filter(|s| s.focus_on_slide).collect();
    assert!(
        focused.len() > 1,
        "focus landed on a slide in only {} of {} steps — deck.js stopped moving focus \
         with the current slide, which strands a keyboard user on an inert slide",
        focused.len(),
        w.states.len()
    );
    for (n, s) in focused.iter().enumerate() {
        assert_eq!(
            s.focus_outline, "none",
            "step {n} (slide {} #{}): the focused slide paints `outline-style: {}` — the \
             audience sees a focus ring on the slide",
            s.index, s.id, s.focus_outline
        );
    }
}

// ---------------------------------------------------------------------------
// Item 125 — deck content becomes auditable
// ---------------------------------------------------------------------------

#[test]
fn every_slide_becomes_auditable_when_the_deck_is_stepped() {
    let Some(w) = walked() else { return };

    let first = &w.states[0];
    let total = first.total;

    // The finding, pinned in the direction it was found: a tool that just LOADS the page
    // audits a small minority of the deck, because every off-camera slide is `inert`. That
    // is the correct implementation (it keeps a screen reader and the tab order on the
    // current slide), so this asserts the design holds, not that it should change.
    assert!(
        first.exposed.len() < total,
        "every slide is exposed at rest ({} of {total}) — off-camera slides lost `inert`, \
         which puts the whole deck in the tab order and the AT tree at once",
        first.exposed.len()
    );

    // And the remedy: stepping reaches all of it. The union over the walk must be every
    // slide, so an audit that steps has 100% coverage of deck content.
    let mut seen: Vec<usize> = w
        .states
        .iter()
        .flat_map(|s| s.exposed.iter().copied())
        .collect();
    seen.sort_unstable();
    seen.dedup();
    let missed: Vec<usize> = (0..total).filter(|i| !seen.contains(i)).collect();
    assert!(
        missed.is_empty(),
        "stepping the whole deck never exposed slide(s) {missed:?} of {total}: \
         their content cannot be audited, or reached by a keyboard or screen-reader user"
    );

    // The control on the verdict below, and the reason it means anything: the scan must
    // have LOOKED at something. A zero-violation report from a selector that matches
    // nothing is the shape this repo has been burned by before, and `corpus/deck.tmd`
    // deliberately carries a captioned image for the scan to land on.
    let scanned: usize = w.states.iter().map(|s| s.scanned).sum();
    assert!(
        scanned > 0,
        "the a11y scan examined no elements across {} steps — it is reporting clean \
         because it selected nothing, not because the deck is clean",
        w.states.len()
    );

    // The claim itself: over 100% of the deck's slides, zero violations of the two
    // `validate_a11y` rules a live DOM can check. This is what the project could not
    // previously say about deck content at all.
    let violations: Vec<&String> = w.states.iter().flat_map(|s| s.a11y.iter()).collect();
    assert!(
        violations.is_empty(),
        "{} accessibility violation(s) across the stepped deck:\n  {}",
        violations.len(),
        violations
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}
