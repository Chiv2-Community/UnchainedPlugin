#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(clippy::upper_case_acronyms)]

use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::raw::c_void;
use widestring::U16CString;

use crate::ue::{EFindName, EObjectFlags, FArchive, FAssetData, FName, FLargeMemoryReader, FString, TArray, UObject};

#[repr(C)]
pub enum ENetMode {
    STANDALONE,
    DEDICATED_SERVER,
    LISTEN_SERVER,
    CLIENT,
    MAX,
}

// Hashes for TMap
use crate::ue::UEHash;
impl UEHash for *mut crate::ue::UClass {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}
impl UEHash for *mut crate::ue::UObject {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}
pub use crate::ue::core::FText;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct FRotator {
    /// Rotation around the right axis (Look up/down), 0=Horizontal, +Up, -Down.
    pub pitch: f32, // 0x0
    /// Rotation around the up axis (Turn left/right), 0=Forward, +Right, -Left.
    pub yaw: f32,   // 0x4
    /// Rotation around the forward axis (Tilt left/right), 0=No tilt, +Clockwise, -CCW.
    pub roll: f32,  // 0x8
}

impl FRotator {
    pub const ZERO: Self = Self { pitch: 0.0, yaw: 0.0, roll: 0.0 };

    pub fn new(pitch: f32, yaw: f32, roll: f32) -> Self {
        Self { pitch, yaw, roll }
    }

    /// Returns a vector pointing in the direction of this rotation
    pub fn to_vector(&self) -> [f32; 3] {
        let p = self.pitch.to_radians();
        let y = self.yaw.to_radians();
        
        let cp = p.cos();
        let sp = p.sin();
        let cy = y.cos();
        let sy = y.sin();

        [cp * cy, cp * sy, sp]
    }
}

// SoftClassPath

#[repr(C)]
#[derive(Debug)]
pub struct FSoftObjectPath {
    pub asset_path_name: FName,   // 0x00
    pub sub_path_string: FString, // 0x08
}

impl FSoftObjectPath {
    pub fn new(path: &str) -> Self {
        Self {
            // A loaded object need not have its full path interned as one FName.
            asset_path_name: FName::with_type(path, EFindName::Add),
            sub_path_string: FString::default(),
        }
    }

    /// Full object path (`/Game/Pkg.Object`, plus `:SubPath` when set). Empty
    /// for an unset path. `FSoftClassPath` shares this layout.
    pub fn to_path_string(&self) -> String {
        if self.asset_path_name.comparison_index.value == 0 {
            return String::new();
        }
        let asset = self.asset_path_name.to_string();
        let sub_path = self.sub_path_string.to_string();
        if sub_path.is_empty() {
            asset
        } else {
            format!("{asset}:{sub_path}")
        }
    }
}
const _: () = assert!(std::mem::size_of::<FSoftObjectPath>() == 0x18);

#[repr(C)]
#[derive(Debug)]
pub struct FSoftObjectPtr {
    pub weak_ptr: [u8; 8],     // 0x00
    pub tag_at_last_test: i32, // 0x08
    pub padding: u32,          // 0x0C

    pub path: FSoftObjectPath, // 0x10
}

const _: () = assert!(std::mem::size_of::<FSoftObjectPtr>() == 0x28);

impl FSoftObjectPtr {
    pub fn from_path(path: &str) -> Self {
        Self {
            // WeakPtr must be zeroed so the engine knows the cache is cold
            weak_ptr: [0u8; 8],
            tag_at_last_test: -1,
            padding: 0,
            path: FSoftObjectPath::new(path),
        }
    }
}

