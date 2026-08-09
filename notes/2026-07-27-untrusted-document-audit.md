# R5: what happens when a user opens a `.tmd` they did not write

> **STATUS: dated record.** Superseded by the [2026-08-08 scope ruling](2026-08-08-scope-ruling.md)
> and the cut it authorised. True when written, not now. **Before acting on anything here, check
> that the file, flag or verb it names still exists.** See [CUT-PROGRESS.md](CUT-PROGRESS.md).

Date: 2026-07-28. Method: source reading plus existing tests. No hostile document was
executed against a live kernel on this machine, per the brief. Every claim below names the
file it was derived from and the measurement that would refute it.

---

## 0. The position, stated first (the question the brief says must not be dodged)

**Executing an untrusted `.tmd` is NOT meant to be safe, and it should not be made safe.**
Execution is the product. The correct deliverable is a *discoverable, honest* statement of
that plus one consent affordance, not a sandbox.

But that answer is only defensible if two things hold, and today **neither does**:

1. **The tool must not claim a safety it does not deliver.** It currently does. The User
   Guide says `--no-exec` lets you "preview untrusted docs safely"
   (`docs/guide/reference/cli.tmd:148`) and the CLI help says it "previews untrusted docs as
   source" (`crates/server/src/main.rs:189`). Measured, `--no-exec` disables *only*
   kernel-backed cells. A `{js}` cell, a raw `<script>` block, and `include-in-header` all
   still run in the reader's browser, and `{{< include >}}` still reads and inlines files.
   That is a false promise pointed at exactly the user this round is about.

2. **The tool must not exceed what "running someone else's code" already implies.** Two
   paths do. `mounts:` in a project's own `_site.yml` resolves an *unbounded* filesystem
   path (`crates/server/src/serve_site/mod.rs:293`), and `check` spawns a binary the
   *project* names (`crates/server/src/check.rs:382` into `crates/server/src/interpreter.rs:150`).
   Neither is "the document's code ran". They are the tool being steered by document
   metadata into acting outside the document. Those are bugs against promises SECURITY.md
   already makes, and they get **enforced**, not documented.

So: informed consent as the frame; two containment fixes as the exception; and the real
work is making the existing position *findable* by someone about to open a stranger's file.

**SECURITY.md is right and nearly invisible.** `SECURITY.md:38-41` already says code cells
execute, "Opening and previewing a `.tmd` document runs its code, exactly like a Jupyter
notebook. Do not preview documents you would not run." That is the correct sentence. It
lives in a file GitHub surfaces only under the Security tab, it is not in either dogfooded
book, it is not in `README.md`, and it is not printed by any command. The one place a user
*does* look, the CLI reference, contradicts it. The finding is not that the position is
missing. It is that the position is unreachable and locally contradicted.

---

## 1. Controls table (all nine paths)

Verdicts: **HEALTHY** (control exists and holds), **BY DESIGN** (capability is the product,
control is consent), **GAP** (control missing or weaker than stated), **DOC GAP** (behavior
is right, what the user is told is wrong).

