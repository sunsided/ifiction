#!/usr/bin/env bash
# Compares this workspace with babel, the reference tool of the Treaty of Babel.
#
#   BABEL=/path/to/babel scripts/babel-conformance.sh
#
# For the example's native and web builds, its HTML page and its iFiction records:
# - babel -ifid must report the same IFID as `ifiction ifid`, and the IFID from Cargo.toml;
# - babel -verify must accept the records, and babel -lint must find no style problems.
set -euo pipefail
cd "$(dirname "$0")/.."

babel=${BABEL:?set BABEL to the babel binary}
work=target/babel-conformance
rm -rf "$work"
mkdir -p "$work"

cargo build -q -p ifiction-cli
cargo build -q -p hello --profile dist --bins
cargo build -q -p hello --profile dist --target wasm32-unknown-unknown --bins

cli=target/debug/ifiction
want_exe=$(sed -n 's/^executable = "\(.*\)"/\1/p' examples/hello/Cargo.toml)
want_web=$(sed -n 's/^html = "\(.*\)"/\1/p' examples/hello/Cargo.toml)
if [[ -z "$want_exe" || -z "$want_web" ]]; then
  echo "FAIL could not read the executable and html IFIDs from examples/hello/Cargo.toml" >&2
  exit 1
fi

"$cli" html inject examples/hello/index.html --manifest examples/hello -o "$work/index.html"
"$cli" meta target/dist/hello > "$work/executable.iFiction"
"$cli" meta target/wasm32-unknown-unknown/dist/hello.wasm > "$work/html.iFiction"

failed=0
fail() {
  echo "FAIL $*" >&2
  failed=1
}

first_ifid() {
  grep -m1 '^IFID: ' | tr -d '\r'
}

check_ifid() {
  local file=$1 want=$2 ours theirs ours_raw raw
  # Neither tool may abort the script (set -e): a missing IFID is a reported failure.
  ours_raw=$("$cli" ifid "$file" 2>&1 || true)
  ours=$(first_ifid <<<"$ours_raw" || true)
  raw=$("$babel" -ifid "$file" 2>&1 || true)
  theirs=$(first_ifid <<<"$raw" || true)
  if [[ -z "$ours" || -z "$theirs" ]]; then
    fail "$file: no IFID from $([[ -z "$ours" ]] && echo ifiction || echo babel)"$'\n'"ifiction: $ours_raw"$'\n'"babel: $raw"
  elif [[ "$ours" != "$theirs" ]]; then
    fail "$file: ifiction says '$ours', babel says '$theirs'"$'\n'"$raw"
  elif [[ -n "$want" && "$ours" != "IFID: $want" ]]; then
    fail "$file: both say '$ours', Cargo.toml says '$want'"
  else
    echo "ok   $file: $ours"
  fi
}

check_ifid target/dist/hello "$want_exe"
check_ifid target/dist/hello-unref "$want_exe"
check_ifid target/wasm32-unknown-unknown/dist/hello.wasm "$want_web"
check_ifid target/wasm32-unknown-unknown/dist/hello-unref.wasm "$want_web"
check_ifid "$work/index.html" "$want_web"
check_ifid "$work/executable.iFiction" "$want_exe"
check_ifid "$work/html.iFiction" "$want_web"
# No marker: both must fall back to the same MD5 IFID. (Not the ifiction binary itself:
# it contains the strings that format detectors look for.)
check_ifid "$(type -P true)" ""
check_ifid examples/hello/index.html ""

for record in "$work/executable.iFiction" "$work/html.iFiction"; do
  out=$("$babel" -verify "$record" 2>&1 || true)
  if grep -q '^Verified ' <<<"$out"; then
    echo "ok   babel -verify $record"
  else
    fail "babel -verify $record:"$'\n'"$out"
  fi
  out=$("$babel" -lint "$record" 2>&1 || true)
  if grep -q 'conforms to iFiction style guidelines' <<<"$out"; then
    echo "ok   babel -lint $record"
  else
    fail "babel -lint $record:"$'\n'"$out"
  fi
done

exit "$failed"