impl<T> TSoftClassPtr<T> {
    pub fn from_path(path: &str) -> Self {
        Self {
            soft_ptr: FSoftObjectPtr::from_path(path),
            _marker: std::marker::PhantomData,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct TSoftClassPtr<T = c_void> {
    pub soft_ptr: FSoftObjectPtr,
    _marker: std::marker::PhantomData<T>,
}

// FName helpers



#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ESpawnActorCollisionHandlingMethod {
    Undefined = 0,
    AlwaysSpawn = 1,
    AdjustIfPossibleButAlwaysSpawn = 2,
    AdjustIfPossibleButDontSpawnIfColliding = 3,
    DontSpawnIfColliding = 4,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ESpawnActorNameMode {
    RequiredFatal = 0,
    RequiredErrorAndReturnNull = 1,
    RequiredReturnNull = 2,
    Requested = 3,
}

// Spawning actors
#[repr(C, align(8))]
pub struct FActorSpawnParameters {
    // 0x00: FName (Name) - Size 0x8
    pub name: u64, 
    // 0x08 - 0x20: Pointer types
    pub template: *const c_void,   // AActor*
    pub owner: *const c_void,      // AActor*
    pub instigator: *const c_void, // APawn*
    pub override_level: *const c_void, // ULevel*
    // 0x28: Size 0x1
    pub spawn_collision_handling_override: ESpawnActorCollisionHandlingMethod, // ESpawnActorCollisionHandlingMethod
    // 0x29: Bitfield uchar:1 (Size 0x1)
    // Offset 0x29 handles 4 bits: bRemoteOwned, bNoFail, bDeferConstruction, bAllowDuringConstructionScript
    pub bitfield: u8,
    // 0x2a: Size 0x1
    pub name_mode: ESpawnActorNameMode, // ESpawnActorNameMode
    // 0x2b: Padding
    pub _padding: u8,
    // 0x2c: Size 0x4
    pub object_flags: EObjectFlags, // EObjectFlags
}

impl Default for FActorSpawnParameters {
    fn default() -> Self {
        Self {
            name: 0, // None
            template: std::ptr::null(),
            owner: std::ptr::null(),
            instigator: std::ptr::null(),
            override_level: std::ptr::null(),
            // Default UE4/5 behavior: AlwaysSpawn
            spawn_collision_handling_override: ESpawnActorCollisionHandlingMethod::AlwaysSpawn,
            bitfield: 0,
            // Default UE4/5 behavior: Requested
            name_mode: ESpawnActorNameMode::Requested,
            _padding: 0,
            // Default UE4/5 behavior: RF_NoFlags
            object_flags: EObjectFlags::RF_NoFlags,
        }
    }
}

impl FActorSpawnParameters {
    pub fn new() -> Self {
        Self::default()
    }
    /// Builder method to set the Spawn Collision Handling mode
    pub fn with_spawn_mode(mut self, mode: ESpawnActorCollisionHandlingMethod) -> Self {
        self.spawn_collision_handling_override = mode;
        self
    }
    /// Builder method to set the Owner
    pub fn with_owner(mut self, owner: *const c_void) -> Self {
        self.owner = owner;
        self
    }
    /// Builder method to set the bNoFail bit
    pub fn no_fail(mut self, enabled: bool) -> Self {
        if enabled {
            self.bitfield |= 0x02;
        } else {
            self.bitfield &= !0x02;
        }
        self
    }
    pub fn remote_owned(&self) -> bool { (self.bitfield & (1 << 0)) != 0 }
    pub fn defer_construction(&self) -> bool { (self.bitfield & (1 << 2)) != 0 }
    pub fn allow_during_construction(&self) -> bool { (self.bitfield & (1 << 3)) != 0 }
}

// Helper functions

/// Target-verified `FLargeMemoryReader` storage: the constructor writes the
/// final `FName` at +0xb0, so the UE 4.25 target object occupies 0xb8 bytes.
/// Do not expose fields: the native constructor/destructor own initialization.
#[repr(C, align(8))]
struct FLargeMemoryReaderStorage {
    bytes: [u8; 0xb8],
}

/// Offsets into the verified `FLargeMemoryReader` layout for this build.
/// `FArchiveState` is 0x90 bytes here: the shipped game is `WITH_EDITORONLY_DATA=0`,
/// so `DEVIRTUALIZE_FLinkerLoad_Serialize` is on and the `ActiveFPLB`/`InlineFPLB`
/// members are present, pushing the `ArIsXxx` bitfields to 0x28 (`ArIsError` is the
/// 9th bit -> byte 0x29, bit 0). Cross-checked
/// against the resolved ctor/dtor byte patterns (`bFreeOnClose` at 0x98, `Data` at
/// 0xa0, total object size 0xb8).
const AR_IS_ERROR_BYTE: usize = 0x29;
const AR_IS_ERROR_BIT: u8 = 0x01;

/// Bytes the registry loader consumes for its header before it reads any asset
/// data: `FGuid` (16) + version `int32` (4). `UAssetRegistryImpl::Serialize` is
/// void and discards `FAssetRegistryState::Serialize`'s success bool, so a load
/// rejected at the version gate (wrong magic GUID or an unsupported/old version)
/// is invisible to the caller except that the reader is left at or before this
/// mark having read nothing — and, per the engine, with no error flag set. See
/// `Runtime/AssetRegistry/Private/AssetData.cpp` `FAssetRegistryVersion::SerializeVersion`
/// (returns before reading the version on a GUID mismatch) and `AssetRegistryState.cpp`
/// `FAssetRegistryState::Serialize` (`Version < RemovedMD5Hash` returns false,
/// without `SetError`).
const REGISTRY_HEADER_BYTES: i64 = 20;

/// Decides whether a finished registry read actually loaded data, from the
/// reader's end state: `errored` is `FArchive::IsError()`, `consumed` is `Tell()`,
/// `total` is `TotalSize()`. A valid load reads the version, then (via
/// `FNameTableArchiveReader`) seeks to the name table stored at the end of the
/// buffer and back, ending at the name-table offset — so `consumed` is well past
/// the header yet legitimately short of `total`. Rejection shows up either as the
/// error flag (corruption detected mid-read) or as a reader that never got past
/// the header (bad GUID or unsupported version, which the engine does not flag).
fn classify_registry_load(errored: bool, consumed: i64, total: i64) -> Result<(), String> {
    if errored {
        return Err(format!(
            "Asset Registry archive reported an error after {consumed}/{total} bytes"
        ));
    }
    if consumed <= REGISTRY_HEADER_BYTES {
        return Err(format!(
            "Asset Registry rejected the header or version (read {consumed}/{total} bytes, no asset data)"
        ));
    }
    Ok(())
}

/// Reads the reader's end state and classifies the load. Must be called while the
/// reader is still alive (before its destructor runs).
///
/// # Safety
/// `reader` must be a live `FLargeMemoryReader` of this build's layout.
unsafe fn registry_load_outcome(reader: *mut FLargeMemoryReader) -> Result<(), String> {
    let base = reader.cast::<u8>();
    let errored = *base.add(AR_IS_ERROR_BYTE) & AR_IS_ERROR_BIT != 0;
    let archive = reader.cast::<FArchive>();
    classify_registry_load(errored, archive.tell(), archive.total_size())
}

/// Deserialize a cooked `ModAssetRegistry.bin` into the live registry.
///
/// Must run on the game thread. The target's `UAssetRegistryImpl::Serialize`
/// owns the internal state/options and calls `CachePathsFromState`; this avoids
/// constructing either private UE object in Rust. The archive borrows `bytes`
/// for the synchronous call, so it uses `ELargeMemoryReaderFlags::None`.
///
/// Returns `Err` when the engine rejected the payload (bad header/version or
/// corruption) so the caller can fall back to header discovery for that pak;
/// see [`classify_registry_load`].
pub unsafe fn serialize_mod_asset_registry_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("ModAssetRegistry.bin is empty".into());
    }
    let size = i64::try_from(bytes.len())
        .map_err(|_| "ModAssetRegistry.bin is too large for FArchive".to_string())?;

    let registry = crate::ue::IAssetRegistry::asset_registry_interface()?;

    crate::resolvers::require_function_bindings(&["FLargeMemoryReaderCtor", "FLargeMemoryReaderDtor"])?;
    let mut storage = std::mem::MaybeUninit::<FLargeMemoryReaderStorage>::zeroed();
    let reader = storage.as_mut_ptr().cast::<FLargeMemoryReader>().construct(
        bytes.as_ptr(),
        size,
        0, // ELargeMemoryReaderFlags::None: borrow bytes, never free Rust memory.
        FName::default()
    );
    if reader.is_null() {
        return Err("FLargeMemoryReader constructor returned null".into());
    }

    registry.serialize(reader.cast::<FArchive>());
    // `UAssetRegistryImpl::Serialize` is void and swallows the engine's success
    // bool, so inspect the reader's end state while it is still alive: a rejected
    // header/version leaves the cursor unadvanced, corruption sets the error flag.
    let load_outcome = registry_load_outcome(reader);
    reader.destruct();
    load_outcome
}

pub unsafe fn get_uobject_from_path(path: &str, load_if_missing: bool) -> *mut UObject {
    let wide_path = U16CString::from_str(path).unwrap();
    
    if !load_if_missing {
        // Try to find it in memory first
        return crate::ue::static_find_object(
            std::ptr::null_mut(), 
            std::ptr::null_mut(), 
            wide_path.as_ptr(), 
            false
        );
    }

    crate::ue::static_load_object(
        std::ptr::null_mut(), // Any class
        std::ptr::null_mut(), // No specific outer
        wide_path.as_ptr(), 
        std::ptr::null(),     // Filename (usually null)
        0,                    // LoadFlags (0 = LoadConfig)
        std::ptr::null_mut(), // Sandbox
        false,                // AllowNativeComponentClass
        std::ptr::null_mut()
    )
}

pub unsafe fn get_assets_by_class(class_name: &str) -> Result<TArray<FAssetData>, String> {
    let registry = crate::ue::IAssetRegistry::asset_registry_interface()?;

    let name_res = FName::with_type(class_name, crate::ue::EFindName::Find);

    let mut asset_data = TArray::<FAssetData>::default();
    if registry.get_assets_by_class(name_res, &mut asset_data, true) {
        Ok(asset_data)
    } else {
        Err(format!("GetAssetsByClass returned false for {class_name}"))
    }
}

/// Queries the registry interface on the game thread.
pub unsafe fn get_assets_by_path(package_path: &str, recursive: bool) -> Result<TArray<FAssetData>, String> {
    let registry = crate::ue::IAssetRegistry::asset_registry_interface()?;

    let name_res = FName::with_type(package_path, crate::ue::EFindName::Find);

    let mut asset_data = TArray::<FAssetData>::default();
    // bool GetAssetsByPath(FName PackagePath, TArray<FAssetData>& OutAssetData, bool bRecursive, bool bIncludeOnlyOnDiskAssets)
    if registry.get_assets_by_path(name_res, &mut asset_data, recursive, false) {
        Ok(asset_data)
    } else {
        Err(format!("GetAssetsByPath returned false for {package_path}"))
    }
}

/// Calls the registry's public files-only scan wrapper (skips the
/// Intermediates `AssetRegistryCache` write). Must run on the game thread.
pub unsafe fn scan_files_synchronous(files: &[String]) -> Result<(), String> {
    let registry = crate::ue::IAssetRegistry::asset_registry_interface()?;

    let Some(allocator) = crate::ue::try_gmalloc() else {
        return Err("engine allocator (GMalloc) is not available; run on the game thread after globals init".into());
    };

    let mut files_array = TArray::<FString>::default();
    for file in files {
        files_array.push_in(FString::from(file.as_str()), allocator);
    }

    registry.scan_files_synchronous(&files_array, false);

    Ok(())
}

/// Partial socket address vtable. ToString is slot 15 in the supported builds.
#[rust_ctables::cpp_struct]
pub struct FInternetAddr {
    #[vtable(
        offset(0x78);
        write_native_string(&self, output: *mut FString, append_port: bool) -> *mut FString;
    )]
    vtable: *const (),
}

