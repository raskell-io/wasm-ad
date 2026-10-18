#!/usr/bin/env bash
# Build command for the Cloudflare Pages project (Git integration):
#   scripts/cf-pages-build.sh        output directory: dist
# Installs Rust with the wasm target when missing, fetches Rainbow River for
# the site's cart slot, then builds the site.
set -euo pipefail

if ! command -v cargo >/dev/null; then
    curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
fi
rustup target add wasm32-unknown-unknown

other=$(mktemp -d)/rainbow-river
git clone --depth 1 https://github.com/raskell-io/rainbow-river.git "$other"
make site RAINBOW_RIVER="$other"