| # | Path | What an untrusted doc can do | Existing control | What the user is told | Verdict |
|---|------|------------------------------|------------------|-----------------------|---------|
| 1 | Kernel code cells (`exec.rs`, `kernel.rs`) | Arbitrary `{python}`/`{r}` on the user's machine, user's privileges, cwd = doc dir, warm kernel reused across edits. Per-cell timeout `TALIESIN_CELL_TIMEOUT` (default 120s) bounds one cell, not the total. | `--no-exec` / `TALIESIN_NO_EXEC` (`exec.rs:405-408`: `run()` returns blocks untouched, no kernel contacted). Timeout + SIGINT. Freeze cache never replays errors. | `SECURITY.md:38-41` states it plainly. `--host` prints a runtime warning (`serve/mod.rs:289`, `serve_site/mod.rs:396`). Loopback preview prints **nothing**. | **BY DESIGN**, but consent is silent on the default path |
| 2 | `{{< include >}}` traversal (`includes.rs`) | Inline any file the boundary allows, verbatim, into the page. Survives `--no-exec` (comment at `includes.rs:738` says so explicitly). | Strong and test-pinned. `try_join_in` (`includes.rs:390-440`) refuses absolute paths and `../` climbs above the root; canonicalizes the target and requires it inside `symlink_root`. `single_doc_root` (`includes.rs:534`) roots a CLI-invoked doc at its own `_site.yml` or its own directory, never at `.git` (PT-2). Pinned by `crates/core/tests/symlink_containment.rs` and `crates/core/tests/include_root_parity.rs`. | `SECURITY.md:49-57` documents the symlink rule accurately. | **HEALTHY** lexically. One premise gap: see §2.4 (an untrusted *archive* carries its own symlinks) |
| 3 | Shortcode args / extension expansion (`render/extension/mod.rs`) | Arg values land in attributes; `{{< embed X >}}` and `{{< video X >}}` put an author string into `iframe src` / `video src`. Arg typos are linted. | Everything is escaped: `escape_attr` for attribute context, `html_escape` for text (`extension/mod.rs:426-594`). No path is read at expansion time. `_extensions/` is CSS-only (`frontmatter.rs:924`), so an extension bundle cannot ship JS. | Nothing specific; not needed. | **HEALTHY** on escaping. **GAP (low)**: `embed`/`video` paths are not scheme-filtered, unlike markdown links which go through `safe_url` (`emit.rs:148`). Not an escalation given path 5, but it is an inconsistency inside a guard the code says it wants. |
| 4 | Front-matter values reaching the page | Two different things. (a) Text into HTML: title, description, JSON-LD, inline JS globals. (b) `include-in-header` / `include-before-body` / `include-after-body` / `css`, which inject **verbatim markup into `<head>` and `<body>`**, including `{text: "<script>..."}` with no escaping at all (`doc_includes.rs:98-124`). `css:` wraps in `<style>` without neutralizing `</style>`. | (a) is disciplined: `html_escape`/`escape_attr` (`render/mod.rs:2846,2952`), JSON-LD escapes every `<` to a `<` unicode escape, with a comment explaining the script-data-double-escaped attack (`site/meta.rs:271-280`), `js_str` neutralizes `<`, U+2028/9 (`serve/mod.rs:911`). (b) has **no** control, deliberately: the file path goes through `safe_join_in`, the contents do not. | Nothing. The front-matter reference does not mark these keys as arbitrary-markup channels. | (a) **HEALTHY**. (b) **BY DESIGN** (it is a raw-markup key) but undisclosed, and it survives `--no-exec` |
| 5 | `{js}` cells in the reader's browser | Arbitrary JS at the preview's origin. Plus raw HTML passthrough: `emit.rs:90-91` emits `HtmlBlock`/`HtmlInline` verbatim, so a bare `<script>` in the markdown body runs too. Same-origin means it can read every page of the site and `GET` any file under the project root through the asset route, then POST it anywhere (no CSP is emitted anywhere in the tree; grep for `Content-Security-Policy` in `crates/` returns nothing outside planning docs). | **None, and correctly so** for the content path. The considered ruling is in `docs/superpowers/specs/2026-07-03-quarto-design-decisions-catalog.md:432`: a meta-delivered CSP is the weak form, must carry `'unsafe-inline'`, and would break legitimate embeds. Do not reverse it. | Nothing distinguishes `{js}` from `{python}` in the user's mind, so `--no-exec` reads as covering both. | **GAP**: `--no-exec` does **not** suppress `{js}`. `{js}` is emitted at *render* time by core (`render/mod.rs:861`, `render/mod.rs:1034-1037`), and core has no knowledge of `TALIESIN_NO_EXEC` (the symbol appears only under `crates/server/`). |
| 6 | `mounts:` serving another project | `_site.yml` names a directory; every `.tmd` under it becomes a live page whose cells **execute** (`serve_site/mod.rs:46-49` says so), and every file under it is served over HTTP (`serve_site/mod.rs:608` passes `project.dir` to `serve_asset_from`). | **None.** `let mroot = root.join(&m.path)` (`serve_site/mod.rs:293`). `Path::join` with an absolute path replaces the base, and `..` is never rejected. `safe_join*` is not called. `validate_mounts` (`config/mod.rs:485`) checks only *key spelling*. Bounded only in that `build` deliberately does not wire mounts (`build.rs:1027-1040`, warns instead). | The `Mount` doc comment says "relative to the site root" (`config/mod.rs:80`). That is an unenforced expectation, not a control. | **GAP (highest severity in this round)** |
| 7 | Asset paths and what a build copies | `preview` serves any file that canonicalizes under the project root. `build` copies referenced local assets into the output. | `serve_asset_from` (`serve/mod.rs:702-716`) canonicalizes both sides and requires `starts_with(root)` plus `is_file`. Build's `copy_local_assets` (`build.rs:749-783`) rejects absolute and `..` refs and then requires `inside_repo(from, repo_boundary(base))`; `deploy_referenced_sources` applies the same. | `SECURITY.md:53-57` describes both accurately, including that preview is stricter than build. | **HEALTHY** as an enforcement mechanism. The residual is the *boundary choice*, not the check: `repo_boundary` is the enclosing `.git`, so a symlink inside an untrusted bundle unpacked into a checkout can pull a sibling file into `_site/` and publish it. See §2.4. |
| 8 | Headless browser (`headless_js.rs`) | `read --run-js` runs the document's `{js}` in a real Chrome launched with `--no-sandbox` (`headless_js.rs:299`), on a `file://` page. | Gated: only when `executed` is true, which is `run && TALIESIN_NO_EXEC unset` (`query.rs:154,177-181`). Throwaway profile, every phase timeout-bounded, browser killed on every exit path. No Chrome degrades to `Skipped`. | The module header claims "offline ... no network". | **HEALTHY** on gating and teardown. **DOC GAP**: the `--no-sandbox` rationale at `headless_js.rs:288-297` explicitly rests on "the author-trusted input the crate's trust model already assumes", which is the assumption this round retires; and "offline" is true of the *fetcher*, not of the page, since a `file://` page's JS can still `fetch`/`sendBeacon` outbound. |
| 9 | `--host` exposure | Preview reachable from the LAN. | Layered and genuinely good. Loopback by default. Per-session UUID token, `HttpOnly`, `SameSite=Lax`, gating all non-loopback requests (`serve/security.rs:124-179`). WebSocket origin check, with the loopback blanket-allow *dropped* under `--host` (`security.rs:20-38`). Unconditional `Host` allowlist as the DNS-rebinding defense, installed even for loopback previews (`security.rs:200-233`). Control surface is two messages: `click_block` (logs only) and `restart_kernel` (`serve/mod.rs:1148-1174`). | `SECURITY.md:42-46` is accurate. Runtime warning printed when `--host` is set without `--no-exec`. | **HEALTHY**. This is the best-defended path in the tool and needs nothing. |

