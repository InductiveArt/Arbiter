#!/usr/bin/env bash
# Stop gate: the graph plane in seconds. A finding blocks the stop and names
# what to read; the full judgment stays `arbiter gate .`. The session may sit
# in any subfolder, so the repository root comes from the harness.
set -uo pipefail
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
found=$(arbiter find . 2>/dev/null | head -5)
if [ -n "$found" ]; then
  echo "arbiter find . is not empty; run arbiter gate --fix . and read .arbiter/last/report.md. First findings:" >&2
  echo "$found" >&2
  exit 2
fi
exit 0
