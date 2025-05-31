# Unchained Plugin
The unchained plugin patches the Chivalry 2 Binary and allows connection to unofficial servers using an unofficial server browser. It also enables the loading of non-vanilla pak files (mods).

## Adding new hooks
Hooks are all defined in src/hooks.  New hooks can be created with `CREATE_HOOK(...)` followed by an `AUTO_HOOK` call. 
CMake will automatically update the `all_hooks.h` file on its next configuration run, and `main.h` will automatically 
initialize/scan/enable any hooks that have an `AUTO_HOOK` call.

Hook addresses/offsets are found by performing a search for a binary signature. Some signatures are the same across all
platforms, and those may be defined with `UNIVERSAL_SIGNATURE`.  Other signatures may differ between platforms. For 
these, separate signatures for each platform must be defined. This can be achieved using the `PLATFORM_SIGNATURES` 
macro, with `PLATFORM_SIGNATURE` invocations within.

Some hooks do not always need to be enabled. For example, some will only need to be active during a server launch.  For 
these situations, we use the `ATTACH_WHEN` macro, then access the global state via `g_state` and provide a statement 
that yields true/false based on if it should be enabled.  For hooks that always should be enabled, `ATTACH_ALWAYS` may 
be used.

## Version Bumps
To bump the version, the build files must be re-generated. Recompiling via cmake is not enough

## Credits
* DrLong
* Nihilianth
* Jacoby6000
* Reciate
