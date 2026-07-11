# Auto SEO + discoverability artifacts (feeds, sitemap, robots, JSON-LD, llms.txt)

**Date:** 2026-07-11
**Status:** design, approved (base + LLM-friendliness addition)
**Audit IDs closed:** `#no-native-feed-rss-swallowed` / `#rss-feed-silently-dropped` /
`#broken-rss-feed-link`, `#missing-sitemap-robots` / `#no-sitemap-robots`,
`#no-json-ld-structured-data` / `#jsonld-structured-data`. Also delivers the
backlog's dropped `llms.txt` / `llms-full.txt` capability (folded into this SEO item).

## Motivation

Taliesin builds a beautiful, offline, semantic site but is invisible to the
machines that route readers to it: no syndication feed (the author configured an
RSS footer link that the engine silently dropped), no `sitemap.xml` / `robots.txt`,
no structured data, and nothing tailored for LLMs (someone asking an assistant
"who is Andreas and what does he do?" has no clean source to draw on). Every gap
here is a **framework default** — fixing it once helps every Taliesin site
("perfect the default"). All artifacts are derived from data the build already has
(`Page`: `date`, `title`, `description`, `url`, `card_image`, `categories`,
`authors`; `SiteConfig`: `url`, `author`, `description`, footer social links).

## Trigger and scope

**Every artifact is gated on `url:` being set in `_site.yml`.** Feeds, sitemaps,
JSON-LD and citable LLM files all need absolute URLs; without a canonical origin
they cannot be correct. So: set `url:`, get the whole discoverability set; leave it
unset and the build is byte-for-byte as today. No new config knob (a better default,
not an option). This matches the audit's "auto when `url:` is set."

**Non-goals (YAGNI):** BreadcrumbList JSON-LD (not selected); per-format RSS 2.0
(Atom only); full-content feeds (summary + link only); an `image/*` sitemap
extension; a discovery `<link>` for `llms.txt` (rely on the well-known path); AI
`llms.txt` head-link markup (unstandardised). None of these are precluded later.

## The artifacts

### 1. Atom 1.0 feeds — one per uncapped, dated listing

`Site::atom_feeds(&self) -> Vec<(String, String)>` returns `(path, xml)` pairs,
written by `build.rs` into the aux-file zone and added to the `keep` set.

- **Which listings.** For each page carrying a `listing:` spec, collect its items
  (reuse `Site::collection`). Emit a feed iff the listing is **dated** (≥1 item has
  a `date:`) **and uncapped** (`max_items` is `None` — the homepage's `max-items: 2`
  teaser gets no `index.xml`). Draft pages are already excluded upstream
  (`discovery.rs:19-21`), so items are draft-free.
- **Path.** The listing page's basename + `.xml` (`blog.tmd` → `blog.xml`,
  `projects.tmd` → `projects.xml`). If one page hosts >1 feed-worthy listing,
  disambiguate with the listing `id` (`blog-<id>.xml`); the single-listing case
  (all corpus pages) stays clean.
- **Entries (summary + link).** Per item, newest first: `<title>`, `<link
  rel="alternate" href>` (absolute), `<id>` (absolute page URL), `<updated>` and
  `<published>` (the item `date:`), a `<category term>` per tag, and `<summary>` =
  the front-matter `description` (plain text). No body: readers click through for
  the live figures/math.
- **Feed head.** `<title>` = listing page title (fallback site title); `<link
  rel="self">` = absolute feed URL; `<id>` = absolute feed URL; `<updated>` = the
  newest entry's date; `<author><name>` from `config.author`; `<generator>Taliesin`.
- **Dates.** `date:` is an ISO date string (`2026-05-15`). Atom needs RFC-3339;
  a date-only value is normalised to midnight UTC (`2026-05-15T00:00:00Z`). An item
  without a parseable date is omitted from the feed (but see sitemap below).
- **Escaping.** All text is XML-escaped.

### 2. sitemap.xml

`Site::sitemap(&self) -> Option<String>` (None without `url:`). One `<url>` per
built HTML page: `<loc>` = absolute clean URL (an index page as its directory,
matching `meta.rs` canonical logic), `<lastmod>` = the page `date:` when present
(omitted otherwise). Drafts already excluded upstream. Deck pages referenced only
by `{{< embed >}}` stay out of the sitemap (as they stay out of nav).

### 3. robots.txt

`Site::robots(&self) -> Option<String>`:

```
User-agent: *
Allow: /
Sitemap: <url>/sitemap.xml
```

