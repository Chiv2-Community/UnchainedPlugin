//! Native source: TBL/Source/TBL/TBLPlayerController.h (shipping PDB).

use crate::chivalry2::engine::FPrimaryAssetId;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::FText;
use crate::chivalry2::engine::TSubclassOfAInventoryItem;
use crate::chivalry2::engine::TSubclassOfATBLCharacter;
use crate::chivalry2::engine::TSubclassOfULoadoutSelection;
use crate::chivalry2::tbl::ATBLPlayerState;
use std::os::raw::c_void;

#[repr(C)]
#[derive(Debug)]
#[allow(non_snake_case)]
pub struct FOwnershipResponse {
    pub(crate) owned: bool,
    crowns: i32,
    gold: i32,
    usdCents: i32,
    levelType: u8,
    pub(crate) level: i32,
}

// Shipping PDB: ATBLPlayerController::ClientReceiveChat_Implementation, RVA 0x1861f00.
define_pattern_resolver!(
    ClientReceiveChat,
    ["40 53 55 57 41 57 48 81 EC B8 00 00 00 41 8B E9 4D 8B F8 48 8B DA 48"]
);

// Methods to enable customization / loadout on custom servers

define_pattern_resolver!(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState, {
    EGS: ["40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD"], // EGS
    STEAM: [
        "40 55 56 41 54 41 55 41 56 41 57 48 8D AC 24 B8",
        "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD" // EGS 2.11.4
        ], // STEAM
    OTHER: ["40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 38 fd"]// PDB
});

define_pattern_resolver!(ATBLPlayerController__CanUseLoadoutItem, {
    EGS: ["48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00"], // EGS
    // "48 89 5C 24 08 48 89 74 24 18 55 57 41 55 41 56 41 57 48 8B EC 48 83 EC", // STEAM
    // from sigga
    // "48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00", // EGS
    STEAM: [
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
    EGS: ["48 89 54 24 10 53 56 57 41 54 48 83 EC 78 48 8B 99 60 02 00 00 48 8B F2 0F B6"], // EGS
    STEAM: ["48 89 54 24 10 53 ?? 57 41 54 48 83 EC 78"], // STEAM
    // From Sigga
    // Did the function change?
    OTHER: ["41 54 48 81 EC 80 00 00 00 80 B9 F8 00 00 00 03 4C 8B E1 ?? ?? ?? ?? ?? ?? 80 B9 20 13 00 00 00 ?? ?? ?? ?? ?? ?? 80 B9 21"], // PDB
});

define_pattern_resolver!(ClientReceiveLocalizedChat, [
    "48 89 5C 24 18 55 57 41 56 48 81 EC 90 00 00 00 48 8B 99 D0 02 00 00 41 8B E8 4C 8B F2 48 8B F9"
]);

unsafe impl crate::chivalry2::engine::UObjectType for ATBLPlayerController {
    const CLASS_NAME: &'static str = "TBLPlayerController";
}

#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct ATBLPlayerController {
    #[inherit]
    pub base: crate::chivalry2::engine::APlayerController,
    _private: [u8; 0x1070],
    pub bOnlineInventoryInitialized: bool,
    pub bPlayerCustomizationReceived: bool,
}

#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    // Targets ClientReceiveChat_Implementation, not the generated RPC wrapper.
    #[unchained_macros::bind_resolved_func(ClientReceiveChat)]
    pub(crate) unsafe fn ClientReceiveChat_Implementation(
        self: *mut Self,
        sender: *mut ATBLPlayerState,
        message: *const FString,
        chat_type: u32,
        is_sender_dev: bool,
        override_color: u32,
    ) {
    }

    #[unchained_macros::bind_resolved_func(
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

    #[unchained_macros::bind_resolved_func(ATBLPlayerController__CanUseLoadoutItem)]
    pub(crate) unsafe fn CanUseLoadoutItem(
        self: *mut Self,
        result: *mut FOwnershipResponse,
        InLoadOutSelection: *mut TSubclassOfULoadoutSelection,
        InItem: *mut TSubclassOfAInventoryItem,
    ) -> *mut FOwnershipResponse {
    }

    #[unchained_macros::bind_resolved_func(ATBLPlayerController__CanUseCharacter)]
    pub(crate) unsafe fn CanUseCharacter(
        self: *mut Self,
        result: *mut FOwnershipResponse,
        CharacterSubclass: *mut TSubclassOfATBLCharacter,
    ) -> *mut FOwnershipResponse {
    }

    // The extra legacy player_state argument is not established by the native signature.
    #[unchained_macros::bind_resolved_func(
        ATBLPlayerController__ConditionalInitializeCustomizationOnServer
    )]
    pub(crate) unsafe fn ConditionalInitializeCustomizationOnServer(
        self: *mut Self,
        player_state: *mut c_void,
    ) -> c_void {
    }

    #[unchained_macros::bind_resolved_func(ClientReceiveLocalizedChat)]
    pub(crate) unsafe fn ClientReceiveLocalizedChat_Implementation(
        self: *mut Self,
        message: *const FText,
        channel: u32,
    ) {
    }
}
