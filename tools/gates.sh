#!/usr/bin/env bash
#
# tools/gates.sh — the ONE committed script that runs every gate in this repo.
#
# Why it exists: the gates are healthy but several of them SKIP SILENTLY when an
# interpreter is absent (no Python with ipykernel, no Node). A stranger who clones this
# repo and runs `cargo test` gets a green run that proves far less than it looks like it
# proves, and nothing tells them so. This script refuses to be green unless every gate
# actually ran.
#
# **It needs TWO external runtimes: Python (with ipykernel) and Node.** It needed four
# until 2026-08-08, when the `{r}` cell language and the headless-Chrome test driver were
# both cut; R + IRkernel and a system Chrome are no longer prerequisites of anything here,
# and neither is silently optional — there is nothing left for them to gate.
#
# **TWELVE gates as of 2026-08-13** (`docs/internals --check-only` is the twelfth; it was
# written here as eleven until 2026-08-17, which is this same paragraph's own failure mode
# one more time). It ran eight while
# claiming ten for two waves: the document gate (wave 9) and the composition gate (wave 11)
# were wired into `.githooks/pre-push` and never added here, because neither can skip and so
# neither looked like this script's problem. A gate that is simply ABSENT from the list
# hollows a "runs every gate" claim out just as completely as a gate that skipped, and it
# does it more quietly, because there is no SKIPPED line to read. The cross-check that stops
# the next one lives in `crates/core/tests/gate_script.rs`.
#
# **Take this number from the script's own verdict line, never by incrementing this one.**
#
#   ./tools/gates.sh                  run every gate; a missing prerequisite is a failure
#   ./tools/gates.sh --allow-missing  run what you can; the verdict is INCOMPLETE, exit 2
#
# Exit codes:  0 = every gate ran and passed
#              1 = a gate ran and failed
#              2 = a gate did not run (missing prerequisite under --allow-missing)
#
# Three traps this script exists to not fall into, all of them observed in this repo:
#
#   1. `cmd 2>&1 | tee log; echo $?` reports TEE's exit status, not cmd's — a failing
#      gate reads as exit 0. Every gate below reads ${PIPESTATUS[0]} explicitly, and
#      `set -o pipefail` is belt to that braces.
#   2. `cargo test --lib -p taliesin-server` ERRORS: the server is a *bin* crate, so
#      `--lib` selects no target. The unit tests inside it are reached by a plain
#      `cargo test --workspace` (or `-p taliesin-server --bins`), never by `--lib`.
#   3. A green `cargo test` says nothing about whether the live-kernel/Node cases ran.
#      Each of them is guarded by a TALIESIN_REQUIRE_* variable that turns a missing
#      interpreter into a hard failure — this script sets both, and then additionally
#      asserts by name that each canary test printed `... ok` and that the run reported
#      ZERO ignored tests. "0 ignored" is the proof the gates ran rather than skipped.
#
# `crates/core/tests/gate_script.rs` pins this file against the tree: every
# TALIESIN_REQUIRE_* variable the Rust sources read must be set here, and every canary
# test named here must still exist, and every command `.githooks/pre-push` runs must appear
# here too. Renaming a canary breaks that test, not this script silently.

set -u
set -o pipefail

cd "$(git rev-parse --show-toplevel)" || exit 1

ALLOW_MISSING=0
for arg in "$@"; do
    case "$arg" in
    --allow-missing) ALLOW_MISSING=1 ;;
    -h | --help)
        sed -n '3,20p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *)
        echo "gates: unknown argument '$arg' (see --help)" >&2
        exit 1
        ;;
    esac
done

# The canary tests. Each one is the single test whose `... ok` proves that the
# interpreter behind its gate was really exercised: with the matching TALIESIN_REQUIRE_*
# set, a missing interpreter makes the canary FAIL rather than skip, so "canary passed"
# and "gate ran" are the same statement. Names are pinned by gate_script.rs.
CANARY_KERNEL="kernel_executes_state_errors_and_interrupts_runaway_cell"
CANARY_NODE="only_a_textual_sink_becomes_a_live_region"

PY="${TALIESIN_PYTHON:-python3}"

LOG_DIR="$(mktemp -d "${TMPDIR:-/tmp}/taliesin-gates.XXXXXX")"
PASSED=()
FAILED=()
SKIPPED=()

if [ -t 1 ]; then
    C_BOLD=$'\033[1m' C_RED=$'\033[31m' C_GREEN=$'\033[32m' C_YELLOW=$'\033[33m' C_OFF=$'\033[0m'
else
    C_BOLD='' C_RED='' C_GREEN='' C_YELLOW='' C_OFF=''
