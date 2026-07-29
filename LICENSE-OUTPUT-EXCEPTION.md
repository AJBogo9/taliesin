# Taliesin Output Exception

**Additional permission under GNU AGPL version 3 section 7.**

Version 1.0, 2026-07-29. Copyright © 2026 Andreas Bogossian.

## Why this exists

Taliesin builds a self-contained HTML document. To do that it copies parts of its
own runtime — stylesheets and scripts such as `base.css`, `tali-js.js`, `deck.js`
and the `code-enhance/` fragments — **into the page it produces**. Those copies are
what make a built page work offline with no CDN and no build step, and they are the
reason this document exists: without an explicit grant, a reader could reasonably
argue that every page built with Taliesin contains AGPL-licensed material and is
therefore itself subject to the AGPL.

That would be an absurd result. The AGPL is here to keep *Taliesin* free — so that
anyone who runs a modified Taliesin as a network service must offer its source to
that service's users. It is not here to make a claim on the blog posts, lecture
notes, papers and books that people write with it. This exception says so in terms
that can be relied on.

## The grant

The **Runtime Assets** are those portions of Taliesin's own CSS, JavaScript, fonts,
and other supporting files that Taliesin copies into, links from, or otherwise
embeds in a document it renders or builds.

You have permission to reproduce, distribute, publish, host, and otherwise convey
the output of Taliesin — including any Runtime Assets contained in that output —
under terms of your choice, with no obligation arising from the GNU Affero General
Public License in respect of that output.

In particular, and without limiting the above:

- **Your document is yours.** Building or serving a document with Taliesin does not
  make that document, its source, or its contents a work covered by the AGPL.
- **No notice is required in your output.** You do not have to reproduce this
  exception, the AGPL, a copyright line, or an offer of source in a page you build.
- **Serving a built page is not a trigger.** Publishing or hosting output that
  contains Runtime Assets does not, by itself, engage AGPL section 13.
- **This holds however the output was produced** — `build`, `render`, `preview`, or
  `publish` — and whether or not you modified the document afterwards.

## What this exception does *not* do

It grants nothing in respect of Taliesin itself. Conveying Taliesin, a modified
version of it, or its source — including the Runtime Assets **as they exist in this
repository**, rather than as emitted into a document — remains governed in full by
the AGPL, section 13 included. Running a modified Taliesin as a network service
still obliges you to offer that service's users the corresponding source.

Nor does it relicense vendored third-party material. Assets that Taliesin
redistributes but did not author — KaTeX, D3, Observable Plot, Mermaid and others —
carry their own licences, which continue to apply to them on their own terms. See
[`THIRD_PARTY.md`](THIRD_PARTY.md) and
[`crates/core/assets/js/LICENSES.md`](crates/core/assets/js/LICENSES.md). Those
licences are permissive and impose no copyleft obligation on your document.

## As applied to a contribution

Contributions are accepted under [`CONTRIBUTING.md`](CONTRIBUTING.md) clause 3,
whose grant is broad enough to cover offering the contributed material under this
exception. A contributor therefore does not need to sign anything further for this
exception to remain effective as the project grows.

---

*If you only wanted to know whether you can publish what you wrote: yes, under any
terms you like, with nothing to attribute.*
