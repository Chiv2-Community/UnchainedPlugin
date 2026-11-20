//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UnrealType.h>.

use crate::Chivalry2::Engine::*;

/// UE 4.25 native property metadata.
///
/// Native size: 0x78.
#[unchained_macros::runtime_bindings]
#[repr(C)]
pub struct FProperty {
    pub field: FField,
    pub array_dim: i32,
    pub element_size: i32,
    pub property_flags: u64,
    pub rep_index: u16,
    pub blueprint_replication_condition: u8,
    /// Byte offset of the value inside its owning object.
    pub offset_internal: i32,
    rep_notify_func: FName,
    property_link_next: *const FProperty,
    next_ref: *const FProperty,
    destructor_link_next: *const FProperty,
    post_construct_link_next: *const FProperty,
}

const _: () = assert!(std::mem::offset_of!(FProperty, offset_internal) == 0x4C);

/// Native array-property metadata (size 0x88).
#[unchained_macros::runtime_bindings]
#[repr(C)]
pub struct FArrayProperty {
    pub property: FProperty,
    inner: *const FProperty,
    array_flags: i32,
}

impl FArrayProperty {
    /// Read native `FArrayProperty::Inner`, which may be null.
    ///
    /// # Safety
    /// Self must point to live, aligned FArrayProperty metadata for this build,
    /// with no concurrent mutation. The returned pointer is borrowed.
    pub unsafe fn inner(self: *const Self) -> *const FProperty {
        std::ptr::addr_of!((*self).inner).read()
    }
}

/// Native struct-property metadata (size 0x80).
#[unchained_macros::runtime_bindings]
#[repr(C)]
pub struct FStructProperty {
    pub property: FProperty,
    // Native UScriptStruct*, viewed through its UStruct prefix.
    script_struct: *const UStruct,
}

impl FStructProperty {
    /// Read native `FStructProperty::Struct` as its UStruct prefix; may be null.
    ///
    /// # Safety
    /// Self must point to live, aligned FStructProperty metadata for this build,
    /// with no concurrent mutation. The returned pointer is borrowed.
    pub unsafe fn script_struct(self: *const Self) -> *const UStruct {
        std::ptr::addr_of!((*self).script_struct).read()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructArrayError {
    NotArray,
    NegativeOffset,
    MisalignedOffset,
    OffsetOutOfBounds,
    UnsupportedFlags(i32),
    MissingInner,
    NotStruct,
    InvalidElementSize,
    MissingStruct,
}

/// Checked heap-array-of-struct metadata. Contains no asset-specific schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructArrayMetadata {
    offset: usize,
    element_size: usize,
    pub script_struct: *const UStruct,
}

impl StructArrayMetadata {
    /// # Safety
    /// `owner` must be null or live native metadata, stable during this call.
    /// Non-null metadata requires the engine name table to be initialized.
    pub unsafe fn find(
        owner: *const UStruct,
        name: &str,
    ) -> Result<Option<Self>, StructArrayError> {
        let Some(field) = UStruct::find_field(owner, name) else {
            return Ok(None);
        };
        Self::from_field(field, (*owner).properties_size, |field, name| {
            let class = (*field).class_private;
            !class.is_null() && (*class).name.to_string() == name
        })
        .map(Some)
    }