fi
bold() { printf '%s%s%s\n' "$C_BOLD" "$*" "$C_OFF"; }
red() { printf '%s%s%s\n' "$C_RED" "$*" "$C_OFF"; }
green() { printf '%s%s%s\n' "$C_GREEN" "$*" "$C_OFF"; }
yellow() { printf '%s%s%s\n' "$C_YELLOW" "$*" "$C_OFF"; }

# ---------------------------------------------------------------------------
# Preflight. Every prerequisite is checked BEFORE the first (slow) gate, so a
# stranger learns everything that is missing in one go instead of one 6-minute
# build at a time. A missing prerequisite is a HARD FAILURE by default: the whole
# point of this script is that a green run means every gate ran.
# ---------------------------------------------------------------------------
MISSING=()
# Counted separately from the rest, because only these two decide whether the workspace
# suite can be run with its REQUIRE gates armed. A missing `cargo-deny` must not cost you
# the test gate as well.
MISSING_INTERPRETERS=0

have() { command -v "$1" >/dev/null 2>&1; }

require() { # require <status of the preceding check> <label> <install hint> [interpreter]
    if [ "$1" -ne 0 ]; then
        MISSING+=("$2"$'\n      '"$3")
        if [ "${4:-}" = interpreter ]; then
            MISSING_INTERPRETERS=$((MISSING_INTERPRETERS + 1))
        fi
    fi
}

have cargo
require $? "cargo (the Rust toolchain)" "install rustup: https://rustup.rs"

cargo fmt --version >/dev/null 2>&1
require $? "rustfmt" "rustup component add rustfmt"

cargo clippy --version >/dev/null 2>&1
require $? "clippy" "rustup component add clippy"

have "$PY" && "$PY" -c 'import ipykernel' >/dev/null 2>&1
require $? "a Python with ipykernel (TALIESIN_PYTHON=$PY)" \
    "$PY -m pip install ipykernel  — or point TALIESIN_PYTHON at one that has it" interpreter

have node
require $? "node" "install Node 20+ (https://nodejs.org)" interpreter

have npx
require $? "npx" "ships with Node 20+"

have npm
require $? "npm" "ships with Node 20+"

have cargo-audit
require $? "cargo-audit" "cargo install cargo-audit --locked"

have cargo-deny
require $? "cargo-deny" "cargo install cargo-deny --locked"

