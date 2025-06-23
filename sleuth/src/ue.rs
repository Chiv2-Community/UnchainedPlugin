
// FROM mint
// https://github.com/trumank/mint/blob/6335041f21b95976d29fe2cfbf282feb0c9f38ac/hook/src/ue/map.rs

#![allow(dead_code, private_interfaces)]
// use std::fmt::Debug;


// pub trait UEHash {
//     fn ue_hash(&self) -> u32;
// }

// #[derive(Default, Debug)]
// #[repr(C)]
// struct TSetElement<V> {
//     value: V,
//     hash_next_id: FSetElementId,
//     hash_index: i32,
// }
// impl<K: UEHash, V> UEHash for TSetElement<TTuple<K, V>> {
//     fn ue_hash(&self) -> u32 {
//         self.value.a.ue_hash()
//     }
// }

// #[derive(Default, Clone, Copy)]
// #[repr(C)]
// pub struct FSetElementId {
//     index: i32,
// }
// impl FSetElementId {
//     pub fn is_valid(self) -> bool {
//         self.index != -1
//     }
// }
// impl Debug for FSetElementId {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "FSetElementId({:?})", self.index)
//     }
// }

// #[derive(Default, Debug)]
// #[repr(C)]
// struct TTuple<A, B> {
//     a: A,
//     b: B,
// }

// #[repr(C)]
// union TSparseArrayElementOrFreeListLink<E> {
//     element: std::mem::ManuallyDrop<E>,
//     list_link: ListLink,
// }

// #[derive(Debug, Clone, Copy)]
// #[repr(C)]
// struct ListLink {
//     next_free_index: i32,
//     prev_free_index: i32,
// }

// #[derive(Debug)]
// #[repr(C)]
// struct TInlineAllocator<const N: usize, V> {
//     inline_data: [V; N],
//     secondary_data: *const V, // TSizedHeapAllocator<32>::ForElementType<unsigned int>,
// }
// impl<const N: usize, V: Default + Copy> Default for TInlineAllocator<N, V> {
//     fn default() -> Self {
//         Self {
//             inline_data: [Default::default(); N],
//             secondary_data: std::ptr::null(),
//         }
//     }
// }

// impl<const N: usize, V> TInlineAllocator<N, V> {
//     fn get_allocation(&self) -> *const V {
//         if !self.secondary_data.is_null() {
//             self.secondary_data
//         } else {
//             self.inline_data.as_ptr()
//         }
//     }
// }

// #[derive(Default, Debug)]
// #[repr(C)]
// struct TBitArray {
//     allocator_instance: TInlineAllocator<4, u32>,
//     num_bits: i32,
//     max_bits: i32,
// }
// impl TBitArray {
//     fn get_data(&self) -> *const u32 {
//         self.allocator_instance.get_allocation()
//     }

//     fn index(&self, index: usize) -> FBitReference<'_> {
//         assert!(index < self.num_bits as usize);
//         let num_bits_per_dword = 32;
//         FBitReference {
//             data: unsafe { &*self.get_data().add(index / num_bits_per_dword) },
//             mask: 1 << (index & (num_bits_per_dword - 1)),
//         }
//     }
// }

// #[derive(Debug, Clone, Copy)]
// #[repr(C)]
// struct FBitReference<'data> {
//     data: &'data u32,
//     mask: u32,
// }
// impl FBitReference<'_> {
//     fn bool(self) -> bool {
//         (self.data & self.mask) != 0
//     }
// }

// #[repr(C)]
// struct TSparseArray<E> {
//     data: TArray<TSparseArrayElementOrFreeListLink<E>>,
//     allocation_flags: TBitArray,
//     first_free_index: i32,
//     num_free_indices: i32,
// }
// impl<E> Default for TSparseArray<E> {
//     fn default() -> Self {
//         Self {
//             data: Default::default(),
//             allocation_flags: Default::default(),
//             first_free_index: 0,
//             num_free_indices: 0,
//         }
//     }
// }
// impl<E> TSparseArray<E> {
//     fn index(&self, index: usize) -> &E {
//         assert!(index < self.data.len() && index < self.allocation_flags.num_bits as usize);
//         assert!(self.allocation_flags.index(index).bool());
//         unsafe { &self.data.as_slice()[index].element }
//     }
// }

