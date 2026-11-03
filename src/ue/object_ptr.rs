use crate::game::engine::UWorld;

use super::UObject;

/// Upcast a native object pointer without dereferencing it or changing mutability.
pub trait AsUObject {
    type Pointer;

    fn as_uobject(self) -> Self::Pointer;
}

// These upcasts rely on the embedded UObject being at offset zero.
const _: () = assert!(std::mem::offset_of!(UWorld, base) == 0);

impl AsUObject for *mut UWorld {
    type Pointer = *mut UObject;

    fn as_uobject(self) -> Self::Pointer {
        self.cast()
    }
}

impl AsUObject for *const UWorld {
    type Pointer = *const UObject;

    fn as_uobject(self) -> Self::Pointer {
        self.cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upcasts_preserve_null_and_mutability() {
        let mutable: *mut UObject = std::ptr::null_mut::<UWorld>().as_uobject();
        let immutable: *const UObject = std::ptr::null::<UWorld>().as_uobject();
        assert!(mutable.is_null());
        assert!(immutable.is_null());
    }

    #[test]
    fn upcasts_preserve_address_without_reading_world() {
        let mut storage = std::mem::MaybeUninit::<UWorld>::uninit();
        let mutable = storage.as_mut_ptr();
        let immutable = storage.as_ptr();
        assert_eq!(mutable.as_uobject().cast::<UWorld>(), mutable);
        assert_eq!(immutable.as_uobject().cast::<UWorld>(), immutable);
    }
}
