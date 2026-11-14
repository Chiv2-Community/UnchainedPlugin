#!/usr/bin/env bash
# Type-check native plugin code locally with an already installed xwin SDK.
set -euo pipefail

cd "$(dirname "$0")/.."
export XWIN_ROOT="${XWIN_ROOT:-$HOME/.cache/cargo-xwin/xwin}"
test -d "$XWIN_ROOT/crt/include"
test -d "$XWIN_ROOT/sdk/include/um"
toolkit=$(mktemp -d "${TMPDIR:-/tmp}/unchained-winres.XXXXXX")
trap 'rm -f "$toolkit/rc.exe"; rmdir "$toolkit"' EXIT
cat > "$toolkit/rc.exe" <<'RC'
#!/bin/sh
exec llvm-rc "/I$XWIN_ROOT/sdk/include/um" "/I$XWIN_ROOT/sdk/include/shared" "$@"
RC
chmod +x "$toolkit/rc.exe"
export WINRES_TOOLKIT_PATH="$toolkit"
export CC_x86_64_pc_windows_msvc
CC_x86_64_pc_windows_msvc=$(command -v clang-cl)
export AR_x86_64_pc_windows_msvc=llvm-lib
export CC_SHELL_ESCAPED_FLAGS=1
export CFLAGS_x86_64_pc_windows_msvc="-imsvc \"$XWIN_ROOT/crt/include\" -imsvc \"$XWIN_ROOT/sdk/include/ucrt\" -imsvc \"$XWIN_ROOT/sdk/include/shared\" -imsvc \"$XWIN_ROOT/sdk/include/um\""
cargo check --locked --offline --target x86_64-pc-windows-msvc --lib "$@"