---

## 2. The gaps

### 2.1 `--no-exec` is the advertised answer for untrusted documents and it is not one (HIGH)

Measurement. `cli.rs:938-942` sets `TALIESIN_NO_EXEC=1`. The only consumer that suppresses
anything is `exec::Executor::run`, which returns blocks untouched (`exec.rs:405-408`), plus
`query.rs:154`. `taliesin-core`, which produces the HTML, never reads the variable. Therefore
with `--no-exec` in force an untrusted document still gets:

- a `{js}` cell executed in the browser (`render/mod.rs:1034-1037`, unconditional);
- a raw `<script>` in the body executed (`emit.rs:90-91`, verbatim passthrough);
- `include-in-header: {text: ...}` injected into `<head>` (`doc_includes.rs:100-104`);
- `css: {text: "</style><script>..."}` injected, since `append_include` does not neutralize
  the closing tag (`doc_includes.rs:115-124`);
- `{{< include ../secret.tmd >}}` read and inlined within the containment root, which the
  code comment at `includes.rs:738` calls out as "surviving `--no-exec`".

And the guide says this mode is for previewing untrusted docs **safely**
(`docs/guide/reference/cli.tmd:148`).

What would refute it: a suppression of `{js}`/raw-HTML emission keyed on the env var
somewhere in `crates/core/`. I grepped `TALIESIN_NO_EXEC` across the workspace; every hit is
under `crates/server/` or in prose. Also refuted if `{js}` were considered out of scope for
"code cells", but the guide and `check`'s own diagnostics treat `{js}` as a cell throughout
(`docs/guide/reference/cli.tmd:42,43,106`).