impl FInternetAddr {
    /// # Safety
    /// This must be a live native socket address with the supported vtable ABI.
    #[inline(always)]
    pub unsafe fn to_native_string(&self, append_port: bool) -> String {
        let mut output = FString::new();
        self.write_native_string(&mut output, append_port);
        output.to_string()
    }
}

/// Partial UNetDriver layout; fields after ServerConnection are omitted.
#[repr(C)]
pub struct UNetDriver {
    pub base: UObject,
    _padding_to_server_connection: [u8; 0x60],
    pub server_connection: *mut UObject, // 0x0088
}

#[repr(C)]
pub struct UIpNetDriver {
    pub base: UNetDriver,
    _padding_to_local_addr: [u8; 0x620],
    pub local_addr: *const FInternetAddr, // 0x06B0 (shared pointer)
}

#[repr(C)]
pub struct UIpConnection {
    pub base: UObject,
    _padding_to_remote_addr: [u8; 0xF0],
    pub remote_addr: *const FInternetAddr, // 0x0118 (shared pointer)
}

// SAFETY: These supported native layouts have their UObject prefix at offset zero.
unsafe impl crate::ue::UObjectType for UIpNetDriver {
    const CLASS_NAME: &'static str = "IpNetDriver";
}
unsafe impl crate::ue::UObjectType for UIpConnection {
    const CLASS_NAME: &'static str = "IpConnection";
}
pub use crate::ue::engine::UWorld;

