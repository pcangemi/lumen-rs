#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
xcrun -sdk macosx metal -fmodules-cache-path="${TMPDIR:-/tmp}/lumen-metal-cache" -c src/pathtrace.metal -o "${TMPDIR:-/tmp}/lumen-pathtrace.air"
cargo test --locked --release --test physics metal_matches_cpu_statistics_and_batching -- --ignored --nocapture
