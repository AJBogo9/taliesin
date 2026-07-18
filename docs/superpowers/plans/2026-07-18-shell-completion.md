# Dynamic `.tmd`-aware Shell Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `tab` after `tali preview`/`build`/etc. offer only `.tmd` files and directories that contain one (site/book roots first), plus subcommand and flag completion, driven by one Rust brain shared across zsh/bash/fish/PowerShell.

**Architecture:** A hidden `taliesin __complete <words…>` subcommand computes candidates and prints them in a cobra-compatible wire format (`value\tdesc` lines + a trailing `:<directive>` line). Four thin generated shims (`taliesin completions <shell>`) relay the current command line to `__complete` and feed its output back to the shell. All logic lives in a new single-purpose module `crates/server/src/complete.rs`, replacing today's static `completions` scripts that fall back to unfiltered file completion.

**Tech Stack:** Rust (edition 2024, std only, no new crates); shell script fragments for bash/zsh/fish/PowerShell; existing test harness (`std::process::Command` + `env!("CARGO_BIN_EXE_taliesin")`, `#[cfg(test)]` unit modules in the bin crate).

## Global Constraints

- **No new dependencies.** std only. The CLI parses `std::env::args()` by hand; do not introduce `clap`/`clap_complete`.
- **Rust edition 2024**, workspace resolver 3. Keep the tree `cargo fmt`-clean (a `PostToolUse` hook runs rustfmt; CI enforces it).
- **`__complete` is hidden:** never add it to `crate::COMMANDS`, `usage()`, or `subcommand_help`. Hidden internal subcommands are `_`-prefixed by convention.
- **Shims register both `taliesin` and `tali`** (the user's primary alias is `tali`, a symlink to `taliesin`), and always *call* `taliesin __complete`.
- **Paths resolve relative to the process CWD** (`std::env::current_dir()`); the brain takes an explicit `cwd: &Path` so tests can point it at a temp tree.
- **Writing style (user global rule):** no em dashes or en dashes in any prose this plan adds to help text or docs. Use commas, colons, or parentheses.
- **Scope:** developer-tooling only. Do not touch rendering, the block model, the deck engine, `serve_site/exec_pool.rs` warm-page eviction, or any load-bearing invariant. This feature is **not** corpus-pinned (nothing under `corpus/` exercises it).
- **Directive bits (cobra-compatible):** `NoSpace = 1`, `NoFileComp = 4`. Directory candidate values end in `/`.
- **Ignore set for directory walks:** `.git`, `target`, `node_modules`, `_site`, `_freeze`, and any dot-directory. Recursive `.tmd` search is depth-capped at 6 and short-circuits on first hit.

---

### Task 1: Extract completion code into `complete.rs` (pure refactor, no behavior change)

Move the existing static completion machinery out of `cli.rs` into a new single-purpose module. No behavior changes: the same three shells, the same scripts, the same tests, just relocated. This isolates the surface the rest of the plan grows.

**Files:**
- Create: `crates/server/src/complete.rs`
- Modify: `crates/server/src/cli.rs` (remove the moved items)
- Modify: `crates/server/src/main.rs` (add `mod complete;`, repoint the `completions` dispatch arm)

**Interfaces:**
- Produces: `complete::cmd_completions(args: &[String]) -> ExitCode`, `complete::completions_script(shell: &str) -> Option<String>` (both moved verbatim from `cli.rs`).

- [ ] **Step 1: Create `crates/server/src/complete.rs` with the moved code**

Cut these from `cli.rs` and paste into the new file: `cmd_completions` (currently `cli.rs:544`), `completions_script` (`cli.rs:561`), the `BASH_COMPLETIONS` / `ZSH_COMPLETIONS` / `FISH_COMPLETIONS` consts (`cli.rs:571`, `591`, `615`), and the entire `mod completions_tests` block (`cli.rs:807-860`). The new file header:

```rust
//! Shell completion: the `completions <shell>` script generators and (added later) the
//! hidden `__complete` runtime brain they call. All completion logic lives here so
//! behavior cannot drift between shells: each shim only relays the brain's output.

use crate::log;
use std::process::ExitCode;

pub(crate) fn cmd_completions(args: &[String]) -> ExitCode {
    match args.get(2).map(String::as_str).and_then(completions_script) {
        Some(script) => {
            print!("{script}");
            ExitCode::SUCCESS
        }
        None => {
            log::error("usage: taliesin completions <bash|zsh|fish>");
            ExitCode::FAILURE
        }
    }
}

/// The completion script for `shell`, or `None` for an unsupported one. Every branch draws
/// its command list from `crate::COMMANDS.join(" ")` (the `@COMMANDS@` placeholder), so a
/// new subcommand appears in all three shells at once and no per-shell list can go stale.
pub(crate) fn completions_script(shell: &str) -> Option<String> {
    let template = match shell {
        "bash" => BASH_COMPLETIONS,
        "zsh" => ZSH_COMPLETIONS,
        "fish" => FISH_COMPLETIONS,
        _ => return None,
    };
    Some(template.replace("@COMMANDS@", &crate::COMMANDS.join(" ")))
}
```

Then paste the three `const *_COMPLETIONS: &str = r#"…"#;` blocks unchanged, and the `#[cfg(test)] mod completions_tests { … }` block unchanged, below them.

- [ ] **Step 2: Remove the moved items from `cli.rs`**

Delete `cmd_completions`, `completions_script`, the three `*_COMPLETIONS` consts, and `mod completions_tests` from `cli.rs`. Leave `mod tests` and `mod new_tests` in place. `cli.rs` keeps `use std::process::ExitCode;` (still used by `cmd_init`/`cmd_serve`/`cmd_new`).

- [ ] **Step 3: Wire the module in `main.rs`**

Add the module declaration in alphabetical position (between `mod cli;` and `mod exec;`):

```rust
mod cli;
mod complete;
mod exec;
```

Repoint the dispatch arm (`main.rs:59`):

```rust
        Some("completions") => complete::cmd_completions(&args),
```

- [ ] **Step 4: Verify it compiles and the moved tests pass from their new home**

Run: `cargo test -p taliesin-server --lib complete`
Expected: PASS. Includes `generates_a_script_for_each_supported_shell_and_nothing_else`, `every_shell_script_offers_exactly_the_dispatched_command_list`, `an_unknown_or_empty_shell_yields_no_script`.

Also run the whole binary test set to be sure nothing referenced the old paths:
Run: `cargo test -p taliesin-server`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/complete.rs crates/server/src/cli.rs crates/server/src/main.rs
git commit -m "refactor(server): move shell-completion code into its own complete.rs module"
```

---

### Task 2: Hidden `__complete` subcommand + subcommand-name brain + wire protocol

Add the dynamic brain's skeleton: dispatch, the `complete_line` entry point (subcommand-name completion only for now), and protocol printing. Extend the dispatch drift-guard to exclude `_`-prefixed hidden arms so `__complete` can stay out of `COMMANDS`.

**Files:**
- Modify: `crates/server/src/complete.rs`
- Modify: `crates/server/src/main.rs` (dispatch arm + guard helper + its test)
- Create: `crates/server/tests/complete_cli.rs`

**Interfaces:**
- Produces: `complete::cmd_complete(args: &[String]) -> ExitCode`; internal `complete_line(words: &[String], cwd: &Path) -> Completion` with `struct Completion { candidates: Vec<Candidate>, directive: u8 }` and `struct Candidate { value: String, desc: Option<&'static str> }`; consts `NO_SPACE: u8 = 1`, `NO_FILE_COMP: u8 = 4`.

- [ ] **Step 1: Write the failing unit test (subcommand completion)**

Append to `complete.rs`'s test module (add a new `#[cfg(test)] mod brain_tests` below `completions_tests`):

```rust
#[cfg(test)]
mod brain_tests {
    use super::*;
    use std::path::Path;

    fn values(words: &[&str]) -> Vec<String> {
        let owned: Vec<String> = words.iter().map(|s| s.to_string()).collect();
        complete_line(&owned, Path::new("."))
            .candidates
            .into_iter()
            .map(|c| c.value)
            .collect()
    }

    #[test]
    fn empty_word_completes_all_subcommands() {
        let got = values(&[""]);
        assert!(got.contains(&"preview".to_string()), "offers preview: {got:?}");
        assert!(got.contains(&"build".to_string()), "offers build: {got:?}");
        // Every dispatched command is offered.
        assert_eq!(got.len(), crate::COMMANDS.len(), "offers exactly COMMANDS: {got:?}");
    }

    #[test]
    fn prefix_filters_subcommands() {
        assert_eq!(values(&["pre"]), vec!["preview".to_string()]);
    }

    #[test]
    fn subcommand_completion_suppresses_file_fallback() {
        assert_eq!(complete_line(&["".to_string()], Path::new(".")).directive, NO_FILE_COMP);
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p taliesin-server --lib brain_tests`
Expected: FAIL to compile (`complete_line`, `Completion`, `NO_FILE_COMP` not defined).

- [ ] **Step 3: Implement the brain skeleton + protocol in `complete.rs`**

Add above the test modules:

```rust
use std::path::Path;

// cobra-compatible directive bits, relayed to the shim.
const NO_SPACE: u8 = 1;
const NO_FILE_COMP: u8 = 4;

/// One completion candidate: the literal word to insert, plus an optional one-line
/// description (shown by zsh/fish/powershell, ignored by bash).
struct Candidate {
    value: String,
    desc: Option<&'static str>,
}

impl Candidate {
    fn plain(value: impl Into<String>) -> Self {
        Candidate { value: value.into(), desc: None }
    }
    fn described(value: impl Into<String>, desc: &'static str) -> Self {
        Candidate { value: value.into(), desc: Some(desc) }
    }
}

struct Completion {
    candidates: Vec<Candidate>,
    directive: u8,
}

/// One-line description per dispatched command, looked up by name so the subcommand
/// completion stays in lockstep with `crate::COMMANDS` (a name missing here trips
/// `every_command_has_a_description`).
fn command_desc(cmd: &str) -> &'static str {
    match cmd {
        "render" => "render a full HTML page to stdout",
        "read" => "project the document to plain text",
        "build" => "build self-contained HTML (a dir builds the site)",
        "blocks" => "list block ids + sourcepos (debug)",
        "schema" => "emit JSON Schemas for editor autocomplete",
        "vocab" => "emit editor autocomplete vocabulary as JSON",
        "symbols" => "list the doc's cross-reference targets",
        "check" => "list located diagnostics (non-zero if any)",
        "map" => "whole-project outline (pages, nav, xref)",
        "mcp" => "stdio MCP server",
        "init" => "scaffold a starter site",
        "new" => "scaffold one document",
        "serve" => "live preview server (alias of preview)",
        "preview" => "live preview server",
        "dev" => "live preview server (alias of preview)",
        "publish" => "build + deploy to Cloudflare Pages",
        "help" => "show this help",
        "completions" => "print a shell completion script",
        _ => "",
    }
}

/// Compute completions for the words typed after `taliesin` (`words.last()` is the
/// current, possibly-empty word), resolving any paths relative to `cwd`.
fn complete_line(words: &[String], _cwd: &Path) -> Completion {
    let empty = String::new();
    let cur = words.last().unwrap_or(&empty).as_str();
    let prior: &[String] = if words.is_empty() { &[] } else { &words[..words.len() - 1] };

    // 1. Completing the subcommand token itself.
    if prior.is_empty() && !cur.starts_with('-') {
        let candidates = crate::COMMANDS
            .iter()
            .filter(|c| c.starts_with(cur))
            .map(|c| Candidate::described(*c, command_desc(c)))
            .collect();
        return Completion { candidates, directive: NO_FILE_COMP };
    }

    // Everything else: nothing yet (grown in later tasks). Fall back to file completion.
    Completion { candidates: Vec::new(), directive: 0 }
}

/// The hidden `__complete` subcommand: prints candidates + a directive line for the shim.
pub(crate) fn cmd_complete(args: &[String]) -> ExitCode {
    let words: Vec<String> = args.get(2..).unwrap_or(&[]).to_vec();
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let completion = complete_line(&words, &cwd);
    let mut out = String::new();
    for c in &completion.candidates {
        out.push_str(&c.value);
        if let Some(d) = c.desc {
            out.push('\t');
            out.push_str(d);
        }
        out.push('\n');
    }
    out.push_str(&format!(":{}\n", completion.directive));
    print!("{out}");
    ExitCode::SUCCESS
}
```

Add a drift-guard unit test for the description table, inside `brain_tests`:

```rust
    #[test]
    fn every_command_has_a_description() {
        for c in crate::COMMANDS {
            assert!(!command_desc(c).is_empty(), "`{c}` needs a description in command_desc");
        }
    }
```

- [ ] **Step 4: Add the dispatch arm and extend the hidden-arm guard in `main.rs`**

Add the arm right after the `completions` arm (`main.rs:59`):

```rust
        Some("completions") => complete::cmd_completions(&args),
        // Hidden: the shell-completion shims call this at runtime. Not in COMMANDS
        // (underscore-prefixed => excluded from did-you-mean + the dispatch guard).
        Some("__complete") => complete::cmd_complete(&args),
```

In `main.rs`'s test module, extend `commands_in_dispatch` (`main.rs:554-557`) to skip `_`-prefixed names too:

```rust
            for lit in rest[..end].split('"').skip(1).step_by(2) {
                // Flags (`--version`, `-h`) and hidden internal subcommands (`__complete`,
                // underscore-prefixed) are not user-facing commands: never suggested, never
                // in `COMMANDS`.
                if !lit.starts_with('-') && !lit.starts_with('_') {
                    out.insert(lit.to_string());
                }
            }
```

Add a case to `the_dispatch_scan_survives_rustfmt_wrapping_and_guards` documenting the convention:

```rust
        // Hidden underscore-prefixed subcommands are excluded (like flags).
        assert!(commands_in_dispatch("Some(\"__complete\") => c(),").is_empty());
```

- [ ] **Step 5: Run unit tests to verify they pass**

Run: `cargo test -p taliesin-server --lib`
Expected: PASS, including `brain_tests`, `every_dispatched_command_is_listed_in_commands`, and `the_dispatch_scan_survives_rustfmt_wrapping_and_guards`.

- [ ] **Step 6: Write the integration smoke test**

Create `crates/server/tests/complete_cli.rs`:

```rust
//! The hidden `__complete` subcommand drives shell completion: it prints candidate lines
//! then a trailing `:<directive>` line. These tests invoke the real binary the way a shim
//! does, so they cover dispatch wiring + the wire protocol end to end.

use std::process::Command;

/// Run `taliesin __complete <words…>` in `cwd`, returning (candidate values, directive).
fn complete(cwd: &std::path::Path, words: &[&str]) -> (Vec<String>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_taliesin"))
        .arg("__complete")
        .args(words)
        .current_dir(cwd)
        .output()
        .expect("run taliesin __complete");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    let mut values = Vec::new();
    let mut directive = String::new();
    for line in text.lines() {
        if let Some(d) = line.strip_prefix(':') {
            directive = d.to_string();
        } else {
            values.push(line.split('\t').next().unwrap_or("").to_string());
        }
    }
    (values, directive)
}

#[test]
fn completes_subcommands_and_suppresses_files() {
    let (values, directive) = complete(std::path::Path::new("."), &[""]);
    assert!(values.contains(&"preview".to_string()), "offers preview: {values:?}");
    assert!(values.contains(&"build".to_string()), "offers build: {values:?}");
    assert_eq!(directive, "4", "NoFileComp for subcommand completion");
}
```

- [ ] **Step 7: Run the integration test**

Run: `cargo test -p taliesin-server --test complete_cli`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/server/src/complete.rs crates/server/src/main.rs crates/server/tests/complete_cli.rs
git commit -m "feat(complete): hidden __complete subcommand + subcommand-name brain"
```

---

### Task 3: Flag-name completion

When the current word starts with `-`, offer the flags valid for the resolved subcommand, with inline descriptions.

**Files:**
- Modify: `crates/server/src/complete.rs`

**Interfaces:**
- Produces: `fn canonical(sub: &str) -> &str` (maps `dev`/`serve` to `preview`); `fn flags_for(sub: &str) -> &'static [(&'static str, bool, &'static str)]` (flag name, takes-a-value, description).

- [ ] **Step 1: Write the failing test**

Add to `brain_tests`:

```rust
    #[test]
    fn dash_completes_subcommand_flags() {
        let got = values(&["preview", "--"]);
        for f in ["--host", "--open", "--no-exec", "--port"] {
            assert!(got.contains(&f.to_string()), "preview offers {f}: {got:?}");
        }
        let got = values(&["build", "--"]);
        for f in ["--out", "--strict", "--bare", "--jobs", "--format"] {
            assert!(got.contains(&f.to_string()), "build offers {f}: {got:?}");
        }
    }

    #[test]
    fn flags_are_offered_through_aliases() {
        // `dev` and `serve` share preview's flags.
        assert!(values(&["dev", "--"]).contains(&"--host".to_string()));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p taliesin-server --lib brain_tests::dash_completes_subcommand_flags`
Expected: FAIL (flags not offered yet; `values(&["preview","--"])` returns empty).

- [ ] **Step 3: Add the flag table and the flag-completion branch**

Add near `command_desc`:

```rust
fn canonical(sub: &str) -> &str {
    match sub {
        "dev" | "serve" => "preview",
        other => other,
    }
}

/// `(flag, takes_a_value, description)` per canonical subcommand. Single source of truth
/// for flag completion; `flag_table_covers_help` guards it against the CLI help text.
fn flags_for(sub: &str) -> &'static [(&'static str, bool, &'static str)] {
    match canonical(sub) {
        "preview" => &[
            ("--host", false, "expose on your LAN + print a phone QR code"),
            ("--open", false, "launch the default browser"),
            ("--no-exec", false, "render code cells as source, never run them"),
            ("--port", true, "port to serve on"),
        ],
        "build" => &[
            ("--out", true, "write a portable folder to <dir>"),
            ("--strict", false, "exit non-zero on a cell error or located warning"),
            ("--bare", false, "emit zero-JS, CSS-only single-doc HTML"),
            ("--jobs", true, "cap parallel page renders"),
            ("--format", true, "machine output format (json)"),
        ],
        "publish" => &[
            ("--project-name", true, "Cloudflare Pages project name"),
            ("--out", true, "output dir"),
            ("--public", false, "deploy un-gated (no passcode)"),
            ("--no-strict", false, "do not fail on located warnings"),
            ("--dry-run", false, "build but skip the deploy"),
            ("--format", true, "machine output format (json)"),
        ],
        "new" => &[
            ("--dir", true, "project root to scaffold into"),
            ("--json", false, "print a json receipt"),
        ],
        "schema" => &[("--out", true, "output dir")],
        "symbols" => &[("--format", true, "human | json")],
        "map" => &[("--format", true, "human | json")],
        "check" => &[("--format", true, "human | json")],
        _ => &[],
    }
}
```

In `complete_line`, after the subcommand branch, insert:

```rust
    let sub = prior.first().map(String::as_str).unwrap_or("");

    // 2. Flag-name completion.
    if cur.starts_with('-') {
        let candidates = flags_for(sub)
            .iter()
            .filter(|(f, _, _)| f.starts_with(cur))
            .map(|(f, _, d)| Candidate::described(*f, d))
            .collect();
        return Completion { candidates, directive: NO_FILE_COMP };
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p taliesin-server --lib brain_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/complete.rs
git commit -m "feat(complete): per-subcommand flag-name completion with descriptions"
```

---

### Task 4: Enumerated positionals + flag-value handling

Complete the fixed value sets: `new <kind>`, `completions <shell>`, and `--format <value>`. Also make a value-taking flag before the cursor consume its slot (so its value is not mistaken for a positional, and non-enumerated flag values fall back to plain file completion).

**Files:**
- Modify: `crates/server/src/complete.rs`

**Interfaces:**
- Produces: `fn enumerated(cur: &str, values: &[&'static str]) -> Completion`; `fn positionals_seen(sub: &str, rest: &[String]) -> usize` (used by Task 5).

- [ ] **Step 1: Write the failing test**

Add to `brain_tests`:

```rust
    #[test]
    fn enumerated_positionals() {
        assert_eq!(values(&["new", ""]), vec!["post", "page", "deck", "paper"]
            .into_iter().map(String::from).collect::<Vec<_>>());
        assert_eq!(values(&["completions", ""]), vec!["bash", "zsh", "fish", "powershell"]
            .into_iter().map(String::from).collect::<Vec<_>>());
    }

    #[test]
    fn format_value_completion() {
        assert_eq!(values(&["build", "--format", ""]), vec!["json".to_string()]);
        let human_json: Vec<String> = ["human", "json"].into_iter().map(String::from).collect();
        assert_eq!(values(&["check", "--format", ""]), human_json);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p taliesin-server --lib brain_tests::enumerated_positionals`
Expected: FAIL (returns empty).

- [ ] **Step 3: Implement enumerated + flag-value handling**

Add helpers:

```rust
fn enumerated(cur: &str, values: &[&'static str]) -> Completion {
    let candidates = values
        .iter()
        .filter(|v| v.starts_with(cur))
        .map(|v| Candidate::plain(*v))
        .collect();
    Completion { candidates, directive: NO_FILE_COMP }
}

/// Count the positional (non-flag, non-flag-value) args already sitting between the
/// subcommand and the cursor.
fn positionals_seen(sub: &str, rest: &[String]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < rest.len() {
        let tok = rest[i].as_str();
        if tok.starts_with('-') {
            // A value-taking flag (`--out dir`) also consumes the next token, unless it
            // was written `--out=dir`.
            if !tok.contains('=')
                && flags_for(sub).iter().any(|(f, takes, _)| *f == tok && *takes)
            {
                i += 1;
            }
        } else {
            n += 1;
        }
        i += 1;
    }
    n
}
```

In `complete_line`, after the flag-name branch and before the fallback, insert:

```rust
    // 3. Value of the flag immediately before the cursor.
    if let Some(prev) = prior.last() {
        if prev == "--format" {
            let vals: &[&str] =
                if canonical(sub) == "build" { &["json"] } else { &["human", "json"] };
            return enumerated(cur, vals);
        }
        // Other value-taking flags (--out/--dir/--jobs/--port/--project-name): let the
        // shell complete the value (a dir, a number, a name); nothing smart to add.
        if flags_for(sub).iter().any(|(f, takes, _)| *f == prev.as_str() && *takes) {
            return Completion { candidates: Vec::new(), directive: 0 };
        }
    }

    // 4. Enumerated first positionals.
    if prior == ["new"] {
        return enumerated(cur, &["post", "page", "deck", "paper"]);
    }
    if prior == ["completions"] {
        return enumerated(cur, &["bash", "zsh", "fish", "powershell"]);
    }
```

Note: `prior == ["new"]` compares a `&[String]` to a `[&str; 1]`. Use `prior.len() == 1 && prior[0] == "new"` if the array comparison does not typecheck.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p taliesin-server --lib brain_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/complete.rs
git commit -m "feat(complete): enumerated positionals (new/completions/--format) + flag-value slots"
```

---

### Task 5: Path filtering brain (`.tmd` files + pruned dirs, site roots first)

The core of the feature: complete a path positional with only `.tmd` files and directories that contain a `.tmd` anywhere below (ignore-set, depth-capped), site/book roots first, dirs suffixed `/`.

**Files:**
- Modify: `crates/server/src/complete.rs`
- Modify: `crates/server/tests/complete_cli.rs`

**Interfaces:**
- Produces: `enum PathKind { File, FileOrDir, Dir }` with `fn offers_files(&self) -> bool`; `fn positional_kind(sub: &str) -> Option<PathKind>`; `fn complete_paths(cur, cwd, kind) -> Completion`; `fn dir_contains_tmd(dir: &Path, depth: usize) -> bool`.

- [ ] **Step 1: Write the failing unit test (builds a temp tree)**

Add to `brain_tests` (uses the repo's temp-dir convention: `std::env::temp_dir()` + a unique suffix, with cleanup):

```rust
    fn fixture(tag: &str) -> std::path::PathBuf {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("tali-complete-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("site")).unwrap();
        fs::create_dir_all(dir.join("nested/deep")).unwrap();
        fs::create_dir_all(dir.join("empty")).unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::write(dir.join("index.tmd"), "# hi\n").unwrap();
        fs::write(dir.join("site/_site.yml"), "title: S\n").unwrap();
        fs::write(dir.join("site/page.tmd"), "# p\n").unwrap();
        fs::write(dir.join("nested/deep/buried.tmd"), "# b\n").unwrap();
        fs::write(dir.join("target/decoy.tmd"), "# decoy\n").unwrap();
        dir
    }

    fn path_values(dir: &std::path::Path, words: &[&str]) -> Vec<String> {
        let owned: Vec<String> = words.iter().map(|s| s.to_string()).collect();
        complete_line(&owned, dir).candidates.into_iter().map(|c| c.value).collect()
    }

    #[test]
    fn path_completion_filters_and_orders() {
        let dir = fixture("filter");
        let got = path_values(&dir, &["preview", ""]);
        assert!(got.contains(&"index.tmd".to_string()), "offers .tmd file: {got:?}");
        assert!(got.contains(&"site/".to_string()), "offers site root: {got:?}");
        assert!(got.contains(&"nested/".to_string()), "offers dir with a buried .tmd: {got:?}");
        assert!(!got.iter().any(|v| v.starts_with("empty")), "hides .tmd-free dir: {got:?}");
        assert!(!got.iter().any(|v| v.starts_with("target")), "hides ignore-set dir: {got:?}");
        // Site/book root is ordered before plain dirs and files.
        let site = got.iter().position(|v| v == "site/").unwrap();
        let nested = got.iter().position(|v| v == "nested/").unwrap();
        assert!(site < nested, "site root first: {got:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dir_only_subcommands_hide_tmd_files() {
        let dir = fixture("dironly");
        let got = path_values(&dir, &["map", ""]);
        assert!(!got.contains(&"index.tmd".to_string()), "map offers no .tmd file: {got:?}");
        assert!(got.contains(&"site/".to_string()), "map still offers a site dir: {got:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_directive_sets_nospace_when_dirs_present() {
        let dir = fixture("directive");
        let d = complete_line(&["preview".to_string(), "".to_string()], &dir).directive;
        assert_eq!(d, NO_SPACE | NO_FILE_COMP, "dirs present => NoSpace|NoFileComp");
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p taliesin-server --lib brain_tests::path_completion_filters_and_orders`
Expected: FAIL (path positional not handled; returns empty / directive 0).

- [ ] **Step 3: Implement path completion**

Add:

```rust
const IGNORE_DIRS: &[&str] = &[".git", "target", "node_modules", "_site", "_freeze"];
const TMD_WALK_DEPTH: usize = 6;

enum PathKind {
    File,
    FileOrDir,
    Dir,
}

impl PathKind {
    /// Whether `.tmd` files are offered as terminal candidates. Directories are always
    /// offered regardless, so a nested target stays reachable by descending.
    fn offers_files(&self) -> bool {
        matches!(self, PathKind::File | PathKind::FileOrDir)
    }
}

/// The first positional's path type, or `None` when the subcommand takes no path.
fn positional_kind(sub: &str) -> Option<PathKind> {
    match canonical(sub) {
        "preview" | "build" | "check" => Some(PathKind::FileOrDir),
        "render" | "read" | "blocks" | "symbols" => Some(PathKind::File),
        "map" | "publish" | "init" => Some(PathKind::Dir),
        _ => None,
    }
}

/// Complete a path word: `.tmd` files (when `kind.offers_files()`) plus directories that
/// contain a `.tmd` anywhere below, site/book roots first, dirs suffixed `/`.
fn complete_paths(cur: &str, cwd: &Path, kind: &PathKind) -> Completion {
    // Split into (dir_part incl. trailing slash, leaf prefix).
    let (dir_part, leaf) = match cur.rfind('/') {
        Some(i) => (&cur[..=i], &cur[i + 1..]),
        None => ("", cur),
    };
    let base = cwd.join(dir_part);
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Completion { candidates: Vec::new(), directive: NO_FILE_COMP };
    };

    let mut site_dirs: Vec<Candidate> = Vec::new();
    let mut dirs: Vec<Candidate> = Vec::new();
    let mut files: Vec<Candidate> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(leaf) {
            continue;
        }
        // Hide dotfiles unless the user is explicitly typing a dot prefix.
        if name.starts_with('.') && !leaf.starts_with('.') {
            continue;
        }
        let Ok(ty) = entry.file_type() else { continue };
        if ty.is_dir() {
            if IGNORE_DIRS.contains(&name.as_str()) {
                continue;
            }
            let path = entry.path();
            if !dir_contains_tmd(&path, TMD_WALK_DEPTH) {
                continue;
            }
            let value = format!("{dir_part}{name}/");
            if path.join("_site.yml").exists() {
                site_dirs.push(Candidate::described(value, "site / book root"));
            } else {
                dirs.push(Candidate::plain(value));
            }
        } else if kind.offers_files() && ty.is_file() && name.ends_with(".tmd") {
            files.push(Candidate::plain(format!("{dir_part}{name}")));
        }
    }

    site_dirs.sort_by(|a, b| a.value.cmp(&b.value));
    dirs.sort_by(|a, b| a.value.cmp(&b.value));
    files.sort_by(|a, b| a.value.cmp(&b.value));

    let mut candidates = site_dirs;
    candidates.append(&mut dirs);
    candidates.append(&mut files);

    let mut directive = NO_FILE_COMP;
    if candidates.iter().any(|c| c.value.ends_with('/')) {
        directive |= NO_SPACE;
    }
    Completion { candidates, directive }
}

/// True if `dir` holds a `.tmd` file within `depth` levels, skipping the build/vcs dirs in
/// `IGNORE_DIRS` and dot-dirs. Short-circuits on the first hit. `file_type()` does not
/// follow symlinks, so symlinked dirs are skipped and cannot cause a cycle.
fn dir_contains_tmd(dir: &Path, depth: usize) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let Ok(ty) = entry.file_type() else { continue };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if ty.is_file() {
            if name.ends_with(".tmd") {
                return true;
            }
        } else if ty.is_dir()
            && depth > 0
            && !name.starts_with('.')
            && !IGNORE_DIRS.contains(&name.as_ref())
        {
            subdirs.push(entry.path());
        }
    }
    subdirs.into_iter().any(|d| dir_contains_tmd(&d, depth - 1))
}
```

In `complete_line`, replace the final fallback with the path branch + fallback:

```rust
    // 5. First path positional (only the first; later positionals fall through to the
    //    shell's own file completion).
    if let Some(kind) = positional_kind(sub) {
        if positionals_seen(sub, &prior[1..]) == 0 {
            return complete_paths(cur, _cwd, &kind);
        }
    }

    // 6. Nothing special: let the shell do its normal file completion.
    Completion { candidates: Vec::new(), directive: 0 }
```

Rename the `complete_line` param from `_cwd` to `cwd` now that it is used.

- [ ] **Step 4: Run unit tests to verify pass**

Run: `cargo test -p taliesin-server --lib brain_tests`
Expected: PASS.

- [ ] **Step 5: Add an end-to-end integration case**

Append to `crates/server/tests/complete_cli.rs` a test that builds the same tree and drives the real binary with `.current_dir()`:

```rust
#[test]
fn path_completion_end_to_end_filters_dirs() {
    use std::fs;
    let dir = std::env::temp_dir().join(format!("tali-complete-e2e-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("site")).unwrap();
    fs::create_dir_all(dir.join("target")).unwrap();
    fs::write(dir.join("index.tmd"), "# hi\n").unwrap();
    fs::write(dir.join("site/_site.yml"), "title: S\n").unwrap();
    fs::write(dir.join("site/page.tmd"), "# p\n").unwrap();
    fs::write(dir.join("target/decoy.tmd"), "# d\n").unwrap();

    let (values, directive) = complete(&dir, &["preview", ""]);
    assert!(values.contains(&"index.tmd".to_string()), "offers .tmd: {values:?}");
    assert!(values.contains(&"site/".to_string()), "offers site dir: {values:?}");
    assert!(!values.iter().any(|v| v.starts_with("target")), "hides target/: {values:?}");
    assert_eq!(directive, "5", "NoSpace|NoFileComp when dirs present");
    let _ = fs::remove_dir_all(&dir);
}
```

- [ ] **Step 6: Run the integration test**

Run: `cargo test -p taliesin-server --test complete_cli`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/complete.rs crates/server/tests/complete_cli.rs
git commit -m "feat(complete): .tmd-aware path filtering (pruned dirs, site roots first)"
```

---

### Task 6: Rewrite shims as dynamic + add PowerShell + update help/script table

Replace the three static shim bodies with dynamic ones that call `__complete`, add a PowerShell shim, extend `completions_script`, and update the help text. Adapt the shim tests (the `@COMMANDS@` premise is gone: the command list now comes from the brain at runtime).

**Files:**
- Modify: `crates/server/src/complete.rs` (shim consts, `completions_script`, `cmd_completions` usage string, `completions_tests`)
- Modify: `crates/server/src/main.rs` (`usage()` line + `subcommand_help("completions")`)

- [ ] **Step 1: Replace the shim consts**

Replace `BASH_COMPLETIONS`, `ZSH_COMPLETIONS`, `FISH_COMPLETIONS` and add `POWERSHELL_COMPLETIONS`. Each registers both `taliesin` and `tali`, and calls `taliesin __complete`.

> **Shim mechanics are validated live in Task 9.** These bodies are correct in structure and protocol (they relay `__complete`'s output), but the exact shell primitives for showing descriptions and suppressing the trailing space (`compadd -d`/`-S` in zsh, `compopt` in bash) are the fiddly part of shell completion and cannot be perfected without a live shell. Expect to adjust them during the Task 9 zsh/bash smoke test; the brain (unit-tested) does not change.

```rust
const BASH_COMPLETIONS: &str = r#"# taliesin bash completion (dynamic).
# Install: taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin
_taliesin() {
    local IFS=$'\n' line directive=0
    local words=("${COMP_WORDS[@]:1:COMP_CWORD}")
    COMPREPLY=()
    for line in $(taliesin __complete "${words[@]}" 2>/dev/null); do
        if [[ $line == :* ]]; then
            directive=${line#:}
            continue
        fi
        COMPREPLY+=("${line%%$'\t'*}")
    done
    (( directive & 1 )) && compopt -o nospace 2>/dev/null
    (( directive & 4 )) || compopt -o default 2>/dev/null
}
complete -F _taliesin taliesin tali
"#;

const ZSH_COMPLETIONS: &str = r#"#compdef taliesin tali
# taliesin zsh completion (dynamic).
# Install (into a dir on $fpath, then run compinit):
#   taliesin completions zsh > "${fpath[1]}/_taliesin"
_taliesin() {
    local -a args reply_dirs reply_files
    local line directive=0
    args=("${(@)words[2,CURRENT]}")
    local out
    out="$(taliesin __complete "${args[@]}" 2>/dev/null)"
    for line in ${(f)out}; do
        if [[ $line == :* ]]; then
            directive=${line#:}
            continue
        fi
        local val=${line%%$'\t'*}
        local desc=${line#*$'\t'}
        [[ $desc == $line ]] && desc=""
        if [[ $val == */ ]]; then
            reply_dirs+=("${val}:${desc}")
        else
            reply_files+=("${val}:${desc}")
        fi
    done
    # Directories: no trailing space, so you keep descending.
    for pair in $reply_dirs; do
        compadd -S '' -d "(${pair#*:})" -- "${pair%%:*}"
    done
    for pair in $reply_files; do
        compadd -- "${pair%%:*}"
    done
    if (( ${#reply_dirs} == 0 && ${#reply_files} == 0 )) && (( (directive & 4) == 0 )); then
        _files
    fi
}
if [ "${funcstack[1]}" = "_taliesin" ]; then
    _taliesin "$@"
else
    compdef _taliesin taliesin tali
fi
"#;

const FISH_COMPLETIONS: &str = r#"# taliesin fish completion (dynamic).
# Install: taliesin completions fish > ~/.config/fish/completions/taliesin.fish
function __taliesin_complete
    set -l tokens (commandline -opc) (commandline -ct)
    set -l words $tokens[2..-1]
    taliesin __complete $words 2>/dev/null | while read -l line
        string match -q ':*' -- $line; and continue
        echo $line
    end
end
complete -c taliesin -f -a '(__taliesin_complete)'
complete -c tali -f -a '(__taliesin_complete)'
"#;

const POWERSHELL_COMPLETIONS: &str = r#"# taliesin PowerShell completion (dynamic).
# Install: taliesin completions powershell >> $PROFILE
Register-ArgumentCompleter -Native -CommandName @('taliesin','tali') -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $elements = $commandAst.CommandElements
    $words = @()
    for ($i = 1; $i -lt $elements.Count; $i++) { $words += $elements[$i].ToString() }
    if ($wordToComplete -eq '' -and ($words.Count -eq 0 -or $words[-1] -ne '')) { $words += '' }
    (& taliesin __complete @words 2>$null) | ForEach-Object {
        if ($_ -like ':*') { return }
        $parts = $_ -split "`t", 2
        $value = $parts[0]
        $desc = if ($parts.Count -gt 1) { $parts[1] } else { $parts[0] }
        [System.Management.Automation.CompletionResult]::new($value, $value, 'ParameterValue', $desc)
    }
}
"#;
```

- [ ] **Step 2: Extend `completions_script` (no more `@COMMANDS@` replace)**

```rust
pub(crate) fn completions_script(shell: &str) -> Option<String> {
    let script = match shell {
        "bash" => BASH_COMPLETIONS,
        "zsh" => ZSH_COMPLETIONS,
        "fish" => FISH_COMPLETIONS,
        "powershell" => POWERSHELL_COMPLETIONS,
        _ => return None,
    };
    Some(script.to_string())
}
```

Update the doc comment above it (the `@COMMANDS@` explanation is obsolete):

```rust
/// The completion script for `shell`, or `None` for an unsupported one. Each script is a
/// thin shim that relays the command line to `taliesin __complete` and feeds the result
/// back to the shell, so the completion logic lives in one place (`complete_line`) and
/// cannot drift between shells.
```

Update the usage string in `cmd_completions`:

```rust
            log::error("usage: taliesin completions <bash|zsh|fish|powershell>");
```

- [ ] **Step 3: Adapt `completions_tests`**

Replace the two shell-set tests. The command-list-drift concern now lives in the brain (`empty_word_completes_all_subcommands` already asserts it), so `every_shell_script_offers_exactly_the_dispatched_command_list` is deleted and replaced by a "each shim delegates to `__complete`" check.

```rust
    #[test]
    fn generates_a_script_for_each_supported_shell_and_nothing_else() {
        for shell in ["bash", "zsh", "fish", "powershell"] {
            let script = completions_script(shell)
                .unwrap_or_else(|| panic!("a `{shell}` completion script"));
            assert!(!script.trim().is_empty(), "`{shell}` script is non-empty");
            assert!(script.contains("taliesin"), "`{shell}` names the binary: {script}");
            // Every shim delegates to the one brain rather than hardcoding logic.
            assert!(script.contains("__complete"), "`{shell}` calls __complete: {script}");
            // Both the canonical name and the `tali` alias get completion. Use a per-shell
            // marker: a bare `contains("tali")` is vacuously true (it is inside "taliesin").
            let alias_marker = match shell {
                "bash" | "zsh" => "taliesin tali",
                "fish" => "-c tali",
                "powershell" => "'tali'",
                _ => unreachable!(),
            };
            assert!(
                script.contains(alias_marker),
                "`{shell}` registers the tali alias ({alias_marker:?}): {script}"
            );
        }
        assert!(
            completions_script("tcsh").is_none(),
            "an unsupported shell yields no script (so the CLI errors, not emits junk)"
        );
    }

    #[test]
    fn an_unknown_or_empty_shell_yields_no_script() {
        assert!(completions_script("").is_none());
        assert!(completions_script("elvish").is_none());
        assert!(completions_script("BASH").is_none(), "shell names are case-sensitive");
    }
```

- [ ] **Step 4: Update `main.rs` help text**

`usage()` line (`main.rs:191`): change to include powershell and mention filtering. Keep it to the two-line format already used:

```rust
    println!(
        "  completions <bash|zsh|fish|powershell>  print a shell completion script"
    );
    println!(
        "                             (subcommand + flag + .tmd-aware path completion)"
    );
```

`subcommand_help("completions")` block (`main.rs:398`): replace with:

```rust
        "completions" => {
            "taliesin completions <bash|zsh|fish|powershell>\n\
             \n\
             Print a shell completion script to stdout. The script is a thin shim that\n\
             asks the running binary for candidates, so Tab offers subcommands, flags, and\n\
             only .tmd files plus directories that contain one (site/book roots first).\n\
             \n\
             Install:\n\
             \x20 bash        taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin\n\
             \x20 zsh         taliesin completions zsh  > \"${fpath[1]}/_taliesin\"   # then: compinit\n\
             \x20 fish        taliesin completions fish > ~/.config/fish/completions/taliesin.fish\n\
             \x20 powershell  taliesin completions powershell >> $PROFILE\n\
             \n\
             Example:\n\
             \x20 taliesin completions zsh\n"
        }
```

- [ ] **Step 5: Run tests + eyeball a generated script**

Run: `cargo test -p taliesin-server`
Expected: PASS.

Run: `cargo run -p taliesin-server -- completions zsh`
Expected: prints the zsh shim containing `#compdef taliesin tali`, `__complete`, and `compadd`.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/complete.rs crates/server/src/main.rs
git commit -m "feat(complete): dynamic shims for zsh/bash/fish/powershell calling __complete"
```

---

### Task 7: Flag-table drift guard

Guard `flags_for` against the CLI help text so a flag added to a subcommand's help but forgotten in the table is caught, mirroring `env_help_lists_every_runtime_env_var`.

**Files:**
- Modify: `crates/server/src/complete.rs`

- [ ] **Step 1: Write the failing test**

Add to `brain_tests`. It scans every help string the CLI prints for `--flag` tokens and asserts each appears in some subcommand's `flags_for` table. The help strings are reachable via `include_str!("main.rs")` (the same technique `commands_in_dispatch` uses to read dispatch), scanning the file's string literals for flag tokens.

```rust
    #[test]
    fn flag_table_covers_help() {
        // Collect every `--flag` mentioned anywhere in main.rs help text.
        let src = include_str!("main.rs");
        let mut mentioned = std::collections::BTreeSet::new();
        let bytes = src.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] == b'-' && bytes[i + 1] == b'-' {
                let start = i;
                let mut j = i + 2;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                    j += 1;
                }
                let tok = &src[start..j];
                // Skip the global flags that are not per-subcommand table entries.
                if tok.len() > 2 && !["--help", "--version"].contains(&tok) {
                    mentioned.insert(tok.to_string());
                }
                i = j;
            } else {
                i += 1;
            }
        }
        // Every mentioned flag lives in at least one subcommand's table.
        let known: std::collections::BTreeSet<&str> = crate::COMMANDS
            .iter()
            .flat_map(|c| flags_for(c).iter().map(|(f, _, _)| *f))
            .collect();
        for flag in &mentioned {
            assert!(
                known.contains(flag.as_str()),
                "`{flag}` appears in help but is missing from flags_for (add it or the table drifts)"
            );
        }
    }