// struct DbgTSparseArrayData<'a, E>(&'a TSparseArray<E>);
// impl<E: Debug> Debug for DbgTSparseArrayData<'_, E> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let mut dbg = f.debug_list();
//         for i in 0..(self.0.allocation_flags.num_bits as usize) {
//             if self.0.allocation_flags.index(i).bool() {
//                 dbg.entry(self.0.index(i));
//             }
//         }
//         dbg.finish()
//     }
// }

// impl<E: Debug> Debug for TSparseArray<E> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("TSparseArray")
//             .field("data", &DbgTSparseArrayData(self))
//             .field("allocation_flags", &self.allocation_flags)
//             .field("first_free_index", &self.first_free_index)
//             .field("num_free_indices", &self.num_free_indices)
//             .finish()
//     }
// }

// #[repr(C)]
// pub struct TMap<K: UEHash, V> {
//     elements: TSparseArray<TSetElement<TTuple<K, V>>>,
//     hash: TInlineAllocator<1, FSetElementId>,
//     hash_size: i32,
// }
// impl<K: UEHash, V> TMap<K, V> {
//     fn hash(&self) -> &[FSetElementId] {
//         unsafe { std::slice::from_raw_parts(self.hash.get_allocation(), self.hash_size as usize) }
//     }
// }
// impl<K: UEHash, V> Default for TMap<K, V> {
//     fn default() -> Self {
//         Self {
//             elements: Default::default(),
//             hash: Default::default(),
//             hash_size: 0,
//         }
//     }
// }
// impl<K: UEHash + Debug, V: Debug> Debug for TMap<K, V> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("TMap")
//             .field("elements", &self.elements)
//             .field("hash", &self.hash())
//             .finish()
//     }
// }

// impl<K: PartialEq + UEHash, V> TMap<K, V> {
//     pub fn find(&self, key: K) -> Option<&V> {
//         let id = self.find_id(key);
//         if id.is_valid() {
//             Some(&self.elements.index(id.index as usize).value.b)
//         } else {
//             None
//         }
//     }
//     pub fn find_id(&self, key: K) -> FSetElementId {
//         if self.elements.data.len() != self.elements.num_free_indices as usize {
//             let key_hash = key.ue_hash();
//             let hash = &self.hash();

//             let mut i: FSetElementId =
//                 hash[(((self.hash_size as i64) - 1) & (key_hash as i64)) as usize];

//             if i.is_valid() {
//                 loop {
//                     let elm = self.elements.index(i.index as usize);

//                     if elm.value.a == key {
//                         return i;
//                     }

//                     i = elm.hash_next_id;
//                     if !i.is_valid() {
//                         break;
//                     }
//                 }
//             }
//         }

//         FSetElementId { index: -1 }
//     }
// }

// #[cfg(test)]
// mod test {
//     use crate::ue::FName;

//     use super::*;
//     const _: [u8; 0x50] = [0; std::mem::size_of::<TMap<FName, [u8; 0x20]>>()];
//     const _: [u8; 0x38] =
//         [0; std::mem::size_of::<TSparseArray<TSetElement<TTuple<FName, [u8; 0x20]>>>>()];
//     const _: [u8; 0x10] = [0; std::mem::size_of::<TInlineAllocator<1, FSetElementId>>()];
// }


// impl UEHash for FNameEntryId {
//     fn ue_hash(&self) -> u32 {
//         let value = self.value;
//         (value >> 4) + value.wrapping_mul(0x10001) + (value >> 0x10).wrapping_mul(0x80001)
//     }
// }

// impl UEHash for FName {
//     fn ue_hash(&self) -> u32 {
//         self.comparison_index.ue_hash() + self.number
//     }
// }

// From trumank/patternsleuth
// https://github.com/trumank/patternsleuth/blob/master/examples/dll_hook/src/ue.rs
// FIXME: Nihi: the file is unchanged. Find a way to get use it via git without submodules


