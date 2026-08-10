# W8 — NOT TAKEN. See `notes/2026-08-10-mvp-publish-session.md` for the ship-path outcome

> ## ⚠ NOT TAKEN, AND STALE. READ `notes/2026-08-10-mvp-publish-session.md` FIRST.
>
> This wave was **not executed**. It was written on 2026-08-10 against `1a82f2ef`, and
> **every line number, and some premises, are stale**: nine commits have landed since
> (through `315d67db`), five of them prose sweeps over the same files this wave names.
> **Grep, do not trust.** If a step's premise is false, drop the step and say so in the
> commit message — that is a success, not a failure.
>
> The plan file these waves referred to (`notes/2026-08-10-mvp-publish-plan.md`) is gone
> with the waves that were taken; the session record above replaces it, and carries the
> ship-path outcome, the surfaced-not-fixed list, and what remains before a tag.

---

> ## ⚠ CORRECTIONS — READ FIRST, THEY OVERRIDE THE SECTION BELOW
>
> A skeptic pass over the assembled plan found 21 defects **in the plan itself**. The ones
> affecting this wave are below. Where a correction contradicts the section body, the
> correction wins. Line numbers throughout were true at `1a82f2ef` on 2026-08-10, and every
> wave that lands before yours shifts them — **grep, do not trust**.
>
> - **THIS WAVE IS DECLINED.** The stop-line synthesis recommends against taking it before publishing, and the section's own Decision block agrees. It is kept in full so the analysis is not lost and so the author can override with one instruction.
> - **If overridden:** run **after W7** (W7 deletes `crates/core/assets/schema/tali-site.schema.json`, which your step B9 regenerates — after W7, B9 becomes "regenerate the companion copy directly", a simpler step) and **after W1**, since you remove 5 more corpus documents and move the census again to 74 / 6,920 / 487. **Add the census-republish step W7 also needs.** A-before-B.
> - **You have no step updating `CLAUDE.md:22`'s corpus count** and you change it. Add one.
> - **Rollback for steps B4-B8** (a public signature change through five crates whose failure mode is a silently different page): keep a `git stash` of the pre-B4 tree; the tripwire is `crates/server/tests/build_reproducibility.rs`.
> - **The hazard that must not ship as silence:** after the cut a mapping-shaped `author:` falls through `scalar()` and silently drops the byline. The `RETIRED_KEYS` entry under the `author key` scope is not optional.
> - **One item is already reassigned:** `corpus/structured-authors/note.tmd:6-8`'s nonexistent site-level byline fallback is now **W4's**, because it must be fixed whether or not this wave is ever taken.

---

### R7 — The last two optional feature cuts: structured `author:` and the shared bibliography

**Branch:** `cut/r7-author-and-shared-bib` (split: `cut/r7a-structured-author`, `cut/r7b-shared-bibliography`) · **Kind:** deletion · **Size:** ~1,400 lines (A ~650, B ~750) · **Blocked by:** none

---

## THE DECISION, FIRST

**Recommendation: do not take R7 before publishing. Take it after real users exist, if they ever ask.**

This is a recommendation, not a hedge, and it runs *against* the standing "always lean towards cutting"
directive. Here is why the directive does not reach this case.

1. **Neither subject is a defect.** The campaign's cuts removed things that were wrong, unfinished, or
   actively misleading. These two work, are tested, cost nothing to keep at runtime, and no user-visible
   surface is degraded by their presence. "Unused in our own corpus" is the *only* charge, and it is the
   charge this project already ruled circular once.
2. **Subject B is the highest-blast-radius refactor left in the tree, for zero user-visible gain.**
   `SiteDefaults` (`crates/core/src/render/model.rs:389`) has exactly one field. Cutting the shared
   bibliography does not shrink it, it deletes it — and with it `render_document_scoped_with_site`
   (`render/mod.rs:198`), the single entry point every page of every verb renders through, called from
   `site/search.rs:168`, `serve_site/mod.rs:597` and `:1165`, `build.rs:1155`, and `lint.rs:408`. A mistake
   there does not fail loudly; it renders a subtly different page. Doing that in the last week before a
   release is the wrong trade.
3. **Subject A's hazard is a first-impression hazard.** A mapping-shaped `author:` is the single most likely
   thing an academic user types, because every peer tool accepts it. After the cut it must produce a warning
   *and* still render a byline (step A4), or the tool silently drops the author's name off the page. Building
   that graceful-degradation path costs about as much thought as the ~650 lines it replaces, and the result
   is strictly worse for the first user who tries a paper than the working feature is.
