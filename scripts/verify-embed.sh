#!/usr/bin/env bash
# Builds the example natively and for the web with fat LTO and stripping (profile `dist`),
# then checks that each build carries its own IFID and an iFiction record that the
# example itself reports. `hello-unref` never reads the block, so it checks that `#[used]`
# alone keeps it through the linker.
#
# Runs on Linux, macOS and Windows (Git Bash). SKIP_WASM=1 skips the web build.
set -euo pipefail
cd "$(dirname "$0")/.."

exe=""
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) exe=".exe" ;;
esac

cargo build -q -p ifiction-cli
cargo build -q -p hello --profile dist --bins

cli=target/debug/ifiction$exe
native=target/dist/hello$exe
unref=target/dist/hello-unref$exe

want_exe=$(sed -n 's/^executable = "\(.*\)"/\1/p' examples/hello/Cargo.toml)
want_web=$(sed -n 's/^html = "\(.*\)"/\1/p' examples/hello/Cargo.toml)

check() {
  local what=$1 got=$2 want=$3
  got=${got//$'\r'/}
  if [[ "$got" != "$want" ]]; then
    echo "FAIL $what: got '$got', want '$want'" >&2
    exit 1
  fi
  echo "ok   $what"
}

echo "== $(uname -s): $("$cli" identify "$native" | tail -n 1)"
check "native IFID (marker)" "$("$cli" ifid "$native")" "IFID: $want_exe"
check "native IFID (self-report)" "$("$native" --ifid)" "$want_exe"
check "native record" "$("$cli" meta "$native")" "$("$native" --ifiction)"
"$cli" lint "$native" >/dev/null
echo "ok   native record lints"
check "unreferenced block kept (marker)" "$("$cli" ifid "$unref")" "IFID: $want_exe"
"$cli" lint "$unref" >/dev/null
echo "ok   unreferenced record lints"

work=target/verify-embed
mkdir -p "$work"
"$cli" html inject examples/hello/index.html --manifest examples/hello -o "$work/index.html"
check "web page IFID (meta tag)" "$("$cli" ifid "$work/index.html")" "IFID: $want_web"

if [[ "${SKIP_WASM:-}" == "1" ]]; then
  echo "skip web build (SKIP_WASM=1)"
  exit 0
fi
cargo build -q -p hello --profile dist --target wasm32-unknown-unknown --bins
web=target/wasm32-unknown-unknown/dist/hello.wasm
web_unref=target/wasm32-unknown-unknown/dist/hello-unref.wasm
check "web IFID (marker)" "$("$cli" ifid "$web")" "IFID: $want_web"
"$cli" meta "$web" | grep -q "<format>html</format>"
echo "ok   web record has format html"
check "web unreferenced block kept (marker)" "$("$cli" ifid "$web_unref")" "IFID: $want_web"