```

- [ ] **Step 2: Run it**

Run: `cargo test -p taliesin-server --lib brain_tests::flag_table_covers_help`
Expected: PASS (the Task 3 table already covers `--host`/`--out`/`--strict`/`--bare`/`--jobs`/`--format`/`--open`/`--no-exec`/`--port`/`--project-name`/`--public`/`--no-strict`/`--dry-run`/`--dir`/`--json`). If it FAILS, it has found a real gap: add the missing flag to `flags_for`.

- [ ] **Step 3: Commit**

```bash
git add crates/server/src/complete.rs
git commit -m "test(complete): guard the flag table against CLI help drift"
```

---

### Task 8: Docs page + reference wiring

Add a short "Shell completion" page to the User Guide (dogfooded `.tmd`), so the guide documents the feature its `--help` now advertises.

**Files:**
- Create: `docs/guide/reference/shell-completion.tmd`
- Modify: whichever reference index/nav lists the `reference/` pages (inspect `docs/guide/_site.yml` and the existing `docs/guide/reference/` pages to match the established pattern; if pages are auto-discovered, no nav edit is needed).

- [ ] **Step 1: Inspect how reference pages are registered**

Run: `ls docs/guide/reference/ && sed -n '1,80p' docs/guide/_site.yml`
Expected: reveals whether `reference/` pages are listed explicitly in `_site.yml` or auto-discovered. Follow the existing pattern (match an existing page's front matter, e.g. `docs/guide/reference/`'s CLI page).

- [ ] **Step 2: Write the page**

Create `docs/guide/reference/shell-completion.tmd`. Match the front-matter shape of a sibling reference page (title/order/etc.). Body (no em dashes, per house style):

```markdown
# Shell completion