4. **The asymmetry runs the other way here.** Keeping costs nothing and is fully reversible. Cutting is
   reversible too, but the corpus pins, the CSS, the docs section and the tests all have to be rebuilt from
   scratch, and the RETIRED_KEYS entry has to be un-retired — a register the project has never reversed.

**Two things from this analysis should be acted on regardless of the decision**, and both are cheap:

- **`docs/guide/reference/frontmatter.tmd:41` is false today.** It tells readers the structured form "adds
  affiliations, ORCIDs and links". `orcid:` was retired 2026-08-08 (`frontmatter.rs:256-260`) and the
  validator now rejects it. `stale_docs.rs`'s `shipped_docs_do_not_use_a_retired_front_matter_key` structurally
  cannot catch it (it matches column-0 YAML in `front-matter key`/`config key` scope; this is `author key`
  scope, in prose). **Fix the word "ORCIDs" out of that row in the truth-sweep wave.**
- **`corpus/structured-authors/note.tmd:6-8` documents a behaviour that does not exist** (see Disproven, below).
  If R7 is not taken, that file still needs its prose corrected.

---

## Verified state (checked 2026-08-10, against `1a82f2ef`)

**Four claims in the brief are wrong. Read these before touching anything.**

- **DISPROVEN — the module path.** There is no `crates/core/src/site/author.rs`. The module is
  **`crates/core/src/author.rs`** (284 lines), declared at `crates/core/src/lib.rs:29`.
  `crates/core/src/cite/author.rs` is an unrelated BibTeX name formatter (`cite/format.rs:3` imports
  `format_authors` from it) and **must not be touched**.
- **DISPROVEN, and this one deletes the wrong file.** The brief names
  `crates/core/src/diagnostics/bibliography.rs (323 lines)` as Subject B. That file is **150 lines** and holds
  `citations_without_bibliography` — a **per-page validator that must survive**. The 323-line shared-bibliography
  module is **`crates/core/src/site/bibliography.rs`**.
- **DISPROVEN — the hazard remedy does not work as specified.** `RETIRED_KEYS` is read only through
  `frontmatter::unknown_key_message` (`frontmatter.rs:821-834`), and its only `author key` caller is the
  sub-key `match` inside `push_one` (`author.rs:117`) — the loop the cut removes. Registering the retirement
  and deleting the `Mapping` arm produces **silence**, which is precisely the failure `author.rs:10-13` warns
  about. The remedy must be a surviving `Mapping` arm (step A4).
- **DISPROVEN — the CSS range.** The block is `base.css:233-280`, not `251-280`, and **`base.css:236-242`
  must survive**: `.tali-byline`, `.tali-author`, `.tali-author a` and its `:hover`/`:focus-visible` are the
  *scalar* byline's styling, asserted by `render/tests.rs:149`. Only **244-280** goes.
- **NEW DISPROOF, and it strengthens the cut case for A.** `corpus/structured-authors/note.tmd:6-8` claims its
  byline "falls back to the site-level `author:` in `_site.yml`". **No such fallback exists.**
  `rg -n '\.authors' --type rust` returns exactly two hits, both `crates/core/src/site/feed.rs` (:162, :172).
  The byline is built from the page's own front matter only (`render/mod.rs:656`). `structured-authors/_site.yml`'s
  site-level `author:` is inert — that project has no dated listing, so no feed reads it either. One of this
  feature's three pins pins nothing.

Confirmed as stated in the brief:

- `crates/core/src/author.rs` is 284 lines; `AUTHOR_KEYS` at **`author.rs:41`**; `byline_html` at
  **`render/mod.rs:1464`**, `appendix_html` at **:1527**, `affiliations_html` at **:1567** (the block runs to
  :1594; `DARK_CSS` resumes at :1596). Call sites: `render/mod.rs:1287` (appendix), `:1432` (byline), `:1452`
  (affiliations).
- The JSON-LD consumer is gone: `author.rs:4` still names `site/meta.rs`, and `rg -n author crates/core/src/site/meta.rs`
  returns **zero lines**.
- `feed.rs:172-175` reads `.map(|a| a.name.as_str())` — the scalar half only. So `affiliation` / `url` / `equal` /
  `contribution` have exactly one live consumer, the rendered byline.