Allow-all deliberately welcomes AI crawlers (GPTBot, ClaudeBot, PerplexityBot,
Google-Extended, …) rather than blocking them — the point is to be *found* and
*understood*.

### 4. JSON-LD structured data (in the page head)

A new `meta::jsonld_head(site, page) -> String` emitted next to `social_head`,
wrapped in `<script type="application/ld+json">`:

- **A post** (`page.date.is_some()`, the same signal as `og:type=article`) →
  `BlogPosting`: `headline` (title), `datePublished` (date), `dateModified` (date),
  `author` (Person from `config.author`), `image` (absolute `card_image`, when set),
  `description`, `mainEntityOfPage` (absolute page URL), `url`.
- **The root `index` page** (depth 0, `index.html`) → a two-node `@graph`:
  - `WebSite`: `name` (site title), `url` (site url), `description`.
  - `Person`: `name` (`config.author`), `url` (site url), `sameAs` = the footer
    social links (LinkedIn / GitHub / …), so search + assistants tie the site to the
    real person. If no footer links, `sameAs` is omitted.

Emitted only when `url:` is set (absolute URLs required). JSON string values are
JSON-escaped (not HTML-escaped) inside the script.

### 5. llms.txt (the curated LLM map)

`Site::llms_txt(&self) -> Option<String>` — a Markdown file at the site root per the
[llmstxt.org](https://llmstxt.org) convention, the first thing an assistant reads to
answer "who is this person and what do they do?":

```
# <site title>            (e.g. "Andreas Bogossian")

> <site description>      (e.g. "Machine learning and statistics, worked out from first principles")

<About paragraph: the home page's hero lead / first prose block, if any>

## Posts
- [<title>](<absolute url>): <description>
- …

## Projects
- [<title>](<absolute url>): <description>
- …

## Pages
- [CV](<absolute url>): <description or nav label>
- [Publications](<absolute url>): …
```

Sections are built from the discovered listings (each dated listing → a section
named after its page title) plus remaining top-level nav pages under "Pages". Links
are absolute; descriptions come from each page's front-matter `description` (or the
nav label). Draft pages are excluded.

### 6. llms-full.txt (the full clean content)

`Site::llms_full_txt(&self) -> Option<String>` — every non-draft page concatenated as
plain text so an assistant can ingest the actual substance, not just the map:

```
# <site title>

<About paragraph>

---

## <page title>
<absolute page url>

<clean prose of the page>

---

## <next page title>
…
```

**Prose extraction.** For each page's block model: skip code-cell blocks
(`block.cell.is_some()`) and rendered-math regions; for the remaining prose blocks
(headings, paragraphs, lists, blockquotes, tables, figure captions) strip HTML tags
**and decode HTML entities** (`&amp;`→`&`, `&nbsp;`→space, `&#8217;`→`'`) to readable
text — a small shared `text_content(html)` helper (the a11y `strip_tags` strips tags
but does not decode entities, so it is extended/wrapped, not reused verbatim). This
mirrors the word-count's "prose only, code and math excluded" rule
and is *more* accurate than the Python scraper the native rewrite dropped, because
the block model already separates prose from code. Math is omitted in v1 (the
identity/explanatory prose is the goal); revisit LaTeX-source inclusion later if
wanted.

### 7. Footer feed link (un-drop)

