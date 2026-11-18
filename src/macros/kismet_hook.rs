//! `CREATE_HOOK_KISMET!` — a `CREATE_HOOK!`-shaped surface for **reflection-resolved, late-attached**
//! native hooks. It is a thin set of forwarding arms; all substantive logic (detour static, detour
//! fn, initialize/enable) is the shared `@emit_core` / `@finish_attach` / `@kismet` impl in
//! `__create_hook_impl` (`hook_macros.rs`), so nothing is duplicated.
//!
//! The target is resolved through the live UObject reflection system:
//! `KismetNativeClass::resolve(native)` → the function's exec thunk → `exec_thunk::inner_native_of`
//! → the inner C++ function (real ABI). Because reflection needs the live object system, the hook
//! registers as a `LateHookRegistration` and attaches after `ENGINE_READY` — not at preinit. Use the
//! plain `CREATE_HOOK!` (signature/offset, preinit) for non-reflected functions or anything needed
//! early; see `docs/kismet-handoff.md`.
//!
//! Surface mirrors `CREATE_HOOK!`, prefixed with the class + reflected name:
//! ```ignore
//! CREATE_HOOK_KISMET!(Class, "Native", Name, (args), { body });                          // ACTIVE, PRE, void
//! CREATE_HOOK_KISMET!(Class, "Native", Name, Status, (args), { body });                  // PRE, void
//! CREATE_HOOK_KISMET!(Class, "Native", Name, { cond }, (args), { body });                // PRE, void
//! CREATE_HOOK_KISMET!(Class, "Native", Name, Ret, (args), { body });                     // ACTIVE, PRE
//! CREATE_HOOK_KISMET!(Class, "Native", Name, Status, Ret, (args), { body });             // PRE
//! CREATE_HOOK_KISMET!(Class, "Native", Name, Status, HookType, Ret, (args), { body });   // full
//! ```
//! `o_<Name>` / `CALL_ORIGINAL!` / PRE/POST/NONE / ACTIVE/INACTIVE/CALLED/`{cond}` all behave as in
//! `CREATE_HOOK!`. (`Class` is a `KismetNativeClass` type; see `kismet_native_class!`.)

#[macro_export]
macro_rules! CREATE_HOOK_KISMET {
    // 1. Basic: ACTIVE, PRE, void
    ($class:ty, $native:literal, $name:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!(@kismet ($class, $native) $name, ACTIVE, PRE, (), ( $( $arg: $ty ),+ ), $body);
    };

    // 2. Status: PRE, void
    ($class:ty, $native:literal, $name:ident, $status:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!(@kismet ($class, $native) $name, $status, PRE, (), ( $( $arg: $ty ),+ ), $body);
    };

    // 3. Condition block/closure: PRE, void
    ($class:ty, $native:literal, $name:ident, { $cond:expr }, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!(@kismet ($class, $native) $name, { $cond }, PRE, (), ( $( $arg: $ty ),+ ), $body);
    };

    // 4. Return type: ACTIVE, PRE
    ($class:ty, $native:literal, $name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!(@kismet ($class, $native) $name, ACTIVE, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };

    // 5. Status + return type: PRE
    ($class:ty, $native:literal, $name:ident, $status:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!(@kismet ($class, $native) $name, $status, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };

    // 6. Full: Status, HookType, return type ($status may be an ident or a { cond } block)
    ($class:ty, $native:literal, $name:ident, $status:tt, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!(@kismet ($class, $native) $name, $status, $hook_type, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
}
