#!/usr/bin/env bash
# Stop gate: the graph plane in seconds. A finding blocks the stop and names
# what to read; the full judgment stays `arbiter gate .`.
set -uo pipefail
found=$(arbiter findings . 2>/dev/null | head -5)
if [ -n "$found" ]; then
  echo "arbiter findings . is not empty; run arbiter fix . and read .arbiter/last/report.md. First findings:" >&2
  echo "$found" >&2
  exit 2
fi
exit 0
