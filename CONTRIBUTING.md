# Contributing

Taliesin is a single-author project with a narrow scope: it renders `.tmd` to **HTML**,
and only HTML. Feature requests that add an output format (PDF, LaTeX, Word, ePub) are
out of scope by design, not by backlog order. For anything larger than a bug fix, open an
issue first — a patch that does not fit the scope is a waste of your evening, and saying
so early is the only kindness available.

## Set up the gates before you write code

```sh
git clone https://github.com/AJBogo9/taliesin && cd taliesin
git config core.hooksPath .githooks   # <- REQUIRED: git does not do this for you
```

That second line is not optional and not automatic. `core.hooksPath` is unset in a fresh
clone, so without it `.githooks/pre-push` does not exist for you and your push is checked
by nothing at all.

## Run every gate, and believe it only when it says PASSED

```sh
./tools/gates.sh
```

This is the single command that runs everything: fmt, clippy, the workspace suite, both
`tsc` type-checks, the VS Code companion's grammar test, `cargo audit` / `cargo deny`,
and the two document gates — `build docs/guide --check-only` and
`tools/build-site.sh --check`, which the pre-push hook also runs. **A plain `cargo test`
is not enough.** The live-kernel and Node cases *skip silently* when their interpreter is
missing, so a green `cargo test` on a machine without Python or Node can be nearly empty
of meaning. `gates.sh` arms both `TALIESIN_REQUIRE_*` variables, checks each canary test
by name, and treats one ignored test as a failure. It prints `PASSED`, `FAILED`, or
`INCOMPLETE` (exit 2) — and `INCOMPLETE` means a gate never ran, so it certifies nothing.
Install what it says is missing rather than reaching for `--allow-missing`.

## What a change has to carry

- **The block contract.** Every emitted block keeps `data-block-id` and `data-sourcepos`
  (plus `data-source-file` when included). Click-to-source, the incremental diff and
  live-state preservation all key off it.
- **A test that fails without your fix.** Verify it by mutation: put the bug back and
  watch the named test fail. A test that passes both ways is not a test.
- **A feature witness in `crates/core/src/render/tests.rs`.** That is where a capability
  is pinned. `corpus/` is still the regression net, but a corpus document earns its place
  by being something a person wanted to read, or a golden no unit test can hold — the old
  "one corpus document per capability" rule was retired as circular evidence.

## Licensing of contributions

Taliesin is AGPL-3.0, and the author is its sole copyright holder — which is what makes
the relicensing right reserved in the [README](README.md#license) real rather than
theoretical. To keep it that way, **by opening a pull request you agree that:**

1. you wrote the contribution, or otherwise have the right to submit it;
2. it is contributed under the **AGPL-3.0**, like the rest of the project; and
3. you grant Andreas Bogossian a perpetual, worldwide, irrevocable, royalty-free,
   sublicensable licence to use, modify, distribute and **relicense** it under any terms,
   including a commercial or proprietary licence.

You keep the copyright in what you wrote. If clause 3 is not something you want to grant,
say so in the pull request rather than after the fact: it means the patch cannot be merged
as-is, and that is a fine outcome to reach in the first comment instead of the twentieth.