Shape, enumerated. The channels that run browser-side code and survive `--no-exec` are
exactly four: `{js}` cells, raw HTML (`HtmlBlock`/`HtmlInline` and the `{=html}` fence),
the three `include-*` front-matter keys, and `css:`. `_extensions/` is **not** one of them
(theme CSS only, `frontmatter.rs:924`), and shortcodes are **not** one of them (all output
is escaped). That is the closed set.

### 2.2 `mounts:` has no containment at all (HIGH)

Measurement. `serve_site/mod.rs:287-317`:

```rust
let mroot = root.join(&m.path);
let mroot = mroot.canonicalize().unwrap_or(mroot);
if !mroot.is_dir() { ...warn, skip... }
```

`m.path` is the raw `_site.yml` string (`config/mod.rs:80-86`, `mounts_from` at
`config/mod.rs:331-353` does no normalization). `Path::join` with an absolute argument
discards the base, so `mounts: { x: /home/<user> }` mounts the home directory, and
`mounts: { x: ../../.. }` climbs. The mounted tree then gets a full `Project`: pages are
discovered and **executed live** (`serve_site/mod.rs:46-49`), and `serve_asset(&project.dir, ...)`
(`serve_site/mod.rs:608`) serves any file under it, because `serve_asset_from`'s containment
root *is* that unbounded directory.

Impact of one hostile `_site.yml` line under `taliesin preview <dir>`: an arbitrary-file HTTP
read primitive rooted anywhere the user can read, plus execution of `.tmd` files from
outside the previewed project. `--no-exec` stops the execution half, not the serving half.

Bounded by: `build` does not wire mounts (`build.rs:1027-1040,1482-1484`), so this is
`preview`-only. The `--host` token and `Host` guard still apply, so it is not remotely
reachable by default.

What would refute it: a containment call on `m.path` anywhere between YAML parse and
`Project` construction. I traced `mounts_from` to `Site::config.mounts` to the `filter_map`
above and found none; `validate_mounts` (`config/mod.rs:485-502`) inspects key names only.

### 2.3 `taliesin check` spawns a binary the untrusted project chose (HIGH)

Measurement. `check.rs:481` and `check.rs:793` call `collect_environment`, which for each
language the document uses builds an `EnvEntry` via `env_entry` (`check.rs:376-392`), which
calls `interpreter::probe`. `probe` (`interpreter.rs:145-165`) runs
`Command::new(bin).arg("--version").output()` and then an import check. `bin` comes from
`resolve_python` (`interpreter.rs:79-95`), whose precedence is:

1. the `_site.yml` `python:` field, used verbatim as a path with no existence or containment
   check (`PathBuf::from(f)`), then
2. a **project-local `.venv/bin/python`**, discovered by existence alone, then
3. `TALIESIN_PYTHON`, then `python3`.

So a bundle that ships either `_site.yml: python: ./anything` or a `.venv/bin/python` file
gets that file executed by the command whose entire purpose is "tell me whether this is OK
before I open it". `collect_environment` has no `TALIESIN_NO_EXEC` gate (grepped: `check.rs`
contains no reference to it). The same reaches the MCP `check` tool, whose description is
"Validate a .tmd file or project directory" (`mcp.rs:57`) with no hint that it spawns
anything, although the `mcp` help text is otherwise commendably blunt that it is not a
sandbox (`main.rs:347-351`).

Mitigating: the probe only fires for languages the document actually uses
(`used_languages`, `check.rs:358-372`), so a prose-only document spawns nothing. That is a
one-line addition for an attacker.

What would refute it: a gate on `exec_disabled()` in the `check` path, or `probe` being
called only after user confirmation. Neither exists. Note the module doc at
`interpreter.rs:7` says probe "never runs the user's document", which is true and is the
wrong reassurance: it runs the *project's chosen binary*.

### 2.4 The symlink premise inverts when the document arrives as an archive (MEDIUM)

`symlink_root` (`includes.rs:472-484`) widens the symlink boundary to the enclosing `.git`,
and `symlink_containment.rs:8-11` states the rationale: a symlink is "a filesystem fact
placed by whoever owns the checkout, not something the document text can conjure". That is
correct for first-party authoring and it fixed a real problem (a book sharing one
`references.bib` with a sibling `paper/`).

