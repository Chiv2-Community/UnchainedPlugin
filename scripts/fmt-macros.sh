#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
# Include proc-macro implementations and standalone Rust test fixtures.
git ls-files -z --cached --others --exclude-standard -- 'unchained-macros-*/*.rs' |
    xargs -0 rustfmt --edition 2021 --config skip_children=true "$@"
