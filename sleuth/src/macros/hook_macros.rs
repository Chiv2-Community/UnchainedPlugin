
// Use pre-release version of rust-analyzer if this causes errors
#[macro_export]
macro_rules! CREATE_HOOK {
    // Format: Name, (args), {body}
    ($name:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
        $crate::__create_hook_impl!($name, ACTIVE, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };
    // Format: Name, ACTIVE, (args), {body}
    ($name:ident, $status:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
        $crate::__create_hook_impl!($name, $status, PRE, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };
    // Format: Name, OutType, (args), {body}
    ($name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
        $crate::__create_hook_impl!($name, ACTIVE, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
    // Format: Name, Status, OutType, (args), {body}
    ($name:ident, $status:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
        $crate::__create_hook_impl!($name, $status, PRE, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
    // Format: Name, Status, HookType, OutType, (args), {body}
    ($name:ident, $status:ident, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
        $crate::__create_hook_impl!($name, $status, $hook_type, $out_type, ( $( $arg: $ty ),+ ), $body);
    };
}

#[macro_export]
macro_rules! __create_hook_impl {
    (@is_active ACTIVE) => { true };
    (@is_active INACTIVE) => { false };

    ($name:ident, $status:ident, $hook_type:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
        paste::paste! {
            ::retour::static_detour! {
                pub static [<o_ $name>]: unsafe extern "C" fn ($( $ty ),+ ) -> $out_type;
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
    (NONE, $name:ident, $out_type:ty, ( $( $arg:expr ),+ ), $body:block) => { 
        $body 
    };
    
    // PRE: User code, then original function.
    (PRE, $name:ident, $out_type:ty, ( $( $arg:expr ),+ ), $body:block) => {
        {
            // internal scope to not leak vars, force () return
            { $body }; 
            unsafe { paste::paste! { [<o_ $name>].call($( $arg ),+) } }
        }
    };

    // POST: Original function, then user code -> return value
    (POST, $name:ident, $out_type:ty, ( $( $arg:expr ),+ ), $body:block) => {
        {
            let ret_val = unsafe { paste::paste! { [<o_ $name>].call($( $arg ),+) } };
            { $body(ret_val) }
        }
    };
}
