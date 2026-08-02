#!/usr/bin/env bash
#
# tools/gates.sh — the ONE committed script that runs every gate in this repo.
#
# Why it exists: the gates are healthy but several of them SKIP SILENTLY when an
# interpreter is absent (no Python with ipykernel, no R with IRkernel, no Node, no
# Chrome). A stranger who clones this repo and runs `cargo test` gets a green run that
# proves far less than it looks like it proves, and nothing tells them so. This script
# refuses to be green unless every gate actually ran.
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
#   3. A green `cargo test` says nothing about whether the live-kernel/Node/Chrome cases
#      ran. Each of them is guarded by a TALIESIN_REQUIRE_* variable that turns a missing
#      interpreter into a hard failure — this script sets all four, and then additionally
#      asserts by name that each canary test printed `... ok` and that the run reported
#      ZERO ignored tests. "0 ignored" is the proof the gates ran rather than skipped.
#
# `crates/core/tests/gate_script.rs` pins this file against the tree: every
# TALIESIN_REQUIRE_* variable the Rust sources read must be set here, and every canary
# test named here must still exist. Renaming a canary breaks that test, not this script
# silently.

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

# The four canary tests. Each one is the single test whose `... ok` proves that the
# interpreter behind its gate was really exercised: with the matching TALIESIN_REQUIRE_*
# set, a missing interpreter makes the canary FAIL rather than skip, so "canary passed"
# and "gate ran" are the same statement. Names are pinned by gate_script.rs.
CANARY_KERNEL="kernel_executes_state_errors_and_interrupts_runaway_cell"
CANARY_R="r_cells_execute_and_persist_state_across_cells"
CANARY_NODE="only_a_textual_sink_becomes_a_live_region"
CANARY_CHROME="read_run_js_reports_svg_produced_and_error_kinds"
# A second browser-backed capability, and it fails independently of the first: the math
# hover rasterizes a KaTeX page, so it can break on the screenshot/clip path while `{js}`
# observation still works. Every other test in that module asserts a string and would stay
# green with rasterizing entirely broken.
CANARY_MATH_HOVER="a_real_browser_rasterizes_real_katex_into_a_data_uri"
# A third browser-backed capability, independent of both: the reactive client. It is the
# only thing that runs a `{glsl}` shader, the `animate` pump, the `point` pad, `tali.state`
# and the numerics bundle at all — every other test of those five features asserts what Rust
# EMITTED, and would stay green with the whole client runtime broken.
CANARY_REACTIVE="a_glsl_cell_compiles_and_paints"
# A fourth browser-backed capability, independent of the other three: the print track. It is
# the only thing that drives paged.js through CDP, and every other print test asserts what
# Rust EMITTED into the stylesheet — all of which stays green with pagination entirely
# broken, including the failure mode that produces a plausible but truncated PDF.
CANARY_PRINT="pdf_paginates_a_real_document_into_more_than_one_page"
# A fifth browser-backed capability, independent of the other four: `{pyodide}` cells. It is
# the only thing that boots the vendored Pyodide runtime at all — every other test of item 158
# asserts what Rust EMITTED (the script tag, the index `<meta>`, the vendored file list) and
# would stay green with the whole 15.7 MiB runtime payload deleted.
CANARY_PYODIDE="a_pyodide_cell_boots_and_publishes_to_a_js_consumer"
# Item 205 put the payload behind the `pyodide` cargo feature, which means the delivery tests
# now VANISH from a default `cargo test` instead of failing. Two canaries, one per altitude,
# because the two gating mechanisms fail differently: the first lives in a whole target gated
# by `required-features` (dropping the feature silently unbuilds the file), the second is a
# lone `#[cfg]`'d test inside an otherwise-ungated file (dropping the feature leaves the file
# green and one assertion shorter, which no summary line reveals).
CANARY_PYODIDE_DELIVERY="a_single_file_build_degrades_a_pyodide_cell_to_visible_source"
CANARY_PYODIDE_SITE="site_build_copies_the_pyodide_runtime_and_stamps_a_page_relative_index"
# A sixth browser-backed capability, independent of the other five: the figure lightbox.
# The whole viewer is built in JS, so nothing about it reaches the served HTML — every
# other test of a figure asserts what Rust EMITTED and would stay green with the viewer's
# open/close handlers inverted.
CANARY_LIGHTBOX="clicking_the_enlarged_image_closes_the_lightbox"

PY="${TALIESIN_PYTHON:-python3}"
R_BIN="${TALIESIN_R:-R}"

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
# Counted separately from the rest, because only these four decide whether the workspace
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

have "$R_BIN" && "$R_BIN" -q -s -e 'library(IRkernel)' >/dev/null 2>&1
require $? "an R with IRkernel (TALIESIN_R=$R_BIN)" \
    "R -e 'install.packages(\"IRkernel\")'  — or point TALIESIN_R at one that has it" interpreter

