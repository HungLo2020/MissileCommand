#!/usr/bin/env bash
# Mirror the CI checks locally and build a validated release archive.
# Usage: verify.sh [--window]
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

run() {
    printf '\n==> %s\n' "$*"
    "$@"
}

run cargo fmt --all -- --check
run cargo clippy --locked --all-targets -- -D warnings
run cargo test --locked --all-targets
run cargo test --locked --no-default-features
run python3 -m unittest discover -s DevUtils -p 'test_*.py'
run python3 DevUtils/verify_engine.py
run cargo build --locked --release

version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
archive="dist/MissileCommand-${version}-linux-x64.zip"

run python3 DevUtils/package.py --version "v${version}"
run python3 DevUtils/validate_package.py "$archive"

if [[ "${1:-}" == "--window" ]]; then
    run python3 DevUtils/window_smoke.py --binary target/release/missile-command
fi

printf '\nLocal gate passed: %s\n' "$archive"
