#!/usr/bin/env bash
# Runs every fuzz target for a short while (FUZZ_SECONDS each, default 30), then replays
# the saved crash inputs in fuzz/regressions/<target>/. Needs cargo-fuzz and nightly Rust.
set -euo pipefail
cd "$(dirname "$0")/.."

seconds=${FUZZ_SECONDS:-30}
cargo="cargo +${FUZZ_TOOLCHAIN:-nightly}"

$cargo fuzz build
for target in $($cargo fuzz list); do
  echo "== $target (${seconds}s)"
  $cargo fuzz run "$target" -- -max_total_time="$seconds"
  if [[ -d fuzz/regressions/$target ]]; then
    echo "== $target: regressions"
    $cargo fuzz run "$target" fuzz/regressions/"$target"/*
  fi
done
