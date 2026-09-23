# Contributing

Taliesin is a single-author project with a narrow scope: it renders `.tmd` to HTML only.
Feature requests that add an output format (PDF, LaTeX, Word, ePub) are out of scope by
design, not deferred. For anything larger than a bug fix, open an issue first, so a patch
that does not fit the scope is caught before you spend an evening on it.

## Set up the gates before you write code

```sh
git clone https://github.com/AJBogo9/taliesin && cd taliesin
git config core.hooksPath .githooks   # <- REQUIRED: git does not do this for you
```

The second line is required. `core.hooksPath` is unset in a fresh clone, so without it
git never runs `.githooks/pre-push` and nothing checks your push.

## Run every gate

```sh
./tools/gates.sh
```

The script runs every gate: fmt, clippy, the workspace suite, both `tsc` type-checks, the VS
Code companion's grammar test, `cargo audit` / `cargo deny`, the two document gates
(`build docs/guide --check-only` and `build docs/internals --check-only`) and the
separate `tools/publish.sh --check` gate, all of which the pre-push hook also runs. A
plain `cargo test` is not enough. The live-kernel and Node cases skip silently when their
interpreter is missing, so a green `cargo test` on a machine without Python or Node says
little. `gates.sh` arms both `TALIESIN_REQUIRE_*` variables, checks each canary test by
name, and treats one ignored test as a failure. It prints `PASSED`, `FAILED`, or
`INCOMPLETE` (exit 2). `INCOMPLETE` means a gate never ran, so it certifies nothing.
Install what it reports as missing instead of passing `--allow-missing`.

## What a change has to carry

- **The block contract.** Every emitted block keeps `data-block-id` and `data-sourcepos`
  (plus `data-source-file` when included). Click-to-source, the incremental diff and
  live-state preservation all key off it.
- **A test that fails without your fix.** Verify it by mutation: put the bug back and
  watch the named test fail.
- **A feature witness in `crates/core/src/render/tests.rs`.** That is where a capability
  is pinned. `corpus/` is still the regression suite, but a document belongs in the corpus
  only if it is something a person wanted to read, or a golden no unit test can hold. The
  old "one corpus document per capability" rule was retired as circular evidence.

## Licensing of contributions

Taliesin is AGPL-3.0, and the author is its sole copyright holder, so the author can
exercise the relicensing right reserved in the [README](README.md#license). To keep it
that way, **by opening a pull request you agree that:**

1. you wrote the contribution, or otherwise have the right to submit it;
2. it is contributed under the **AGPL-3.0**, like the rest of the project; and
3. you grant Andreas Bogossian a perpetual, worldwide, irrevocable, royalty-free,
   sublicensable licence to use, modify, distribute and **relicense** it under any terms,
   including a commercial or proprietary licence.

You keep the copyright in what you wrote. If you do not want to grant clause 3, say so
when you open the pull request. Declining it means the patch cannot be merged as-is,
which is an acceptable outcome.