Tab completion for the `taliesin` (and `tali`) command. It offers subcommands, flags,
and, for a path argument, only `.tmd` files plus directories that contain one, with
site and book roots listed first. Directories with no `.tmd` inside (and build folders
like `target/`) are hidden.

All the logic lives in the binary, so every shell stays in sync. Install the script for
your shell once:

- **bash:** `taliesin completions bash > ~/.local/share/bash-completion/completions/taliesin`
- **zsh:** `taliesin completions zsh > "${fpath[1]}/_taliesin"` then run `compinit`
- **fish:** `taliesin completions fish > ~/.config/fish/completions/taliesin.fish`
- **PowerShell:** `taliesin completions powershell >> $PROFILE`

Open a new shell (or re-run `compinit` for zsh) and press Tab after `tali preview`.
```

- [ ] **Step 3: Verify the guide still builds**

Run: `cargo run -p taliesin-server -- build docs/guide --out /tmp/guide-check`
Expected: success, and `/tmp/guide-check` contains a `reference/shell-completion.html` (or the page appears under the reference section).

- [ ] **Step 4: Commit**

```bash
git add docs/guide/reference/shell-completion.tmd docs/guide/_site.yml
git commit -m "docs(guide): shell-completion reference page"
```

(Drop `_site.yml` from the `git add` if Step 1 showed pages are auto-discovered.)

---

### Task 9: Launcher fast-path, live smoke, full verification

Final integration gate: make the machine launcher skip its rebuild check for `__complete`, smoke-test the real shells available in this environment, and run the whole suite. The launcher lives outside the repo, so it is not committed; this task's repo deliverable was Task 8's docs.

**Files:**
- Modify (machine-local, NOT committed): `~/.local/bin/taliesin`

- [ ] **Step 1: Add the launcher fast-path**

Edit `~/.local/bin/taliesin`: right after `set -euo pipefail` and the `REPO`/`BIN` lines, before the `find -newer` rebuild check, add:

```bash
# Completion runs on every keystroke: never pay the rebuild check for it.
if [ "${1:-}" = "__complete" ]; then
    exec "$BIN" "$@"