- `samples/paper.tmd:4` is `author: "A. N. Author"` — a plain scalar, in the tool's own paper archetype.
- `corpus/structured-authors/` is referenced from outside itself by exactly **one** line:
  `corpus/README.md:42`. Nothing in `docs/`, `site/` or any test.
- `corpus/shared-bib/_site.yml:5` is the **only** `_site.yml` of eleven declaring `bibliography:`
  (`find . -name _site.yml -not -path '*/_site/*'` → 11). **19** `.tmd` files declare the per-page key,
  including all seven `corpus/tech-blog/posts/*/index.tmd`, `site/formats.tmd:24`, `samples/paper.tmd:5`,
  `docs/guide/using/writing.tmd:4`.
- The generic-fixture trap is **real**: `crates/core/tests/standalone_document_chrome.rs:26` and **:44** and
  `crates/server/tests/project_required.rs:107` use `corpus/shared-bib/` for reasons unrelated to bibliographies.
  `crates/core/src/diagnostics/bibliography.rs:18` names `corpus/shared-bib/index.tmd` as the regression that
  caught the citations-without-bibliography rule.
- The cite cascade is as described: `local: HashSet` at `cite/mod.rs:58`, `overlay` at `:91`, `uncited_local`
  at `:99`, site-wide `uncited` at `:114`. `overlay` call sites: `render/mod.rs:1673`, `cite/tests.rs:860`,
  `cite/tests.rs:878`. `uncited_local` has one caller (`cite/render.rs:124`); `uncited` has one
  (`site/bibliography.rs:156`).
- `_site.yml`'s `bibliography` is a **derived** key: `site/config/mod.rs:108` `NATIVE_KEYS` (entry at **:139**),
  struct field at **:70**, parse at **:321**, tests at **:655-685**. The JSON schema is generated from
  `NATIVE_KEYS` (`crates/core/src/schema.rs:78`) and golden-locked (`schema.rs:117`), with a bundled copy at
  `editor/vscode/schema/tali-site.schema.json:6` gated only by the companion's `node --test`
  (`editor/vscode/src/test/manifest.test.ts:396`) — i.e. **only `gates.sh` catches that copy**.
- No global dead-CSS gate exists. `tali-affiliation*` / `tali-contributions` / `tali-author-mark` /
  `tali-equal-note` appear in exactly three files: `render/mod.rs`, `base.css`, `render/tests.rs`.
- Corpus floors have headroom: `corpus.rs:266` and `:345` are `>= 5` file counts, `:496` `>= 5` includes,
  `:893` `site.pages.len() >= 10` scoped to `corpus/tech-blog`. Corpus is 82 `.tmd` files today; R7 removes 5.
  **No floor is threatened.**

---

## Files

**Subject A — structured `author:` (~650)**
- Modify: `crates/core/src/author.rs` (284 → ~75), `crates/core/src/render/mod.rs` (:1287, :1432, :1452,
  :1460-1594), `crates/core/src/render/tests.rs` (:153-155, :158-319), `crates/core/src/frontmatter.rs`
  (:256-266 out, one new entry in), `crates/core/assets/css/base.css` (:244-280),
  `docs/guide/reference/frontmatter.tmd` (:41, :94-162), `corpus/README.md` (:42)
- Delete: `corpus/structured-authors/` (5 files, 66 lines)

**Subject B — shared bibliography (~750)**
- Delete: `crates/core/src/site/bibliography.rs` (323), `crates/core/tests/shared_bibliography.rs` (130),
  `corpus/shared-bib/` (5 files, 65 lines)
- Modify: `crates/core/src/render/model.rs` (:386-395), `crates/core/src/render/mod.rs` (:190-232, :460-470,
  :1598-1676), `crates/core/src/site/mod.rs` (:171, :355, :405, :569, :1065),
  `crates/core/src/site/search.rs` (:20, :25, :61, :68, :164, :168), `crates/core/src/site/config/mod.rs`
  (:64-70, :139, :321, :655-685), `crates/core/src/cite/mod.rs` (:46-58, :91-125),
  `crates/core/src/cite/render.rs` (:124), `crates/core/src/cite/tests.rs` (:855-895),
  `crates/server/src/build.rs` (:1155-1160, :1899), `crates/server/src/lint.rs` (:394, :408, :433),
  `crates/server/src/serve_site/mod.rs` (:597-601, :1160-1165),
  `crates/core/assets/schema/tali-site.schema.json`, `editor/vscode/schema/tali-site.schema.json`,
  `docs/guide/reference/configuration.tmd` (:109, :113-126), `docs/guide/using/writing.tmd` (:215-217),
  `corpus/README.md` (:41)