`chrome.rs::footer_html` currently drops any local `.xml` link ("this build
generates no RSS feed"). Change: **honor a local `.xml` link when `config.url` is
set** (a feed is now generated); keep dropping it only when `url:` is unset. Add an
`rss` glyph to the bundled social-icon set (`social_icon`) if absent. Then re-add
`{ icon: rss, href: blog.xml }` to `corpus/tech-blog/_site.yml` footer — the audit's
original intent, now honest.

## Module layout

- **`crates/core/src/site/feed.rs`** (new) — Atom builder + `Site::atom_feeds`.
- **`crates/core/src/site/seo.rs`** (new) — `Site::sitemap`, `Site::robots`.
- **`crates/core/src/site/llms.rs`** (new) — `Site::llms_txt`, `Site::llms_full_txt`,
  and the shared prose-extraction helper.
- **`crates/core/src/site/meta.rs`** — add `jsonld_head`, wired into the head
  assembly beside `social_head`.
- **`crates/core/src/site/chrome.rs`** — footer `.xml` un-drop + `rss` icon.
- **`crates/server/src/build.rs`** — in the aux-file zone (~1104-1156), after
  search/hover-index, when `site.config.url.is_some()`: write each feed, `sitemap.xml`,
  `robots.txt`, `llms.txt`, `llms-full.txt`; add each to the `keep` set so stale
  copies are pruned on rebuild.
- **`corpus/tech-blog/_site.yml`** — re-add the `rss` footer item.

Each `Site` method is a pure function of the already-discovered model (no I/O);
`build.rs` owns all file writes. JSON-LD rides the existing per-page head path, so
it needs no `build.rs` change and also appears in live preview.

## Data flow

```
build_site_async
  └─ render every page  (JSON-LD already injected via meta::jsonld_head in the head)
  └─ write search-index.js / hover-index.js / 404.html         (existing)
  └─ if site.config.url.is_some():                              (NEW)
       for (path, xml) in site.atom_feeds():  write(out/path);  keep += path
       if let Some(x) = site.sitemap():       write(out/"sitemap.xml"); keep += it
       if let Some(x) = site.robots():        write(out/"robots.txt");  keep += it
       if let Some(x) = site.llms_txt():      write(out/"llms.txt");    keep += it
       if let Some(x) = site.llms_full_txt(): write(out/"llms-full.txt"); keep += it
```

## Edge cases

- **No `url:`** → no artifacts; footer keeps dropping `.xml`; build unchanged.
- **No dated listing** → no feeds (sitemap/robots/llms still emit).
- **Capped listing** (homepage teaser) → no feed.
- **Undated item** → omitted from a feed; still listed in sitemap/llms (no `lastmod`).
- **`url:` trailing slash** → trimmed once (as `meta.rs` already does).
- **XML/JSON escaping** → all interpolated text escaped for its format.
- **A book** (`chapters:`) has no listing, so no feed; it still gets
  sitemap/robots/llms and per-chapter JSON-LD where a chapter has a `date:`.

## Testing

**Unit (in the new modules):**
- Atom: date-only → RFC-3339; XML escaping; empty/undated listing → no feed; capped
  listing skipped; entry carries absolute link + description summary.
- sitemap/robots: `None` without `url:`; `Sitemap:` line references the configured url.
- llms: `llms.txt` lists discovered non-draft pages with absolute links; a draft is
  absent from both files; prose extraction drops a code cell's source and keeps
  surrounding prose.
- JSON-LD: a dated page yields `"@type":"BlogPosting"`; the index yields `WebSite` +
  `Person`; none emitted without `url:`.

**Corpus pin (`crates/core/tests/tech_blog.rs`; tech-blog has `url:` set):**
- `Site` exposes the artifacts; assert `blog.xml` is Atom
  (`<feed xmlns="http://www.w3.org/2005/Atom">`) with an `<entry>` per post carrying
  an absolute `andreasbogossian.com` link and the post description as `<summary>`;
  `projects.xml` emitted; the homepage teaser yields no `index.xml`.
- `sitemap.xml` lists the post + top-level pages; `robots.txt` names the sitemap.
- `llms.txt` opens with the site title + description and lists the posts with absolute
  links; `llms-full.txt` contains a post's prose and **excludes** any `draft:` page.
- A post page's built HTML contains `"@type":"BlogPosting"`; `index.html` contains
  `"@type":"WebSite"` and `"@type":"Person"`.
- The footer now renders the `rss` link (no longer dropped) pointing at `blog.xml`.

Because the corpus is the regression net, every assertion matches an emitted
**string** (feed XML / file content / class attribute), never an inlined-CSS-or-JS
substring (the "gate the gate" lesson).

## Invariant safety

- **Offline / zero-CDN:** all artifacts are static files served from the same origin;
  no external request added.
- **Single editing surface:** all read-only, build-time derivations of the source; no
  write-back path.
- **HTML-only output invariant:** feeds/sitemap/robots/llms are *sidecars* (the same
  category as the already-emitted `sitemap`-less `search-index.js`), **not** a new
  document output format — the page output stays HTML. JSON-LD is metadata inside the
  HTML head.
- **Minimal config:** one trigger (`url:`, already used for canonical/OG), no new knob.
- **Theme system:** untouched (no CSS).

## Build order (for the plan)

1. `feed.rs` (Atom) + `Site::atom_feeds` + unit tests.
2. `seo.rs` (sitemap + robots) + unit tests.
3. `llms.rs` (+ shared prose extractor) + unit tests.
4. `meta::jsonld_head` wired into the head + unit tests.
5. `build.rs` aux-zone writes + `keep` set.
6. `chrome.rs` footer un-drop + `rss` icon + `_site.yml` re-add.
7. `tech_blog.rs` corpus pins; browser + `taliesin build` spot-check of the emitted files.