    /// Validate kind before reading any derived-property slots.
    unsafe fn from_field(
        field: *const FField,
        properties_size: i32,
        is_kind: impl Fn(*const FField, &str) -> bool,
    ) -> Result<Self, StructArrayError> {
        if field.is_null() || !is_kind(field, "ArrayProperty") {
            return Err(StructArrayError::NotArray);
        }
        let array = &*field.cast::<FArrayProperty>();
        let offset = usize::try_from(array.property.offset_internal)
            .map_err(|_| StructArrayError::NegativeOffset)?;
        if offset % std::mem::align_of::<FScriptArray>() != 0 {
            return Err(StructArrayError::MisalignedOffset);
        }
        if offset
            .checked_add(std::mem::size_of::<FScriptArray>())
            .is_none_or(|end| end > properties_size.max(0) as usize)
        {
            return Err(StructArrayError::OffsetOutOfBounds);
        }
        // UnrealType.h's FScriptArrayHelper also supports memory-image storage.
        // Only EArrayPropertyFlags::None is supported by our borrowed heap view.
        if array.array_flags != 0 {
            return Err(StructArrayError::UnsupportedFlags(array.array_flags));
        }
        let inner = array.inner;
        if inner.is_null() {
            return Err(StructArrayError::MissingInner);
        }
        if !is_kind(inner.cast(), "StructProperty") {
            return Err(StructArrayError::NotStruct);
        }
        let element_size = (*inner).element_size;
        if element_size <= 0 {
            return Err(StructArrayError::InvalidElementSize);
        }
        let script_struct = inner.cast::<FStructProperty>().script_struct();
        if script_struct.is_null() {
            return Err(StructArrayError::MissingStruct);
        }
        Ok(Self {
            offset,
            element_size: element_size as usize,
            script_struct,
        })
    }

    /// # Safety
    /// `object` must be a live instance of the owner used by `find`. Its array
    /// buffer must be live and stable for the borrow; metadata must not change.
    pub unsafe fn view<'a>(
        &self,
        object: &'a UObject,
    ) -> Result<FScriptArrayView<'a>, ScriptArrayError> {
        let storage = &*(object as *const UObject)
            .cast::<u8>()
            .add(self.offset)
            .cast::<FScriptArray>();
        storage.view(self.element_size)
    }
}

