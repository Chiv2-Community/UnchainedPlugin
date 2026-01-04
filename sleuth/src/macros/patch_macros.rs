
/// A macro for registering memory patches with support for offsets, tags and conditions.
///
///
/// # Components
/// - **Name**: The lookup key in the offsets HashMap.
/// - **Tag**: (Optional) An identifier used to create unique function names when patching the same function multiple times.
/// - **Extra**: (Optional) A numeric literal offset to add to the base function address.
/// - **Op**: The operation to perform (`NOP`, `BYTES`, or `WRITE`).
/// - **Val**: The payload (byte count for `NOP`, `&[u8]` for `BYTES`, or a reference for `WRITE`).
/// - **IF { block }**: (Optional) A runtime check. The patch only applies if the block returns `true`.
///
/// # Examples
/// 
/// ```rust
/// // 1. Basic usage: NOP 2 bytes at the start of 'PatchSomething'
/// CREATE_PATCH!(PatchSomething, NOP, 2);
/// 
/// // 2. Offset usage: NOP 2 bytes at 'PatchSomething' + 0x10
/// CREATE_PATCH!(PatchSomething, 0x10, NOP, 2);
/// 
/// // 3. Tagged usage: Allows multiple patches on the same function name
/// CREATE_PATCH!(ProcessInput @ FixX, 0x42, NOP, 3);
/// CREATE_PATCH!(ProcessInput @ FixY, 0x88, BYTES, &[0x90, 0x90]);
/// 
/// // 4. Conditional usage: Only apply if a config flag is set
/// CREATE_PATCH!(InfiniteSomething, NOP, 5, IF {
///     crate::config::get().infinite_ammo_enabled
/// });
/// 
/// // 5. Direct Value Write: Change a float constant in memory
/// CREATE_PATCH!(GlobalMultiplier, WRITE, &1.5f32);
/// ```
///
/// # Address Calculation
/// The final target memory address is calculated as:
/// $Address = BaseAddress + Offset(Name) + Extra$
/// 
/// # Internal Generated Code
/// 
/// This macro creates a `PatchRegistration` struct that is automatically 
/// collected via `inventory`. It handles address calculation, identifier generation via `paste!`, 
/// and maps operations to internal memory tools.
/// 
/// For a call like `CREATE_PATCH!(MyFunc, NOP, 1)`, the macro generates:
/// - A unique function: `pub unsafe fn apply_patch_MyFunc_base(...)`
/// - An `inventory::submit!` call containing the function pointer and the metadata.
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
                    tag: stringify!($suffix),
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

/// A macro for registering platform-specific memory patches with support for offsets, tags and conditions.
/// 
/// ```rust
/// //Platform-specific usage: Only apply on Steam
/// CREATE_PATCH_PLATFORM!(STEAM, DRMCheck, NOP, 6);
/// 
/// // Platform + Condition: Apply on Epic if experimental is enabled
/// CREATE_PATCH_PLATFORM!(EPIC, UnlockFPS, 0x150, BYTES, &[0x01], IF {
///     crate::config::get().experimental
/// });
/// ```
#[macro_export]
macro_rules! CREATE_PATCH_PLATFORM {
    // tag + offset + if
    ($platform:ident, $name:ident @ $tag:ident, $extra:literal, $op:ident, $val:expr, IF $cond:block) => {
        $crate::CREATE_PATCH!($name @ $tag, $extra, $op, $val, IF {
            $crate::__platform_check!($platform) && $cond
        });
    };

    // name + offset + if
    ($platform:ident, $name:ident, $extra:literal, $op:ident, $val:expr, IF $cond:block) => {
        $crate::CREATE_PATCH!($name, $extra, $op, $val, IF {
            $crate::__platform_check!($platform) && $cond
        });
    };

    // tag + offset (no if)
    ($platform:ident, $name:ident @ $tag:ident, $extra:literal, $op:ident, $val:expr) => {
        $crate::CREATE_PATCH!($name @ $tag, $extra, $op, $val, IF {
            $crate::__platform_check!($platform)
        });
    };

    // name + offset (no if)
    ($platform:ident, $name:ident, $extra:literal, $op:ident, $val:expr) => {
        $crate::CREATE_PATCH!($name, $extra, $op, $val, IF {
            $crate::__platform_check!($platform)
        });
    };
    
    // name + op + val (no offset, no if)
    ($platform:ident, $name:ident, $op:ident, $val:expr) => {
        $crate::CREATE_PATCH!($name, $op, $val, IF {
            $crate::__platform_check!($platform)
        });
    };
}

#[macro_export]
macro_rules! __platform_check {
    ($platform:ident) => {
        match $crate::tools::hook_globals::globals_initialized() {
            true => $crate::globals().get_platform() == $crate::resolvers::PlatformType::$platform,
            _ => false,
        }
    };
}