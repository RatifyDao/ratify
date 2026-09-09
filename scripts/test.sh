#!/usr/bin/env bash
# The whole suite, including the factory tests that deploy a real community.
#
# Those need the contracts compiled to wasm first, which is why they are behind
# a feature and why this script exists. Everything else runs under a plain
# `cargo test`.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo "Building contracts to wasm..."
stellar contract build --manifest-path packages/contracts/ratify-factory/Cargo.toml

echo
echo "Running tests..."
cargo test --workspace --features ratify-factory/wasm-tests
