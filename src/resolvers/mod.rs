// #[macro_use]
// pub mod macros;

use std::sync::{LazyLock, OnceLock};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[allow(clippy::upper_case_acronyms)]
pub enum PlatformType {
    EGS,
    STEAM,
    XBOX,
    OTHER,
}

pub static PLATFORM: OnceLock<PlatformType> = OnceLock::new();

mod platforms;
pub use platforms::{supports_platform, ResolverPlatform};
pub use unchained_macros::{
    FunctionBinding, FunctionBindingRegistration, ValueBinding, ValueBindingRegistration,
    initialize_value_binding, initialize_value_bindings, is_required_value,
};

/// # Safety
/// RVAs must resolve each registered binding to process-lifetime native code
/// with the declared ABI and behavioral contract for the installed build.
pub unsafe fn initialize_function_bindings(offsets: &std::collections::HashMap<String, u64>) -> Result<(), String> {
    for registration in inventory::iter::<FunctionBindingRegistration> {
        let binding = registration.binding;
        let offset = offsets.get(binding.name())
            .ok_or_else(|| format!("No address found for native binding {}", binding.name()))?;
        unsafe { binding.initialize(*BASE_ADDR, *offset)?; }
    }
    Ok(())
}

pub fn function_is_bound(name: &str) -> bool {
    inventory::iter::<FunctionBindingRegistration>.into_iter()
        .any(|registration| registration.binding.name() == name && registration.binding.address().is_some())
}

/// Preserve fallible helper behavior before native binding initialization.
pub fn require_function_bindings(names: &[&str]) -> Result<(), String> {
    for name in names {
        if !function_is_bound(name) {
            return Err(format!("Native binding {name} is not initialized"));
        }
    }
    Ok(())
}

pub fn current_platform() -> PlatformType {
    *PLATFORM.get().expect("Platform not initialized")
}

impl std::fmt::Display for PlatformType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::str::FromStr for PlatformType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "EGS" => Ok(PlatformType::EGS),
            "STEAM" => Ok(PlatformType::STEAM),
            "XBOX" => Ok(PlatformType::XBOX),
            "OTHER" => Ok(PlatformType::OTHER),
            _ => Err(()),
        }
    }
}

pub static BASE_ADDR: LazyLock<usize> = LazyLock::new(|| unsafe {
    windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
        .expect("Failed to get module handle")
        .0 as usize
});


pub type CreateHookFn = unsafe fn(std::collections::HashMap<String, u64>, bool) -> Result<Option<usize>, Box<dyn std::error::Error>>;

pub struct HookRegistration {
    pub name: &'static str,
    pub hook_fn: CreateHookFn,
    pub condition: fn() -> bool,
    // pub auto_activate: bool,
}

pub type RegistrationPredicate = fn() -> bool;
pub type CreatePatchFn = unsafe fn(std::collections::HashMap<String, u64>) -> Result<(), Box<dyn std::error::Error>>;

pub struct PatchRegistration {
    pub name: &'static str,
    pub tag: &'static str,
    pub patch_fn: CreatePatchFn,
    pub enabled_fn: RegistrationPredicate, // The runtime check
}

/// A named group of offsets to help categorize a set of named offsets.
pub struct OffsetRegisty {
    pub name: &'static str,
    pub map: fn() -> std::collections::HashMap<String, u64>,
}



inventory::collect!(HookRegistration);
inventory::collect!(PatchRegistration);
inventory::collect!(OffsetRegisty);

#[cfg(test)]
mod binding_tests {
    use super::*;

    #[test]
    fn native_detours_keep_their_registered_names_and_state_access() {
        let expected = [
            "LoadFrontEndMap", "UGameEngineTick", "OnPreLoadMap", "OnPostLoadMap",
            "SetMatchState", "ExecuteConsoleCommand", "PakPlatformFileMount",
            "ClientReceiveChat", "BroadcastChat", "ClientMessage",
            "ClientReceiveLocalizedChat", "ConsoleOutputDevice__Serialize", "GetDataForMap",
            "ATBLPlayerController__GetOwnershipFromPlayerControllerAndState",
            "ATBLPlayerController__CanUseLoadoutItem", "ATBLPlayerController__CanUseCharacter",
            "ATBLPlayerController__ConditionalInitializeCustomizationOnServer",
            "DedicatedServerInit", "ProcessEvent", "UGameplay_IsDedicatedServer",
            "PlayFabEndAuthentication", "LoadGameFromSlot", "SaveGameToSlot",
            "ATBLGameMode__PreLogin", "ATBLGameMode__Logout", "SendRequest", "SetStatInner",
            "OnGetCurrentGames_", "PingSomeServers", "FStringFind", "UDPEcho",
            "FStringParseIntoArray", "GetConfigName", "LoadConfig", "GetGameInfo",
            "GetPost_GetMotd", "GetPost_GetCurrentGames", "ClientTravelInternal",
        ];
        for name in expected {
            assert_eq!(inventory::iter::<HookRegistration>.into_iter()
                .filter(|hook| hook.name == name).count(), 1, "Hook {name}");
            assert_eq!(inventory::iter::<unchained_macros::DetourBindingRegistration>.into_iter()
                .filter(|hook| hook.name == name).count(), 1, "State access for {name}");
        }
        assert_eq!(crate::resolvers().filter(|resolver| resolver.name == "GetTBLGameMode").count(), 1);
    }