use std::{
    cell::UnsafeCell,
    ffi::c_void,
    fmt::Display,
    ops::{Deref, DerefMut},
};

use windows::Win32::System::Threading::{
    EnterCriticalSection, LeaveCriticalSection, CRITICAL_SECTION,
};

use crate::globals;

pub type FnFFrameStep =
    unsafe extern "system" fn(stack: &mut kismet::FFrame, *mut UObject, result: *mut c_void);
pub type FnFFrameStepExplicitProperty = unsafe extern "system" fn(
    stack: &mut kismet::FFrame,
    result: *mut c_void,
    property: *const FProperty,
);

pub type FnFNameToString = unsafe extern "system" fn(&FName, &mut FString);
impl Display for FName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = FString::new();
        unsafe {
            (globals().fname_to_string())(self, &mut string);
        };
        write!(f, "{string}")
    }
}

pub type FnUObjectBaseUtilityGetPathName =
    unsafe extern "system" fn(&UObjectBase, Option<&UObject>, &mut FString);
impl UObjectBase {
    pub fn get_path_name(&self, stop_outer: Option<&UObject>) -> String {
        let mut string = FString::new();
        unsafe {
            (globals().uobject_base_utility_get_path_name())(self, stop_outer, &mut string);
        }
        string.to_string()
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FMalloc {
    vtable: *const FMallocVTable,
}
unsafe impl Sync for FMalloc {}
unsafe impl Send for FMalloc {}
impl FMalloc {
    pub fn malloc(&self, count: usize, alignment: u32) -> *mut c_void {
        unsafe { ((*self.vtable).malloc)(self, count, alignment) }
    }
    pub fn realloc(&self, original: *mut c_void, count: usize, alignment: u32) -> *mut c_void {
        unsafe { ((*self.vtable).realloc)(self, original, count, alignment) }
    }
    pub fn free(&self, original: *mut c_void) {
        unsafe { ((*self.vtable).free)(self, original) }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FMallocVTable {
    pub __vec_del_dtor: *const (),
    pub exec: *const (),
    pub malloc:
        unsafe extern "system" fn(this: &FMalloc, count: usize, alignment: u32) -> *mut c_void,
    pub try_malloc:
        unsafe extern "system" fn(this: &FMalloc, count: usize, alignment: u32) -> *mut c_void,
    pub realloc: unsafe extern "system" fn(
        this: &FMalloc,
        original: *mut c_void,
        count: usize,
        alignment: u32,
    ) -> *mut c_void,
    pub try_realloc: unsafe extern "system" fn(
        this: &FMalloc,
        original: *mut c_void,
        count: usize,
        alignment: u32,
    ) -> *mut c_void,
    pub free: unsafe extern "system" fn(this: &FMalloc, original: *mut c_void),
    pub quantize_size: *const (),
    pub get_allocation_size: *const (),
    pub trim: *const (),
    pub setup_tls_caches_on_current_thread: *const (),
    pub clear_and_disable_tlscaches_on_current_thread: *const (),
    pub initialize_stats_metadata: *const (),
    pub update_stats: *const (),
    pub get_allocator_stats: *const (),
    pub dump_allocator_stats: *const (),
    pub is_internally_thread_safe: *const (),
    pub validate_heap: *const (),
    pub get_descriptive_name: *const (),
}

#[derive(Debug)]
#[repr(C)]
pub struct FWindowsCriticalSection(UnsafeCell<CRITICAL_SECTION>);
impl FWindowsCriticalSection {
    fn crit_ptr_mut(&self) -> *mut CRITICAL_SECTION {
        &self.0 as *const _ as *mut _
    }
    unsafe fn lock(&self) {
        simple_log::info!("LOCKING objects");
        EnterCriticalSection(self.crit_ptr_mut());
    }
    unsafe fn unlock(&self) {
        simple_log::info!("UNLOCKING objects");
        LeaveCriticalSection(self.crit_ptr_mut());
    }
}

pub struct CriticalSectionGuard<'crit, 'data, T: ?Sized + 'data> {
    critical_section: &'crit FWindowsCriticalSection,
    data: &'data UnsafeCell<T>,
}
impl<'crit, 'data, T: ?Sized> CriticalSectionGuard<'crit, 'data, T> {
    fn lock(critical_section: &'crit FWindowsCriticalSection, data: &'data UnsafeCell<T>) -> Self {
        unsafe {
            critical_section.lock();
        }
        Self {
            critical_section,
            data,
        }
    }
}
impl<T: ?Sized> Drop for CriticalSectionGuard<'_, '_, T> {
    fn drop(&mut self) {
        unsafe { self.critical_section.unlock() }
    }
}
impl<T: ?Sized> Deref for CriticalSectionGuard<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.data.get() }
    }
}
impl<T: ?Sized> DerefMut for CriticalSectionGuard<'_, '_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FUObjectCreateListener;