- **Re-point, do NOT delete:** `crates/core/tests/standalone_document_chrome.rs:26,29,44`,
  `crates/server/tests/project_required.rs:107`, `crates/core/src/diagnostics/bibliography.rs:18` (doc comment)

---

## Steps — Subject A (branch `cut/r7a-structured-author`)

- [ ] **A1.** Delete `corpus/structured-authors/` entirely (5 files). Delete its row at `corpus/README.md:42`.
      **This is the ordering rule: it happens in this commit and no earlier one.** Confirm nothing else names it:
      `rg -n structured-authors --glob '!_site/*' --glob '!notes/*' .` must return only `corpus/README.md` before
      the edit and nothing after.
- [ ] **A2.** In `crates/core/src/author.rs`, delete `AUTHOR_KEYS` (:41), the `affiliations` / `url` / `equal` /
      `contribution` fields of `Author` (:47-59), `string_list` (:153-158), `affiliation_index` (:164-176) and
      `marks` (:179-186). Keep `Author { name }`, `named`, `From<&str>`, `parse`, `push_one`, `scalar`.
      Rewrite the module doc (:1-36) to one short paragraph: `author:` is a scalar or a list of scalars, the
      byline prints it verbatim, and the compatibility warning at :10-13 stays because it is still true.
- [ ] **A3.** In `crates/core/src/author.rs`'s `#[cfg(test)] mod tests` (from ~:184), delete every test that
      builds a mapping-shaped author; keep `every_older_spelling_still_parses` (~:189) and add the mapping case
      from A4 to it.
- [ ] **A4. THE HAZARD STEP — do not skip it, and do not implement it as the brief describes.** Keep the
      `serde_yaml::Value::Mapping(map)` arm of `push_one` (`author.rs:101`), reduced to:

      ```rust
      serde_yaml::Value::Mapping(map) => {
          warnings.push(
              "the structured `author:` form was removed on 2026-08-10: write the name(s) \
               as a string or a list of strings; affiliations, links and contributions \
               belong in the document body"
                  .to_string(),
          );
          if let Some(name) = map.get("name").and_then(scalar) {
              out.push(Author::named(name));
          }
      }
      ```

      Rationale, recorded here because it contradicts the brief: `RETIRED_KEYS` is only ever consulted by
      `unknown_key_message` (`frontmatter.rs:821-834`), whose sole `author key` caller is the sub-key loop being
      deleted. Registering the retirement and dropping the arm makes a mapping fall through to the `other =>`
      branch, where `scalar()` on a `Mapping` returns `None` and **nothing is pushed** — the byline vanishes with
      no diagnostic, on a page whose source never mentions the author's name. The arm above warns *and* keeps the
      byline. The warning is already located at line 1 by `render/mod.rs:660`.
- [ ] **A5.** Delete the now-unreachable `author key` entries from `RETIRED_KEYS`
      (`crates/core/src/frontmatter.rs:255-266`: `orcid` and `email`). After A2 nothing calls
      `unknown_key_message("author key", …)`, so they are dead register weight. Verify with
      `rg -n '"author key"' --type rust` → must return nothing.
- [ ] **A6.** In `crates/core/src/render/mod.rs`: delete `appendix_html` (:1527-1565), `affiliations_html`
      (:1567-1594) and their call sites at `:1287` and `:1452`; reduce `byline_html` (:1464-1525) to joining the
      names into `<span class="tali-author">…</span>` items separated by `", "`, wrapped in
      `<span class="tali-byline">`, returning `None` when nobody is named. Keep the `url` link? **No** — `url`
      is gone with the struct field; the byline is plain text.
- [ ] **A7.** In `crates/core/src/render/tests.rs`: delete the six tests at `:158-319`
      (`a_structured_author_list_renders_a_byline_with_numbered_affiliations`,
      `the_appendix_renders_each_authors_contribution`, `no_contributions_emits_no_appendix`,
      `a_page_declaring_none_of_the_appendix_keys_emits_no_appendix`, `the_appendix_is_deterministic_across_renders`,
      `a_structured_author_still_reaches_the_byline_at_all`). Delete the `tali-affiliations` assertion at
      `:153-155`. **Add one test in its place**: a mapping-shaped `author:` warns with the retirement note *and*
      still renders `<span class="tali-author">` containing the `name:`. That is the whole point of A4 and it must
      be pinned.
