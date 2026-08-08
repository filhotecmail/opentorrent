#!/usr/bin/env bash
# Build helper for OpenTorrent
# Usage: ./build.sh [release|debug] [verbose|clean|check|test|clippy|fmt]

set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")"

MODE="${1:-release}"
ACTION="${2:-build}"

case "$MODE" in
  release|r)
    PROFILE="--release"
    ;;
  debug|d)
    PROFILE=""
    ;;
  *)
    echo "Uso: $0 [release|debug] [verbose|clean|check|test|clippy|fmt|build]"
    exit 1
    ;;
esac

case "$ACTION" in
  verbose|v)
    cargo build $PROFILE --verbose
    ;;
  clean|c)
    cargo clean
    ;;
  check)
    cargo check $PROFILE
    ;;
  test|t)
    cargo test $PROFILE
    ;;
  clippy)
    cargo clippy --all-targets -- -D warnings
    ;;
  fmt)
    cargo fmt --check
    ;;
  build|b|"")
    cargo build $PROFILE
    ;;
  *)
    echo "Ação desconhecida: $ACTION"
    echo "Disponíveis: verbose, clean, check, test, clippy, fmt, build"
    exit 1
    ;;
esac