It stops being correct the moment the document arrives from elsewhere, because tar, zip
with Unix extensions, and git all carry symlinks. Unpack a hostile bundle into a directory
*inside* your own checkout, and a symlink shipped in that bundle canonicalizes to a sibling
file, passes `symlink_root`, and is inlined verbatim into the page (via `{{< include >}}`)
or copied into `_site/` and published (via `copy_local_assets`, `build.rs:751,795` and the
mirror at `build.rs:2168`). It survives `--no-exec`.

Not exploitable when the bundle is a standalone clone: the clone's own `.git` stops the walk
at the bundle, so it is self-contained. The escape requires nesting inside a larger checkout.

This is **documentation, not a code change**, in my reading. Narrowing the symlink boundary
was tried and reverted for a good reason (the sibling-`.bib` case in
`symlink_containment.rs:67-80`), and re-narrowing it would trade a real authoring capability
for a threat that consent already covers. Do not re-litigate it as code. Do add the sentence
to `SECURITY.md`: the symlink allowance assumes *you* placed the symlink, so unpack
documents from elsewhere outside your checkouts.

Explicitly not re-filed: the include symlink-loop SIGABRT. It was refuted (Linux caps at
`MAXSYMLINKS=40`) and is on the do-not-re-add list.

### 2.5 The preview is an unauthenticated read of the whole project tree, and the document can drive it (MEDIUM, amplifier)

`serve_asset_from` correctly confines to the project root, but *everything* under that root
is readable over HTTP with no allowlist: `.tmd` sources, `_freeze/`, `.env`, `.git/config` if
the root is a checkout. Combined with §2.1 (document-supplied same-origin script) and the
absence of any CSP, an untrusted document dropped into the user's own project can read that
project and POST it out. Nothing here is a bug on its own. It is why "the document runs code"
is a bigger statement than a Jupyter-notebook analogy suggests, and it belongs in the
consent text.

### 2.6 `theme:`'s extension arm bypasses the guard its neighbour uses (LOW)

`render/theme.rs:44-48`:

```rust
ext => base_dir.and_then(|b| {
    std::fs::read_to_string(b.join("_extensions").join(ext).join("theme.css")).ok()
})
```

The arm immediately above (`theme.rs:29-39`) routes a `.css`/`.scss` value through
`safe_join_in`. This one does not, so `theme: ../../../../home/<user>/x` reads
`<base>/_extensions/../../../../home/<user>/x/theme.css` and inlines it into a `<style>`
block. Narrow: the basename is fixed at `theme.css`, and kernel path resolution requires
`_extensions/` to actually exist, which an untrusted bundle simply ships. Refuted if `join`
normalized `..` (it does not) or if the read failed through a non-existent `_extensions`
(it would, which is why the bundle must ship the directory).

### 2.7 Two documentation defects on the safety-relevant sentences (LOW, but they are the ones that matter)

- `docs/internals/repository.tmd:182` states `--host` "turns this on automatically" for
  `TALIESIN_NO_EXEC`. It does not. `cli.rs:938` sets the variable only from `--no-exec`;
  `serve/mod.rs:289` and `serve_site/mod.rs:396` only print a warning. A reader of the
  Internals book would conclude LAN previews are non-executing. They are not.
- `docs/guide/reference/cli.tmd:148`, the "safely" claim, per §2.1.

---

## 3. Measured healthy (recorded, no action)

Confirmations are results. These were examined and hold:

- **The `--host` layer** (path 9): token, origin check with the loopback allowance
  deliberately dropped under `--host`, unconditional DNS-rebinding `Host` allowlist even for
  loopback previews. Test-pinned in `serve/security.rs:248-415`. Nothing to add.
- **The websocket control surface** is two messages and neither is a capability: `click_block`
  logs a line, `restart_kernel` restarts the user's own kernel (`serve/mod.rs:1148-1174`).
  The comment there already reasons about unauthenticated control and reaches the right
  conclusion.
- **Include containment** (path 2) is genuinely strong: lexical plus canonicalized-symlink,
  with the bare-filename empty-root hole already closed and pinned
  (`symlink_containment.rs:36-66`), and PT-2's `.git`-widening escape closed for single-doc
  invocations (`include_root_parity.rs:140`).