    #[test]
    fn colocated_patches_keep_their_resolvers_and_registration() {
        let expected = [
            "UTBLLocalPlayer_Exec", "AuthenticateConsoleCommandCaller",
            "AllowAlternateBackend", "EACAntiCheatMesssage", "UNetDriver_GetNetMode",
            "RedpointSteamSkipTokenCheck",
            #[cfg(feature = "move-autonomous-desync")]
            "MoveAutonomous",
            #[cfg(feature = "move-autonomous-desync")]
            "TickActor",
        ];
        for name in expected {
            assert_eq!(inventory::iter::<PatchRegistration>.into_iter()
                .filter(|patch| patch.name == name).count(), 1, "Patch {name}");
            assert_eq!(crate::resolvers().filter(|resolver| resolver.name == name).count(),
                1, "Resolver for patch {name}");
        }
    }

    #[test]
    fn call_only_resolvers_are_bindings_not_detours() {
        let expected = [
            "APlayerState_GetPlayerName", "BroadcastLocalizedChat",
            "FLargeMemoryReaderCtor", "FLargeMemoryReaderDtor", "FNameCtorWchar",
            "FString_AppendChars", "FText_AsCultureInvariantCopy", "FText_Release",
            "FText_ToString", "GetAllActorsOfClass", "GetAssetRegistry",
            "GetDefaultObject", "GetTBLGameMode", "InternalGetNetMode",
            "SpawnActor", "StaticFindObject", "StaticLoadObject",
            "TBLGameInstance_AddChatLine", "UAssetRegistryHelpers__GetAssetRegistry",
            "UWorld__GetMapName",
        ];
        let bindings: std::collections::HashSet<_> =
            inventory::iter::<FunctionBindingRegistration>.into_iter()
                .map(|registration| registration.binding.name()).collect();
        let hooks: std::collections::HashSet<_> =
            inventory::iter::<HookRegistration>.into_iter()
                .map(|registration| registration.name).collect();
        for name in expected {
            assert!(bindings.contains(name), "Missing binding {name}");
            assert!(!hooks.contains(name), "Call-only resolver {name} still detours");
            assert!(crate::resolvers().any(|resolver| resolver.name == name), "Missing resolver {name}");
        }
        assert_eq!(bindings.len(), expected.len());
        for name in ["DedicatedServerInit", "ClientMessage", "UGameEngineTick",
            "ATBLCharacter__OnKilled", "ATBLCharacter__OnDamageTaken"] {
            assert!(hooks.contains(name), "Active hook {name} was removed");
        }
    }

    #[test]
    fn borrowed_binding_signatures_preserve_return_ownership() {
        use crate::ue::{APlayerState, FString, FText, AssetRegistryScriptInterface, UWorld};
        let _: for<'a> unsafe fn(&'a FText) -> &'a FString = FText::to_native_string;
        let _: for<'a> unsafe fn(*const APlayerState, &'a mut FString) -> &'a mut FString =
            APlayerState::get_player_name_native;
        let _: for<'a> unsafe fn(*const UWorld, &'a mut FString) -> &'a mut FString = UWorld::get_map_name;
        let _: for<'a> unsafe fn(&'a mut AssetRegistryScriptInterface) -> &'a mut AssetRegistryScriptInterface =
            crate::ue::UAssetRegistryHelpers::get_asset_registry;
    }

    #[test]
    fn required_values_are_bindings_not_functions_or_hooks() {
        let values: std::collections::HashSet<_> = inventory::iter::<ValueBindingRegistration>.into_iter()
            .map(|registration| registration.binding.name()).collect();
        assert_eq!(values, std::collections::HashSet::from(["GMalloc", "GUObjectArray"]));
        for name in values {
            assert!(!inventory::iter::<FunctionBindingRegistration>.into_iter()
                .any(|registration| registration.binding.name() == name));
            assert!(!inventory::iter::<HookRegistration>.into_iter()
                .any(|registration| registration.name == name));
            assert!(crate::resolvers().any(|resolver| resolver.name == name), "Missing resolver {name}");
        }
    }
}