#[cfg(test)]
mod registry_load_tests {
    use super::classify_registry_load;

    // A plausible ModAssetRegistry.bin size; the exact value is irrelevant since
    // a valid read ends at the name-table offset, short of the total.
    const TOTAL: i64 = 8192;

    #[test]
    fn a_wrong_magic_guid_header_is_rejected() {
        // SerializeVersion reads only the 16-byte FGuid, then bails on mismatch.
        assert!(classify_registry_load(false, 16, TOTAL).is_err());
    }

    #[test]
    fn an_unsupported_version_is_rejected() {
        // GUID matches, the version int is read (20 bytes), then the engine
        // returns early because Version < RemovedMD5Hash — with no error flag.
        assert!(classify_registry_load(false, 20, TOTAL).is_err());
    }

    #[test]
    fn a_truncated_or_empty_buffer_is_rejected() {
        assert!(classify_registry_load(false, 0, TOTAL).is_err());
    }

    #[test]
    fn a_mid_stream_error_flag_is_rejected() {
        // Corruption detected deep in the stream: read past the header, flag set.
        assert!(classify_registry_load(true, 4096, TOTAL).is_err());
    }

    #[test]
    fn a_successful_read_ending_before_the_trailing_name_table_succeeds() {
        // A valid load ends at the name-table offset, legitimately short of total.
        assert!(classify_registry_load(false, 4096, TOTAL).is_ok());
        // And one that happens to consume right up to the end is fine too.
        assert!(classify_registry_load(false, TOTAL, TOTAL).is_ok());
    }
}
