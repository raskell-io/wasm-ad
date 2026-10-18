#!/usr/bin/env bash
# Builds a TinyGo cart package into build/<name>.wasm and adds its metadata.
#
#   ./build.sh examples/starter [key=value ...]
#
# Needs tinygo on PATH (or TINYGO=/path/to/tinygo) and Node for the
# metadata step. Metadata defaults to the starter's.
set -euo pipefail
cd "$(dirname "$0")"
pkg="${1:-examples/starter}"
shift || true
name="$(basename "$pkg")"
out="build/$name.wasm"
mkdir -p build
"${TINYGO:-tinygo}" build -target ./wasm-ad.json -scheduler=none -panic=trap -opt=z -no-debug -o "$out" "./$pkg"
if [ $# -eq 0 ]; then
    set -- title=Starter author=raskell.io "description=The WASM-AD starter cart, in Go (TinyGo)"
fi
node ../../tools/wasmad.mjs meta "$out" "$@"
node ../../tools/wasmad.mjs check "$out"
