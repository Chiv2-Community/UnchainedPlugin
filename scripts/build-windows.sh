#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/build-windows.sh [cargo xwin build options]

Cross-compile the Windows x86_64 plugin from Linux.
Defaults to a locked debug library build; pass --release or --offline as needed.

Environment:
  XWIN_CACHE_DIR    SDK cache directory (default: cargo-xwin's platform cache)
  CARGO_TARGET_DIR  Build output directory (default: target)
  CFLAGS           Additional C compiler flags
  RUSTFLAGS        Additional Rust compiler flags
  PATH             Locations of cargo-xwin and the LLVM tools

See README.md for the pinned cargo-xwin installation.
EOF
}

if [[ ${1:-} == --help || ${1:-} == -h ]]; then
    usage
    exit 0
fi

missing_tools=false
missing_executables=()
require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        missing_tools=true
        missing_executables+=("$1: $2")
    fi
}

tools=(
    cargo "Rust package manager"
    rustc "Rust compiler"
    cargo-xwin "Windows MSVC cross-compilation helper (see README.md for installation)"
    clang-cl "MSVC-compatible C compiler"
    llvm-lib "MSVC-compatible archiver"
    llvm-rc "Windows resource compiler"
    lld-link "MSVC-compatible linker"
    dirname "Locate the repository from the script path"
)
for ((i = 0; i < ${#tools[@]}; i += 2)); do
    require_command "${tools[i]}" "${tools[i + 1]}"
done
if [[ $missing_tools == true ]]; then
    printf 'Missing Executable(s):\n' >&2
    printf '  %s\n' "${missing_executables[@]}" >&2
    exit 1
fi

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repo_root"

target=x86_64-pc-windows-msvc
export XWIN_ARCH=x86_64
exec cargo xwin build --locked --target "$target" --lib "$@"