- **Escaping discipline** on front-matter to HTML, JSON-LD, and inline JS globals (path 4a).
  The JSON-LD `<` handling and `js_str`'s U+2028 handling are both better than typical.
- **Build asset mirroring** applies its boundary in all three places
  (`build.rs:751,795,2168`), and `build` refuses to wire `mounts:` at all.
- **The headless path** respects `--no-exec`, bounds every phase, and always tears down the
  browser and profile.
- **`_extensions/` is CSS-only**, so extension bundles are not a code channel. This was
  checked because it is the obvious place for one.

---

## 4. Document versus enforce

**ENFORCE (three, all narrow, none is a sandbox):**

1. **`mounts:` path containment.** Resolve through `safe_join_in` against the site root,
   or at minimum refuse absolute paths and `..` climbs. This is not restricting what a
   document may compute; it is restricting what a *config key* may point the server at, and
   the key's own doc comment already says "relative to the site root". No new knob, no
   behavior change for any real project (the repo's own `mounts: docs: ../docs` stays inside
   the checkout, so scope the root to the repo boundary rather than the `_site.yml` dir).
2. **`theme:`'s `_extensions` arm** goes through `safe_join_in` like the arm above it.
   Two lines, restores an invariant the file already intends.
3. **Gate `check`'s interpreter probe.** Either skip the probe under `TALIESIN_NO_EXEC`, or
   (better default, no knob) refuse to spawn a `Provenance::Field` or `Provenance::Venv`
   binary that lives *inside the project being checked* without an explicit opt-in, and
   report `runs: unknown (project-supplied interpreter, not probed)`. A user's own
   `.venv` is the common case, so this needs care: the honest split is that `check`
   validates, and probing a project-supplied binary is execution, which `check` should not
   do silently.

**DOCUMENT (everything else):**

4. Make `--no-exec`'s claim honest. Two options, and I recommend both:
   (a) fix the words: it stops kernel cells, not browser-side code; and
   (b) make it also suppress `{js}` execution and raw-HTML `<script>` when the *user* asked
   for it. (b) is not a sandbox: it is a flag the user opted into doing what its name says.
   If (b) is judged out of scope, then (a) alone is mandatory and the flag should stop being
   described as an untrusted-document mode anywhere.
5. Put the trust model where it is found: a short "Documents you did not write" section in
   `docs/guide/`, linked from `README.md` and from the CLI's `preview --help`. `SECURITY.md`
   keeps the canonical text; the guide carries the reader-facing version.
6. Say that `include-in-header`/`include-before-body`/`include-after-body`/`css` are
   raw-markup channels in the front-matter reference.
7. Correct `docs/internals/repository.tmd:182`.
8. Add the archive-symlink caveat to `SECURITY.md:49-57`.

**A consent affordance, once.** On the first preview or build of a document that carries
executable cells *and* whose project directory is not one this machine has run before, print
one line naming the document, the languages, and the resolved interpreter path, and say that
running continues. Not a prompt (it would break the warm-loop workflow and every script),
not a config knob, not remembered per document. A single stderr line, the same register as
the existing `--host` warning. That is the whole deliverable of "informed consent".

**Explicitly do NOT build:** a code-cell sandbox, a CSP (ruled out on merits in the Quarto
decisions catalog and I concur), an HTML sanitizer on the content path, or a network egress
boundary. Each would either break the product or provide theatre.

---

## 5. Proposed items

**109. `--no-exec` does not stop the document's browser-side code, and the guide calls it
"safe".** (HIGH.) Measured: `TALIESIN_NO_EXEC` is read only under `crates/server/`;
`{js}` is emitted at render time (`render/mod.rs:861,1034-1037`), raw HTML passes through
(`emit.rs:90-91`), `include-*`/`css:` inject verbatim (`doc_includes.rs:98-124`), and
includes still read files (`includes.rs:738`). Fix (a) the wording in
`docs/guide/reference/cli.tmd:148` and `main.rs:189,273`, and (b) preferably make the flag
suppress `{js}` and raw-HTML script too. Refuted by finding any core-side suppression keyed
on the flag.

