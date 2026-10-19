# Unchained Plugin
The Unchained Plugin patches the Chivalry 2 binary to allow connection to unofficial servers via an unofficial server browser. It also enables loading of non-vanilla pak files (mods).

## Development with VS Code and RustRover

### Prerequisites
1.  **Chivalry 2**: Ensure you have the game installed.
2.  **Rust Toolchain**: Install Rust via [rustup](https://rustup.rs/).
3.  **IDE Specifics**:
    *   **VS Code**: Install `rust-analyzer` and `C/C++` extensions.
    *   **RustRover**: No additional extensions needed.

### Configuration
Both VS Code and RustRover configurations rely on the `CHIVALRY2_DIR` environment variable to locate the game files.

1.  **Define `CHIVALRY2_DIR`**: 
    Copy the `.env.example` file to a new file named `.env` in the project root. Then, update the `CHIVALRY2_DIR` path to your Chivalry 2 installation directory.
    ```env
    CHIVALRY2_DIR=C:\Program Files (x86)\Steam\steamapps\common\Chivalry 2
    ```
    *Note: The path should be the root of the Chivalry 2 installation (the folder containing the `TBL` directory).
2.  **Environment Variables in IDEs**: 
    - **VS Code**: 
        - **Debugging**: The `launch.json` configuration uses the `envFile` property to automatically load variables from your `.env` file.
        - **Tasks**: VS Code tasks use `cargo xtask` (defined in the `xtask` directory) to handle building and installation.
    - **RustRover**:
        - **Environment Variables**: For configurations that use `$CHIVALRY2_DIR$` (like launching the game), you must ensure the variable is available to the IDE. You can set it in your system environment or use a plugin like **EnvFile** to load it from `.env`.
        - **Install Task**: The **Install UnchainedPlugin** run configuration uses `cargo xtask install` which handles `.env` internally.


## Encrypted RCON

RCON uses a 256-bit pre-shared key and AES-256-GCM through a Noise handshake.
Plaintext clients are no longer supported. The listener remains loopback-only.

1. Generate a key file:
   ```sh
   python -c "import secrets; from pathlib import Path; Path('rcon.psk').write_text(secrets.token_hex(32))"
   ```
2. Restrict the file to the server administrator (Unix: `chmod 600 rcon.psk`;
   Windows: restrict its file permissions). Transfer client copies securely.
3. Start the server with `--rcon 9001 --rcon-psk-file /path/to/rcon.psk`.
   Without a readable, valid key file, RCON stays disabled; it never falls back to plaintext.
4. Build the matching client with `cargo build --release --bin rcon-client`.
   Run `rcon-client --address 127.0.0.1:9001 --psk-file /path/to/rcon.psk`,
   then enter one command per line. An optional positional argument sends one command.

The key file contains 64 hexadecimal characters, with optional surrounding whitespace.
Keep it out of source control. Rotate it by replacing the file, restarting the server,
and updating clients. Anyone holding the key can issue all RCON commands.

Commands are limited to 4096 UTF-8 bytes and cannot contain newlines or NUL.
The server closes invalid or unauthenticated connections and idle connections after
five minutes. The client does not report command results or execution acknowledgments.

See [the RCON protocol](docs/rcon.md) for client interoperability and security limits.

## Registry integration

CI and release workflows validate `plugin.json` using zero-infra-mod-registry's
metadata action. Release publication waits for validation.
`actions/automated-publish` automatically creates an issue in
[C2ModRegistry](https://github.com/Chiv2-Community/C2ModRegistry) for approval.

See [plugin workflow setup](https://github.com/Chiv2-Community/zero-infra-mod-registry/blob/v1/docs/plugin/README.md)
and [release-request inputs](https://github.com/Chiv2-Community/zero-infra-mod-registry/blob/v1/docs/plugin/release-requests.md).

## Credits
* DrLong
* Nihilianth
* Jacoby6000
* Reciate
