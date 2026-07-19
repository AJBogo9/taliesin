# PL16 — Group the 16-command `help` by purpose

`usage()` printed one flat block of 16 commands, mixing the everyday three with ten an author
rarely types. git/cargo/gh group by purpose; clig.dev endorses it.

**Fix (pure formatting).** Section the `COMMANDS:` block with flush-left headers (so each
command line stays unindented):

- **Author** — init, new
- **Preview & build** — preview, build, publish
- **Inspect** — check, doctor, map, read, render, blocks, symbols
- **Editor & agent** — schema, vocab, mcp, completions
- (help, --version at the end)

No command text changed; no behaviour changed — the commands were only reordered under headers.

**Test.** `crates/server/tests/help_cli.rs::help_groups_commands_by_purpose` pins the four
section headers (present + in order), that each command lands under the right header, and that
no command was dropped in the reorder. Mutation-checked (dropping a header fails it).