if [ ${#MISSING[@]} -gt 0 ]; then
    red "gates: ${#MISSING[@]} prerequisite(s) missing"
    for m in "${MISSING[@]}"; do echo "    - $m"; done
    echo
    if [ "$ALLOW_MISSING" -eq 0 ]; then
        red "Refusing to run: a partial run would look green while whole gates never ran."
        echo "Install the above, or re-run with --allow-missing to get an INCOMPLETE verdict."
        exit 2
    fi
    yellow "--allow-missing: continuing, but this run CANNOT certify the tree."
    echo
fi

# ---------------------------------------------------------------------------
# Gate runner. `${PIPESTATUS[0]}` is the load-bearing line (trap 1 above).
# ---------------------------------------------------------------------------
run_gate() { # run_gate <name> <logfile> <cmd...>
    local name="$1" log="$LOG_DIR/$2"
    shift 2
    bold "── $name"
    "$@" 2>&1 | tee "$log"
    local rc=${PIPESTATUS[0]}
    if [ "$rc" -eq 0 ]; then
        green "   ok  $name"
        PASSED+=("$name")
    else
        red "   FAIL  $name (exit $rc, log: $log)"
        FAILED+=("$name")
    fi
    return "$rc"
}

skip_gate() { # skip_gate <name> <why>
    yellow "── $1: SKIPPED ($2)"
    SKIPPED+=("$1 — $2")
}

fail_gate() { # fail_gate <name> <why>
    red "   FAIL  $1: $2"
    FAILED+=("$1 — $2")
}

# ---------------------------------------------------------------------------
# 1. Format
# ---------------------------------------------------------------------------
run_gate "cargo fmt --check" fmt.log cargo fmt --all -- --check

# ---------------------------------------------------------------------------
# 2. Clippy
# ---------------------------------------------------------------------------
run_gate "cargo clippy -D warnings" clippy.log \
    cargo clippy --workspace --all-targets -- -D warnings

# ---------------------------------------------------------------------------
# 3. The workspace suite, with both interpreter gates armed.
#
# --test-threads=1 because several tests own process-global state (the cell-timeout
# OnceLock, the kernel pool), and a raced run is how this suite produces both flakes and
# vacuous passes.
#
# There is no feature flag to pass any more. Until 2026-08-08 this line carried
# `--features taliesin-server/headless-js`, because the browser test driver was off by
# default and the chrome canary would otherwise have gone missing. Both the driver and the
# test binary behind it are gone, so the default-feature build IS the whole workspace.
# ---------------------------------------------------------------------------
TEST_NAME="cargo test --workspace (both gates)"
if [ "$MISSING_INTERPRETERS" -gt 0 ] && [ "$ALLOW_MISSING" -eq 1 ]; then
    # Arming a REQUIRE_* gate whose interpreter is absent turns the canary into a
    # failure, which would report as "a gate failed" when the truth is "a gate could
    # not run". Keep those two verdicts distinct.
    skip_gate "$TEST_NAME" "an interpreter is missing; the two REQUIRE gates cannot be armed"
else
    TALIESIN_PYTHON="$PY" \
        TALIESIN_REQUIRE_KERNEL=1 \
        TALIESIN_REQUIRE_NODE=1 \
        run_gate "$TEST_NAME" test.log \
        cargo test --workspace -- --test-threads=1
    test_rc=$?

    # Only assert on the output when cargo itself succeeded: a build failure produces a
    # log with no test lines at all, and reporting two missing canaries on top of it
    # buries the actual error.
    if [ "$test_rc" -eq 0 ]; then
        log="$LOG_DIR/test.log"

        # (a) Zero ignored. An `#[ignore]` (or a `--skip`) is a gate that did not run,
        #     and cargo reports it inside a PASSING summary line, so it never reaches
        #     the exit code. Sum every binary's count rather than trusting the last one.
        ignored=$(grep -E '^test result:' "$log" | grep -Eo '[0-9]+ ignored' |
            awk '{ s += $1 } END { print s + 0 }')
        if [ "$ignored" -ne 0 ]; then
            fail_gate "$TEST_NAME" "$ignored ignored test(s) — an ignored test is a gate that did not run"
            grep -E '\.\.\. ignored' "$log" | sed 's/^/       /'
        fi

        # (b) Each canary printed `... ok`. This catches the case the REQUIRE_* vars
        #     cannot: a canary that was RENAMED or deleted, which silently removes the
        #     only proof that its interpreter was exercised.
        for pair in \
            "python kernel:$CANARY_KERNEL" \
            "node:$CANARY_NODE"; do
            what="${pair%%:*}"
            canary="${pair#*:}"
            if ! grep -Eq "^test [A-Za-z0-9_:]*${canary} \.\.\. ok$" "$log"; then
                fail_gate "$TEST_NAME" \
                    "the $what canary \`$canary\` did not report ok — it was renamed, filtered out, or it failed"
            fi
        done
    fi
fi

# ---------------------------------------------------------------------------
# 4-5. The two tsc type-checks. Both carry `// @ts-check` / strict and have
#      regressed to errors before; nothing else gates them.
# ---------------------------------------------------------------------------
# `env -C` would be shorter but does not exist on macOS; a subshell `cd` is portable.
tsc_in() { (cd "$1" && npx -y -p typescript tsc -p jsconfig.json); }

tsc_gate() { # tsc_gate <label> <dir> <logfile>
    if [ "$ALLOW_MISSING" -eq 1 ] && ! have npx; then
        skip_gate "$1" "npx unavailable"
        return
    fi
    run_gate "$1" "$3" tsc_in "$2"
}
tsc_gate "tsc: web-client" web-client tsc-client.log
tsc_gate "tsc: bundled assets JS" crates/core/assets/js tsc-assets.log

# ---------------------------------------------------------------------------
# 6. The VS Code companion: build, type-check, and the OFFLINE TextMate grammar
#    tokenization test (which needs the base grammars fetched once).
# ---------------------------------------------------------------------------
if [ "$ALLOW_MISSING" -eq 1 ] && ! have npm; then
    skip_gate "VS Code companion" "npm unavailable"
else
    (
        cd editor/vscode &&
            npm ci &&
            npm run build &&
            npx tsc -p . --noEmit &&
            node scripts/ensure-vscode.cjs &&
            npm test
    ) >"$LOG_DIR/vscode.log" 2>&1
    rc=$?
    bold "── VS Code companion (build + tsc + grammar test)"
    if [ "$rc" -eq 0 ]; then
        green "   ok  VS Code companion"
        PASSED+=("VS Code companion")
    else
        red "   FAIL  VS Code companion (exit $rc, log: $LOG_DIR/vscode.log)"
        tail -30 "$LOG_DIR/vscode.log" | sed 's/^/       /'
        FAILED+=("VS Code companion")
    fi
fi

# ---------------------------------------------------------------------------
# 7-8. Dependency advisories and dependency POLICY (deny.toml: redistributable
#      licences only, no unknown registries or git sources).
# ---------------------------------------------------------------------------
if [ "$ALLOW_MISSING" -eq 1 ] && ! have cargo-audit; then
    skip_gate "cargo audit" "cargo-audit not installed"
else
    run_gate "cargo audit" audit.log cargo audit
fi
if [ "$ALLOW_MISSING" -eq 1 ] && ! have cargo-deny; then
    skip_gate "cargo deny check" "cargo-deny not installed"
else
    run_gate "cargo deny check" deny.log cargo deny check
fi

# ---------------------------------------------------------------------------
# 9-10. The two DOCUMENT gates: this project's own manual, and the composed deploy.
#
# Neither needs an interpreter, which is the whole reason they went missing here. A
# gate that cannot skip has nothing to prove to this script, so wave 9 wired the
# document gate and wave 11 the composition gate straight into `.githooks/pre-push`
# and neither was added here — after which this script printed PASSED while running
# 8 of the repo's 10 enforced gates, and CLAUDE.md told every session to trust it.
# That is the same hollowing-out the header describes, arriving from the opposite
# direction: not a gate that skipped, but a gate that was never listed.
# `crates/core/tests/gate_script.rs` now compares the two lists on every run.
#
# The hook keeps running both. It is the only gate that runs automatically and this
# script is manual, so the two are a pair, not a move.
#
# DEBUG profile on purpose: the clippy and test gates above have already built the
# workspace, so these cost a link rather than a second full build (the hook records
# the same reasoning at its step 4). `--check-only` renders in memory and writes
# nothing; `--no-exec` is what keeps both of them kernel-free.
# ---------------------------------------------------------------------------
# BOTH books. Wave 9 wired the guide and covered one of two already-separate books;
# `gate_script.rs` derives the list from the tree, so a third book cannot slip through.
run_gate "build docs/guide --check-only" docs-guide.log \
    cargo run -q -p taliesin-server -- build docs/guide --check-only --no-exec

run_gate "build docs/internals --check-only" docs-internals.log \
    cargo run -q -p taliesin-server -- build docs/internals --check-only --no-exec

run_gate "tools/publish.sh --check" publish.log ./tools/publish.sh --check

# ---------------------------------------------------------------------------
# 11. The published census still reproduces.
#
# `README.md` and `docs/guide/using/choosing.tmd` open with "measured rather than asserted"
# and then hand the reader the command: `python3 tools/portability-census.py`. That makes a
# mismatch self-refuting rather than merely stale — the one claim in the read set where the
# instrument is in the reader's hands.
#
# It has now rotted TWICE. The script was committed on 2026-08-03 precisely as the remedy
# for the first rot, and choosing.tmd still carries the footnote saying so; six days later
# eight cut waves had removed ~40% of the corpus and every figure was wrong again, by
# 133/11,534/7.1% published against 82/7157/7.0% measured. A doc-comment is not a control.
#
# It is gated where the build times are not because it is the only figure here that is
# MACHINE-INDEPENDENT: a deterministic sub-second pass over tracked files. Wall clocks and
# binary sizes vary by machine and are labelled indicative; gating those would gate the box.
#
# No new prerequisite: this script already hard-requires Python, so no TALIESIN_REQUIRE_*
# appears and `gate_script.rs::the_gate_script_arms_every_require_gate_in_the_tree` is
# untouched. Not added to `.githooks/pre-push` either —
# `every_pre_push_command_is_also_run_by_the_gate_script` constrains hook ⊆ script, not the
# reverse.
# ---------------------------------------------------------------------------
run_gate "portability census --verify" census.log python3 tools/portability-census.py --verify

# ---------------------------------------------------------------------------
# Verdict
# ---------------------------------------------------------------------------
echo
bold "════ gates ════"
for p in "${PASSED[@]:-}"; do [ -n "$p" ] && green "  pass     $p"; done
for s in "${SKIPPED[@]:-}"; do [ -n "$s" ] && yellow "  SKIPPED  $s"; done
for f in "${FAILED[@]:-}"; do [ -n "$f" ] && red "  FAIL     $f"; done
echo "logs: $LOG_DIR"
echo

if [ ${#FAILED[@]} -gt 0 ]; then
    red "FAILED — ${#FAILED[@]} gate(s) failed."
    exit 1
fi
if [ ${#SKIPPED[@]} -gt 0 ]; then
    yellow "INCOMPLETE — ${#SKIPPED[@]} gate(s) never ran, so this run certifies nothing."
    yellow "Install the missing prerequisites and re-run without --allow-missing."
    exit 2
fi
# The count is part of the verdict, not decoration: this script ran 8 gates while
# claiming 10 for two waves, and a bare "every gate passed" is exactly as reassuring at
# either number. Read it against the eleven stanzas above.
green "PASSED — every gate ran and passed (${#PASSED[@]} gates)."
exit 0
