#!/usr/bin/env bash
# Runs every fuzz target for a short while (FUZZ_SECONDS each, default 30), then replays
# the saved crash inputs in fuzz/regressions/<target>/. Needs cargo-fuzz and nightly Rust.
set -euo pipefail
cd "$(dirname "$0")/.."

seconds=${FUZZ_SECONDS:-30}
toolchain=${FUZZ_TOOLCHAIN:-nightly}
cargo="cargo +$toolchain"
# cargo-fuzz defaults to the triple it was built for; a prebuilt cargo-fuzz (as CI installs)
# is a musl binary, and the address sanitizer cannot work with static musl. Use rustc's host.
triple=${FUZZ_TARGET:-$(rustc "+$toolchain" -vV | sed -n 's/^host: //p')}

$cargo fuzz build --target "$triple"
for target in $($cargo fuzz list); do
  echo "== $target (${seconds}s, $triple)"
  $cargo fuzz run --target "$triple" "$target" -- -max_total_time="$seconds"
  if [[ -d fuzz/regressions/$target ]]; then
    echo "== $target: regressions"
    $cargo fuzz run --target "$triple" "$target" fuzz/regressions/"$target"/*
  fi
done
