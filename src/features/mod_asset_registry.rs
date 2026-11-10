//! Native merge boundary for `ModAssetRegistry.bin`.
//!
//! The SDK packages the engine's cooked Asset Registry payload under this
//! namespaced filename; it is merged into the live registry on the game thread.

use std::fmt;

pub const MOD_ASSET_REGISTRY_FILENAME: &str = "ModAssetRegistry.bin";

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
    crate::game::engine::serialize_mod_asset_registry_bytes(bytes)
        .map_err(ModAssetRegistryMergeError::Engine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_fails_before_any_engine_boundary() {
        assert_eq!(
            validate_payload(&[]),
            Err(ModAssetRegistryMergeError::EmptyPayload)
        );
    }

    #[test]
    fn nonempty_payload_can_be_validated_without_an_engine_call() {
        assert_eq!(
            validate_payload(&[1, 2, 3]),
            Ok(())
        );
    }
}
