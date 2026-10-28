# Console forwarding

For integrators implementing structured plugin output capture, without
reading log files or scraping a Wine console. The Windows plugin uses the same
transport on Windows and under Wine/Proton. It is output-only; it accepts no commands.

For plugin flags controlling attachment and console suppression, see
[console output options](console-attachment.md).

## Windows handle setup

Minimal Windows integration recipe:

1. Use `CreatePipe` with inheritable security attributes. Clear `HANDLE_FLAG_INHERIT`
   on the parent read end with `SetHandleInformation`; keep the child write end
   inheritable. This dedicated pipe is separate from stdout/stderr.
2. Build a private child Unicode environment block. Set `UNCHAINED_CONSOLE_HANDLE`
   to the write handle's **unsigned decimal Windows HANDLE value**, not a Linux FD.
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

## Wine/Proton integration

The plugin requires a Windows pipe handle inside Wine, not a native Linux file
descriptor. A native parent needs a Windows-side process to perform the handle
setup above and pass the handle to the game. How that process is started and
supervised is outside the plugin's interface.

If relaying records through that process's stdout, reserve it for structured
frames only. Keep game stdout/stderr separate, drain all captured streams
concurrently, and sanitize records before terminal display. Do not mix raw game
output or helper diagnostics into the JSON stream.
