//! The shared headless-Chrome launch policy: find a system browser, start one in a
//! throwaway profile, bound every phase, and always tear both down.
//!
//! **Why it is a module and not part of its caller.** The print track (`pdf.rs`) drives
//! paged.js through CDP, and getting the launch/teardown wrong leaks a browser process and a
//! profile directory per invocation. [`with_browser`] is that policy in one place. It had a
//! second caller — `read --run`'s `{js}` observation, which reported whether a browser-run
//! cell actually painted an `<svg>` — until Wave 2 cut the verb; the policy is unchanged, and
//! `every_browser_await_is_bounded` still enforces it over whatever calls it.
//!
//! **Invariants held:** offline (a local `file://` page whose assets are already inlined + a
//! local browser, no network — the browser-download `fetcher` feature is off); gated +
//! optional (no Chrome, or a launch/timeout failure, is a reported skip, never a hang).
//!
//! **Compiled always, driven only under `--features headless-js`.** The CDP driver is what
//! the feature gates (the `chromiumoxide` half of it is 24% of a clean release build), but
//! the Chrome-discovery walk and the timeout policy are pure logic worth testing in every
//! configuration — so the module stays compiled and only the fns that *name* `chromiumoxide`
//! are `#[cfg]`-ed out. Without the feature the binary calls none of this, hence the blanket
//! `dead_code` allow: it means "the driver is off", never "this is unused code" — every item
//! below is still exercised by the unit tests at the bottom.
#![cfg_attr(not(feature = "headless-js"), allow(dead_code, unused_imports))]

use std::path::PathBuf;
use std::time::Duration;

/// The system Chrome to drive: `$CHROME_PATH` (an explicit but non-existent value means
/// "skip", so the negative test can force a Chrome-less run), else the first of these on
/// `PATH`. Mirrors `tools/ui-audit/lib/browser.mjs`'s candidate list.
pub(crate) fn chrome_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHROME_PATH") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    const CANDIDATES: &[&str] = &[
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ];
    CANDIDATES.iter().find_map(|name| which_on_path(name))
}

