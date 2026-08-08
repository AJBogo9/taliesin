# The paged.js traps, preserved verbatim

**Saved 2026-08-08, cut wave 4**, before `crates/core/src/render/print.rs` was deleted
with the rest of the PDF track. Sibling of `notes/retired/diagnostics-explanations.rs`:
the code is recoverable from the `pre-cut` tag, but these two findings were bought with
real debugging and are not re-derivable by reading the source that encodes them.

Both are about the SAME underlying bug wearing two hats. If a print/PDF track is ever
rebuilt (`notes/ROADMAP.md`'s `print-pdf-track`, cut in wave 4), read this first: a
naive implementation walks into the deadlock, mis-diagnoses it as "paged.js cannot
paginate documents with images", and then encodes `auto: false` as a load-bearing
necessity it never was.

---

## Trap 1 — the `loading="lazy"` deadlock

Verbatim, from `print.rs`'s `eager_media` doc comment (the function was a one-liner:
`html.replace(" loading=\"lazy\"", " loading=\"eager\"")`):

> Make every deferred `<img>`/`<iframe>` load eagerly.
>
> `loading="lazy"` is a **scrolling** optimization: the browser starts the fetch only when
> the element nears the viewport. A paginated rendering never scrolls, so anything far
> enough down the document is never requested at all — and that is fatal twice over.
>
> It hangs: [`PAGED_START`] waits for every image before it lets the chunker run, and a
> lazy image that was never requested has `complete === false` with neither `load` nor
> `error` ever firing, so the wait never settles and pagination never begins. Measured in
> the headless driver: zero `.pagedjs_page` boxes, polyfill loaded, fonts settled.
>
> And even if it did not hang, it would paginate wrongly: an unfetched image has no
> intrinsic size, so the chunker would flow the document around a figure of zero height.
>
> The screen renderer is right to emit it (`render/image_meta.rs` — lazy is a real win
> below the fold), which is exactly why the correction belongs here, on the print page,
> rather than in the shared emitter.

And from `PAGED_START`, the wait it describes:

> Start pagination once the things that determine layout have settled — web fonts (a font
> swap re-flows every line) and every image (see [`PAGED_CONFIG`]) — then stamp completion
> from `preview()`'s own promise.
>
> `i.onerror` is wired beside `i.onload` on purpose: a missing image must not wedge the
> run, which is the same failure mode inverted.
>
> **Neither event covers an image the browser never REQUESTS**, and that is the hole this
> wait fell through for a whole session. A `loading="lazy"` image far from the viewport is
> never fetched, so it stays `complete === false` and fires nothing at all — the wait
> deadlocks before the chunker starts. [`eager_media`] closes it at the source; do not
> weaken that and assume `onerror` is a backstop here, because it is not.

## Trap 2 — `PAGED_CONFIG`, and what `auto: false` is and is not for

Verbatim, from `print.rs`'s `PAGED_CONFIG` doc comment (the const was
`window.PagedConfig = { auto: false };`):

> Declared BEFORE the polyfill loads, because paged.js reads `window.PagedConfig` as it
> loads: declared after, it is never seen. `crates/core/tests/print_page.rs` pins the order.
>
> **`auto: false` is a deliberate choice, but NOT the load-bearing one this once claimed.**
> It was documented as necessary because paginate-on-load "never finishes chunking a
> document containing an image that actually loads", measured at a 10 s and a 60 s budget.
> That was the `loading="lazy"` deadlock (see [`eager_media`]) wearing a different hat:
> paged.js's own chunker also awaits image load, so a lazy image that is never requested
> wedges it exactly as it wedged [`PAGED_START`]. Re-measured once images load eagerly,
> `auto: true` with an `after:` stamp paginates `corpus/print/paged.tmd` to output
> byte-identical to this path — same page count, same resolved `(p. N)` on every reference.
>
> What the explicit start still buys is ordering that `auto: true` does not offer: chunking
> begins only after `document.fonts.ready`, so page breaks are never computed against
> fallback metrics and then shifted by a font swap. That is a conservative choice, not a
> measured necessity — the corpus pin's fonts are bundled, so it would not detect the
> difference either way. Keep it, but do not cite a hang for it.
>
> **No `after:` hook here, deliberately.** paged.js runs
> `if (config.auto !== false) { done = await previewer.preview(…) } if (config.after) {
> await config.after(done) }` — so with `auto: false` an `after` hook fires *immediately*,
> before any pagination. Stamping completion from it produced a page that captured
> half-rendered: content present, but every `target-counter()` unresolved, so cross-refs
> lost their page numbers and the margin boxes came out empty. The stamp must come from
> the `preview()` promise, which is the only signal that actually means "paginated".

## A third, smaller one, saved because it is the same genus

`Paper::max_float_height`'s comment records a correction to a claim that was never even
in effect, and the real reason for the cap:

> **This was once documented as a paged.js hang fix. That was wrong** — and the rule it
> feeds was never actually in `print.css`, so the claim was never even in effect
> (`git log -S __TALI_MAX_FLOAT_H__` on the stylesheet finds nothing). The hang it
> described was the `loading="lazy"` deadlock that [`eager_media`] now fixes; capping
> image height only ever *appeared* to cure it, by shrinking the document enough to pull
> the lazy image inside Chrome's load threshold.
>
> What the cap is really for is plainer. `print.css` sets `break-inside: avoid` on
> `figure`, so a figure taller than one page's content box can neither be broken nor
> fit: measured on a 600x3000 image, it bled past both page margins, lost its caption,
> and stranded two near-blank pages ahead of it.

Values were the page height minus the 44 mm of vertical margin `print.css` set, minus
roughly a quarter for the caption and surrounding text: A4 190mm, Letter 175mm, A5 125mm.