#[derive(Debug)]
#[repr(C)]
pub struct FUObjectDeleteListener;

type ObjectIndex = i32;

#[derive(Debug)]
#[repr(C)]
pub struct FUObjectArray {
    obj_first_gcindex: i32,
    obj_last_non_gcindex: i32,
    max_objects_not_considered_by_gc: i32,
    open_for_disregard_for_gc: bool,

    obj_objects: UnsafeCell<FChunkedFixedUObjectArray>,
    obj_objects_critical: FWindowsCriticalSection,
    obj_available_list: [u8; 0x88],
    uobject_create_listeners: TArray<*const FUObjectCreateListener>,
    uobject_delete_listeners: TArray<*const FUObjectDeleteListener>,
    uobject_delete_listeners_critical: FWindowsCriticalSection,
    master_serial_number: std::sync::atomic::AtomicI32,
}
impl FUObjectArray {
    pub fn objects(&self) -> CriticalSectionGuard<'_, '_, FChunkedFixedUObjectArray> {
        CriticalSectionGuard::lock(&self.obj_objects_critical, &self.obj_objects)
    }
    pub fn allocate_serial_number(&self, index: ObjectIndex) -> i32 {
        use std::sync::atomic::Ordering;

        let objects = unsafe { &*self.obj_objects.get() };
        let item = objects.item(index);

        let current = item.serial_number.load(Ordering::SeqCst);
        if current != 0 {
            current
        } else {
            let new = self.master_serial_number.fetch_add(1, Ordering::SeqCst);

            let exchange =
                item.serial_number
                    .compare_exchange(0, new, Ordering::SeqCst, Ordering::SeqCst);
            match exchange {
                Ok(_) => new,
                Err(old) => old,
            }
        }
    }
}

