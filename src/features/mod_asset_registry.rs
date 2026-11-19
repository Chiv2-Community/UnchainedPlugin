//! Native merge boundary for `ModAssetRegistry.bin`.
//!
//! The SDK packages the engine's cooked Asset Registry payload under this
//! namespaced filename; it is merged into the live registry on the game thread.

use crate::chivalry2::engine::{
    FArchive, FLargeMemoryReader, FLargeMemoryReaderStorage, FName, IAssetRegistry,
};
use std::fmt;

pub const MOD_ASSET_REGISTRY_FILENAME: &str = "ModAssetRegistry.bin";

// Shipped FArchiveState is 0x90 bytes (WITH_EDITORONLY_DATA=0). Its ArIsError
// bit is byte 0x29, bit 0; the reader ctor/dtor confirm the surrounding layout.
const AR_IS_ERROR_BYTE: usize = 0x29;
const AR_IS_ERROR_BIT: u8 = 0x01;

// FGuid (16) + version int32 (4), before any asset data is consumed.
const REGISTRY_HEADER_BYTES: i64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModAssetRegistryMergeError {
    EmptyPayload,
    Engine(String),
}

impl fmt::Display for ModAssetRegistryMergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPayload => write!(f, "ModAssetRegistry.bin is empty"),
            Self::Engine(error) => write!(f, "ModAssetRegistry merge failed: {error}"),
        }
    }
}

impl std::error::Error for ModAssetRegistryMergeError {}

fn validate_payload(bytes: &[u8]) -> Result<(), ModAssetRegistryMergeError> {
    if bytes.is_empty() {
        Err(ModAssetRegistryMergeError::EmptyPayload)
    } else {
        Ok(())
    }
}

/// Game-thread-only native merge entry point.
///
/// The target's `UAssetRegistryImpl::Serialize(FArchive&)` owns its state and
/// options, so Rust only constructs the verified `FLargeMemoryReader` wrapper.
pub unsafe fn append_mod_asset_registry_on_game_thread(
    bytes: &[u8],
) -> Result<(), ModAssetRegistryMergeError> {
    validate_payload(bytes)?;
    serialize_mod_asset_registry_bytes(bytes).map_err(ModAssetRegistryMergeError::Engine)
}

fn classify_registry_load(errored: bool, consumed: i64, total: i64) -> Result<(), String> {
    if errored {
        return Err(format!(
            "Asset Registry archive reported an error after {consumed}/{total} bytes"
        ));
    }
    // UAssetRegistryImpl::Serialize discards the state's success bool.
    // Invalid magic/version can return without setting the archive error bit.
    if consumed <= REGISTRY_HEADER_BYTES {
        return Err(format!(
            "Asset Registry rejected the header or version (read {consumed}/{total} bytes, no asset data)"
        ));
    }
    Ok(())
}

/// # Safety
/// `reader` must be a live native FLargeMemoryReader of the supported layout.
unsafe fn registry_load_outcome(reader: *mut FLargeMemoryReader) -> Result<(), String> {
    let errored = *reader.cast::<u8>().add(AR_IS_ERROR_BYTE) & AR_IS_ERROR_BIT != 0;
    let archive = reader.cast::<FArchive>();
    classify_registry_load(errored, archive.tell(), archive.total_size())
}

/// # Safety
/// Call on the game thread with a nonempty cooked registry payload.
unsafe fn serialize_mod_asset_registry_bytes(bytes: &[u8]) -> Result<(), String> {
    let size = i64::try_from(bytes.len())
        .map_err(|_| "ModAssetRegistry.bin is too large for FArchive".to_string())?;
    let registry = IAssetRegistry::asset_registry_interface()?;
    crate::resolvers::require_function_bindings(&[
        "FLargeMemoryReaderCtor",
        "FLargeMemoryReaderDtor",
    ])?;
    let mut storage = std::mem::MaybeUninit::<FLargeMemoryReaderStorage>::zeroed();
    let reader = storage.as_mut_ptr().cast::<FLargeMemoryReader>().construct(
        bytes.as_ptr(),
        size,
        0, // ELargeMemoryReaderFlags::None: borrow bytes; never free Rust memory.
        FName::default(),
    );
    if reader.is_null() {
        return Err("FLargeMemoryReader constructor returned null".into());
    }

    registry.serialize(reader.cast::<FArchive>());
    // Inspect before destruction. A valid read ends at the name-table offset,
    // which can be short of TotalSize; only the header/error checks imply failure.
    let outcome = registry_load_outcome(reader);
    reader.destruct();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTAL: i64 = 8192;

    #[test]
    fn a_wrong_magic_guid_header_is_rejected() {
        assert!(classify_registry_load(false, 16, TOTAL).is_err());
    }

    #[test]
    fn an_unsupported_version_is_rejected() {
        assert!(classify_registry_load(false, 20, TOTAL).is_err());
    }

    #[test]
    fn a_truncated_or_empty_buffer_is_rejected() {
        assert!(classify_registry_load(false, 0, TOTAL).is_err());
    }

    #[test]
    fn a_mid_stream_error_flag_is_rejected() {
        assert!(classify_registry_load(true, 4096, TOTAL).is_err());
    }

    #[test]
    fn a_successful_read_ending_before_the_trailing_name_table_succeeds() {
        assert!(classify_registry_load(false, 4096, TOTAL).is_ok());
        assert!(classify_registry_load(false, TOTAL, TOTAL).is_ok());
    }

    #[test]
    fn empty_payload_fails_before_any_engine_boundary() {
        assert_eq!(
            unsafe { append_mod_asset_registry_on_game_thread(&[]) },
            Err(ModAssetRegistryMergeError::EmptyPayload)
        );
    }

    #[test]
    fn nonempty_payload_can_be_validated_without_an_engine_call() {
        assert_eq!(validate_payload(&[1, 2, 3]), Ok(()));
    }
}
