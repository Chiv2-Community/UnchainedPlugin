
// This garbage is needed because new rust-analyzer version chokes on the previous implementation
#[macro_export]
macro_rules! CREATE_HOOK {
    ($($tt:tt)*) => {
        #[cfg(not(rust_analyzer))]
        $crate::__create_hook_impl!($($tt)*);
    };
}

#[macro_export]
macro_rules! __create_hook_impl {
    // Helpers to convert tokens to boolean for inventory
    (@is_active ACTIVE) => { true };
    (@is_active INACTIVE) => { false };


    // No Status + No Ret Type + Args
    // Matches: Name, (args), {body}
    ($name:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, ACTIVE, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };

    // Explicit Status + Args (No Ret Type)
    // Matches: Name, ACTIVE, (args), {body}
    ($name:ident, $status:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };

    // No Status + Ret Type + Args
    // Matches: Name, bool, (args), {body}
    ($name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, ACTIVE, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
    
    // Explicit Status + Ret Type + Args
    // Matches: Name, ACTIVE, bool, (args), {body}
    ($name:ident, $status:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        $crate::__create_hook_impl!($name, $status, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };

    // FINAL (5 Args)
    ($name:ident, $status:ident, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:expr) => {
        paste::paste! {
            ::retour::static_detour! {
                pub static [<o_ $name>]: unsafe extern "C" fn ($( $ty ),+ ) -> $out_type;
            }

            $crate::__create_hook_impl![@gen_detour $hook_type, $name, $out_type, ( $( $arg: $ty ),+ ), $body];

            #[allow(non_snake_case)]
            pub unsafe fn [<attach_ $name>](
                base_address: usize,
                offsets: std::collections::HashMap<String, u64>
            ) -> Result<Option<usize>, Box<dyn std::error::Error>> {
                match offsets.get(stringify![$name]) {
                    None => Err("No address found.".into()),
                    Some(_) => {
                        let rel_address = offsets[stringify![$name]] as usize;
                        #[allow(non_camel_case_types)]
                        type [<Fn $name>] = unsafe extern "C" fn ($( $ty ),+ ) -> $out_type;
                        let target: [<Fn $name>] = std::mem::transmute(base_address + rel_address);
                        let _ = [<o_ $name>].initialize(target, [<$name _detour_fkt>]);
                        let _ = [<o_ $name>].enable();
                        Ok(Some(rel_address))
                    }
                }
            }

            // AUTO-REGISTRATION
            inventory::submit! {
                $crate::resolvers::HookRegistration {
                    name: stringify!($name),
                    hook_fn: [<attach_ $name>],
                    auto_activate: $crate::__create_hook_impl![@is_active $status],
                }
            }
        }
    };

    // --- Detour Generators (Existing logic) ---
    (@gen_detour NONE, $name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ ), $body:expr) => {
        paste::paste! { 
            #[allow(non_snake_case)]
            pub fn [<$name _detour_fkt>]( $( $arg: $ty ),+ ) -> $out_type { $body } 
        }
    };
    (@gen_detour POST, $name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ ), $body:expr) => {
        paste::paste! {
            #[allow(non_snake_case)]
            pub fn [<$name _detour_fkt>]( $( $arg: $ty ),+ ) -> $out_type {
                let ret_val = unsafe { [<o_ $name>].call ( $( $arg ),+ ) };
                ($body(ret_val))
            }
        }
    };
    (@gen_detour PRE, $name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ ), $body:expr) => {
        paste::paste! {
            #[allow(non_snake_case)]
            pub fn [<$name _detour_fkt>]( $( $arg: $ty ),+ ) -> $out_type {
                $body
                unsafe { [<o_ $name>].call ( $( $arg ),+ ) }
            }
        }
    };
}