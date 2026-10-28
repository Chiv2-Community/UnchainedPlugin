# Console attachment and forwarding

Use this guide when your program starts Chivalry 2 and loads UnchainedPlugin.
The plugin is a DLL inside the game process, not a separate executable. Pass its
flags and environment settings to the game process before loading the plugin.

Choose how your program receives plugin output, or let the plugin display it in a
console. These options work on Windows and under Wine/Proton. They do not require
custom logging YAML or change configured file logging, levels, or message formatting.

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

Your program creates a dedicated pipe for plugin records, passes its write handle
to the game, and reads JSON records from the read end. This stream contains the
plugin's configured `console` records and startup banner, not the game's raw
stdout/stderr. Each log record includes a severity field, so your program does not
need to parse severity from formatted text.

Do not pass `--attach-stdio` in this mode. A valid forwarding handle already prevents
the plugin from allocating a separate console. You do not need `--no-spawn-console`
unless you also want to prevent allocation if the handle is invalid.

### Windows handle setup

Set up the pipe before loading the plugin. These steps use Windows APIs; for a
native Linux calling program, also follow the [Wine/Proton requirements](#wineproton-requirements).

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

The plugin reads the environment value once during startup, validates that the
handle refers to a pipe, takes ownership, and disables further handle inheritance.
Use one pipe per game. If the handle is invalid, normal console behavior applies
unless you also pass `--no-spawn-console`.

There is no listening socket, named endpoint, token, or authentication handshake.
Possession of the inherited handle selects the receiver. Legacy TCP environment
variables are ignored. This is launch-local isolation, not a security boundary
against another process with permission to inspect or manipulate the game.

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

Validate the protocol version and frame limits in your reader. Use the level
metadata for presentation rather than parsing severity from formatted text.
Display banner records separately from log records.

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

If your program runs natively on Linux, use a Windows-side process inside Wine
to create the pipe and pass its Windows handle to the game. You cannot pass a
Linux file descriptor to the plugin. How your program starts and supervises that
Windows-side process is outside the plugin's interface.

If your Windows-side process relays records over its stdout, reserve that stream
for structured frames. Keep game stdout/stderr and diagnostics separate. Read all
captured streams concurrently and sanitize records before terminal display.

## Capture stdio

Your program redirects the game's stdout/stderr and passes `--attach-stdio` in the
game's arguments. The plugin preserves valid inherited destinations, including
pipes, files, and mixed redirection. It does not create capture pipes or a reader
for your program.

1. Add `--attach-stdio` to the game's existing arguments before creating the game
   process.
2. Create stdout/stderr pipes with inheritable child write ends and noninheritable
   read ends for your program. Supply a valid stdin handle too, such as an EOF pipe
   if you do not intend to send input.
3. When creating the game on Windows, use `STARTF_USESTDHANDLES` to supply those
   standard handles. Include the child handles in the explicit inheritance list,
   as in [Windows handle setup](#windows-handle-setup).
4. Close your copies of the child pipe ends after process creation and start
   concurrent stdout/stderr readers before injecting the DLL or waiting for plugin
   initialization. Slow or unread pipes can block the game.

Your program receives plain text, including the banner and enabled console records.
This is process-wide stdio, so game output and output from other loaded DLLs can
appear in the same streams. It has no JSON framing or separate severity field.
Configured log files remain separate.

If a destination is missing, the plugin tries to attach to your program's console
without replacing valid inherited destinations. It never creates a replacement
console. A GUI calling program without a console must supply capture handles;
passing the flag alone is not enough.

On Linux, ensure your Wine/Proton invocation preserves the standard streams all
the way to the game. If you use a Windows-side helper, redirecting only its own
stdout/stderr is not enough unless it also connects or relays the game's streams.
No particular wrapper or helper protocol is required by the plugin.

Redirected output adds no console styling. Sanitize message-embedded controls
before displaying text in a terminal. Attachment does not provide a command
protocol; the plugin leaves stdin unchanged and stops reading at EOF.

## Use a separate console

To let UnchainedPlugin display its output instead of capturing it in your program:

- Do not pass `--attach-stdio` or `--no-spawn-console`.
- Remove `UNCHAINED_CONSOLE_HANDLE` from the game's environment.

The plugin attempts to allocate a console during initialization and writes its
banner and enabled console records there. If the game already has a console,
Windows can reject a second allocation; this is not a guarantee of a new window.
Console availability and window presentation also depend on Windows or Wine.

## Don't spawn a separate console

Pass `--no-spawn-console` to prevent the plugin from allocating or recreating a
console. Your program does not need to capture anything to use this flag.

This flag does not choose an output destination, attach to your program's console,
or close an existing console. Structured capture and usable existing output handles
continue to work. Without either, there may be no visible console output; consult
the [configured log file](logging.md#log-location).

To capture output without a separate console, follow either capture section above.
Structured capture with a valid handle and `--attach-stdio` each already prevent
plugin console creation. This flag does not suppress windows created by the game,
the calling program, or other software.
