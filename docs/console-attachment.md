# Console attachment and forwarding

Choose where plugin output appears when running a game or server with the plugin
loaded. No custom logging YAML is needed; file logging, filtering, function
prefixes, and namespace formatting remain unchanged. The same options work on
Windows and under Wine/Proton.

## Choose a mode

Add these plugin flags to the game or server process's launch arguments,
preserving its other arguments.

| Goal | Choose |
| --- | --- |
| Show plugin output in the parent terminal | `--attach-stdio`. Requires inherited output handles or an available parent console. |
| Forward structured plugin records to a parent process | Use a parent process that supports the [capture protocol](#structured-forwarding). No separate plugin console opens. |
| Capture stdout/stderr instead of structured records | `--attach-stdio`. It also prevents a new plugin console window. |
| Prevent a new plugin console without changing output routing | `--no-spawn-console`. It does not make output visible by itself. |

Without either flag or a valid forwarding handle, the plugin retains its usual
console behavior.
Both flags can be combined, but `--attach-stdio` already prevents console creation.

## What the flags change

`--attach-stdio` preserves valid inherited stdout/stderr, including files, pipes,
and mixed redirection. If a destination is missing, it tries the parent console
without replacing a valid inherited destination. It overrides structured
forwarding, selects output before logging and the banner, and never falls back
to creating a console. Shared console modes/palettes are not changed.

`--no-spawn-console` prevents plugin console allocation, including Wine's palette
fallback. It does not attach to a parent console, redirect streams, or disable
structured forwarding. Existing output handles remain usable; a direct launch
without usable handles or forwarding may have no visible console output.

Game argument normalization also accepts `-attach-stdio` and `-no-spawn-console`.
The old name `--attach-std` is not an alias.

## Limits and troubleshooting

- If attachment has no usable destination, no replacement window opens. Check
  the [configured log file](logging.md#log-location).
- These flags do not provide command forwarding. The plugin leaves stdin
  unchanged and stops reading at EOF.
- Ordinary stdout/stderr writes can block on a slow reader. The default
  structured transport uses a bounded queue to avoid receiver backpressure on
  game threads.

## Capture redirected output

Use `--attach-stdio` when the parent process already redirects the game's
stdout/stderr to files, pipes, or a terminal. The flag preserves those destinations;
it does not create capture pipes or a reader. Merely attaching to a GUI parent does
not establish capture. A parent capturing pipes must drain both streams.

Redirected output adds no console styling. Message-embedded controls may still
need sanitization before terminal display.

For an existing PowerShell command that already loads the plugin:

```powershell
& $gameExecutable @gameArguments --attach-stdio > .\plugin-console.log 2>&1
```

Set `$gameExecutable` to your game's executable path and `$gameArguments` to its
existing argument array. Keep the arguments needed to load the plugin; this
example does not inject it. The file receives the banner and enabled console
records without a new console window. Configured log files remain separate.

## Structured forwarding

Use this mode when your parent process supports the protocol below. Unlike raw
stdout/stderr capture, it supplies separate log-level metadata and banner records.
It is output-only and accepts no commands.

There is no plugin flag that enables forwarding by itself. Your capture tool must
set up the inherited pipe. With a compatible tool, expect the startup banner and
enabled console records in its output; configured file logging remains unchanged.
`--attach-stdio` overrides forwarding. `--no-spawn-console` does not change the
capture route.

If you are using an existing capture tool, no further setup is needed in the
plugin. The remaining reference is only for creating a custom capture tool.

## Optional: custom capture-tool reference

The parent must create a dedicated inherited Windows pipe and set the game's
`UNCHAINED_CONSOLE_HANDLE` environment variable to its write handle. Setting an
arbitrary number or a Linux file descriptor does not enable capture. If you use an
existing capture tool, it must perform this setup; the variable is not a path or
network address.

With a valid handle, the plugin forwards configured `console` appenders and its
startup banner instead of allocating/writing its own console. File appenders,
log levels, filters, custom encoder patterns, function prefixes, and namespaces
are unchanged. No logging YAML override or UE logging flags are needed.
Without a valid handle, existing console behavior is unchanged.
Explicit `--attach-stdio` takes precedence; the structured pipe is not used.

The plugin reads the environment value once during startup, validates that the
handle refers to a pipe, takes ownership, and disables further handle inheritance.
Use one pipe per game.

There is no listening socket, named endpoint, token, or authentication handshake.
Possession of the inherited handle selects the receiver. Legacy TCP environment
variables are ignored. This is launch-local isolation, not a security boundary
against another process with permission to inspect or manipulate the game.

### Windows handle setup

This reference is for custom capture tools; ordinary use requires no Win32 setup
if the parent already supports the protocol.

1. Use `CreatePipe` with inheritable security attributes. Clear `HANDLE_FLAG_INHERIT`
   on the parent read end with `SetHandleInformation`; keep the child write end
   inheritable. This dedicated pipe is separate from stdout/stderr.
2. Build a private child Unicode environment block. Set `UNCHAINED_CONSOLE_HANDLE`
   to the write handle's **unsigned decimal Windows HANDLE value**.
   Do not modify the parent process's environment.
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

### Protocol v1

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

Receivers must validate the protocol version and frame limits. Use the level
metadata for presentation rather than parsing severity from formatted text.
Banner text is separate from log records.

### Bounds and failure behavior

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

### Wine/Proton requirements

The plugin requires a Windows pipe handle inside Wine, not a native Linux file
descriptor. A native parent needs a Windows-side process to perform the handle
setup above and pass the handle to the game. How that process is started and
supervised is outside the plugin's interface.

If relaying records through that process's stdout, reserve it for structured
frames only. Keep game stdout/stderr separate, drain all captured streams
concurrently, and sanitize records before terminal display. Do not mix raw game
output or helper diagnostics into the JSON stream.
