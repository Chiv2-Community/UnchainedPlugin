
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
    // Format: Name, (args), {body}
    ($name:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, ACTIVE, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };
    // Format: Name, ACTIVE, (args), {body}
    ($name:ident, $status:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };
    // Format: Name, OutType, (args), {body}
    ($name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, ACTIVE, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
    // Format: Name, Status, OutType, (args), {body}
    ($name:ident, $status:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
    // Format: Name, Status, HookType, OutType, (args), {body}
    ($name:ident, $status:ident, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, $hook_type, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
}

#[macro_export]
macro_rules! __create_hook_impl {
    (@is_active ACTIVE) => { true };
    (@is_active INACTIVE) => { false };

    ($name:ident, $status:ident, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
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
                offsets: std::collections::HashMap<String, u64>
            ) -> Result<Option<usize>, Box<dyn std::error::Error>> {
                match offsets.get(stringify!($name)) {
                    None => Err(format!("No address found for {}", stringify!($name)).into()),
                    Some(off) => {
                        let rel_address = *off as usize;
                        type FnPtr = unsafe extern "C" fn ($( $ty ),+ ) -> $out_type;
                        let target: FnPtr = std::mem::transmute(base_address + rel_address);
                        // FIXME: initialize seems to fail (already initialized). Find out what causes this
                        let _ = [<o_ $name>].initialize(target, [<$name _detour_fkt>]);
                        [<o_ $name>].enable()?;
                        Ok(Some(rel_address))
                    }
                }
            }

            inventory::submit! {
                $crate::resolvers::HookRegistration {
                    name: stringify!($name),
                    hook_fn: [<attach_ $name>],
                    auto_activate: $crate::__create_hook_impl![@is_active $status],
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
            unsafe { paste::paste! { [<o_ $name>].call($( $arg ),+) } }
        }
    };

    // POST: Original function, then user code -> return value
    (POST, $name:ident, $out_type:ty, ( $( $arg:expr ),+ ), $body:expr) => {
        {
            let ret_val = unsafe { paste::paste! { [<o_ $name>].call($( $arg ),+) } };
            { $body(ret_val) }
        }
    };
}

/// Calls the original detour function `o_<name>` with arguments.
/// Preserves type hints for Rust Analyzer.
#[macro_export]
macro_rules! CALL_ORIGINAL {
    ($name:ident ( $($arg:expr),* $(,)? )) => {{
        // Use paste to reconstruct o_<name>
        paste::paste! {
            unsafe { [<o_ $name>].call($($arg),*) }
        }
    }};
}