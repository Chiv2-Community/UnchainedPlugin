# Unchained Plugin
The Unchained Plugin patches the Chivalry 2 binary to allow connection to unofficial servers via an unofficial server browser. It also enables loading of non-vanilla pak files (mods).

## Additional pak directory

Pass `--additional-pak-dir "C:\Mods\Paks"` to the game to load paks from an
additional directory. The directory must already exist. Relative paths are
resolved from the game's working directory; the default pak directories remain enabled.

Mod paks do not need `.sig` sidecars. Steam and EGS use the same archive-hook
signature; runtime loading has been verified on Steam only.
Do not pass `-signed` or `-signedpak`; these force signed reader setup.

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
