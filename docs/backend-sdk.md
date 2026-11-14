# Generated backend SDK

`crates/server-browser-api` contains generated routes/models from the authoritative
backend OAS snapshot in `api/server-browser.yaml`. Module-owned templates handle
`anyOf` listings most-specific-first and keep diagnostic fields intact.

`unchained-backend` adapts generated operations for registration, update, heartbeat,
deletion and typed ban checks. It centralizes deadlines, redirect suppression,
source-IP binding and safe debug output. Legacy ban checks remain fail-open.
Engine-owned HTTP interception remains in engine hooks.

## Validation

```sh
uv run --script scripts/generate-server-browser.py --check
cargo test --locked -p unchained-backend
uv run --project ../C2ServerBrowserBackend-auth-sdk python scripts/check-browser-backend.py
TMPDIR=/tmp/opencode bash scripts/check-windows.sh
```

Generation requires Java 11+ and the pinned Python dependencies. Refresh from an
authoritative backend checkout with `--backend-root <path>`; never edit generated
sources. The loopback harness exercises browser operations against local Uvicorn.

The Windows type check requires the Windows Rust target, LLVM clang-cl/llvm-lib/
llvm-rc and a cached xwin CRT/SDK at `XWIN_ROOT` (default
`~/.cache/cargo-xwin/xwin`). `WINRES_TOOLKIT_PATH` selects the resource compiler;
normal Windows builds retain SDK discovery. This does not run a game.

Discord authentication and native admission are separate work, not enabled here.
