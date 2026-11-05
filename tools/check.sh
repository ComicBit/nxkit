#!/bin/sh
# Quality gates for the nxbuild workspace: format, lints, tests, layers, size limits.
# Usage: tools/check.sh            (run from anywhere)
# Environment: CARGO_BUILD_JOBS (default 4, from .cargo/config.toml), NXB_TEST_TMP (scratch for store tests).
set -eu
cd "$(dirname "$0")/.."

step() { printf '\n== %s\n' "$*"; }

step "rustfmt"
# workspace members only: --all would also check path dependencies outside the workspace (air-front)
cargo fmt --check

step "clippy (deny warnings)"
cargo clippy --workspace --all-targets --locked --offline -- -D warnings

step "tests"
cargo test --workspace --locked --offline

step "docs (deny warnings)"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline --quiet

step "layers and limits"
cargo run --quiet --locked --offline -p xtask -- layers limits

printf '\nall checks passed\n'