/// First executable named `name` on `$PATH` (a tiny `which`, so no `which` crate is pulled).
fn which_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// Per-page settle budget for `{js}` observation. `TALIESIN_JS_TIMEOUT` (seconds; `0`/unset
/// → default 10) is its own knob so a long `TALIESIN_CELL_TIMEOUT` python budget doesn't
/// bloat the browser wait.
pub(crate) fn settle_timeout() -> Duration {
    let secs = std::env::var("TALIESIN_JS_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(10);
    Duration::from_secs(secs)
}

/// The wall-clock bound on **each** browser phase — launch, open, navigate, evaluate, close,
/// wait. The settle budget plus the same slack the in-page eval already used, so no phase can
/// hang `read --run-js` and the worst case is a small multiple of a budget the author set.
///
/// This exists because chromiumoxide has bounds of its own and they are *not* ours: a silent
/// 20 s `launch_timeout` and a 30 s `request_timeout`, neither derived from
/// `TALIESIN_JS_TIMEOUT`, and `launch_timeout` covers only reading the DevTools URL off the
/// child's stderr — the websocket connect that follows it is unbounded, and so are
/// `close()`/`wait()`. Setting the library's knobs *and* wrapping each await means a future
/// change to either default cannot silently unbound this path.
fn phase_timeout() -> Duration {
    settle_timeout() + Duration::from_secs(5)
}

/// The wall-clock bound on the in-page evaluation, which is the one phase whose subject
/// already has a deadline of its own: the observe script counts `budget` down itself and then
/// answers. So this bound exists only for a page that never answers at all (a crashed
/// renderer), and it **must outlast the budget it wraps** — at or below it, it fires first and
/// reports `timed out` for a page that was about to return its results.
pub(crate) fn eval_timeout(budget: Duration) -> Duration {
    budget + Duration::from_secs(5)
}

/// Launch a throwaway headless Chrome (its own temp profile, so it never collides with a
/// dev/MCP Chrome), run `f` against it, then always tear the browser + profile down.
///
/// **Every phase is bounded** ([`phase_timeout`]) and every exit removes the profile, so a
/// browser that starts and then stops answering degrades to an `Err(reason)` like any other
/// failure instead of hanging the command with no diagnostic (L3-1).
///
/// Extracted from `observe_inner` so the print driver (`pdf.rs`, backlog 159) reuses this
/// exact launch/teardown policy rather than growing a second copy that drifts. `f` borrows
/// the browser and is *not* separately bounded: bounding it here would drop its future
/// mid-flight and skip the profile removal below, which is the leak L3-1 closed. Every
/// phase inside `f` is expected to carry its own bound, which
/// `every_browser_await_is_bounded` enforces for both callers.
#[cfg(feature = "headless-js")]
pub(crate) async fn with_browser<T>(
    f: impl AsyncFnOnce(&chromiumoxide::Browser, Duration) -> Result<T, String>,
) -> Result<T, String> {
    use chromiumoxide::{Browser, BrowserConfig};
    use futures::StreamExt;

    let exe = chrome_path().ok_or_else(|| "chrome unavailable".to_string())?;
    let profile = unique_profile_dir();
    let phase = phase_timeout();
    let config = BrowserConfig::builder()
        .chrome_executable(&exe)
        .new_headless_mode()
        // `--no-sandbox` (L3-2, the reasoning this decision was missing). Chrome's own
        // sandbox needs either unprivileged user namespaces or the setuid helper, and it is
        // unavailable in exactly the environments this runs in unattended: containers, CI
        // images, and any host with `kernel.unprivileged_userns_clone=0`. Without this flag
        // the browser exits at startup there and every `{js}` cell reports `skipped`, which
        // is a silent loss of the whole feature. What the flag gives up is the OS-level
        // containment of *page* content — and the page here is a `file://` document this
        // tool just rendered from the user's own source, with no network (`observe_page`
        // never leaves `file://`) and no third-party origin, i.e. exactly the author-trusted
        // input the crate's trust model already assumes. The browser is throwaway, has its
        // own empty profile, and is killed at the end of the call.
        .no_sandbox()
        .user_data_dir(&profile)
        .window_size(1280, 900)
        // Make the library's own bounds ours, so they track `TALIESIN_JS_TIMEOUT` instead of
        // its silent 20 s / 30 s defaults.
        .launch_timeout(phase)
        .request_timeout(phase)
        .args(vec![
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            "--disable-extensions",
        ])
        .build()
        .map_err(|e| format!("chrome config: {e}"))?;

    // The outer bound covers the one part of launching that `launch_timeout` does not: the
    // websocket connect that follows DevTools-URL detection. It sits just above the config
    // bound so the library's own error — which carries the browser's stderr — wins whenever
    // it can. Dropping this future is safe: the spawned child is `kill_on_drop`, and the
    // profile directory is removed on this path like every other.
    let launched = tokio::time::timeout(phase + Duration::from_secs(2), Browser::launch(config))
        .await
        .map_err(|_| "chrome launch timed out".to_string())
        .and_then(|r| r.map_err(|e| format!("chrome launch failed: {e}")));
    let (mut browser, mut handler) = match launched {
        Ok(pair) => pair,
        Err(reason) => {
            let _ = std::fs::remove_dir_all(&profile);
            return Err(reason);
        }
    };
    let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let result = f(&browser, phase).await;

    // Tear down regardless of the observation result, and never wait forever for a browser
    // that has decided not to leave: ask politely, then kill. `close()` can hang when the
    // handler never answers, and `wait()` when Chrome accepts the close and then does not
    // exit — the two unbounded awaits that made a wedged browser hang the whole command.
    let closed = tokio::time::timeout(phase, browser.close()).await.is_ok();
    let exited = closed && tokio::time::timeout(phase, browser.wait()).await.is_ok();
    if !exited {
        let _ = tokio::time::timeout(phase, browser.kill()).await;
    }
    handler_task.abort();
    let _ = std::fs::remove_dir_all(&profile);
    result
}

/// A unique temp user-data dir, `tali-headless-<pid>_<uuid>`. Distinct prefix from the
/// kernel/warm-pool dirs so the startup sweep leaves it alone; `observe_inner` removes it.
#[cfg(feature = "headless-js")]
fn unique_profile_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "tali-headless-{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `CHROME_PATH` is process-global, and two tests here set it. Without this lock the
    /// wedged-launch test below **passed vacuously in 0.02 s** whenever it raced
    /// `chrome_path_skips_an_explicit_nonexistent_binary`: it read that test's
    /// `/nonexistent/…`, skipped every cell as "chrome unavailable" instantly, and satisfied
    /// its own elapsed-time assertion by never launching anything. Run alone it takes 7 s.
    static CHROME_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Replace every character inside a `"…"` literal with a space, preserving length and
    /// line structure, so a source scan can find statement boundaries without tripping over
    /// punctuation that lives in a string. Handles `\"` escapes.
    fn blank_string_literals(src: &str) -> String {
        // A multi-byte char is blanked to as many spaces as it had BYTES, so an index into
        // the result indexes the original too. Blanking it to one space instead shifted every
        // later offset, and the failure report quoted a window a few bytes off the line it
        // named (`"330: .\n    let _ = browser.close().awai"`).
        let blank = |ch: char, out: &mut String| {
            for _ in 0..ch.len_utf8() {
                out.push(' ');
            }
        };
        let mut out = String::with_capacity(src.len());
        let mut in_str = false;
        let mut escaped = false;
        for ch in src.chars() {
            if !in_str {
                out.push(ch);
                if ch == '"' {
                    in_str = true;
                }
                continue;
            }
            if escaped {
                escaped = false;
                blank(ch, &mut out);
            } else if ch == '\\' {
                escaped = true;
                out.push(' ');
            } else if ch == '"' {
                in_str = false;
                out.push('"');
            } else if ch == '\n' {
                out.push('\n');
            } else {
                blank(ch, &mut out);
            }
        }
        debug_assert_eq!(out.len(), src.len(), "blanking must preserve byte offsets");
        out
    }

    #[test]
    fn chrome_path_skips_an_explicit_nonexistent_binary() {
        // The negative integration case relies on this: CHROME_PATH set but missing → skip,
        // NOT a fall back to a real PATH Chrome. Uses a path that cannot exist.
        // SAFETY: `CHROME_ENV` makes this the only test touching `CHROME_PATH` right now;
        // restore the prior value after.
        let _env = CHROME_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("CHROME_PATH");
        unsafe { std::env::set_var("CHROME_PATH", "/nonexistent/definitely-not-chrome") };
        assert!(chrome_path().is_none());
        match prev {
            Some(v) => unsafe { std::env::set_var("CHROME_PATH", v) },
            None => unsafe { std::env::remove_var("CHROME_PATH") },
        }
    }

    /// The settle budget's fallbacks, which are what the author actually meets: the knob is
    /// unset for almost everyone, and the two ways to write it wrong (`0`, or something that
    /// is not a number) must both land on the default rather than on a **zero** budget, which
    /// would make every `{js}` cell time out instantly with no way to tell why.
    #[test]
    fn settle_timeout_falls_back_to_the_default_for_absent_zero_or_unparseable() {
        // SAFETY: `TALIESIN_JS_TIMEOUT` is process-global and the wedged-launch test sets it
        // too, so this takes the same lock. Restored below.
        let _env = CHROME_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("TALIESIN_JS_TIMEOUT");
        let with = |v: Option<&str>| {
            unsafe {
                match v {
                    Some(v) => std::env::set_var("TALIESIN_JS_TIMEOUT", v),
                    None => std::env::remove_var("TALIESIN_JS_TIMEOUT"),
                }
            }
            settle_timeout()
        };

        assert_eq!(with(None), Duration::from_secs(10), "unset → the default");
        assert_eq!(with(Some("2")), Duration::from_secs(2), "a set budget wins");
        assert_eq!(
            with(Some("0")),
            Duration::from_secs(10),
            "`0` is not a zero budget — it means the default, like the other timeout knobs"
        );
        assert_eq!(
            with(Some("not-a-number")),
            Duration::from_secs(10),
            "an unparseable budget falls back rather than disabling the wait"
        );
        // The bound on each browser phase is derived from it, so it moves with the knob.
        assert_eq!(with(Some("3")), Duration::from_secs(3));
        assert!(
            phase_timeout() > Duration::from_secs(3),
            "a phase must outlast the settle budget inside it"
        );

        unsafe {
            match prev {
                Some(v) => std::env::set_var("TALIESIN_JS_TIMEOUT", v),
                None => std::env::remove_var("TALIESIN_JS_TIMEOUT"),
            }
        }
    }

    /// The in-page evaluation is the one phase whose subject already counts its own deadline
    /// down, so its wrapper is only there for a page that never answers. An outer bound at or
    /// below the budget fires first and turns "your cells settled at 9 s" into "timed out".
    #[test]
    fn the_eval_bound_outlasts_the_budget_it_wraps() {
        for secs in [1, 2, 10, 60] {
            let budget = Duration::from_secs(secs);
            assert!(
                eval_timeout(budget) > budget,
                "a {secs}s budget got an outer bound of {:?}, which fires first",
                eval_timeout(budget)
            );
        }
    }

    /// L3-1: a **wedged** Chrome must degrade to `Skipped` on the module's own budget, not
    /// hang `read --run-js`. The module's contract covered a launch/navigation/eval *failure*
    /// and this is neither: the browser starts and then never speaks.
    ///
    /// Reproduced without a real wedged browser by pointing `CHROME_PATH` at a program that
    /// launches happily and then sleeps — which is exactly what chromiumoxide's launch path
    /// blocks on (it reads the child's stderr for the DevTools websocket URL). Before the fix
    /// this returned on the *library's* silent 20 s default instead of the budget the author
    /// set, so the assertion is on the clock, not on the outcome.
    ///
    /// Also asserts the throwaway profile directory is gone: bounding a phase by dropping its
    /// future is only safe if teardown still runs, and leaking one temp profile per timed-out
    /// run is the failure this item explicitly warned against re-creating.
    ///
    /// Driven through `Runtime::block_on` rather than `#[tokio::test]` so it enters the
    /// module the way its caller does, and so the `CHROME_ENV` guard is not held across an
    /// await. It drove `observe_js_cells` until Wave 2 cut that path; the policy under test
    /// is [`with_browser`] either way, and `pdf` is now the caller that would hang.
    // The one unit test that drives the CDP loop rather than the pure logic around it, so
    // it is the one that follows the driver behind the feature. It needs no real Chrome
    // (it points `CHROME_PATH` at a shell script that sleeps), only the code that launches.
    #[cfg(feature = "headless-js")]
    #[test]
    fn a_chrome_that_launches_and_then_hangs_is_bounded_by_our_own_budget() {
        let dir = std::env::temp_dir().join(format!("tali-hangchrome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let fake = dir.join("fake-chrome");
        // Never writes the `DevTools listening on ws://…` line, never exits: the shape of a
        // browser that is up but wedged.
        std::fs::write(&fake, "#!/bin/sh\nsleep 300\n").expect("write fake chrome");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let page = dir.join("page.html");
        std::fs::write(&page, "<!doctype html><html><body></body></html>").unwrap();

        // SAFETY: this test owns both variables for its duration; restored below. It is the
        // only test that sets `TALIESIN_JS_TIMEOUT`, and `CHROME_ENV` keeps it off the other
        // `CHROME_PATH` test (see that lock's own comment for what racing it looked like).
        let _env = CHROME_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let prev_chrome = std::env::var_os("CHROME_PATH");
        let prev_budget = std::env::var_os("TALIESIN_JS_TIMEOUT");
        unsafe {
            std::env::set_var("CHROME_PATH", &fake);
            std::env::set_var("TALIESIN_JS_TIMEOUT", "2");
        }

        let started = std::time::Instant::now();
        let out: Result<(), String> = tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(with_browser(async |_browser, _phase| Ok(())));
        let elapsed = started.elapsed();

        unsafe {
            match prev_chrome {
                Some(v) => std::env::set_var("CHROME_PATH", v),
                None => std::env::remove_var("CHROME_PATH"),
            }
            match prev_budget {
                Some(v) => std::env::set_var("TALIESIN_JS_TIMEOUT", v),
                None => std::env::remove_var("TALIESIN_JS_TIMEOUT"),
            }
        }

        // The reason must be the *launch* giving up, not "chrome unavailable" — that is
        // the shape this test takes when it is not testing anything (see `CHROME_ENV`).
        //
        // And it must be the LIBRARY's failure, not our outer wrapper's "chrome launch
        // timed out": the outer bound sits deliberately above the configured
        // `launch_timeout` so the library's error, which carries the browser's stderr,
        // is what the author reads. Ordering them the other way loses that diagnostic on
        // every wedged launch, and both messages contain "launch", so only naming which
        // one can tell.
        match &out {
            Err(why) => assert!(
                why.starts_with("chrome launch failed"),
                "expected the library's launch error (it carries the browser's stderr), \
                 got {why:?}"
            ),
            Ok(()) => panic!("a wedged browser must fail the launch, not succeed"),
        }
        // The library's own default is 20 s. Anything at or above that means the budget the
        // author set is not the one in force.
        assert!(
            elapsed < std::time::Duration::from_secs(15),
            "a 2 s budget took {elapsed:?}: the bound is the library's default, not ours"
        );
        // No leaked profile directory from the timed-out launch.
        let leaked: Vec<_> = std::fs::read_dir(std::env::temp_dir())
            .expect("temp dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(&format!("tali-headless-{}_", std::process::id())))
            .collect();
        assert!(leaked.is_empty(), "leaked browser profile dirs: {leaked:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The other half of L3-1, which the wedged-launch test above cannot reach: a Chrome that
    /// *accepts* `Browser.close` and then does not exit leaves `browser.wait()` awaiting the
    /// child forever, with the observation already complete. Reproducing that needs a browser
    /// that speaks CDP and then lies, so this is a source-level guard instead — the same
    /// trade `tests/kernel_start_is_retried.rs` makes, and for the same reason: a behavioural
    /// test for it would be less reliable than the property it is checking.
    ///
    /// The property: every `.await` in the browser orchestration is inside a
    /// `tokio::time::timeout`, with the exemptions named below.
    ///
    /// **Both drivers are scanned.** `pdf.rs` (backlog 159) shares `with_browser`, so a
    /// wedged Chrome could hang `taliesin pdf` in exactly the way L3-1 stopped it hanging
    /// `read --run-js`. Scanning only this file would have left the newer command
    /// unguarded while the test's name implied otherwise.
    #[test]
    fn every_browser_await_is_bounded() {
        const SOURCES: &[(&str, &str)] = &[
            ("headless_js.rs", include_str!("headless_js.rs")),
            ("pdf.rs", include_str!("pdf.rs")),
        ];

        // Deliberate exemptions, each bounded by something other than a wrapper:
        //   * the CDP event pump — bounding it would tear down the connection mid-observation,
        //     and `handler_task.abort()` already ends it;
        //   * `observe_inner` / `observe_page` — bounded by construction, since every phase
        //     inside them is. Wrapping either *again* would drop the future mid-flight and
        //     skip the profile-directory removal, which is the leak this item warned against.
        //   * `f(&browser, phase)` — `with_browser`'s caller-supplied body, for exactly the
        //     same reason as `observe_page`: it IS the observation, and bounding it here
        //     would drop it mid-flight and skip the teardown below it. Its own phases carry
        //     their own bounds, which this scan checks at their definition site — for
        //     `observe_page` here, and for the print driver via `pdf.rs`'s inclusion below.
        //   * `with_browser(` — bounded by construction like `observe_inner`, and wrapping
        //     it would drop the teardown ladder inside it;
        //   * `ex.run(` — the KERNEL executor, not browser orchestration at all. Its bound
        //     is the per-cell silence cap (`TALIESIN_CELL_SILENCE`), a different policy with
        //     its own tests; a `tokio::time::timeout` here would cap a legitimately long
        //     compute, which is exactly what item 175(a) removed.
        const EXEMPT: &[&str] = &[
            "handler.next()",
            "observe_inner(",
            "observe_page(",
            "f(&browser, phase)",
            "with_browser(",
            "capture_pdf(",
            "ex.run(",
        ];

        let mut unbounded = Vec::new();
        let mut awaits = 0usize;
        let mut wrappers = 0usize;
        for (file, full) in SOURCES {
            // Only the module body; the tests below await on purpose.
            let src = &full[..full.find("\n#[cfg(test)]").unwrap_or(full.len())];
            // Statement boundaries are found by scanning back for `;`/`{`/`}`, so string
            // literals must be neutralised first: the injected `"window.taliOpenPageSource =
            // function () {};"` contains all three, and cutting a statement inside it made
            // this scan report a bounded await as unbounded.
            let code = blank_string_literals(src);
            awaits += code.matches(".await").count();
            wrappers += code.matches("tokio::time::timeout(").count();

            for (at, _) in code.match_indices(".await") {
                let line_start = code[..at].rfind('\n').map(|p| p + 1).unwrap_or(0);
                let line_end = code[at..].find('\n').map(|e| at + e).unwrap_or(code.len());
                // A comment mentioning `.await` is not an await.
                if code[line_start..at].contains("//") {
                    continue;
                }
                let stmt_start = code[..at]
                    .rfind([';', '{', '}'])
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let stmt = &code[stmt_start..line_end];
                if stmt.contains("tokio::time::timeout(") || EXEMPT.iter().any(|e| stmt.contains(e))
                {
                    continue;
                }
                unbounded.push(format!(
                    "{file}:{}: {}",
                    code[..at].matches('\n').count() + 1,
                    src[line_start..line_end].trim()
                ));
            }
        }
        assert!(
            unbounded.is_empty(),
            "these browser awaits are unbounded, so a wedged Chrome can hang \
             `read --run-js` or `taliesin pdf`: {unbounded:#?}"
        );
        // Controls: the scan means nothing if it sees no awaits, or if it sees no wrappers —
        // either way it would keep passing after every bound was deleted. Raised from 6 to 8
        // now that two files are scanned, so dropping one file entirely is also caught.
        assert!(
            awaits > 8,
            "the scan found almost no awaits ({awaits}) — it is passing vacuously"
        );
        assert!(
            wrappers >= 8,
            "far fewer wrappers ({wrappers}) than there are phases to bound"
        );
    }

    /// The teardown decision itself, which the bounding scan above says nothing about: it
    /// checks that each phase *has* a bound, not what is done when one fires. What must hold
    /// is that a browser which did not exit on its own is killed — the two ways to break it
    /// are counting a timed-out `close()` as an exit (`&&` → `||`) and inverting the guard so
    /// the kill fires for the browser that already left instead of the one that stayed.
    /// Either leaks a Chrome process and its profile per run.
    ///
    /// This is a **source-level** guard, like `every_browser_await_is_bounded` above and for
    /// the same reason, stated there: reaching this code needs a browser that speaks CDP and
    /// then lies, and no fake binary can get past the launch handshake. It is mutation-checked
    /// against exactly the two operators it guards.
    #[test]
    fn a_browser_that_does_not_exit_is_killed() {
        const FULL: &str = include_str!("headless_js.rs");
        let src = &FULL[..FULL.find("\n#[cfg(test)]").unwrap_or(FULL.len())];
        let teardown = src
            .split_once("let closed = ")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split_once("handler_task.abort()"))
            .map(|(block, _)| block)
            .expect("the teardown block, between the close attempt and the handler abort");

        assert!(
            teardown.contains("let exited = closed && "),
            "a `close()` that timed out must not count as an exit; got:\n{teardown}"
        );
        assert!(
            teardown.contains("if !exited {"),
            "the kill must fire for the browser that did NOT exit; got:\n{teardown}"
        );
        // The control: both assertions above are satisfied by a block that never kills
        // anything, so the scan means nothing unless the kill is in there.
        assert!(
            teardown.contains("browser.kill()"),
            "the teardown must end in a kill; got:\n{teardown}"
        );
    }
}
