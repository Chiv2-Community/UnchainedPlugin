
// Use pre-release version of rust-analyzer if this causes errors
/// Generates a detour hook for a function, handles auto-registration, and 
/// 
/// provides rust-analyzer compatible type-hinting within the hook body.
///
/// # Supported Formats
///
/// 1. **Basic Shorthand** (Defaults: ACTIVE, PRE, void)
///    ```rust,ignore
///    CREATE_HOOK!(Name, (args), { body }); 
///    ```
/// 2. **Explicit Status** (Defaults: PRE, void)
///    ```rust,ignore
///    CREATE_HOOK!(Name, Status, (args), { body });
///    ```
///
/// 3. **Explicit Return Type** (Defaults: ACTIVE, PRE)
///    ```rust,ignore
///    CREATE_HOOK!(Name, RetType, (args), { body });
///    ```
///
/// 4. **Status & Return Type** (Defaults: PRE)
///    ```rust,ignore
///    CREATE_HOOK!(Name, Status, RetType, (args), { body });
///    ```
///
/// 5. **Full Configuration**
///    ```rust,ignore
///    CREATE_HOOK!(Name, Status, HookType, RetType, (args), { body });
///    ```
///
/// # Note on Testing
/// These examples are marked `ignore` because they require external state (like `o_Name` statics) 
/// and complex boilerplate to be fully runnable in a doctest.
///
/// # Arguments
/// * `Name` - The identifier of the function (used to generate `o_Name` and `attach_Name`).
/// * `Status` - `ACTIVE` or `INACTIVE`. Controls if the hook enables on startup.
/// * `HookType` - 
///     * `NONE`: The body is the complete implementation. You are responsible for calling `o_Name.call()`.
///     * `PRE`: The body runs first, then the original function is called automatically.
///     * `POST`: The original function runs first, then the result is passed to your body.
///     * If no HookType is provided, defaults to PRE
/// * `RetType` - The Rust type returned by the original function (e.g., `*mut c_void`, `bool`).
///   Shorthands without a return type are only for native `void` functions;
///   pointer-returning targets must specify their pointer type explicitly.
/// * `(args)` - A comma-separated list of `name: Type` pairs matching the function signature.
/// * `{ body }` - The code block for the detour. Must return `RetType`.
/// 
/// 
/// 
#[macro_export]
macro_rules! CREATE_HOOK {
    ($($tt:tt)*) => {
        $crate::__create_hook_inner!($($tt)*);
    };
}

#[macro_export]
macro_rules! __create_hook_inner {
    // 1. Basic Shorthand: Name, (args), {body}
    ($name:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, ACTIVE, PRE, (), ( $( $arg: $ty ),+ ), $body);
    };

    // 2. Status Identifier: Name, ACTIVE/INACTIVE, (args), {body}
    ($name:ident, $status:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, PRE, (), ( $( $arg: $ty ),+ ), $body);
    };

    // 3. NEW: Explicit Condition Block/Closure: Name, { || cond }, (args), {body}
    ($name:ident, { $cond:expr }, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, { $cond }, PRE, (), ( $( $arg: $ty ),+ ), $body);
    };

    // 4. Explicit Return Type: Name, OutType, (args), {body}
    ($name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, ACTIVE, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };

    // 5. Status & Return Type: Name, Status, OutType, (args), {body}
    ($name:ident, $status:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };

    // 6. Full Configuration: Name, Status, HookType, OutType, (args), {body}
    // $status can now be an ident (ACTIVE) or a block { || globals().enabled }
    ($name:ident, $status:tt, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, $hook_type, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
}

