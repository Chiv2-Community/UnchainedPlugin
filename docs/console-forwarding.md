# Console forwarding

Launchers can receive plugin console output directly, without reading log files
or scraping a Wine console. The transport works on Windows and under Wine/Proton.
It is output-only; it does not accept commands.

## Enable

Minimal Windows integration recipe:

1. Use `CreatePipe` with inheritable security attributes. Clear `HANDLE_FLAG_INHERIT`
   on the parent read end with `SetHandleInformation`; keep the child write end
   inheritable. This dedicated pipe is separate from stdout/stderr.
2. Build a private child Unicode environment block. Set `UNCHAINED_CONSOLE_HANDLE`
   to the write handle's **unsigned decimal Windows HANDLE value**, not a Linux FD.
   Do not modify the launcher's environment.
3. Initialize a `STARTUPINFOEXW` and its attribute list; set `StartupInfo.cb` to
   `sizeof(STARTUPINFOEXW)`. Prepare a whitelist containing the write handle and
   only other intended child handles.
4. **If redirecting stdio**, set `STARTF_USESTDHANDLES`, supply valid
   `hStdInput`, `hStdOutput`, and `hStdError`, and include those inheritable child
   handles in the whitelist. Keep parent pipe ends noninheritable. The dedicated
   console pipe alone does not require three stdio pipes or that startup flag.
5. Set `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` to the completed whitelist, then call
   `CreateProcessW` with `bInheritHandles=TRUE`,
   `EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT`, and that startup
   structure/environment. For suspended DLL injection, also use `CREATE_SUSPENDED`;
   suspension controls injection ordering, not pipe inheritance.
6. After successful process creation the child owns inherited copies, so close
   the parent's child-end copies immediately. Release the attribute list after
   creation. Start the background reader **before DLL injection**: plugin
   initialization can emit records before the game's main thread resumes.
   Close all parent-owned handles on every failure path;
   terminate/clean up an owned suspended child if injection fails. Never close a
   child-owned handle from the parent.
7. Drain frames continuously through EOF. At shutdown allow a bounded final drain,
   then close the read end: descendants can retain a write handle indefinitely.

There is no listening socket, named endpoint, token, or authentication handshake.
Possession of the inherited handle selects the receiver. Legacy TCP environment
variables are ignored. This is launch-local isolation, not a security boundary
against another process with permission to inspect or manipulate the game.

With valid settings, the plugin forwards configured `console` appenders and its
startup banner instead of allocating/writing its own console. File appenders,
log levels, filters, custom encoder patterns, function prefixes, and namespaces
are unchanged. No logging YAML override or UE logging flags are needed.
Without valid settings, existing console behavior is unchanged.
Explicit [`--attach-stdio`](console-attachment.md) overrides these settings and uses
standard output instead; the structured pipe is not used.

Environment values are read once during startup. The plugin validates that the
handle refers to a pipe, takes ownership, and disables further handle inheritance.
Use one pipe per game. Receivers must still validate versions and frame limits.

## Protocol v1

UTF-8 JSON objects, one per LF-delimited frame. Embedded newlines are JSON-escaped.
The write-only stream starts directly with records of this shape:

```json
{"version":1,"kind":"log","level":"INFO","text":"[12:34:56 INFO ] [UP::module              ] message\r\n","dropped":0}
```

- `kind`: `log` or `banner`.
- `level`: uppercase log level, or `null` for the banner.
- `text`: console-encoder output, with native console styling removed. The
  default encoder emits no ANSI escapes. Message text/custom patterns can still
  contain controls: receivers must sanitize before displaying in a terminal.
- `dropped`: number of records lost to queue pressure since the previous
  successfully queued frame. A trailing disconnect can prevent a final loss report.

The receiver should retain level metadata for presentation, not parse the
formatted text to recover severity. Banner text is separate from log records.

## Bounds and failure behavior

- Game threads encode into a bounded buffer and try a 256-record queue; they
  never perform pipe I/O or wait for the receiver.
- Each record contains at most 16 KiB of UTF-8 text. Oversized records end in
  ` [truncated]`; a partial UTF-8 character is replaced safely.
- JSON escaping can expand text. Receivers should limit frames to 128 KiB and
  bound retained history independently of the pipe buffer.
- Only the background worker writes to the pipe. A slow reader can block that
  worker, not game threads; the bounded queue drops new records when full.
  Closing the read end releases a blocked write. There is no write timeout.
- A failed/disconnected receiver stops forwarding for that launch. There is no
  reconnect or replay. Configured file appenders continue independently; the
  plugin does not create a fallback graphical console or recursively log errors.
- `flush` never waits for pipe I/O. Abrupt process exit can lose queued records.

`tty_only` console settings do not disable an explicitly requested transport.
Custom file-only configurations do not gain a console appender automatically.

## Launcher integration

The Windows CLI creates the pipe before the game and passes its handle in a
private Unicode environment block. On Linux, **UnchainedLauncher's bundled Windows helper** creates
the game pipe and relays only its records through its reserved stdout pipe to
the native CLI. Game stdout/stderr are drained separately and discarded in
structured mode; they cannot mix with JSON records. `runinprefix` preserves the
helper's inherited streams, unlike the `run`/Steam.exe wrapper.
Neither backend changes the launcher's environment.
The receiver validates frames,
sanitizes controls, and keeps bounded per-server history. `L` switches between
the dashboard and full console, Page Up/Page Down scroll, and `End` follows the
latest output. No-TUI mode prints received lines without terminal controls.

This requires a plugin build containing the transport; older plugins retain
their own console. Client launches remain detached and do not start a TUI receiver.

### Alternate Linux launchers

Proton does not supply this helper. Implement your own Windows helper using the
recipe above, then launch it under the selected Proton tool. The helper creates
the real Windows pipe inside Wine, injects the DLL, and resumes the game. Reserve
helper stdout for structured records only; send diagnostics to stderr. Capture
game stdout/stderr separately and drain both (discard them in structured mode).
The native parent must concurrently drain helper stdout **and** stderr.

Example invocation for an **integrator-owned** helper (paths are examples):

```bash
steam="$HOME/.local/share/Steam"
compat="/path/to/steamapps/compatdata/1824220" # contains initialized pfx/system.reg
proton="/path/to/selected/Proton/proton"
helper="/absolute/path/to/YourConsoleHelper.exe"
test -f "$compat/pfx/system.reg" || exit 1
env STEAM_COMPAT_CLIENT_INSTALL_PATH="$steam" STEAM_COMPAT_DATA_PATH="$compat" \
    WINEPREFIX="$compat/pfx" STEAM_COMPAT_APP_ID=1824220 \
    SteamAppId=1824220 SteamGameId=1824220 WINEDEBUG=-all \
    "$proton" runinprefix "$helper" >console.jsonl 2>helper.stderr
```

Initialize the game prefix through Steam first; keep Steam running for Steam
authentication. Use the same selected Proton tool/prefix as the game. Pass any
helper-specific arguments according to your own protocol; convert game/DLL paths
with that prefix's `winepath`, and resolve Windows known folders rather than
assuming a username. The files above stand in for concurrent native stream readers.

The bundled `UnchainedLauncher.ProtonHelper.exe` request-file, readiness, and
heartbeat protocol is **private and version-coupled to its CLI**. It is not a
standalone integrator API. Use the matching published CLI bundle, or own your
helper and its lifecycle protocol.