have node
require $? "node" "install Node 20+ (https://nodejs.org)" interpreter

have npx
require $? "npx" "ships with Node 20+"

have npm
require $? "npm" "ships with Node 20+"

# Mirrors headless_js.rs::chrome_path — CHROME_PATH wins, else the first candidate on PATH.
chrome_found=1
if [ -n "${CHROME_PATH:-}" ]; then
    [ -x "$CHROME_PATH" ] && chrome_found=0
else
    for c in google-chrome google-chrome-stable chromium chromium-browser; do
        if have "$c"; then
            chrome_found=0
            break
        fi
    done
fi
require $chrome_found "a system Chrome" \
    "install google-chrome or chromium, or set CHROME_PATH to a browser binary" interpreter

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
# 3. The workspace suite, with all four interpreter gates armed.
#
# --test-threads=1 because several tests own process-global state (CHROME_PATH,
# the cell-timeout OnceLock, the kernel pool), and a raced run is how this suite
# produces both flakes and vacuous passes.
#
# `--features taliesin-server/headless-js` because the browser driver is OFF by
# default (it is 24% of a clean release build; see `crates/server/Cargo.toml`), and
# `read_run_js` / `print_pdf` / `deck_browser` / `reactive_browser` / `pyodide_browser` /
# `reader_chrome_browser`
# declare it in `required-features` — so without this
# flag cargo would quietly skip building them and the chrome canary below would go
# missing. That pairing is deliberate: forgetting the feature turns this gate RED
# rather than shrinking the suite silently.
#
# `--features taliesin-server/pyodide` for exactly the same reason (item 205): the 15.7 MiB
# vendored runtime is off by default, `crates/core/tests/pyodide.rs` declares the feature in
# `required-features`, and four more tests are `#[cfg]`'d on it. `pyodide_browser` needs BOTH
# features. The two `CANARY_PYODIDE_*` names above are what turn a dropped flag red.
#
# Between the two gates both configurations are covered: clippy (gate 2) runs with
# DEFAULT features, i.e. the no-driver, no-runtime build, which is the one that must also
# compile the feature-OFF arms (`pyodide_feature_off.rs`); and this one compiles every
# target with both on.
# ---------------------------------------------------------------------------
TEST_NAME="cargo test --workspace (all four gates)"
if [ "$MISSING_INTERPRETERS" -gt 0 ] && [ "$ALLOW_MISSING" -eq 1 ]; then
    # Arming a REQUIRE_* gate whose interpreter is absent turns the canary into a
    # failure, which would report as "a gate failed" when the truth is "a gate could
    # not run". Keep those two verdicts distinct.
    skip_gate "$TEST_NAME" "an interpreter is missing; the four REQUIRE gates cannot be armed"
else
    TALIESIN_PYTHON="$PY" \
        TALIESIN_R="$R_BIN" \
        TALIESIN_REQUIRE_KERNEL=1 \
        TALIESIN_REQUIRE_R=1 \
        TALIESIN_REQUIRE_NODE=1 \
        TALIESIN_REQUIRE_CHROME=1 \
        run_gate "$TEST_NAME" test.log \
        cargo test --workspace --features taliesin-server/headless-js,taliesin-server/pyodide -- --test-threads=1
    test_rc=$?

    # Only assert on the output when cargo itself succeeded: a build failure produces a
    # log with no test lines at all, and reporting four missing canaries on top of it
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
            "R kernel:$CANARY_R" \
            "node:$CANARY_NODE" \
            "chrome:$CANARY_CHROME" \
            "chrome (math hover):$CANARY_MATH_HOVER" \
            "chrome (reactive client):$CANARY_REACTIVE" \
            "chrome (print track):$CANARY_PRINT" \
            "chrome (pyodide):$CANARY_PYODIDE" \
            "pyodide feature (delivery):$CANARY_PYODIDE_DELIVERY" \
            "pyodide feature (site build):$CANARY_PYODIDE_SITE" \
            "chrome (lightbox):$CANARY_LIGHTBOX"; do
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
# 6. The publish passcode gate. Security-critical, and this dependency-free
#    node:test file is its only functional test.
# ---------------------------------------------------------------------------
if [ "$ALLOW_MISSING" -eq 1 ] && ! have node; then
    skip_gate "node --test: publish passcode" "node unavailable"
else
    run_gate "node --test: publish passcode" middleware.log \
        node --test crates/server/src/assets/_middleware.test.mjs
fi

# ---------------------------------------------------------------------------
# 7. The VS Code companion: build, type-check, and the OFFLINE TextMate grammar
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
# 8-9. Dependency advisories and dependency POLICY (deny.toml: redistributable
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
green "PASSED — every gate ran and passed."
exit 0