- [ ] **A8.** In `crates/core/assets/css/base.css`, delete **:244-280** only (the comment at :244, `.tali-author-mark`,
      `.tali-affiliations`, `.tali-affiliation-list`, `.tali-affiliation-list li`, `.tali-affiliation-num`,
      `.tali-equal-note`, the appendix comment, `.tali-contributions` + `dt`/`dd` and the media-query override at
      :278-280). **Leave :233-242 alone** — `.tali-byline` / `.tali-author` are the scalar byline. Remember the
      `include_str!` rule: `cargo build` before inspecting a rebuilt site.
- [ ] **A9.** In `docs/guide/reference/frontmatter.tmd`: rewrite the `author` row at **:41** to
      `| \`author\` | string or list | Author name(s), printed as the byline |` (this also removes the false
      "ORCIDs" claim — see the Decision section). Cut `:103-133` (the structured form, its table, the
      "You never write an affiliation number" argument) and the whole `## The appendix {#the-appendix}` section
      at `:135-162`. Keep `:94-102` (the scalar/list examples) and the `{#sec-authors}` anchor. **Then grep for
      dangling anchors:** `rg -n 'sec-authors|the-appendix' docs/ site/ corpus/` — after the edit only the
      heading at :94 may remain, and the `[appendix](#the-appendix)` link at :123 must be gone with its row.
      `AUTHOR_KEYS`' guide rows are **ungated** (`the_reference_page_documents_every_known_key` covers top-level
      `KNOWN_KEYS` only), so nothing will fail if you miss one.

## Steps — Subject B (branch `cut/r7b-shared-bibliography`, only after A is green on `main`)

- [ ] **B1. Re-point the generic fixtures FIRST, on a tree where `corpus/shared-bib/` still exists**, so the
      re-point is proved before the deletion. Use **`corpus/analyst/`**: it has an `_site.yml` with `nav.left`
      (two items), two pages, and no `bibliography:` anywhere.
      - `crates/core/tests/standalone_document_chrome.rs:26` → `corpus("analyst/index.tmd")`; message at `:29` →
        "corpus/analyst HAS an _site.yml, so its pages keep project chrome".
      - `crates/core/tests/standalone_document_chrome.rs:44` → `corpus("analyst")`.
      - `crates/server/tests/project_required.rs:107` → `corpus("analyst")` (the `--no-exec` flag stays; analyst
        has 8 code cells across its two pages).
      Run `cargo test -p taliesin-core --test standalone_document_chrome` and
      `cargo test -p taliesin-server --test project_required` and confirm green **before** B2.
- [ ] **B2.** Delete `corpus/shared-bib/` (5 files), its row at `corpus/README.md:41`, and
      `crates/core/tests/shared_bibliography.rs`. Update the doc comment at
      `crates/core/src/diagnostics/bibliography.rs:18` — that regression is now pinned by the surviving
      per-page path, so name what actually pins it (a `bibliography:` file that exists but is empty resolves
      nothing and hits the same branch) or drop the parenthetical. **Do not delete that file.**
- [ ] **B3.** Delete `crates/core/src/site/bibliography.rs` (323 lines) and its `mod`/`pub(crate) use` at
      `crates/core/src/site/mod.rs:171`. Delete the `bibliography` resolution at `site/mod.rs:355`.
- [ ] **B4.** Delete `SiteDefaults` from `crates/core/src/render/model.rs:386-395` and its re-export from
      `render/mod.rs:18` and `crates/core/src/lib.rs`.
- [ ] **B5.** Delete the public entry point `render_document_scoped_with_site` (`render/mod.rs:189-201`) and the
      `site: Option<&SiteDefaults>` parameter from `render_doc_with_includes_impl` (:230), and the two other
      internal signatures at :379 and :440 and :462. In `render_single_doc` (:213-224) drop the `SiteDefaults`
      construction and the `crate::site::shared_for_single_doc` call, passing `None` where the parameter was.
      **Note the self-defeating keep-argument is now spent for real:** `shared_for_single_doc` existed only to
      stop `preview post.tmd` and `preview <dir>` diverging over this key; with the key gone there is nothing to
      diverge over.
