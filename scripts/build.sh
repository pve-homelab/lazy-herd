#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p bin
cargo build --release
TARGET_ROOT="${CARGO_TARGET_DIR:-target}"
cp -f "${TARGET_ROOT}/release/lazy-herd" bin/lazy-herd
chmod +x bin/lazy-herd
echo "Installed bin/lazy-herd"
