//! Plugin registration and native-binding integration checks.

#[cfg(test)]
mod binding_tests {
    use unchained_macros::*;

    #[test]
    fn native_detours_keep_their_registered_names_and_state_access() {
        let expected = [
            "UTBLGameInstance__LoadFrontEndMap", "UGameEngineTick", "UTBLGameInstance__OnPreLoadMap", "UTBLGameInstance__OnPostLoadMap",
            "AGameMode__SetMatchState", "TBLUtility__ExecuteConsoleCommand", "FPakPlatformFile__Mount",
            "ATBLPlayerController__ClientReceiveChat_Implementation", "ATBLGameMode__BroadcastChat", "APlayerController__ClientMessage",
            "ATBLPlayerController__ClientReceiveLocalizedChat_Implementation", "FConsoleOutputDevice__Serialize", "UMapSpecificLoadingScreenConfig__GetDataForMap",
            "ATBLPlayerController__GetOwnershipFromPlayerControllerAndState",
            "ATBLPlayerController__CanUseLoadoutItem", "ATBLPlayerController__CanUseCharacter",
            "ATBLPlayerController__ConditionalInitializeCustomizationOnServer",
            "DedicatedServerInit", "UObject__ProcessEvent", "UGameplay__IsDedicatedServer",
            "FOnlineAuthPlayFab__EndAuthentication", "UGameplayStatics__LoadGameFromSlot", "UGameplayStatics__SaveGameToSlot",
            "ATBLGameMode__PreLogin", "ATBLGameMode__Logout", "PlayFabRequestHandler__SendRequest", "FTBLOnlineStats__SetStatInner",
            "FOnlineSessionPlayFab__OnGetCurrentGames", "FOnlineSessionPlayFab__PingSomeServers", "FString__Find", "FUDPPing__UDPEcho",
            "FString__ParseIntoArray", "UClass__GetConfigName", "UObject__LoadConfig", "UUserFeedbackAndBugReportsLibrary__GetGameInfo",
            "GetPost_GetMotd", "GetPost_GetCurrentGames", "APlayerController__ClientTravelInternal",
        ];
        for name in expected {
            assert_eq!(inventory::iter::<HookRegistration>.into_iter()
                .filter(|hook| hook.name == name).count(), 1, "Hook {name}");
            assert_eq!(inventory::iter::<unchained_macros::DetourBindingRegistration>.into_iter()
                .filter(|hook| hook.name == name).count(), 1, "State access for {name}");
        }
        assert_eq!(crate::resolvers().filter(|resolver| resolver.name == "UTBLSystemLibrary__GetTBLGameMode").count(), 1);
    }

    #[test]
    fn udp_echo_binding_and_plugin_replacement_share_one_native_target() {
        use crate::Chivalry2::Engine::Runtime::Online::ICMP::Icmp::FUDPPing;
        let binding = FUDPPing::__UNCHAINED_BINDING_UDPEcho.binding();
        assert_eq!(binding.name(), "FUDPPing__UDPEcho");
        assert_eq!(inventory::iter::<FunctionBindingRegistration>.into_iter()
            .filter(|registration| std::ptr::eq(registration.binding, binding)).count(), 1);
        assert_eq!(inventory::iter::<HookRegistration>.into_iter()
            .filter(|hook| hook.name == binding.name()).count(), 1);
    }

    #[test]
    fn colocated_patches_keep_their_resolvers_and_registration() {
        let expected = [
            "UTBLLocalPlayer__Exec", "AuthenticateConsoleCommandCaller",
            "AllowAlternateBackend", "EACAntiCheatMesssage", "UNetDriver__GetNetMode",
            "FLogUserIntoOnlineSubsystemNode__GetAuthTokenPatchSite",
            #[cfg(feature = "move-autonomous-desync")]
            "UCharacterMovementComponent__MoveAutonomous",
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
            "APlayerState__GetPlayerName", "ATBLGameMode__BroadcastLocalizedChat",
            "FLargeMemoryReader__FLargeMemoryReader", "FLargeMemoryReader__Destructor", "FName__FName",
            "FString__AppendChars", "FText__AsCultureInvariant", "FText__Destructor",
            "FText__ToString", "UGameplayStatics__GetAllActorsOfClass", "UAssetManager__GetAssetRegistry",
            "UClass__GetDefaultObject", "UTBLSystemLibrary__GetTBLGameMode", "UWorld__InternalGetNetMode",
            "UWorld__SpawnActor", "StaticFindObject", "StaticLoadObject",
            "UTBLGameInstance__AddChatLine", "UAssetRegistryHelpers__GetAssetRegistry",
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
        // Detached replacements retain a model binding as well as a hook.
        assert_eq!(bindings, expected.into_iter().chain(hooks.iter().copied()).collect());
        for name in &hooks {
            assert_eq!(inventory::iter::<FunctionBindingRegistration>.into_iter()
                .filter(|registration| registration.binding.name() == *name).count(), 1,
                "Detached hook {name} must target one model binding");
        }
        for name in ["DedicatedServerInit", "APlayerController__ClientMessage", "UGameEngineTick",
            "ATBLCharacter__OnKilled", "ATBLCharacter__OnDamageTaken"] {
            assert!(hooks.contains(name), "Active hook {name} was removed");
        }
    }

    #[test]
    fn borrowed_binding_signatures_preserve_return_ownership() {
        use crate::Chivalry2::Engine::APlayerState;
        use crate::Chivalry2::Engine::FString;
        use crate::Chivalry2::Engine::FText;
        use crate::Chivalry2::Engine::AssetRegistryScriptInterface;
        use crate::Chivalry2::Engine::UWorld;
        let _: for<'a> unsafe fn(&'a FText) -> &'a FString = FText::ToString;
        let _: for<'a> unsafe fn(*const APlayerState, &'a mut FString) -> &'a mut FString =
            APlayerState::GetPlayerName;
        let _: for<'a> unsafe fn(*const UWorld, &'a mut FString) -> &'a mut FString = UWorld::GetMapName;
        let _: for<'a> unsafe fn(&'a mut AssetRegistryScriptInterface) -> &'a mut AssetRegistryScriptInterface =
            crate::Chivalry2::Engine::UAssetRegistryHelpers::GetAssetRegistry;
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
