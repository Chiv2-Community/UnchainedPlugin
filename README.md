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

For server-browser registration, `--local-ip` is the advertised address. Add
`--use-ipv6` to discover a non-loopback global IPv6 address automatically,
bind the registration HTTP client to it, and use it for the transport. An IPv6 backend URL
selects the registration transport family. An explicit `--local-ip <IPv6>`
still takes precedence when the host has multiple global IPv6 addresses. The
server-browser backend URL must itself be reachable over IPv6 for the request
source address to be IPv6.

In normal IPv4 mode, the default `local_ip` remains `127.0.0.1` so local test
servers can appear in the browser. In IPv6 auto mode, the discovered IPv6 is
used internally for source binding but is not published unless `--local-ip`
was explicitly supplied.

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


### Building the plugin

The root `Cargo.toml` is a virtual workspace. `unchained-plugin/` contains the
DLL crate, its sources, and resources. Workspace pre-build steps live in [`build/`](build/README.md). On Windows,
run `cargo build` from the workspace root; the plugin is the default member.

### Building the Windows DLL on Linux

`scripts/build-windows.sh` cross-compiles the Windows DLL with
[cargo-xwin](https://github.com/rust-cross/cargo-xwin), which downloads and
configures the Windows SDK and CRT.

1. Install the native C/C++ build tools and LLVM. On Debian/Ubuntu:

   ```sh
   sudo apt update
   sudo apt install build-essential clang llvm lld
   ```

   On other distributions, install the equivalent packages. Ensure `clang-cl`,
   `llvm-lib`, `llvm-rc`, and `lld-link` are available on `PATH`.

2. Install Rust using [rustup](https://rustup.rs/), then open a new shell so
   `cargo` and `rustup` are available on the path.

3. Install cargo-xwin. This revision includes the unreleased
   fix for SDK cache paths containing spaces:

   ```sh
   cargo install cargo-xwin --locked \
     --git https://github.com/rust-cross/cargo-xwin.git \
     --rev 046254898add819f57f584feb4ddabdf2f321b6a
   ```

4. Build the DLL. The first build needs network access for the SDK and Cargo
   dependencies:

   ```sh
   bash scripts/build-windows.sh --release
   ```

Additional arguments go to `cargo xwin build`. Without `--release`, it builds the
debug DLL. Output is
`target/x86_64-pc-windows-msvc/release/UnchainedPlugin.dll` (or `debug`).

Or build manually without the script:

```sh
XWIN_ARCH=x86_64 cargo xwin build --locked --target x86_64-pc-windows-msvc --lib --release
```

## Discord delivery

See [Discord delivery](docs/discord-delivery.md) for queueing, batching, ordering,
and diagnosing delayed messages.

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
