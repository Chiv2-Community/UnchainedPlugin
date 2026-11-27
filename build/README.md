# Workspace pre-build steps

[`build.rs`](build.rs) is the shared Cargo build script. It invokes `build_script!`
from [`plan/`](plan/src/lib.rs). The macro reads `[package.metadata].pre-build-steps`
from the **package being built** and calls each step's `run()` in list order.
An omitted or empty list runs no steps. Cargo then compiles the package as usual.

| Step | Implementation |
| --- | --- |
| `format` | [`format.rs`](format.rs) |
| `generate-native-binding-docs` | [`generate_native_binding_docs.rs`](generate_native_binding_docs.rs) |
| `embed-build-metadata` | [`embed_build_metadata.rs`](embed_build_metadata.rs) |

## Add a step

1. Create a file in this directory. A name such as `check-assets` maps to
   `build/check_assets.rs`: each hyphen becomes an underscore. Use lowercase
   letters and digits separated into words by hyphens. `build` is reserved.
2. Give it a `pub fn run()` entry point. For example:

   ```rust
   pub fn run() {
       println!("cargo:rerun-if-changed=assets");
       // Check the package's assets here.
   }
   ```

3. Add the step to the consuming package's `Cargo.toml`:

   ```toml
   [package.metadata]
   pre-build-steps = ["format", "check-assets"]
   ```

Only listed steps are compiled. Put any crates a step uses in the consuming
package's `[build-dependencies]`. Cargo runs the script from that package's
directory, so paths such as `assets` are relative to the package, not to `build/`.
No edit to `plan/src/lib.rs` is needed.