- [ ] **B6.** In `crates/core/src/render/mod.rs`, drop the `shared: &[PathBuf]` parameter from
      `load_bibliography` (:1608-1614), the `shared_text` accumulation (:1616-1622), and the two-layer merge at
      `:1672-1675` — it collapses to `let (bib, bib_warnings) = crate::cite::parse_bib_warned(&text);`.
      Rewrite the "Layer order is the feature" doc paragraph (:1603-1607).
- [ ] **B7.** In `crates/core/src/cite/mod.rs`: delete the `local: HashSet<String>` field (:58), `overlay`
      (:84-95) and the site-wide `uncited` (:110-124). **Rename `uncited_local` → `uncited`** (:97-108) and drop
      its "page-local" doc wording; update its one caller `cite/render.rs:124`. Rewrite the two-layer paragraph
      in the `Bibliography` doc comment (:46-52). Delete the two `overlay` tests at `cite/tests.rs:855-895`.
- [ ] **B8.** In `crates/server/`: `build.rs:1155-1160` and `serve_site/mod.rs:597-601` and `:1160-1165` and
      `lint.rs:394,408` switch to the surviving scoped render entry point; delete the
      `validate_shared_bibliography` loops at `build.rs:1899` and `lint.rs:433`. In `site/search.rs` drop the
      `site_defaults` parameter from all three functions (:20, :61, :164) and their internal passes (:25, :68, :168).
      In `site/mod.rs` drop the `render_defaults()` arguments at :405, :569, :1065.
- [ ] **B9. The derived-key steps, in this order.** Delete `"bibliography"` from `NATIVE_KEYS`
      (`site/config/mod.rs:139`), the `pub bibliography: Vec<String>` field (:64-70), the parse at :321, and the
      test `parses_a_site_level_bibliography_in_both_shapes` (:655-685). Then regenerate the schema:
      `TALIESIN_BLESS=1 cargo test -p taliesin-core --lib schema`. Then **copy the regenerated
      `crates/core/assets/schema/tali-site.schema.json` over `editor/vscode/schema/tali-site.schema.json`** —
      `cargo test --workspace` is green with that copy stale; only `./tools/gates.sh` (via the companion's
      `node --test`, `manifest.test.ts:396`) catches it.
- [ ] **B10.** Add ONE `RETIRED_KEYS` entry, scope `"config key"`, key `"bibliography"`, one sentence naming the
      date and the successor: the per-page front-matter `bibliography:`. This one **does** work through the
      register, unlike A4's: `site/config/mod.rs:384` routes unknown `_site.yml` keys through
      `unknown_key_message` under the `config key` scope. **Do not write a tombstone test** — none is derived for
      this scope, and the register entry is the whole obligation.
- [ ] **B11.** Docs: delete the `bibliography` row at `docs/guide/reference/configuration.tmd:109` and the whole
      **A shared bibliography** section at `:113-126` (both paragraphs, including the never-cited-check prose and
      the `--strict` sentence). At `docs/guide/using/writing.tmd:215-217` delete the "In a site or book,
      `bibliography:` can instead live once in `_site.yml`" paragraph. Leave `configuration.tmd:29` and `:68`
      alone if they refer to the **front-matter** key — check each: `:68` is inside a `_site.yml` example and
      **must go**; `:29` is the front-matter table and **stays**.

---

## Traps

- **The two wrong file paths in the source brief** (`site/author.rs`, `diagnostics/bibliography.rs`). The second
  will make you delete a live per-page validator. Re-read the Verified state section before your first edit.
- **The RETIRED_KEYS remedy for Subject A does not work.** It is the one thing the brief insists on and the one
  thing that produces exactly the silence it is meant to prevent. Step A4 is the real fix.
- **`.tali-byline` and `.tali-author` survive.** `base.css:236-242`. Deleting them takes the byline off every
  document in the tree, which is the exact silent-furniture failure `author.rs:10-13` warns about, and
  `render/tests.rs:149` is the only thing that will catch it.
- **The ordering rule, twice.** `corpus/structured-authors/` dies in A's commit and no earlier one;
  `corpus/shared-bib/` dies in B's. `crates/core/tests/corpus.rs` sweeps whatever exists, so an early deletion
  silently removes coverage with every gate green. `notes/2026-08-08-cut-playbook.md`'s STEP 8 names both
  directories under exactly this prohibition.
- **`corpus/shared-bib/` is a generic project fixture in three places** (`standalone_document_chrome.rs:26,44`,
  `project_required.rs:107`), none about bibliographies. Re-point them to `corpus/analyst/` and prove the
  re-point green **before** deleting, or you will not know whether a red gate is the re-point or the cut.
