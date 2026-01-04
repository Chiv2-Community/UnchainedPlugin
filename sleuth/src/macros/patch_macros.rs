
/// Examples
/// ```rust
/// // Always active
/// CREATE_PATCH!(PatchSomething, NOP, 2);
/// 
/// // Always active with offset
/// CREATE_PATCH!(PatchSomething, 0x10, NOP, 2);
/// 
/// // Only apply if a specific feature is enabled in your config
/// CREATE_PATCH!(InfiniteSomething, NOP, 5, IF {
///     crate::config::get().infinite_ammo_enabled
/// });
/// 
/// // Only apply if a CLI argument was passed (assuming a global lazy_static or once_cell)
/// CREATE_PATCH!(SkipIntro, BYTES, &[0xEB, 0x05], IF {
///     std::env::args().any(|x| x == "--skip-intro")
/// });
/// 
/// ```
#[macro_export]
macro_rules! CREATE_PATCH {
    // tag + offset + condition
    // Name @ Suffix, 0x10, NOP, 5, IF { ... }
    ($name:ident @ $tag:ident, $extra:literal, $op:ident, $val:expr, IF $cond:block) => {
        $crate::__create_patch_impl!($name, $tag, $extra, $op, $val, || $cond);
    };

    // tag + condition (no offset)
    // Name @ Suffix, NOP, 5, IF { ... }
    ($name:ident @ $tag:ident, $op:ident, $val:expr, IF $cond:block) => {
        $crate::__create_patch_impl!($name, $tag, 0, $op, $val, || $cond);
    };

    // name + offset + condition (auto-tag via offset)
    // Name, 0x10, NOP, 5, IF { ... }
    ($name:ident, $extra:literal, $op:ident, $val:expr, IF $cond:block) => {
        $crate::__create_patch_impl!($name, $extra, $extra, $op, $val, || $cond);
    };

    // name + condition (no offset, no tag)
    // Name, NOP, 5, IF { ... }
    ($name:ident, $op:ident, $val:expr, IF $cond:block) => {
        $crate::__create_patch_impl!($name, base, 0, $op, $val, || $cond);
    };

    // no tags

    // default + offset
    // Name, 0x10, NOP, 5
    ($name:ident, $extra:literal, $op:ident, $val:expr) => {
        $crate::__create_patch_impl!($name, $extra, $extra, $op, $val, || true);
    };

    // default (no offset)
    // Name, NOP, 5
    ($name:ident, $op:ident, $val:expr) => {
        $crate::__create_patch_impl!($name, base, 0, $op, $val, || true);
    };
}

#[macro_export]
macro_rules! __create_patch_impl {
    ($name:ident, $suffix:tt, $extra:expr, $op:ident, $val:expr, $cond_fn:expr) => {
        paste::paste! {
            #[allow(non_snake_case)]
            pub unsafe fn [<apply_patch_ $name _ $suffix>](
                base_address: usize,
                offsets: std::collections::HashMap<String, u64>
            ) -> Result<(), Box<dyn std::error::Error>> {
                match offsets.get(stringify!($name)) {
                    None => Err(format!("Offset for {} not found", stringify!($name)).into()),
                    Some(offset) => {
                        let addr = (base_address + (*offset as usize) + ($extra as usize)) as *mut u8;
                        // Map to existing ops (memtools)
                        $crate::__apply_patch_op!($op, addr, $val);
                        Ok(())
                    }
                }
            }

            inventory::submit! {
                $crate::resolvers::PatchRegistration {
                    name: stringify!($name),
                    patch_fn: [<apply_patch_ $name _ $suffix>],
                    enabled_fn: $cond_fn,
                }
            }
        }
    };
}

#[macro_export]
macro_rules! __apply_patch_op {
    (BYTES, $addr:expr, $val:expr) => { $crate::tools::memtools::patch($addr, $val) };
    (NOP, $addr:expr, $val:expr) => { $crate::tools::memtools::nop($addr, $val) };
    (WRITE, $addr:expr, $val:expr) => { $crate::tools::memtools::write_ptr($addr as *mut _, $val) };
}