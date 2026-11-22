//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLPlayerController.h>.
//! Native owner: TBL/Source/TBL/TBLPlayerController.h.

use crate::Chivalry2::Engine::FPrimaryAssetId;
use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::FText;
use crate::Chivalry2::Engine::TSubclassOfAInventoryItem;
use crate::Chivalry2::Engine::TSubclassOfATBLCharacter;
use crate::Chivalry2::Engine::TSubclassOfULoadoutSelection;
use crate::Chivalry2::TBL::ATBLPlayerState;
use crate::Chivalry2::TBL::FOwnershipResponse;
use std::os::raw::c_void;
use crate::PlatformType;

// Targets ATBLPlayerController::ClientReceiveChat_Implementation.
define_pattern_resolver!(
    ATBLPlayerController__ClientReceiveChat_Implementation,
    ["40 53 55 57 41 57 48 81 EC B8 00 00 00 41 8B E9 4D 8B F8 48 8B DA 48"]
);

// Methods to enable customization / loadout on custom servers

define_pattern_resolver!(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState, {
    PlatformType::EGS: [
        "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD",
        "40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 38 fd",
    ],
    PlatformType::STEAM: [
        "40 55 56 41 54 41 55 41 56 41 57 48 8D AC 24 B8",
        "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD", // EGS 2.11.4
        "40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 38 fd",
        ], // STEAM
    PlatformType::OTHER: ["40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 38 fd"]
});

define_pattern_resolver!(ATBLPlayerController__CanUseLoadoutItem, {
    PlatformType::EGS: ["48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00"], // EGS
    // "48 89 5C 24 08 48 89 74 24 18 55 57 41 55 41 56 41 57 48 8B EC 48 83 EC", // STEAM
    // from sigga
    // "48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00", // EGS
    PlatformType::STEAM: [
        "48 89 5C 24 08 48 89 74 24 18 55 57 41 55 41 56 41 57 48 8B EC 48 83 EC 60 49 8B 31 33 FF C6 02 00",
        "48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00" // EGS 2.11.4
        ], // STEAM
});

define_pattern_resolver!(
    ATBLPlayerController__CanUseCharacter,
    [
        "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 50 49 8B 18", // universal
    ]
);

define_pattern_resolver!(ATBLPlayerController__ConditionalInitializeCustomizationOnServer, {
    PlatformType::EGS: [
        "48 89 54 24 10 53 56 57 41 54 48 83 EC 78 48 8B 99 60 02 00 00 48 8B F2 0F B6",
        "41 54 48 81 EC 80 00 00 00 80 B9 F8 00 00 00 03 4C 8B E1 ?? ?? ?? ?? ?? ?? 80 B9 20 13 00 00 00 ?? ?? ?? ?? ?? ?? 80 B9 21",
    ],
    PlatformType::STEAM: [
        "48 89 54 24 10 53 ?? 57 41 54 48 83 EC 78",
        "41 54 48 81 EC 80 00 00 00 80 B9 F8 00 00 00 03 4C 8B E1 ?? ?? ?? ?? ?? ?? 80 B9 20 13 00 00 00 ?? ?? ?? ?? ?? ?? 80 B9 21",
    ],
    // From Sigga
    // Did the function change?
    PlatformType::OTHER: ["41 54 48 81 EC 80 00 00 00 80 B9 F8 00 00 00 03 4C 8B E1 ?? ?? ?? ?? ?? ?? 80 B9 20 13 00 00 00 ?? ?? ?? ?? ?? ?? 80 B9 21"],
});

define_pattern_resolver!(ATBLPlayerController__ClientReceiveLocalizedChat_Implementation, [
    "48 89 5C 24 18 55 57 41 56 48 81 EC 90 00 00 00 48 8B 99 D0 02 00 00 41 8B E8 4C 8B F2 48 8B F9"
]);

unsafe impl crate::Chivalry2::Engine::UObjectType for ATBLPlayerController {
    const CLASS_NAME: &'static str = "TBLPlayerController";
}

#[unchained_macros_core::runtime_bindings]
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct ATBLPlayerController {
    #[inherit]
    pub base: crate::Chivalry2::Engine::APlayerController,
    _private: [u8; 0x1070],
    pub bOnlineInventoryInitialized: bool,
    pub bPlayerCustomizationReceived: bool,
}

#[unchained_macros_core::runtime_bindings]
impl ATBLPlayerController {
    // Targets ClientReceiveChat_Implementation, not the generated RPC wrapper.
    #[unchained_macros_core::bind_resolved_func(ATBLPlayerController__ClientReceiveChat_Implementation)]
    pub(crate) unsafe fn ClientReceiveChat_Implementation(
        self: *mut Self,
        sender: *mut ATBLPlayerState,
        message: *const FString,
        chat_type: u32,
        is_sender_dev: bool,
        override_color: u32,
    ) {
    }

    #[unchained_macros_core::bind_resolved_func(
        ATBLPlayerController__GetOwnershipFromPlayerControllerAndState
    )]
    pub(crate) unsafe fn GetOwnershipFromPlayerControllerAndState(
        result: *mut FOwnershipResponse,
        PlayerController: *mut ATBLPlayerController,
        PlayerState: *mut ATBLPlayerState,
        AssetIdToCheck: *mut FPrimaryAssetId,
        BaseOnly: bool,
    ) -> *mut FOwnershipResponse {
    }

    #[unchained_macros_core::bind_resolved_func(ATBLPlayerController__CanUseLoadoutItem)]
    pub(crate) unsafe fn CanUseLoadoutItem(
        self: *mut Self,
        result: *mut FOwnershipResponse,
        InLoadOutSelection: *mut TSubclassOfULoadoutSelection,
        InItem: *mut TSubclassOfAInventoryItem,
    ) -> *mut FOwnershipResponse {
    }

    #[unchained_macros_core::bind_resolved_func(ATBLPlayerController__CanUseCharacter)]
    pub(crate) unsafe fn CanUseCharacter(
        self: *mut Self,
        result: *mut FOwnershipResponse,
        CharacterSubclass: *mut TSubclassOfATBLCharacter,
    ) -> *mut FOwnershipResponse {
    }

    // The extra legacy player_state argument is not established by the native signature.
    #[unchained_macros_core::bind_resolved_func(
        ATBLPlayerController__ConditionalInitializeCustomizationOnServer
    )]
    pub(crate) unsafe fn ConditionalInitializeCustomizationOnServer(
        self: *mut Self,
        player_state: *mut c_void,
    ) -> c_void {
    }

    #[unchained_macros_core::bind_resolved_func(ATBLPlayerController__ClientReceiveLocalizedChat_Implementation)]
    pub(crate) unsafe fn ClientReceiveLocalizedChat_Implementation(
        self: *mut Self,
        message: *const FText,
        channel: u32,
    ) {
    }
}
