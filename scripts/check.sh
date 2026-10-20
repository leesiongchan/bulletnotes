#!/usr/bin/env bash
set -euo pipefail
app_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$app_root"
python3 scripts/prepare-slint.py
rustup run 1.99.1 cargo fmt --check
rustup run 1.99.1 cargo clippy --all-targets --all-features --locked -- -D warnings
rustup run 1.99.1 cargo test --lib --bins --tests --examples --all-features --locked