**110. `mounts:` resolves an unbounded filesystem path.** (HIGH.) `serve_site/mod.rs:293`
does `root.join(&m.path)` with no containment; an absolute value replaces the root and `..`
climbs. Result under `preview <dir>`: arbitrary directories served as pages (executed) and
as static files. ENFORCE containment. Refuted by a containment call I did not find between
`mounts_from` (`config/mod.rs:331`) and the `filter_map` at `serve_site/mod.rs:292`.

**111. `check` spawns a project-chosen interpreter.** (HIGH.) `check.rs:382` to
`interpreter.rs:150` runs `_site.yml python:` verbatim, or a bundled `.venv/bin/python`,
with no `TALIESIN_NO_EXEC` gate. The MCP `check` tool inherits it. ENFORCE a gate; the
honest framing is that probing a project-supplied binary is execution. Refuted by an
`exec_disabled()` check in the `check` path.

**112. A discoverable "documents you did not write" section, plus a one-line first-run
notice.** (HIGH value, low cost.) `SECURITY.md:33-63` already holds the correct position and
no reader-facing surface points at it; the one that does contradicts it. Add the guide
section, link it from `README.md` and `preview --help`, and print one stderr line naming the
document, languages, and resolved interpreter on a first execution in an unseen project
directory. Not a prompt, no knob, no memory of individual documents.

**113. `theme:`'s `_extensions` arm bypasses `safe_join_in`.** (MEDIUM.) `theme.rs:44-48`
reads `<base>/_extensions/<value>/theme.css` with `join` only, while the arm at
`theme.rs:29-39` guards the equivalent path. Route it through `safe_join_in`. Refuted if a
`..` in the value cannot escape, which it can once the bundle ships an `_extensions/`
directory.

**114. `SECURITY.md`'s symlink allowance assumes you placed the symlink.** (MEDIUM,
documentation only.) `symlink_root` widens to the enclosing `.git`
(`includes.rs:472-484`); tar, zip and git all carry symlinks, so a bundle unpacked *inside*
a checkout can read and publish sibling files, surviving `--no-exec`. Do **not** narrow the
boundary (the sibling-`.bib` case at `symlink_containment.rs:67-80` is why it is wide). Add
one sentence to `SECURITY.md:49-57`.

**115. `docs/internals/repository.tmd:182` claims `--host` auto-enables `--no-exec`.**
(LOW, but it is a safety sentence.) It only warns (`serve/mod.rs:289`,
`serve_site/mod.rs:396`); `cli.rs:938` sets the variable from the flag alone. Correct the
table row.

**116. Document that `include-in-header`/`include-before-body`/`include-after-body`/`css`
are raw-markup channels.** (LOW.) `doc_includes.rs:115-124` injects `{text: ...}` verbatim
and does not neutralize `</style>` in the `css:` wrapper. This is by design; the
front-matter reference should say so, since these are the head-injection channels and the
set is closed (verified: `_extensions/` is CSS-only, shortcodes are fully escaped).

**117. `{{< embed >}}` and `{{< video >}}` sources are not scheme-filtered.** (LOW.)
`extension/mod.rs:590-594` escapes the value into `iframe src` but does not apply
`safe_url`, which `emit.rs:148` applies to markdown links precisely because "an include, a
third-party README" may not be fully authored. Not an escalation while raw HTML passes
through; file it as a consistency fix inside a guard the code already wants.

**118. The headless `--no-sandbox` rationale rests on the assumption this round retires.**
(LOW.) `headless_js.rs:288-297` justifies the flag with "exactly the author-trusted input
the crate's trust model already assumes", and calls the path "offline" on the strength of
the fetcher being off, although a `file://` page's JS can still send data outbound. The
gating is correct (`query.rs:154,177-181`) and needs no change; update the comment so the
next reader does not inherit a premise that only holds for first-party documents.

**119. Record as measured-healthy (no change).** The `--host` security layer, the two-message
websocket control surface, include containment, front-matter-to-HTML escaping, build asset
boundaries, `build`'s refusal to wire mounts, and headless teardown were all examined against
a hostile document and hold. Worth banking in `notes/` so the next round does not re-derive
them: the preview server's *network* boundary is in good shape, and every real gap this round
found is on the *document-as-input* boundary instead.
