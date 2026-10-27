# Console forwarding

Launchers can receive plugin console output directly, without reading log files
or scraping a Wine console. The transport works on Windows and under Wine/Proton.
It is output-only; it does not accept commands.

## Enable

Before starting the game, the launcher must:

1. Create a private anonymous pipe with a parent read end and an inheritable child
   write end, separate from stdout/stderr.
2. Set `UNCHAINED_CONSOLE_HANDLE` in the child's environment to the unsigned
   decimal Windows handle value of the write end.
3. Pass only the intended handles through `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`
   during suspended process creation. Close the parent's copy of the child end
   after startup, including on failure.
4. Continuously drain frames until the game exits; close the read end on disposal.

There is no listening socket, named endpoint, token, or authentication handshake.
Possession of the inherited handle selects the receiver. Legacy TCP environment
variables are ignored. This is launch-local isolation, not a security boundary
against another process with permission to inspect or manipulate the game.

With valid settings, the plugin forwards configured `console` appenders and its
startup banner instead of allocating/writing its own console. File appenders,
log levels, filters, custom encoder patterns, function prefixes, and namespaces
are unchanged. No logging YAML override or UE logging flags are needed.
Without valid settings, existing console behavior is unchanged.
Explicit [`--attach-std`](console-attachment.md) overrides these settings and uses
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
private Unicode environment block. On Linux, Proton's Windows helper creates
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
