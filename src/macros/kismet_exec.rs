//! Macros for hooking Unreal **UFunction native exec thunks** (`execPrintString`, `execIsServer`, …).
//!
//! These targets are resolved by their reflected name from a UClass's `RegisterNatives` table at
//! runtime, so they must attach LATE (postinit, after `init_globals`) — they cannot go through the
//! preinit `CREATE_HOOK!`/`HookRegistration` path. Each hook submits a `LateHookRegistration` that
//! `postinit_rustlib` drains once.
//!
//! Surface:
//! - `kismet_native_class!(UKismetSystemLibrary => globals.kismet_system_library_native);`
//!   binds a class to its existing `globals()` resolver (see `KismetNativeClass`).
//! - `kismet_exec_observe!(Class::Native => detour(args…) { body })` — step the real frame once per
//!   declared argument (in order), run `P_FINISH`, run `body`, and DO NOT call the original. For
//!   dummied no-op callees (the print family). `bool` reads a 4-byte VM bool; `FString` frees on
//!   drop; `FText` is wrapped in `OwnedText` (released on drop) and exposes `.to_string()`.
//! - `kismet_exec_forward!(Class::Native => detour { body })` — call the original (which consumes
//!   the frame), then run `body` with `context`/`result` in scope (e.g. overwrite `*result`).
//!
//! Each invocation emits exactly one `retour::static_detour!` (retour's macro recursion limit is
//! hit if several are combined in one invocation).

/// Bind a reflected UClass to the `globals()` method that resolves its natives by name.
/// Expands to a unit struct + `impl KismetNativeClass`. The resolver method must already exist
/// (it is what locates that class's `RegisterNatives` table during `init_globals`).
#[macro_export]
macro_rules! kismet_native_class {
    ($class:ident => globals.$method:ident) => {
        #[allow(non_camel_case_types, dead_code)]
        pub struct $class;
        impl $crate::ue::kismet::KismetNativeClass for $class {
            const CLASS_NAME: &'static str = stringify!($class);
            unsafe fn resolve(native: &str) -> ::core::option::Option<usize> {
                $crate::tools::hook_globals::globals().$method(native)
            }
        }
    };
}

/// Observe/consume a native exec thunk: step every declared argument off the real frame, finish,
/// run the body, and never call the original. See the module docs.
#[macro_export]
macro_rules! kismet_exec_observe {
    // ── argument stepping (internal tt-muncher) ────────────────────────────────
    (@step $frame:ident, ) => {};
    (@step $frame:ident) => {};
    (@step $frame:ident, $name:ident : bool $(, $($rest:tt)*)?) => {
        let $name: bool = $crate::ue::kismet::FFrame::arg::<u32>($frame) != 0;
        $crate::kismet_exec_observe!(@step $frame, $($($rest)*)?);
    };
    (@step $frame:ident, $name:ident : FText $(, $($rest:tt)*)?) => {
        let $name = $crate::resolvers::messages::broadcast_display::OwnedText(
            $crate::ue::kismet::FFrame::arg::<$crate::game::engine::FText>($frame),
        );
        $crate::kismet_exec_observe!(@step $frame, $($($rest)*)?);
    };
    (@step $frame:ident, $name:ident : $ty:ty $(, $($rest:tt)*)?) => {
        let $name: $ty = $crate::ue::kismet::FFrame::arg::<$ty>($frame);
        $crate::kismet_exec_observe!(@step $frame, $($($rest)*)?);
    };

    // ── public entry ───────────────────────────────────────────────────────────
    ($class:ident :: $native:ident => $detour:ident ( $($args:tt)* ) $body:block) => {
        paste::paste! {
            #[cfg(not(rust_analyzer))]
            ::retour::static_detour! {
                static [<ORIG_ $detour>]: unsafe extern "system" fn(
                    *mut $crate::ue::UObject, *mut $crate::ue::kismet::FFrame, *mut ::core::ffi::c_void,
                );
            }

            #[allow(non_snake_case, unused_variables, unused_unsafe)]
            unsafe extern "system" fn $detour(
                _context: *mut $crate::ue::UObject,
                stack: *mut $crate::ue::kismet::FFrame,
                _result: *mut ::core::ffi::c_void,
            ) {
                let ::core::option::Option::Some(frame) = (unsafe { stack.as_mut() }) else { return };
                unsafe {
                    $crate::kismet_exec_observe!(@step frame, $($args)*);
                    $crate::ue::kismet::FFrame::finish(frame);
                    $body
                }
            }

            #[allow(non_snake_case)]
            unsafe fn [<attach_ $detour>]() -> ::core::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {
                let addr = <$class as $crate::ue::kismet::KismetNativeClass>::resolve(stringify!($native))
                    .ok_or_else(|| ::std::format!(
                        "{}::{} not resolved",
                        <$class as $crate::ue::kismet::KismetNativeClass>::CLASS_NAME,
                        stringify!($native),
                    ))?;
                unsafe {
                    let target: $crate::ue::kismet::FNativeExec = ::core::mem::transmute(addr);
                    [<ORIG_ $detour>].initialize(target, |c, s, r| $detour(c, s, r))?;
                    [<ORIG_ $detour>].enable()?;
                }
                ::core::result::Result::Ok(())
            }

            ::inventory::submit! {
                $crate::resolvers::LateHookRegistration {
                    class: <$class as $crate::ue::kismet::KismetNativeClass>::CLASS_NAME,
                    native: stringify!($native),
                    attach: [<attach_ $detour>],
                }
            }
        }
    };
}

/// Forward a native exec thunk: call the original (which steps + finishes the frame), then run the
/// body with `context`/`result` in scope. Use when the callee is NOT a no-op and you only want to
/// observe or override its result. See the module docs.
#[macro_export]
macro_rules! kismet_exec_forward {
    ($class:ident :: $native:ident => $detour:ident $body:block) => {
        paste::paste! {
            #[cfg(not(rust_analyzer))]
            ::retour::static_detour! {
                static [<ORIG_ $detour>]: unsafe extern "system" fn(
                    *mut $crate::ue::UObject, *mut $crate::ue::kismet::FFrame, *mut ::core::ffi::c_void,
                );
            }

            #[allow(non_snake_case, unused_variables, unused_unsafe)]
            unsafe extern "system" fn $detour(
                context: *mut $crate::ue::UObject,
                stack: *mut $crate::ue::kismet::FFrame,
                result: *mut ::core::ffi::c_void,
            ) {
                unsafe { [<ORIG_ $detour>].call(context, stack, result) };
                unsafe { $body }
            }

            #[allow(non_snake_case)]
            unsafe fn [<attach_ $detour>]() -> ::core::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {
                let addr = <$class as $crate::ue::kismet::KismetNativeClass>::resolve(stringify!($native))
                    .ok_or_else(|| ::std::format!(
                        "{}::{} not resolved",
                        <$class as $crate::ue::kismet::KismetNativeClass>::CLASS_NAME,
                        stringify!($native),
                    ))?;
                unsafe {
                    let target: $crate::ue::kismet::FNativeExec = ::core::mem::transmute(addr);
                    [<ORIG_ $detour>].initialize(target, |c, s, r| $detour(c, s, r))?;
                    [<ORIG_ $detour>].enable()?;
                }
                ::core::result::Result::Ok(())
            }

            ::inventory::submit! {
                $crate::resolvers::LateHookRegistration {
                    class: <$class as $crate::ue::kismet::KismetNativeClass>::CLASS_NAME,
                    native: stringify!($native),
                    attach: [<attach_ $detour>],
                }
            }
        }
    };
}
