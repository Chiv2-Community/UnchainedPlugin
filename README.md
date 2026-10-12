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


## Credits
* DrLong
* Nihilianth
* Jacoby6000
* Reciate
