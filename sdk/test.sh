#!/usr/bin/env bash
# Builds every SDK's starter cart whose toolchain is installed, then checks
# them against SPEC.md and against each other (sdk/conformance.mjs).
#
#   sdk/test.sh            (or `make sdk-test`)
#
# Set REQUIRE_ALL=1 to fail instead of skipping a missing toolchain.
set -euo pipefail
cd "$(dirname "$0")/.."

carts=()
skip() {
    if [ -n "${REQUIRE_ALL:-}" ]; then echo "error: $1" >&2; exit 1; fi
    echo "skip $1"
}

cargo build -q --release --target wasm32-unknown-unknown -p wasm-ad --example starter
carts+=(rust=target/wasm32-unknown-unknown/release/examples/starter.wasm)

if make -s -C sdk/c >/dev/null 2>&1; then
    carts+=(c=sdk/c/build/starter.wasm c++=sdk/c/build/starter-cpp.wasm)
else
    skip "c: needs clang with the WebAssembly backend and wasm-ld"
fi

if command -v zig >/dev/null; then
    (cd sdk/zig && zig build)
    carts+=(zig=sdk/zig/zig-out/bin/starter.wasm)
else
    skip "zig: zig not found"
fi

if command -v npm >/dev/null; then
    (cd sdk/assemblyscript && { [ -d node_modules ] || npm ci --silent; } && npm run -s build)
    carts+=(assemblyscript=sdk/assemblyscript/build/starter.wasm)
else
    skip "assemblyscript: npm not found"
fi

if command -v "${TINYGO:-tinygo}" >/dev/null; then
    sdk/tinygo/build.sh examples/starter >/dev/null
    carts+=(tinygo=sdk/tinygo/build/starter.wasm)
else
    skip "tinygo: tinygo not found (set TINYGO=/path/to/tinygo)"
fi

node sdk/conformance.mjs "${carts[@]}"
