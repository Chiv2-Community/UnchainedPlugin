# Console output and attachment

UnchainedPlugin uses log4rs appenders and encoders for all logs, including the startup
banner. Choose the format in [logging configuration](logging.md#use-a-custom-configuration).
Choose the destination with the appender, independently of console attachment.

| Goal | Setup |
| --- | --- |
| Capture structured plugin logs | [Select the JSON encoder](#capture-structured-logs), then capture stdio or configure a file appender. |
| Capture stdio | [Redirect the game's stdout/stderr](#capture-stdio) and pass `--attach-stdio`. |
| Use a separate console | [Leave console allocation enabled](#use-a-separate-console). |
| Don't spawn a separate console | [Pass `--no-spawn-console`](#dont-spawn-a-separate-console). |

## What the flags change

- `--attach-stdio` preserves valid inherited stdout/stderr. If either is missing,
  the plugin attempts to attach to the caller's console. It never allocates a
  console or changes the shared console's modes or palette.
- `--no-spawn-console` suppresses plugin console allocation and Wine palette
  recreation. It does not redirect output or attach to a parent console.
- Both leave log4rs formats, filters, and file destinations unchanged.
- Game argument normalization also accepts `-attach-stdio` and `-no-spawn-console`.
  Use the double-hyphen spelling in new integrations.

## Capture structured logs

Select `kind: json` on each appender that should emit JSON. For example:

```yaml
appenders:
  console:
    kind: console
    target: stdout
    encoder:
      kind: json
  file:
    kind: file
    path: 'C:\Unchained\plugin.jsonl'
    encoder:
      kind: json
root:
  level: debug
  appenders: [console, file]
```

Pass the configuration with `--log4rs-yaml C:\Unchained\logging.yaml`. A custom
configuration replaces the default; retain any other required appenders and filters.
For file-only logging, remove the console appender and its root reference.

Each JSON appender writes one UTF-8 JSON object per line, with the
[standard log4rs fields](https://docs.rs/log4rs/latest/log4rs/encode/json/index.html)
at the root:

```json
{"time":"2026-10-28T12:34:56.789123456+00:00","level":"INFO","message":"[start] Server started","module_path":"UnchainedPlugin::server","file":"src/server.rs","line":42,"target":"UnchainedPlugin::server","thread":"main","thread_id":1234,"mdc":{},"attributes":{"__unchained_function":"start"}}
```

- Read function names from `attributes.__unchained_function` when present.
- Preserve explicit `fn;` prefixes: they are intentional message content.
- Expect unavailable `module_path`, `file`, and `line` fields to be omitted.
- Handle the startup banner as an ordinary `INFO` log message.
- There is no plugin envelope, protocol version, dedicated handle, or loss counter.
- Select the encoder independently per sink; JSON files can coexist with a
  human-readable console.

## Capture stdio

1. Pass `--attach-stdio` to the game before creating the process.
2. Redirect stdout/stderr to files, or create ordinary stdio pipes. For Windows
   pipes, make only the child ends inheritable and pass them with
   `STARTF_USESTDHANDLES` and an explicit handle inheritance list.
3. Supply valid stdin too; use an EOF pipe if no input is needed.
4. Close parent copies of child pipe ends after process creation. Start concurrent
   stdout/stderr readers before DLL injection or waiting for initialization.
5. Drain both streams through EOF. Slow sinks can block logging threads.

- The flag does not create pipes. Supply valid destinations even for a GUI caller.
- Read the format selected by each log4rs appender: text or JSON.
- Expect game and other DLL output on the same streams. For a dedicated
  machine-readable log, configure a JSON file appender instead.
- Under Wine/Proton, preserve or relay the game's stdio through any Windows-side
  helper. Redirecting only that helper is insufficient.
- Sanitize embedded controls before terminal display. Redirected plugin output
  adds no console styling.
- The plugin leaves stdin unchanged and stops reading at EOF. Do not expect an
  input protocol.

## Use a separate console

- Do not pass `--attach-stdio` or `--no-spawn-console`.
- Expect an attempt to allocate a console. A new window is not guaranteed: the
  game may already have a console, or allocation may fail.
- Configure a console appender for output there. File-only configuration remains
  file-only, including the banner.

## Don't spawn a separate console

- Pass `--no-spawn-console`; no capture setup is required.
- Select an output destination separately. This flag does not redirect streams,
  attach to a parent console, or close an existing console.
- Use `--attach-stdio` for inherited output; it already prevents plugin allocation.
- Check the [configured log file](logging.md#log-location) if no usable output
  destination exists.
- This flag does not suppress windows created by the game or other software.