pub struct ObjectIterator<'a> {
    array: &'a FChunkedFixedUObjectArray,
    index: i32,
}
impl<'a> Iterator for ObjectIterator<'a> {
    type Item = Option<&'a UObjectBase>;
    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.array.num_elements as usize;
        (size, Some(size))
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let n = n as i32;
        if self.index < n {
            self.index = n;
        }
        self.next()
    }
    fn next(&mut self) -> Option<Option<&'a UObjectBase>> {
        if self.index >= self.array.num_elements {
            None
        } else {
            let obj = unsafe { self.array.item(self.index).object.as_ref() };

            self.index += 1;
            Some(obj)
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FChunkedFixedUObjectArray {
    pub objects: *const *const FUObjectItem,
    pub pre_allocated_objects: *const FUObjectItem,
    pub max_elements: i32,
    pub num_elements: i32,
    pub max_chunks: i32,
    pub num_chunks: i32,
}
impl FChunkedFixedUObjectArray {
    pub fn iter(&self) -> ObjectIterator<'_> {
        ObjectIterator {
            array: self,
            index: 0,
        }
    }
    fn item_ptr(&self, index: ObjectIndex) -> *const FUObjectItem {
        let per_chunk = self.max_elements / self.max_chunks;

        unsafe {
            (*self.objects.add((index / per_chunk) as usize)).add((index % per_chunk) as usize)
        }
    }
    fn item(&self, index: ObjectIndex) -> &FUObjectItem {
        unsafe { &*self.item_ptr(index) }
    }
    fn item_mut(&mut self, index: ObjectIndex) -> &mut FUObjectItem {
        unsafe { &mut *(self.item_ptr(index) as *mut FUObjectItem) }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FUObjectItem {
    pub object: *const UObjectBase,
    pub flags: i32,
    pub cluster_root_index: i32,
    pub serial_number: std::sync::atomic::AtomicI32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FWeakObjectPtr {
    object_index: i32,
    object_serial_number: i32,
}
impl FWeakObjectPtr {
    pub fn new(object: &UObjectBase) -> Self {
        Self::new_from_index(object.internal_index)
    }
    pub fn new_from_index(index: ObjectIndex) -> Self {
        Self {
            object_index: index,
            // serial allocation performs only atomic operations
            object_serial_number: unsafe {
                globals()
                    .guobject_array_unchecked()
                    .allocate_serial_number(index)
            },
        }
    }
    pub fn get(&self, object_array: &FUObjectArray) -> Option<&UObjectBase> {
        // TODO check valid
        unsafe {
            let objects = &*object_array.obj_objects.get();
            let item = objects.item(self.object_index);
            Some(&*item.object)
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone)]
    pub struct EObjectFlags: u32 {
        const RF_NoFlags = 0x0000;
        const RF_Public = 0x0001;
        const RF_Standalone = 0x0002;
        const RF_MarkAsNative = 0x0004;
        const RF_Transactional = 0x0008;
        const RF_ClassDefaultObject = 0x0010;
        const RF_ArchetypeObject = 0x0020;
        const RF_Transient = 0x0040;
        const RF_MarkAsRootSet = 0x0080;
        const RF_TagGarbageTemp = 0x0100;
        const RF_NeedInitialization = 0x0200;
        const RF_NeedLoad = 0x0400;
        const RF_KeepForCooker = 0x0800;
        const RF_NeedPostLoad = 0x1000;
        const RF_NeedPostLoadSubobjects = 0x2000;
        const RF_NewerVersionExists = 0x4000;
        const RF_BeginDestroyed = 0x8000;
        const RF_FinishDestroyed = 0x00010000;
        const RF_BeingRegenerated = 0x00020000;
        const RF_DefaultSubObject = 0x00040000;
        const RF_WasLoaded = 0x00080000;
        const RF_TextExportTransient = 0x00100000;
        const RF_LoadCompleted = 0x00200000;
        const RF_InheritableComponentTemplate = 0x00400000;
        const RF_DuplicateTransient = 0x00800000;
        const RF_StrongRefOnFrame = 0x01000000;
        const RF_NonPIEDuplicateTransient = 0x02000000;
        const RF_Dynamic = 0x04000000;
        const RF_WillBeLoaded = 0x08000000;
    }
}
bitflags::bitflags! {
    #[derive(Debug, Clone)]
    pub struct EFunctionFlags: u32 {
        const FUNC_None = 0x0000;
        const FUNC_Final = 0x0001;
        const FUNC_RequiredAPI = 0x0002;
        const FUNC_BlueprintAuthorityOnly = 0x0004;
        const FUNC_BlueprintCosmetic = 0x0008;
        const FUNC_Net = 0x0040;
        const FUNC_NetReliable = 0x0080;
        const FUNC_NetRequest = 0x0100;
        const FUNC_Exec = 0x0200;
        const FUNC_Native = 0x0400;
        const FUNC_Event = 0x0800;
        const FUNC_NetResponse = 0x1000;
        const FUNC_Static = 0x2000;
        const FUNC_NetMulticast = 0x4000;
        const FUNC_UbergraphFunction = 0x8000;
        const FUNC_MulticastDelegate = 0x00010000;
        const FUNC_Public = 0x00020000;
        const FUNC_Private = 0x00040000;
        const FUNC_Protected = 0x00080000;
        const FUNC_Delegate = 0x00100000;
        const FUNC_NetServer = 0x00200000;
        const FUNC_HasOutParms = 0x00400000;
        const FUNC_HasDefaults = 0x00800000;
        const FUNC_NetClient = 0x01000000;
        const FUNC_DLLImport = 0x02000000;
        const FUNC_BlueprintCallable = 0x04000000;
        const FUNC_BlueprintEvent = 0x08000000;
        const FUNC_BlueprintPure = 0x10000000;
        const FUNC_EditorOnly = 0x20000000;
        const FUNC_Const = 0x40000000;
        const FUNC_NetValidate = 0x80000000;
        const FUNC_AllFlags = 0xffffffff;
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct UObjectBase {
    pub vtable: *const c_void,
    pub object_flags: EObjectFlags,
    pub internal_index: i32,
    pub class_private: *const UClass,
    pub name_private: FName,
    pub outer_private: *const UObject,
}

#[derive(Debug)]
#[repr(C)]
pub struct UObjectBaseUtility {
    pub uobject_base: UObjectBase,
}

#[derive(Debug)]
#[repr(C)]
pub struct UObject {
    pub uobject_base_utility: UObjectBaseUtility,
}

#[derive(Debug)]
#[repr(C)]
struct FOutputDevice {
    vtable: *const c_void,
    b_suppress_event_tag: bool,
    b_auto_emit_line_terminator: bool,
}

#[derive(Debug)]
#[repr(C)]
pub struct UField {
    pub uobject: UObject,
    pub next: *const UField,
}

#[derive(Debug)]
#[repr(C)]
pub struct FStructBaseChain {
    pub struct_base_chain_array: *const *const FStructBaseChain,
    pub num_struct_bases_in_chain_minus_one: i32,
}

#[derive(Debug)]
#[repr(C)]
struct FFieldClass {
    // TODO
    name: FName,
}

#[derive(Debug)]
#[repr(C)]
struct FFieldVariant {
    container: *const c_void,
    b_is_uobject: bool,
}

#[derive(Debug)]
#[repr(C)]
pub struct FField {
    class_private: *const FFieldClass,
    owner: FFieldVariant,
    next: *const FField,
    name_private: FName,
    flags_private: EObjectFlags,
}

pub struct FProperty {
    // TODO
}

#[derive(Debug)]
#[repr(C)]
pub struct UStruct {
    pub ufield: UField,
    pub fstruct_base_chain: FStructBaseChain,
    pub super_struct: *const UStruct,
    pub children: *const UField,
    pub child_properties: *const FField,
    pub properties_size: i32,
    pub min_alignment: i32,
    pub script: TArray<u8>,
    pub property_link: *const FProperty,
    pub ref_link: *const FProperty,
    pub destructor_link: *const FProperty,
    pub post_construct_link: *const FProperty,
    pub script_and_property_object_references: TArray<*const UObject>,
    pub unresolved_script_properties: *const (), //TODO pub TArray<TTuple<TFieldPath<FField>,int>,TSizedDefaultAllocator<32> >*
    pub unversioned_schema: *const (),           //TODO const FUnversionedStructSchema*
}

#[derive(Debug)]
#[repr(C)]
pub struct UFunction {
    pub ustruct: UStruct,
    pub function_flags: EFunctionFlags,
    pub num_parms: u8,
    pub parms_size: u16,
    pub return_value_offset: u16,
    pub rpc_id: u16,
    pub rpc_response_id: u16,
    pub first_property_to_init: *const FProperty,
    pub event_graph_function: *const UFunction,
    pub event_graph_call_offset: i32,
    pub func: unsafe extern "system" fn(*mut UObject, *mut kismet::FFrame, *mut c_void),
}

#[derive(Debug)]
#[repr(C)]
pub struct UClass {
    pub ustruct: UStruct,
}

// #[derive(Debug, Clone, Copy)]
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(C)]
pub struct FName {
    pub comparison_index: FNameEntryId,
    pub number: u32,
}

// #[derive(Debug, Clone, Copy)]
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(C)]
pub struct FNameEntryId {
    pub value: u32,
}

#[derive(Debug)]
#[repr(C)]
pub struct TSharedPtr<T> {
    pub object: *const T,
    pub reference_controller: *const FReferenceControllerBase,
}

#[derive(Debug)]
#[repr(C)]
pub struct FReferenceControllerBase {
    pub shared_reference_count: i32,
    pub weak_reference_count: i32,
}

pub type FString = TArray<u16>;

#[derive(Debug)]
#[repr(C)]
pub struct TArray<T> {
    data: *const T,
    num: i32,
    max: i32,
}
impl<T> TArray<T> {
    fn new() -> Self {
        Self {
            data: std::ptr::null(),
            num: 0,
            max: 0,
        }
    }
}
impl<T> Drop for TArray<T> {
    fn drop(&mut self) {
        unsafe {
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(
                self.data.cast_mut(),
                self.num as usize,
            ))
        }
        globals().gmalloc().free(self.data as *mut c_void);
    }
}
impl<T> Default for TArray<T> {
    fn default() -> Self {
        Self {
            data: std::ptr::null(),
            num: 0,
            max: 0,
        }
    }
}
impl<T> TArray<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: globals().gmalloc().malloc(
                capacity * std::mem::size_of::<T>(),
                std::mem::align_of::<T>() as u32,
            ) as *const T,
            num: 0,
            max: capacity as i32,
        }
    }
    pub fn len(&self) -> usize {
        self.num as usize
    }
    pub fn capacity(&self) -> usize {
        self.max as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn as_slice(&self) -> &[T] {
        if self.num == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.data, self.num as usize) }
        }
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.num == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.data as *mut _, self.num as usize) }
        }
    }
    pub fn clear(&mut self) {
        let elems: *mut [T] = self.as_mut_slice();

        unsafe {
            self.num = 0;
            std::ptr::drop_in_place(elems);
        }
    }
    pub fn push(&mut self, new_value: T) {
        if self.num >= self.max {
            self.max = u32::next_power_of_two((self.max + 1) as u32) as i32;
            let new = globals().gmalloc().realloc(
                self.data as *mut c_void,
                self.max as usize * std::mem::size_of::<T>(),
                std::mem::align_of::<T>() as u32,
            ) as *const T;
            self.data = new;
        }
        unsafe {
            std::ptr::write(self.data.add(self.num as usize).cast_mut(), new_value);
        }
        self.num += 1;
    }
}

