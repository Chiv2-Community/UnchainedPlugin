# Backend SDK integration

The backend repository owns generated Rust models/client crates, OAS, templates,
generation checks and SDK releases. This plugin consumes an exact backend Git
revision; it does not generate or carry a second contract copy.

The plugin depends directly on backend client and lifecycle SDK crates.
`src/backend.rs` configures its user-agent using the SDK transport defaults for
deadlines, redirect suppression, source-IP binding and safe debug output. No
plugin adapter library or reexport layer is needed. The backend-owned
`server-browser-client-sdk` manages registration identity, updates, heartbeat
scheduling, bounded retries and deletion over generated bindings. The plugin
only collects A2S/game metadata, chooses addresses and owns the worker thread.
It polls metadata every five seconds; updates do not bypass SDK retry backoff.
Pausing retains the SDK manager; deregistration calls SDK cleanup. Ambiguous
registration responses halt rather than creating duplicate registrations.
Typed ban checks use generated operations. Legacy ban
checks remain fail-open. Engine-owned HTTP interception remains in engine hooks.

```sh
cargo test --locked -p plugin-portable-tests
cargo test --locked -p plugin-portable-tests --features container-tests
TMPDIR=/tmp/opencode bash scripts/check-windows.sh
```

The Windows type check needs the Windows Rust target, LLVM clang-cl/llvm-lib/
llvm-rc and cached xwin CRT/SDK at `XWIN_ROOT` (default
`~/.cache/cargo-xwin/xwin`). `WINRES_TOOLKIT_PATH` selects the resource compiler;
normal Windows builds retain SDK discovery. This does not run a game.

`tests/portable` is a test-only Cargo package. It imports `src/backend.rs` directly
to run transport tests and container integration without Windows/game
dependencies; it provides no library and is not a runtime dependency.

Container tests require a running local Linux Docker daemon. Testcontainers
downloads the exact backend Git revision used by the SDK dependencies and builds
its authoritative Dockerfile. No backend checkout, Python environment, hosted
service, or copied contract is needed. Each test starts a fresh container with
private ephemeral state and a random loopback-only port; readiness is bounded
and Testcontainers removes the container on drop, including assertion failures.
Missing Docker fails the explicitly enabled tests rather than silently skipping.
The Linux CI job runs these tests; Windows builds leave container tests disabled.

Update the SDK dependency revision and lockfile to adopt backend changes. Change
the backend OAS/templates first; never add handwritten wire DTOs or routes here.
Discord verification and native admission remain separate work.
