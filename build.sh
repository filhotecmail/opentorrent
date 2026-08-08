#!/usr/bin/env bash
# Build helper for OpenTorrent
# Usage: ./build.sh [release|debug] [build|verbose|clean|check|test|clippy|fmt]
#
# A cada build (release ou debug) a versão patch em Cargo.toml é incrementada
# automaticamente, e a compilação roda em modo verbose para acompanhar o
# processo (compilação por crate).

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
    echo "Uso: $0 [release|debug] [build|verbose|clean|check|test|clippy|fmt]"
    exit 1
    ;;
esac

# Incrementa a versão patch em Cargo.toml (ex.: 0.1.0 -> 0.1.1).
bump_version() {
  local cur major minor patch next
  cur="$(grep -m1 -E '^version = ' Cargo.toml | sed -E 's/^version = "([0-9]+)\.([0-9]+)\.([0-9]+)"/\1.\2.\3/')"
  IFS=. read -r major minor patch <<< "$cur"
  patch=$((patch + 1))
  next="$major.$minor.$patch"
  sed -i -E "s/^version = \"[0-9]+\.[0-9]+\.[0-9]+\"/version = \"$next\"/" Cargo.toml
  echo "OpenTorrent v$next — build em andamento (verbose)"
}

case "$ACTION" in
  build|b|"")
    bump_version
    cargo build $PROFILE --verbose
    ;;
  verbose|v)
    bump_version
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
  *)
    echo "Ação desconhecida: $ACTION"
    echo "Disponíveis: build, verbose, clean, check, test, clippy, fmt"
    exit 1
    ;;
esac
