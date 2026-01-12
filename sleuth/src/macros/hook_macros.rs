
// Use pre-release version of rust-analyzer if this causes errors
/// Generates a detour hook for a function, handles auto-registration, and 
/// 
/// provides rust-analyzer compatible type-hinting within the hook body.
///
/// # Supported Formats
///
/// 1. **Basic Shorthand** (Defaults: ACTIVE, NONE, void)
///    ```rust
///    CREATE_HOOK!(Name, (args), { body }); 
///    ```
/// 2. **Explicit Status** (Defaults: NONE, void)
///    ```rust
///    CREATE_HOOK!(Name, Status, (args), { body });
///    ```
///
/// 3. **Explicit Return Type** (Defaults: ACTIVE, NONE)
///    ```rust
///    CREATE_HOOK!(Name, RetType, (args), { body });
///    ```
///
/// 4. **Status & Return Type** (Defaults: NONE)
///    ```rust
///    CREATE_HOOK!(Name, Status, RetType, (args), { body });
///    ```
///
/// 5. **Full Configuration**
///    ```rust
///    CREATE_HOOK!(Name, Status, HookType, RetType, (args), { body });
///    ```
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
        $crate::__create_hook_impl!($name, ACTIVE, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };

    // 2. Status Identifier: Name, ACTIVE/INACTIVE, (args), {body}
    ($name:ident, $status:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };

    // 3. NEW: Explicit Condition Block/Closure: Name, { || cond }, (args), {body}
    ($name:ident, { $cond:expr }, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, { $cond }, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
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
    (@as_cond { $cond:expr }) => { $cond }; 
    (@as_cond $cond:expr) => { $cond };

    ($name:ident, $status:tt, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        paste::paste! {
            #[cfg(not(rust_analyzer))]
            ::retour::static_detour! {
                pub static [<o_ $name>]: unsafe extern "C" fn($( $ty ),+) -> $out_type;
            }

            #[allow(non_snake_case, unused_variables)]
            pub fn [<$name _detour_fkt>]($( $arg: $ty ),+ ) -> $out_type {
                $crate::__hook_dispatch!($hook_type, $name, $out_type, ( $( $arg ),+ ), $body)
            }

            #[allow(non_snake_case)]
            pub unsafe fn [<attach_ $name>](
                base_address: usize,
                offsets: std::collections::HashMap<String, u64>,
                auto_activate: bool
            ) -> Result<Option<usize>, Box<dyn std::error::Error>> {
                match offsets.get(stringify!($name)) {
                    None => Err(format!("No address found for {}", stringify!($name)).into()),
                    Some(off) => {
                        let rel_address = *off as usize;
                        type FnPtr = unsafe extern "C" fn ($( $ty ),+ ) -> $out_type;
                        let target: FnPtr = std::mem::transmute(base_address + rel_address);
                        
                        
                        // Leak detour closure
                        let detour_fn: Box<fn($( $ty ),+) -> $out_type> =
                            Box::new([<$name _detour_fkt>]);
                        let detour_fn: &'static _ = Box::leak(detour_fn);
                        [<o_ $name>].initialize(target, detour_fn).unwrap();
                        // let _ = [<o_ $name>].initialize(target, [<$name _detour_fkt>]);
                        $crate::sinfo!(f; "Set up {}", stringify!([<$name _detour_fkt>]));
                        
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
            let is_init = [<o_ $name>].is_enabled() || [<o_ $name>].disable().is_ok();
            if !is_init {
                eprintln!("⚠️ Detour {} is not initialized. Skipping.", stringify!($name));
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
                eprintln!("Error: {}", e);
                return;
            }
        }
    };
}