- **`SiteDefaults` has one field, so the cut deletes a public API.** `render_document_scoped_with_site` is called
  from `taliesin-core` and `taliesin-server` both. The compiler catches every call site — this trap is that the
  *diff* is far larger than "delete a lint", and reviewing it as a small change is how a rendering regression
  gets through.
- **`corpus/structured-authors/note.tmd` pins nothing.** Its stated behaviour (site-level `author:` byline
  fallback) does not exist. Do not try to preserve it "somewhere else"; there is nothing to preserve.
- **The VS Code schema copy is invisible to `cargo test --workspace`.** Step B9's copy is mandatory and only
  `./tools/gates.sh` will tell you if you forgot.
- **Tests that go vacuous:** `render/tests.rs:140-157` keeps working after A but loses its negative assertion —
  replace it with the A4 pin rather than leaving the test thinner.
- **Floors checked and clear:** `corpus.rs:266` (`>= 5`), `:345` (`>= 5`), `:496` (`>= 5`), `:893`
  (`site.pages.len() >= 10`, scoped to `corpus/tech-blog`). Corpus goes 82 → 77 `.tmd`; no floor is near.
- **No global dead-CSS gate exists**, so leftover selectors in `base.css` will not fail anything. Grep by hand:
  `rg -n 'tali-affiliation|tali-contributions|tali-author-mark|tali-equal-note' crates/core/assets/css/`.
- **`crates/core/src/cite/author.rs` is not this feature.** It is the BibTeX name formatter
  (`cite/format.rs:3`). Do not touch it.

---

## Verification

Run per branch, on the tree you are about to commit:

```sh
TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh
```

Ten gates must run and pass — the count is printed and is itself the assertion. Then, per subject:

**Subject A**

```sh
rg -n 'structured-authors' --glob '!_site/*' --glob '!notes/*' .          # → nothing
rg -n '"author key"' --type rust                                         # → nothing
rg -n 'AUTHOR_KEYS|affiliation_index|appendix_html|affiliations_html' --type rust   # → nothing
rg -n 'tali-affiliation|tali-contributions|tali-author-mark|tali-equal-note' crates/  # → nothing
rg -n 'tali-byline|tali-author' crates/core/assets/css/base.css          # → still present
rg -n 'ORCID' docs/                                                       # → nothing
```

Plus a positive proof of the hazard fix — the byline must survive a mapping and the warning must be located:

```sh
printf -- '---\ntitle: T\nauthor:\n  - name: Ada Lovelace\n    affiliation: X\n---\n\nbody\n' > /tmp/a.tmd
cargo run -q -p taliesin-server -- build /tmp/a.tmd --stdout --no-exec | rg -n 'tali-author'
```
must print a line containing `Ada Lovelace`, and the same file under
`cargo run -q -p taliesin-server -- build /tmp/a.tmd --check-only` must report the retirement note with a line
number. **If the byline is absent, the cut has shipped the exact silence this task exists to prevent — stop and
fix A4.**

**Subject B**

```sh
rg -n 'shared-bib' --glob '!_site/*' --glob '!notes/*' .                  # → nothing
rg -n 'SiteDefaults|render_defaults|render_document_scoped_with_site|shared_for_single_doc' --type rust  # → nothing
rg -n 'validate_shared_bibliography|uncited_local|\.overlay\(' --type rust  # → nothing
rg -n bibliography crates/core/assets/schema/tali-site.schema.json editor/vscode/schema/tali-site.schema.json  # → nothing
diff crates/core/assets/schema/tali-site.schema.json editor/vscode/schema/tali-site.schema.json  # → identical
```

and a positive proof the retirement is diagnosed rather than silent:

```sh
mkdir -p /tmp/p && printf 'title: X\nbibliography: refs.bib\n' > /tmp/p/_site.yml
printf -- '---\ntitle: T\n---\n\nbody\n' > /tmp/p/index.tmd
cargo run -q -p taliesin-server -- build /tmp/p --check-only
```
must name `bibliography` with the register's own sentence and no "did you mean".

---

**Done when** `TALIESIN_PYTHON="$PWD/.venv/bin/python" ./tools/gates.sh` reports ten gates green on a tree where
every grep above returns what it says, and a mapping-shaped `author:` still renders a byline while drawing a
located retirement warning.
