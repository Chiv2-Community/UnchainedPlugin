#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
# Include proc-macro implementations and standalone Rust test fixtures.
mapfile -d '' files < <(git ls-files -z --cached --others --exclude-standard -- 'unchained-macros-*/*.rs')
cargo run --quiet --locked -p xtask --bin fmt-macros -- "$@" "${files[@]}"
