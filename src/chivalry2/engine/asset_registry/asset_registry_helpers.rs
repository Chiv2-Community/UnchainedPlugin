//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/AssetRegistry/Public/AssetRegistryHelpers.h>.

use crate::chivalry2::engine::AssetRegistryScriptInterface;
use crate::chivalry2::engine::*;
use unchained_macros::bind_resolved_func;

define_pattern_resolver!(UAssetRegistryHelpers__GetAssetRegistry,[
    "48 89 5C 24 ?? 57 48 83 EC 20 48 8B F9 48 8D 15 ?? ?? ?? ?? 48 8D 4C 24 ?? 41 B8 01 00 00 00 E8 ?? ?? ?? ?? 48 8B 18 E8 ?? ?? ?? ?? 48 8B D3 48 8B C8 E8 ?? ?? ?? ?? 48 8B C8 48 8B 10 FF 52 ?? 48 85 C0"
    ]);

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

#[cfg(test)]
mod registry_load_tests {
    use super::UAssetRegistryHelpers;

    // A plausible ModAssetRegistry.bin size; the exact value is irrelevant since
    // a valid read ends at the name-table offset, short of the total.
    const TOTAL: i64 = 8192;

    #[test]
    fn a_wrong_magic_guid_header_is_rejected() {
        // SerializeVersion reads only the 16-byte FGuid, then bails on mismatch.
        assert!(UAssetRegistryHelpers::classify_registry_load(false, 16, TOTAL).is_err());
    }

    #[test]
    fn an_unsupported_version_is_rejected() {
        // GUID matches, the version int is read (20 bytes), then the engine
        // returns early because Version < RemovedMD5Hash — with no error flag.
        assert!(UAssetRegistryHelpers::classify_registry_load(false, 20, TOTAL).is_err());
    }

    #[test]
    fn a_truncated_or_empty_buffer_is_rejected() {
        assert!(UAssetRegistryHelpers::classify_registry_load(false, 0, TOTAL).is_err());
    }

    #[test]
    fn a_mid_stream_error_flag_is_rejected() {
        // Corruption detected deep in the stream: read past the header, flag set.
        assert!(UAssetRegistryHelpers::classify_registry_load(true, 4096, TOTAL).is_err());
    }

    #[test]
    fn a_successful_read_ending_before_the_trailing_name_table_succeeds() {
        // A valid load ends at the name-table offset, legitimately short of total.
        assert!(UAssetRegistryHelpers::classify_registry_load(false, 4096, TOTAL).is_ok());
        // And one that happens to consume right up to the end is fine too.
        assert!(UAssetRegistryHelpers::classify_registry_load(false, TOTAL, TOTAL).is_ok());
    }
}

/// Static native helpers only; this is not a UObject instance layout.
pub struct UAssetRegistryHelpers {
    _private: (),
}

impl UAssetRegistryHelpers {
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
        UAssetRegistryHelpers::classify_registry_load(errored, archive.tell(), archive.total_size())
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
    /// see [`UAssetRegistryHelpers::classify_registry_load`].
    pub unsafe fn serialize_mod_asset_registry_bytes(bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Err("ModAssetRegistry.bin is empty".into());
        }
        let size = i64::try_from(bytes.len())
            .map_err(|_| "ModAssetRegistry.bin is too large for FArchive".to_string())?;

        let registry = crate::chivalry2::engine::IAssetRegistry::asset_registry_interface()?;

        crate::resolvers::require_function_bindings(&[
            "FLargeMemoryReaderCtor",
            "FLargeMemoryReaderDtor",
        ])?;
        let mut storage = std::mem::MaybeUninit::<FLargeMemoryReaderStorage>::zeroed();
        let reader = storage.as_mut_ptr().cast::<FLargeMemoryReader>().construct(
            bytes.as_ptr(),
            size,
            0, // ELargeMemoryReaderFlags::None: borrow bytes, never free Rust memory.
            FName::default(),
        );
        if reader.is_null() {
            return Err("FLargeMemoryReader constructor returned null".into());
        }

        registry.serialize(reader.cast::<FArchive>());
        // `UAssetRegistryImpl::Serialize` is void and swallows the engine's success
        // bool, so inspect the reader's end state while it is still alive: a rejected
        // header/version leaves the cursor unadvanced, corruption sets the error flag.
        let load_outcome = UAssetRegistryHelpers::registry_load_outcome(reader);
        reader.destruct();
        load_outcome
    }

    pub unsafe fn get_assets_by_class(class_name: &str) -> Result<TArray<FAssetData>, String> {
        let registry = crate::chivalry2::engine::IAssetRegistry::asset_registry_interface()?;

        let name_res = FName::with_type(class_name, crate::chivalry2::engine::EFindName::Find);

        let mut asset_data = TArray::<FAssetData>::default();
        if registry.get_assets_by_class(name_res, &mut asset_data, true) {
            Ok(asset_data)
        } else {
            Err(format!("GetAssetsByClass returned false for {class_name}"))
        }
    }

    /// Queries the registry interface on the game thread.
    pub unsafe fn get_assets_by_path(
        package_path: &str,
        recursive: bool,
    ) -> Result<TArray<FAssetData>, String> {
        let registry = crate::chivalry2::engine::IAssetRegistry::asset_registry_interface()?;

        let name_res = FName::with_type(package_path, crate::chivalry2::engine::EFindName::Find);

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
        let registry = crate::chivalry2::engine::IAssetRegistry::asset_registry_interface()?;

        let allocator = crate::chivalry2::engine::FMalloc::global();

        let mut files_array = TArray::<FString>::default();
        for file in files {
            files_array.push_in(FString::from(file.as_str()), allocator);
        }

        registry.scan_files_synchronous(&files_array, false);

        Ok(())
    }

    /// UAssetRegistryHelpers::GetAssetRegistry, with explicit hidden return storage.
    ///
    /// # Safety
    /// Call on the engine thread after registry initialization; output must be
    /// writable native TScriptInterface storage. The returned pointers are borrowed.
    #[bind_resolved_func(self::UAssetRegistryHelpers__GetAssetRegistry)]
    pub unsafe fn get_asset_registry<'a>(
        output: &'a mut AssetRegistryScriptInterface,
    ) -> &'a mut AssetRegistryScriptInterface {
    }
}