#[macro_export]
macro_rules! __create_hook_impl {
    // Internal helper to turn Status/Expressions into fn() -> bool
    (@as_cond ACTIVE) => { || true };
    (@as_cond INACTIVE) => { || false };
    (@as_cond CALLED) => { || false };
    (@as_cond DISABLED) => { || false };
    (@as_cond { $cond:expr }) => { $cond }; 
    (@as_cond $cond:expr) => { $cond };

    ($name:ident, $status:tt, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        paste::paste! {
            #[cfg(not(rust_analyzer))]
            ::retour::static_detour! {
                pub static [<o_ $name>]: unsafe extern "C" fn($( $ty ),+) -> $out_type;
            }

            #[allow(non_snake_case, unused_variables)]
            pub unsafe extern "C" fn [<$name _detour_fkt>]($( $arg: $ty ),+ ) -> $out_type {
                $crate::__hook_dispatch!($hook_type, $name, $out_type, ( $( $arg ),+ ), $body)
            }

            #[allow(non_snake_case)]
            pub unsafe fn [<attach_ $name>](
                offsets: std::collections::HashMap<String, u64>,
                auto_activate: bool
            ) -> Result<Option<usize>, Box<dyn std::error::Error>> {
                match offsets.get(stringify!($name)) {
                    None => Err(format!("No address found for {}", stringify!($name)).into()),
                    Some(off) => {
                        let rel_address = *off as usize;
                        type FnPtr = unsafe extern "C" fn ($( $ty ),+ ) -> $out_type;
                        let base_address = * $crate::resolvers::BASE_ADDR;
                        let target: FnPtr = std::mem::transmute(base_address + rel_address);
                        
                        
                        [<o_ $name>].initialize(target, |$( $arg ),+| {
                            unsafe { [<$name _detour_fkt>]($( $arg ),+) }
                        }).unwrap();
                        // $crate::info!("Set up {}", stringify!([<$name _detour_fkt>]));
                        
                        // We combine the global 'auto_activate' flag with the local condition
                        // Note: We evaluate the condition here at attachment time
                        let condition_met = ($crate::__create_hook_impl![@as_cond $status])();
                        
                        if auto_activate && condition_met {
                            [<o_ $name>].enable()?;
                        }
                        Ok(Some(rel_address))
                    }
                }
            }

            inventory::submit! {
                $crate::resolvers::HookRegistration {
                    name: stringify!($name),
                    hook_fn: [<attach_ $name>],
                    // Store the condition as a function pointer for runtime re-checks if needed
                    condition: $crate::__create_hook_impl![@as_cond $status] as fn() -> bool,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! __hook_dispatch {
    // NONE: Classic hook
    (NONE, $name:ident, $out_type:ty, ( $( $arg:expr ),+ ), $body:expr) => { 
        $body 
    };
    
    // PRE: User code, then original function.
    (PRE, $name:ident, $out_type:ty, ( $( $arg:expr ),+ ), $body:expr) => {
        {
            // internal scope to not leak vars, force () return
            { $body }; 
            CALL_ORIGINAL![$name($( $arg ),+)]
        }
    };

    // POST: Original function, then user code -> return value
    (POST, $name:ident, $out_type:ty, ( $( $arg:expr ),+ ), $body:expr) => {
        {
            let ret_val = CALL_ORIGINAL![$name($( $arg ),+)];
            { $body(ret_val) }
        }
    };
}

/// Calls the original detour function `o_<name>` with arguments.
/// Preserves type hints for Rust Analyzer.
#[macro_export]
macro_rules! CALL_ORIGINAL {
    ($name:ident ( $($arg:expr),* $(,)? )) => {{
        paste::paste! {
            #[allow(clippy::macro_metavars_in_unsafe)] // FIXME: I guess
            #[allow(unused_unsafe)]
            unsafe { [<o_ $name>].call($($arg),*) }
        }
    }};
}

/// Calls the original detour function `o_<name>` with arguments.
/// Preserves type hints for Rust Analyzer.
/// Returns if detour is disabled
#[macro_export]
macro_rules! TRY_CALL_ORIGINAL {
    ($name:ident ( $($arg:expr),* $(,)? )) => {{
        paste::paste! {
            #[allow(unused_unsafe)]
            let is_init = [<o_ $name>].is_enabled() || unsafe { [<o_ $name>].disable().is_ok() };
            if !is_init {
                $crate::warn!("Detour {} is not initialized. Skipping.", stringify!($name));
                return; // The "TRY" part: exit the caller
            }
            
            $crate::CALL_ORIGINAL!($name($($arg),*))
        }
    }};
}
/// Calls the original detour function `o_<name>` with arguments.
/// Preserves type hints for Rust Analyzer.
/// Returns Err if detour is disabled
#[macro_export]
macro_rules! CALL_ORIGINAL_SAFE {
    ($name:ident ( $($arg:expr),* $(,)? )) => {{
        paste::paste! {
            let is_init = [<o_ $name>].is_enabled() || unsafe { [<o_ $name>].disable().is_ok() };
            if !is_init {
                Err(format!("Detour {} is not initialized", stringify!($name)))
            } else {
                Ok(unsafe { [<o_ $name>].call($($arg),*) })
            }
        }
    }};
}

#[macro_export]
macro_rules! TRY_OR_RETURN {
    ($call:expr) => {
        match $call {
            Ok(val) => val,
            Err(e) => {
                $crate::error!("Error: {}", e);
                return;
            }
        }
    };
}

#[cfg(test)]
mod tests {
    CREATE_HOOK!(TestVoidBasic, (value: i32), {});
    CREATE_HOOK!(TestVoidStatus, INACTIVE, (value: i32), {});
    CREATE_HOOK!(TestVoidCondition, { || false }, (value: i32), {});
    CREATE_HOOK!(TestConstPointer, INACTIVE, *const std::ffi::c_void,
        (value: *const std::ffi::c_void), {});

    #[test]
    fn shorthand_hooks_return_unit() {
        // Check the generated detours and original-call signatures without
        // attaching hooks or calling uninitialized detours.
        let _: unsafe extern "C" fn(i32) = TestVoidBasic_detour_fkt;
        let _: unsafe extern "C" fn(i32) = TestVoidStatus_detour_fkt;
        let _: unsafe extern "C" fn(i32) = TestVoidCondition_detour_fkt;
        let _: fn(i32) = |value| CALL_ORIGINAL!(TestVoidBasic(value));
        let _: fn(i32) = |value| CALL_ORIGINAL!(TestVoidStatus(value));
        let _: fn(i32) = |value| CALL_ORIGINAL!(TestVoidCondition(value));
    }

    #[test]
    fn explicit_const_pointer_return_is_preserved() {
        type Pointer = *const std::ffi::c_void;
        let _: unsafe extern "C" fn(Pointer) -> Pointer = TestConstPointer_detour_fkt;
        let _: fn(Pointer) -> Pointer = |value| CALL_ORIGINAL!(TestConstPointer(value));
    }
}
