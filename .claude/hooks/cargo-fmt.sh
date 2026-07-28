#!/usr/bin/env bash
# PostToolUse hook: format the just-edited Rust file with rustfmt so the tree
# stays `cargo fmt`-clean (which the pre-push hook enforces). Reads the hook payload on stdin
# and only acts on *.rs files; never fails the tool call.
set -euo pipefail

payload=$(cat)
file=$(printf '%s' "$payload" | jq -r '.tool_input.file_path // empty')

case "$file" in
  *.rs)
    if command -v rustfmt >/dev/null 2>&1 && [ -f "$file" ]; then
      rustfmt --edition 2024 "$file" >/dev/null 2>&1 || true
    fi
    ;;
esac
exit 0
