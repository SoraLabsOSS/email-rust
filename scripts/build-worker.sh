#!/bin/sh
set -e

# Cloudflare Workers Builds caches npm's ~/.npm only — not rustup, cargo, or
# target/. Park Rust there on CI so later builds reuse the toolchain instead
# of compiling worker-build from source every time (~5–8 min).
if [ "${WORKERS_CI:-}" = "1" ]; then
  npm_cache="${npm_config_cache:-${HOME}/.npm}"
  export RUSTUP_HOME="${npm_cache}/_rustup"
  export CARGO_HOME="${npm_cache}/_cargo"
  export CARGO_TARGET_DIR="${npm_cache}/_target"
  export XDG_CACHE_HOME="${npm_cache}/_xdg"
  mkdir -p "$RUSTUP_HOME" "$CARGO_HOME/bin" "$CARGO_TARGET_DIR" "$XDG_CACHE_HOME"
  export PATH="${CARGO_HOME}/bin:${PATH}"
fi

if [ -f "${CARGO_HOME:-$HOME/.cargo}/env" ]; then
  . "${CARGO_HOME:-$HOME/.cargo}/env"
elif [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:${PATH}"

if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --profile minimal --default-toolchain stable
  . "${CARGO_HOME:-$HOME/.cargo}/env"
fi

rustup target add wasm32-unknown-unknown

if ! command -v worker-build >/dev/null 2>&1; then
  cargo install -q "worker-build@^0.8"
fi

worker-build --release
