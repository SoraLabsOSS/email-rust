#!/bin/sh
set -e

# Cloudflare Workers Builds has no Rust toolchain. Install it when missing
# so `wrangler deploy` can run the same command locally and in CI.
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
fi

if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi

export PATH="$HOME/.cargo/bin:$PATH"

rustup target add wasm32-unknown-unknown
cargo install -q "worker-build@^0.8"
worker-build --release