fi
```

This must come before the `if [ ! -x "$BIN" ] || find …` block. Note the guard assumes `$BIN` already exists (it will, in normal use); if the release binary has never been built, run `tali --version` once first.

- [ ] **Step 2: Build the release binary the launcher runs**

Run: `cargo build --release -p taliesin-server`
Expected: success (the shims call `taliesin`, which is this release binary via the launcher).

- [ ] **Step 3: Live-smoke zsh**

Source the generated zsh shim in a zsh subshell and drive completion programmatically. Run:

```bash
cargo run -p taliesin-server -- completions zsh > /tmp/_taliesin
# Confirm the brain returns filtered candidates from a temp project:
mkdir -p /tmp/tali-smoke/site && printf 'title: S\n' > /tmp/tali-smoke/site/_site.yml \
  && printf '# p\n' > /tmp/tali-smoke/site/page.tmd && printf '# i\n' > /tmp/tali-smoke/index.tmd \
  && mkdir -p /tmp/tali-smoke/target && printf '# d\n' > /tmp/tali-smoke/target/decoy.tmd
(cd /tmp/tali-smoke && "$(git rev-parse --show-toplevel)/target/release/taliesin" __complete preview '')
```

Expected: prints `site/` (with a `site / book root` description after a tab), `index.tmd`, and a trailing `:5`; does NOT print `target`.

- [ ] **Step 4: Live-smoke bash (if available)**

Run: `command -v bash && cargo run -p taliesin-server -- completions bash > /tmp/taliesin.bash && bash -lc 'source /tmp/taliesin.bash && echo sourced-ok'`
Expected: prints `sourced-ok` with no syntax error. (Interactive Tab behavior is confirmed by the `__complete` output in Step 3, which both shims relay verbatim.)

- [ ] **Step 5: Note fish / PowerShell as not-live-verified**

If `command -v fish` / `command -v pwsh` are absent (expected in this sandbox), record in the final report that their shims are structurally tested (Task 6) and protocol-identical to zsh/bash, but were not exercised in a live fish/PowerShell session. The author should confirm on their machine.

- [ ] **Step 6: Full suite + format + client typecheck**

Run: `cargo test -p taliesin-core -p taliesin-server && cargo fmt --check`
Expected: PASS, clean.

Run: `cd web-client && npx -y -p typescript tsc -p jsconfig.json`
Expected: no new errors (this change touches no client JS; run only as a tree-health check).

- [ ] **Step 7: Self-review the diff**

Run: `git diff main...HEAD --stat` and re-read the full diff. Confirm: `__complete` is absent from `COMMANDS`/`usage()`; no new dependency in any `Cargo.toml`; no em dashes in the added help text or docs page; the warm-page eviction code in `serve_site/exec_pool.rs` is untouched.

- [ ] **Step 8: Report**

Summarize what was verified live (zsh + bash) and what was not (fish, PowerShell, the machine-local launcher edit), so the author can finish the shell-specific confirmation.
