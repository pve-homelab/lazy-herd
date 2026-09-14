#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HERDR="${HERDR_BIN_PATH:-herdr}"
ENTRY="menu"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT) ENTRY="menu-windows" ;;
esac
exec "$HERDR" plugin pane open --plugin lazy-herd --entrypoint "$ENTRY"
