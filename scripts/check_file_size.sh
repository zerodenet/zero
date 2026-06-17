#!/usr/bin/env bash
# CI gate: fail if any .rs source file exceeds 300 lines.
# This enforces the project's own "Split files around 300 lines" rule.
#
# Usage: bash scripts/check_file_size.sh
# Exit code 0 = all files within limit; 1 = violations found.

set -euo pipefail
LIMIT=300
VIOLATIONS=""

while IFS= read -r -d '' file; do
    lines=$(wc -l < "$file")
    if [ "$lines" -gt "$LIMIT" ]; then
        VIOLATIONS="${VIOLATIONS}$(printf '%6d  %s\n' "$lines" "$file")"
    fi
done < <(find crates protocols src -name '*.rs' -print0 2>/dev/null)

if [ -n "$VIOLATIONS" ]; then
    count=$(printf '%s\n' "$VIOLATIONS" | grep -c . || true)
    echo "FAIL: $count file(s) exceed the $LIMIT-line limit:"
    echo "$VIOLATIONS" | sort -rn
    echo ""
    echo "Split these files per AGENTS.md: 'Split files around 300 lines'."
    exit 1
fi

echo "OK: all .rs files within $LIMIT-line limit."
exit 0
