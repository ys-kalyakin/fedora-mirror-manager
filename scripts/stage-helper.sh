#!/usr/bin/env bash
# Stages the release helper binary where Tauri's externalBin expects it:
# src-tauri/binaries/fmm-apply-<host-triple>. Runs as part of
# `npm run prebundle` before `tauri build`, so the privileged helper is
# bundled next to the app binary and the in-app helper setup can install
# it from the released package.
set -euo pipefail

cd "$(dirname "$0")/.."

host_triple="$(rustc -vV | awk '/^host:/ {print $2}')"
target_dir="${CARGO_TARGET_DIR:-target}"

cargo build --release -p fmm-apply

mkdir -p src-tauri/binaries
install -m 0755 "$target_dir/release/fmm-apply" "src-tauri/binaries/fmm-apply-$host_triple"
echo "staged: src-tauri/binaries/fmm-apply-$host_triple"