impl<T: Clone> Clone for TArray<T> {
    fn clone(&self) -> Self {
        if self.num == 0 {
            return Self::default();
        }

        let mut new_array = TArray::with_capacity(self.num as usize);
        for item in self.as_slice() {
            new_array.push(item.clone());
        }
        new_array
    }
}

impl<T> From<&[T]> for TArray<T>
where
    T: Copy,
{
    fn from(value: &[T]) -> Self {
        let mut new = Self::with_capacity(value.len());
        // TODO this is probably unsound
        new.num = value.len() as i32;
        new.as_mut_slice().copy_from_slice(value);
        new
    }
}

impl Display for FString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let slice = self.as_slice();
        let last = slice.len()
            - slice
                .iter()
                .cloned()
                .rev()
                .position(|c| c != 0)
                .unwrap_or_default();
        write!(
            f,
            "{}",
            widestring::U16Str::from_slice(&slice[..last])
                .to_string()
                .unwrap()
        )
    }
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct FVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct FLinearColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

pub mod kismet {
    use super::*;

    #[derive(Debug)]
    #[repr(C)]
    pub struct FFrame {
        pub base: FOutputDevice,
        pub node: *const c_void,
        pub object: *mut UObject,
        pub code: *const c_void,
        pub locals: *const c_void,
        pub most_recent_property: *const FProperty,
        pub most_recent_property_address: *const c_void,
        pub flow_stack: [u8; 0x30],
        pub previous_frame: *const FFrame,
        pub out_parms: *const c_void,
        pub property_chain_for_compiled_in: *const FField,
        pub current_native_function: *const c_void,
        pub b_array_context_failed: bool,
    }

    pub fn arg<T: Sized>(stack: &mut FFrame, output: &mut T) {
        let output = output as *const _ as *mut _;
        unsafe {
            if stack.code.is_null() {
                let cur = stack.property_chain_for_compiled_in;
                stack.property_chain_for_compiled_in = (*cur).next;
                (globals().fframe_step_explicit_property())(stack, output, cur as *const FProperty);
            } else {
                (globals().fframe_step())(stack, stack.object, output);
            }
        }
    }
}
