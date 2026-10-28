# Console attachment and forwarding

- Pass plugin flags and environment settings to the Chivalry 2 process before
  loading UnchainedPlugin. The plugin is a DLL, not a separate executable.
- Use the same interfaces on Windows and under Wine/Proton.
- Keep existing logging configuration. File logging, filtering, and console/file
  display patterns remain unchanged.

## Choose a mode

| Your program's goal | What to configure |
| --- | --- |
| Capture structured logs from UnchainedPlugin | [Create a dedicated pipe](#capture-structured-logs) and pass its write handle through `UNCHAINED_CONSOLE_HANDLE`. Read JSON records from the pipe. |
| Capture stdio from UnchainedPlugin | [Redirect the game's stdout/stderr](#capture-stdio) and pass `--attach-stdio`. Read plain text from those streams. |
| Use a separate console to view UnchainedPlugin output | [Leave capture disabled](#use-a-separate-console). The plugin attempts to allocate a console. |
| Don't spawn a separate console for UnchainedPlugin output | [Pass `--no-spawn-console`](#dont-spawn-a-separate-console). This does not select an output destination. |

## What the flags change

Preserve the game's existing launch arguments when adding plugin flags.

- `--attach-stdio` selects inherited stdout/stderr and overrides structured
  capture, even if you supply `UNCHAINED_CONSOLE_HANDLE`. It prevents plugin
  console creation without changing shared console modes or palettes.
- `--no-spawn-console` only prevents plugin console creation, including Wine's
  palette fallback. It does not attach to a console, redirect streams, or disable
  structured capture. You can combine it with `--attach-stdio`, but attachment
  already prevents console creation.

Game argument normalization also accepts `-attach-stdio` and `-no-spawn-console`.
The old name `--attach-std` is not an alias.

## Capture structured logs

- Create a dedicated pipe and pass its write handle to the game as described below.
- Read JSON records from the read end: plugin console logs and banner, not game stdio.
- Do not pass `--attach-stdio`; it overrides structured capture.
- Add `--no-spawn-console` only to suppress console creation if handle setup fails.
  A valid forwarding handle already prevents plugin console creation.

### Windows handle setup

Use these Windows APIs before loading the plugin. For native Linux integration,
also follow the [Wine/Proton requirements](#wineproton-requirements).

1. Use `CreatePipe` with inheritable security attributes. Clear `HANDLE_FLAG_INHERIT`
   on the parent read end with `SetHandleInformation`; keep the child write end
   inheritable. This dedicated pipe is separate from stdout/stderr.
2. Build a private child Unicode environment block. Set `UNCHAINED_CONSOLE_HANDLE`
   to the write handle's **unsigned decimal Windows HANDLE value**.
   Do not modify your program's own environment. This value is not a path, network
   address, or Linux file descriptor.
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

- Use one pipe per game. The plugin validates and takes ownership of the inherited
  handle at startup, then disables further inheritance.
- Expect normal console behavior if the handle is invalid, unless
  `--no-spawn-console` is set.
- Do not send a handshake. There is no socket, endpoint, or token; legacy TCP
  variables are ignored.
- Do not treat handle inheritance as protection from processes allowed to inspect
  or manipulate the game.

### Protocol v1

UTF-8 JSON objects, one per LF-delimited frame. Embedded newlines are JSON-escaped.
The write-only stream starts directly with records of this shape:

```json
{"version":1,"kind":"log","record":{"time":"2026-10-28T12:34:56.789123456+00:00","level":"INFO","message":"message","module_path":"UnchainedPlugin::example","file":"src/example.rs","line":42,"target":"UnchainedPlugin::example","thread":"main","thread_id":1234,"mdc":{},"attributes":{"__unchained_function":"start"}},"dropped":0}
```

- `version`: `1`.
- `kind`: `log` or `banner`.
- `record`: output from [log4rs' JSON encoder](https://docs.rs/log4rs/latest/log4rs/encode/json/index.html),
  captured on the emitting thread before queuing:
  - `time`: RFC 3339 local timestamp with UTC offset and variable fractional precision.
  - `level`: uppercase log level.
  - `message`: message without display-pattern headers or styling. Explicit `fn;`
    prefixes, literal content, and embedded newlines remain intact.
  - `module_path`, `file`, `line`: source metadata; omitted when unavailable.
    `line` is an unsigned 32-bit integer.
  - `target`: log target, which can differ from the module path.
  - `thread`, `thread_id`: emitting thread name (`null` when unnamed) and numeric
    native thread ID, not the pipe worker's. Read the ID as an unsigned 64-bit integer.
  - `mdc`: log4rs mapped diagnostic context.
  - `attributes`: structured log fields as strings. Captured function names use
    `__unchained_function`; that key is absent when unavailable.
- `dropped`: unsigned 64-bit count of records dropped since the previous successfully
  queued frame, including queue pressure and formatting/encoding failures.
  A trailing disconnect can prevent a final loss report.

- Validate the protocol version and frame limits.
- Read severity from `record.level`, not formatted text.
- Handle `banner` separately: `record.message` contains the banner, encoded as a
  synthetic `INFO` record without source metadata.
- Sanitize message content and metadata before terminal display.
- Do not use console encoder patterns to format structured output. Format the
  metadata in the receiving program; console/file display patterns are unchanged.

### Bounds and failure behavior

- Game threads encode into a bounded buffer and try a 256-record queue; they
  never perform pipe I/O or wait for the receiver.
- Each message contains at most 16 KiB of UTF-8 text. Oversized messages end in
  ` [truncated]`; a partial UTF-8 character is replaced safely.
- Encoded log4rs records exceeding 120 KiB are dropped, not partially emitted.
  Encoding failures also count toward `dropped`.
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

### Wine/Proton requirements

- Create the pipe inside Wine using a Windows-side process. Pass a Windows handle,
  not a Linux file descriptor.
- If relaying records over that process's stdout, reserve it for JSON frames.
  Keep game stdio and diagnostics separate.
- Read captured streams concurrently. Sanitize records before terminal display.

## Capture stdio

1. Add `--attach-stdio` to the game's arguments before creating the process.
2. Create stdout/stderr pipes. Make child write handles inheritable and parent read
   handles noninheritable. Supply valid stdin too; use an EOF pipe if no input is needed.
3. Pass the standard handles with `STARTF_USESTDHANDLES`. Include child handles in
   the inheritance list; see [Windows handle setup](#windows-handle-setup).
4. After process creation, close parent copies of child pipe ends. Start concurrent
   stdout/stderr readers **before DLL injection** or waiting for initialization.
5. Read both streams through EOF. Slow or unread pipes can block the game.

- Expect plain text: banner and enabled console records, potentially mixed with
  game output and other DLL output. There is no JSON framing or severity field.
- Redirect to files instead of pipes if no live reader is needed. The plugin
  preserves valid inherited destinations, including mixed redirection.
- Supply capture handles even for a GUI caller. The flag does not create pipes.
  Missing destinations trigger an attempt to attach to the caller's console,
  never creation of a new console.
- Under Wine/Proton, preserve the streams through to the game. If using a helper,
  connect or relay the game's stdio; redirecting only the helper is insufficient.
- Sanitize embedded controls before terminal display. Redirected output adds no
  console styling.
- Do not send commands expecting a protocol. The plugin leaves stdin unchanged
  and stops reading at EOF.

## Use a separate console

- Do not pass `--attach-stdio` or `--no-spawn-console`.
- Remove `UNCHAINED_CONSOLE_HANDLE` from the game's environment.
- Expect an attempt to allocate a console for the banner and enabled console logs.
  A new window is not guaranteed: the game may already have a console, or allocation
  may fail. Presentation depends on Windows or Wine.

## Don't spawn a separate console

- Pass `--no-spawn-console`. No capture setup is required.
- Choose an output destination separately. The flag does not redirect streams,
  attach to the caller's console, or close an existing console.
- Use structured capture or stdio capture for live output. Both already prevent
  plugin console creation when configured correctly.
- Check the [configured log file](logging.md#log-location) if no usable output
  destination exists.
- Do not expect this flag to suppress windows created by the game or other software.
