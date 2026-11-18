//! `CREATE_HOOK_KISMET!` — like `CREATE_HOOK!`, but the target is resolved at runtime via reflection
//! and the hook attaches LATE, instead of by a byte signature at preinit.
//!
//! Resolution: `KismetNativeClass::resolve(native)` gives the function's **exec thunk**
//! (`UFunction::func`); `exec_thunk::inner_native_of` then recovers the **inner C++ function** the
//! thunk calls — the real-ABI function a normal `CREATE_HOOK!` would target. The attach fn is
//! registered as a `LateHookRegistration` and drained after the engine/UObject system is ready
//! (see `postinit_rustlib`), because reflection needs the live object system.
//!
//! The body reuses `__hook_dispatch!` / `CALL_ORIGINAL!`, so `PRE`/`POST`/`NONE`, `ACTIVE`/`INACTIVE`,
//! `o_<Name>` and `CALL_ORIGINAL!(<Name>(..))` behave exactly as with `CREATE_HOOK!`.
//!
//! Caveat: it hooks the inner callee, which assumes that inner is a *distinct* function (not inlined
//! into the thunk) and not the shipping shared no-op stub. Verify at runtime for a given function.

#[macro_export]
macro_rules! CREATE_HOOK_KISMET {
    // Shorthand: default HookType = PRE.
    ($name:ident, $class:ty, $native:literal, $status:ident, $ret:ty,
     ( $( $arg:ident : $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::CREATE_HOOK_KISMET!($name, $class, $native, $status, PRE, $ret, ( $( $arg : $ty ),+ ), $body);
    };

    // Full: Name, Class (a KismetNativeClass type), "ReflectedFuncName", Status, HookType, Ret, (args), { body }
    ($name:ident, $class:ty, $native:literal, $status:ident, $hook_type:ident, $ret:ty,
     ( $( $arg:ident : $ty:ty ),+ $(,)? ), $body:expr) => {
        paste::paste! {
            #[cfg(not(rust_analyzer))]
            ::retour::static_detour! {
                pub static [<o_ $name>]: unsafe extern "C" fn( $( $ty ),+ ) -> $ret;
            }

            #[allow(non_snake_case, unused_variables)]
            pub unsafe extern "C" fn [<$name _detour_fkt>]( $( $arg : $ty ),+ ) -> $ret {
                $crate::__hook_dispatch!($hook_type, $name, $ret, ( $( $arg ),+ ), $body)
            }

            #[allow(non_snake_case, unused_unsafe)]
            unsafe fn [<attach_ $name>]() -> ::core::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {
                let class_name = <$class as $crate::ue::kismet::KismetNativeClass>::CLASS_NAME;
                let thunk = <$class as $crate::ue::kismet::KismetNativeClass>::resolve($native)
                    .ok_or_else(|| ::std::format!("{}::{} not resolved (reflection)", class_name, $native))?;
                // UFunction::func is the exec thunk; hook the inner C++ function it calls (real ABI).
                let target_addr = $crate::ue::exec_thunk::inner_native_of(thunk)
                    .ok_or_else(|| ::std::format!("{}::{} inner callee not found", class_name, $native))?;
                $crate::sdebug!("{}::{} -> thunk 0x{:x}, inner 0x{:x}", class_name, $native, thunk, target_addr);
                type FnPtr = unsafe extern "C" fn( $( $ty ),+ ) -> $ret;
                let target: FnPtr = ::core::mem::transmute(target_addr);
                [<o_ $name>].initialize(target, |$( $arg ),+| { unsafe { [<$name _detour_fkt>]( $( $arg ),+ ) } })?;
                if ($crate::__create_hook_impl![@as_cond $status])() {
                    [<o_ $name>].enable()?;
                }
                ::core::result::Result::Ok(())
            }

            ::inventory::submit! {
                $crate::resolvers::LateHookRegistration {
                    class: <$class as $crate::ue::kismet::KismetNativeClass>::CLASS_NAME,
                    native: $native,
                    attach: [<attach_ $name>],
                }
            }
        }
    };
}
