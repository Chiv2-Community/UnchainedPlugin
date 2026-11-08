//! Placeholder-gated native merge boundary for `ModAssetRegistry.bin`.
//!
//! The SDK packages the engine's cooked Asset Registry payload under this
//! namespaced filename. The actual UE calls are deliberately disabled until
//! owner-provided target-build signatures or vtable slots have been verified.

use std::fmt;

pub const MOD_ASSET_REGISTRY_FILENAME: &str = "ModAssetRegistry.bin";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModAssetRegistryMergeError {
    EmptyPayload,
    SignaturesUnavailable,
}

impl fmt::Display for ModAssetRegistryMergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPayload => write!(f, "ModAssetRegistry.bin is empty"),
            Self::SignaturesUnavailable => write!(
                f,
                "ModAssetRegistry merge is disabled: verified FAssetRegistryState::Serialize and UAssetRegistryImpl::AppendState signatures/vtable slots are required"
            ),
        }
    }
}

impl std::error::Error for ModAssetRegistryMergeError {}

/// Placeholder-gated, game-thread-only native merge entry point.
///
/// Keeping this boundary callable now lets pak classification hand off bytes
/// safely. It performs no engine call until the two target-build call
/// boundaries in `resolvers::asset_registry` are verified and enabled.
pub fn append_mod_asset_registry_on_game_thread(
    bytes: &[u8],
) -> Result<(), ModAssetRegistryMergeError> {
    if bytes.is_empty() {
        return Err(ModAssetRegistryMergeError::EmptyPayload);
    }
    Err(ModAssetRegistryMergeError::SignaturesUnavailable)
}

/// Queue a registry payload for the existing game-thread job queue.
///
/// Pak reads may run off-thread. The callback always runs on the game thread,
/// and presently reports `SignaturesUnavailable` for non-empty input rather
/// than attempting an unverified UE call.
pub fn append_mod_asset_registry_async<F>(bytes: Vec<u8>, on_complete: F)
where
    F: FnOnce(Result<(), ModAssetRegistryMergeError>) + Send + 'static,
{
    crate::resolvers::unchained_integration::run_on_game_thread(move || {
        on_complete(append_mod_asset_registry_on_game_thread(&bytes));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_fails_before_any_engine_boundary() {
        assert_eq!(
            append_mod_asset_registry_on_game_thread(&[]),
            Err(ModAssetRegistryMergeError::EmptyPayload)
        );
    }

    #[test]
    fn nonempty_payload_is_explicitly_gated_until_signatures_are_verified() {
        assert_eq!(
            append_mod_asset_registry_on_game_thread(&[1, 2, 3]),
            Err(ModAssetRegistryMergeError::SignaturesUnavailable)
        );
    }
}
