# Backend SDK integration

The backend repository owns generated Rust models/client crates, OAS, templates,
generation checks and SDK releases. This plugin consumes an exact backend Git
revision; it does not generate or carry a second contract copy.

`unchained-backend` supplies plugin transport policy: deadlines, redirect
suppression, source-IP binding and safe debug output. Registration, update,
heartbeat, deletion and typed ban checks use generated operations. Legacy ban
checks remain fail-open. Engine-owned HTTP interception remains in engine hooks.

```sh
cargo test --locked -p unchained-backend
uv run --project ../C2ServerBrowserBackend-language-sdks python scripts/check-browser-backend.py
TMPDIR=/tmp/opencode bash scripts/check-windows.sh
```

The Windows type check needs the Windows Rust target, LLVM clang-cl/llvm-lib/
llvm-rc and cached xwin CRT/SDK at `XWIN_ROOT` (default
`~/.cache/cargo-xwin/xwin`). `WINRES_TOOLKIT_PATH` selects the resource compiler;
normal Windows builds retain SDK discovery. This does not run a game.

Update the SDK dependency revision and lockfile to adopt backend changes. Change
the backend OAS/templates first; never add handwritten wire DTOs or routes here.
Discord verification and native admission remain separate work.
