# Console forwarding

Launchers can receive plugin console output directly, without reading log files
or scraping a Wine console. The transport works on Windows and under Wine/Proton.
It is output-only; it does not accept commands.

## Enable

Before starting the game, the launcher must:

1. Listen on an ephemeral TCP port on `127.0.0.1` (or `::1`).
2. Generate a fresh 32-byte cryptographically random token, encoded as 64 hex digits.
3. Set these variables in the game's environment:
   - `UNCHAINED_CONSOLE_ENDPOINT`: numeric loopback address and port.
   - `UNCHAINED_CONSOLE_TOKEN`: the token. Never put it in game arguments or logs.
4. Authenticate the connection and continuously drain frames until the game exits.

With valid settings, the plugin forwards configured `console` appenders and its
startup banner instead of allocating/writing its own console. File appenders,
log levels, filters, custom encoder patterns, function prefixes, and namespaces
are unchanged. No logging YAML override or UE logging flags are needed.
Without valid settings, existing console behavior is unchanged.
Explicit [`--attach-std`](console-attachment.md) overrides these settings and uses
standard output instead; no IPC connection is made.

Environment values are read once during startup. Use one listener/token per
game. Environment inheritance is not a security boundary against other processes
running as the same user; receivers must still validate the token and frame limits.

## Protocol v1

UTF-8 JSON objects, one per LF-delimited frame. Embedded newlines are JSON-escaped.
The plugin connects and sends:

```json
{"version":1,"kind":"hello","token":"<64 hex digits>"}
```

The receiver validates version, kind, and token, then writes exactly `OK\n`.
No log contents are sent before that acknowledgement. Reject invalid handshakes
without accepting their output, and continue waiting for the intended game.

Subsequent frames have this shape:

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
  never perform network I/O or wait for the receiver.
- Each record contains at most 16 KiB of UTF-8 text. Oversized records end in
  ` [truncated]`; a partial UTF-8 character is replaced safely.
- JSON escaping can expand text. Receivers should limit frames to 128 KiB and
  bound retained history independently of the socket buffer.
- A background worker has two-second connection, handshake, and write timeouts.
- A failed/disconnected receiver stops forwarding for that launch. There is no
  reconnect or replay. Configured file appenders continue independently; the
  plugin does not create a fallback graphical console or recursively log errors.
- `flush` never waits for network I/O. Abrupt process exit can lose queued records.

`tty_only` console settings do not disable an explicitly requested transport.
Custom file-only configurations do not gain a console appender automatically.

## Launcher integration

The CLI starts the listener before creating the game. On Windows, it passes a
private Unicode environment block to the suspended process; on Linux, settings
pass through Proton's helper. Neither changes the launcher's environment.
The receiver validates/authenticates frames,
sanitizes controls, and keeps bounded per-server history. `L` switches between
the dashboard and full console, Page Up/Page Down scroll, and `End` follows the
latest output. No-TUI mode prints received lines without terminal controls.

This requires a plugin build containing the transport; older plugins retain
their own console. Client launches remain detached and do not start a TUI receiver.
