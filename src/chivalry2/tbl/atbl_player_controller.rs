//! ArgonSDK main: [ATBLPlayerController](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLPlayerController.h).

// Methods to enable customization / loadout on custom servers

use crate::game::fownership_response::FOwnershipResponse;

define_pattern_resolver!(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState, {
    EGS: ["40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD"], // EGS
    STEAM: [
        "40 55 56 41 54 41 55 41 56 41 57 48 8D AC 24 B8",
        "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD" // EGS 2.11.4
        ], // STEAM
    OTHER: ["40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 38 fd"]// PDB
});

#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState)]
    pub(crate) unsafe fn get_ownership_from_player_controller_and_state_detour(
        result: *mut FOwnershipResponse,
        PlayerController: *mut ATBLPlayerController,
        PlayerState: *mut ATBLPlayerState,
        AssetIdToCheck: *mut FPrimaryAssetId,
        BaseOnly: bool,
    ) -> *mut FOwnershipResponse {
        // Caller-provided result storage precedes the controller in the native ABI.
        let response = unsafe {
            Self::get_ownership_from_player_controller_and_state(
                result,
                PlayerController,
                PlayerState,
                AssetIdToCheck,
                BaseOnly,
            )
        };
        if let Some(resp) = unsafe { response.as_mut() } {
            resp.level = 0;
            resp.owned = true;
        }
        response
    }
}

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

#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ATBLPlayerController__CanUseLoadoutItem)]
    pub(crate) unsafe fn can_use_loadout_item_detour(
        self: *mut Self,
        result: *mut FOwnershipResponse,
        InLoadOutSelection: *mut TSubclassOfULoadoutSelection,
        InItem: *mut TSubclassOfAInventoryItem,
    ) -> *mut FOwnershipResponse {
        let response = unsafe { self.can_use_loadout_item(result, InLoadOutSelection, InItem) };
        if let Some(resp) = unsafe { response.as_mut() } {
            resp.level = 0;
            resp.owned = true;
        }
        if let Some(res) = unsafe { result.as_mut() } {
            res.owned = true;
        }
        response
    }
}

define_pattern_resolver!(
    ATBLPlayerController__CanUseCharacter,
    [
        "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 50 49 8B 18", // universal
    ]
);

#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ATBLPlayerController__CanUseCharacter)]
    pub(crate) unsafe fn can_use_character_detour(
        self: *mut Self,
        result: *mut FOwnershipResponse,
        CharacterSubclass: *mut TSubclassOfATBLCharacter,
    ) -> *mut FOwnershipResponse {
        let response = unsafe { self.can_use_character(result, CharacterSubclass) };
        if let Some(resp) = unsafe { response.as_mut() } {
            resp.level = 0;
            resp.owned = true;
        }
        response
    }
}

define_pattern_resolver!(ATBLPlayerController__ConditionalInitializeCustomizationOnServer, {
    EGS: ["48 89 54 24 10 53 56 57 41 54 48 83 EC 78 48 8B 99 60 02 00 00 48 8B F2 0F B6"], // EGS
    STEAM: ["48 89 54 24 10 53 ?? 57 41 54 48 83 EC 78"], // STEAM
    // From Sigga
    // Did the function change?
    OTHER: ["41 54 48 81 EC 80 00 00 00 80 B9 F8 00 00 00 03 4C 8B E1 ?? ?? ?? ?? ?? ?? 80 B9 20 13 00 00 00 ?? ?? ?? ?? ?? ?? 80 B9 21"], // PDB
});

use std::os::raw::c_void;

use crate::{
    game::chivalry2::ATBLPlayerState,
    ue::{
        FPrimaryAssetId, TSubclassOfAInventoryItem, TSubclassOfATBLCharacter,
        TSubclassOfULoadoutSelection,
    },
};
mod layout;
mod broadcast_display;
pub use layout::*;
// The extra legacy player_state argument is not established by the native signature.
#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ATBLPlayerController__ConditionalInitializeCustomizationOnServer)]
    pub(crate) unsafe fn conditional_initialize_customization_on_server_detour(
        self: *mut Self,
        player_state: *mut c_void,
    ) -> c_void {
        let this = self;
        if let Some(pc) = unsafe { this.as_mut() } {
            pc.bOnlineInventoryInitialized = true;
            pc.bPlayerCustomizationReceived = true;
        }
        unsafe { self.conditional_initialize_customization_on_server(player_state) }
    }
}