const _: () = {
    assert!(std::mem::size_of::<FProperty>() == 0x78);
    assert!(std::mem::offset_of!(FArrayProperty, inner) == 0x78);
    assert!(std::mem::size_of::<FArrayProperty>() == 0x88);
    assert!(std::mem::offset_of!(FStructProperty, script_struct) == 0x78);
    assert!(std::mem::size_of::<FStructProperty>() == 0x80);
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of, MaybeUninit};

    // Resolve synthetic kinds without accessing the game's global name table.
    unsafe fn struct_array_fixture(
        array: &FArrayProperty,
        inner: &FStructProperty,
        size: i32,
    ) -> Result<StructArrayMetadata, StructArrayError> {
        StructArrayMetadata::from_field(
            std::ptr::addr_of!(array.property.field),
            size,
            |field, kind| {
                (field == std::ptr::addr_of!(array.property.field) && kind == "ArrayProperty")
                    || (field == std::ptr::addr_of!(inner.property.field)
                        && kind == "StructProperty")
            },
        )
    }

    #[test]
    fn reflected_struct_array_metadata_checks_kinds_before_casting() {
        unsafe {
            assert_eq!(
                StructArrayMetadata::find(std::ptr::null(), "Missing"),
                Ok(None)
            );
            assert_eq!(
                StructArrayMetadata::from_field(std::ptr::null(), 64, |_, _| true),
                Err(StructArrayError::NotArray)
            );
            // Only base storage exists: rejecting its kind must precede derived reads.
            let field: FField = std::mem::zeroed();
            assert_eq!(
                StructArrayMetadata::from_field(&field, 64, |_, _| false),
                Err(StructArrayError::NotArray)
            );
            let mut array: FArrayProperty = std::mem::zeroed();
            array.property.offset_internal = 16;
            array.inner = (&field as *const FField).cast();
            let array_kind_only = |_: *const FField, kind: &str| kind == "ArrayProperty";
            assert_eq!(
                StructArrayMetadata::from_field(&array.property.field, 64, array_kind_only),
                Err(StructArrayError::NotStruct)
            );
        }
    }

    #[test]
    fn reflected_struct_array_metadata_checks_flags_nulls_size_and_offset() {
        unsafe {
            let mut array: FArrayProperty = std::mem::zeroed();
            let mut inner: FStructProperty = std::mem::zeroed();
            let script_struct = MaybeUninit::<UStruct>::uninit();
            array.property.offset_internal = 16;
            array.inner = std::ptr::addr_of!(inner.property);
            inner.property.element_size = 24;
            inner.script_struct = script_struct.as_ptr();
            let metadata = struct_array_fixture(&array, &inner, 64).unwrap();
            assert_eq!(metadata.offset, 16);
            assert_eq!(metadata.element_size, 24);
            assert_eq!(metadata.script_struct, script_struct.as_ptr());
            assert!(!std::mem::needs_drop::<StructArrayMetadata>());
            for flags in [1, 2, -1] {
                array.array_flags = flags;
                assert_eq!(
                    struct_array_fixture(&array, &inner, 64),
                    Err(StructArrayError::UnsupportedFlags(flags))
                );
            }
            array.array_flags = 0;
            array.inner = std::ptr::null();
            assert_eq!(
                struct_array_fixture(&array, &inner, 64),
                Err(StructArrayError::MissingInner)
            );
            array.inner = std::ptr::addr_of!(inner.property);
            for size in [0, -1] {
                inner.property.element_size = size;
                assert_eq!(
                    struct_array_fixture(&array, &inner, 64),
                    Err(StructArrayError::InvalidElementSize)
                );
            }
            inner.property.element_size = 24;
            inner.script_struct = std::ptr::null();
            assert_eq!(
                struct_array_fixture(&array, &inner, 64),
                Err(StructArrayError::MissingStruct)
            );
            inner.script_struct = script_struct.as_ptr();
            array.property.offset_internal = -1;
            assert_eq!(
                struct_array_fixture(&array, &inner, 64),
                Err(StructArrayError::NegativeOffset)
            );
            array.property.offset_internal = 1;
            assert_eq!(
                struct_array_fixture(&array, &inner, 64),
                Err(StructArrayError::MisalignedOffset)
            );
            array.property.offset_internal = 56;
            assert_eq!(
                struct_array_fixture(&array, &inner, 64),
                Err(StructArrayError::OffsetOutOfBounds)
            );
        }
    }

    #[test]
    fn property_layouts_match_shipping_pdb() {
        assert_eq!(size_of::<FProperty>(), 0x78);
        assert_eq!(offset_of!(FProperty, rep_notify_func), 0x50);
        assert_eq!(offset_of!(FProperty, property_link_next), 0x58);
        assert_eq!(offset_of!(FProperty, post_construct_link_next), 0x70);
        assert_eq!(offset_of!(FArrayProperty, property), 0);
        assert_eq!(offset_of!(FArrayProperty, inner), 0x78);
        assert_eq!(offset_of!(FArrayProperty, array_flags), 0x80);
        assert_eq!(size_of::<FArrayProperty>(), 0x88);
        assert_eq!(offset_of!(FStructProperty, property), 0);
        assert_eq!(offset_of!(FStructProperty, script_struct), 0x78);
        assert_eq!(size_of::<FStructProperty>(), 0x80);
        assert_eq!(align_of::<FProperty>(), 8);
        assert_eq!(align_of::<FArrayProperty>(), 8);
        assert_eq!(align_of::<FStructProperty>(), 8);
    }

    #[test]
    fn native_pointer_accessors_preserve_addresses_and_nulls() {
        // Initialize only the pointer slots; accessors must not read other metadata.
        let mut array = MaybeUninit::<FArrayProperty>::uninit();
        let mut strukt = MaybeUninit::<FStructProperty>::uninit();
        let property = MaybeUninit::<FProperty>::uninit();
        let script_struct = MaybeUninit::<UStruct>::uninit();
        unsafe {
            std::ptr::addr_of_mut!((*array.as_mut_ptr()).inner).write(property.as_ptr());
            std::ptr::addr_of_mut!((*strukt.as_mut_ptr()).script_struct)
                .write(script_struct.as_ptr());
            assert_eq!(array.as_ptr().inner(), property.as_ptr());
            assert_eq!(strukt.as_ptr().script_struct(), script_struct.as_ptr());

            std::ptr::addr_of_mut!((*array.as_mut_ptr()).inner).write(std::ptr::null());
            std::ptr::addr_of_mut!((*strukt.as_mut_ptr()).script_struct).write(std::ptr::null());
            assert!(array.as_ptr().inner().is_null());
            assert!(strukt.as_ptr().script_struct().is_null());
        }
    }
}
