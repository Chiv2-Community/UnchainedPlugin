//! Permit customization and loadouts on custom servers.

use crate::chivalry2::engine::{
    FPrimaryAssetId, TSubclassOfAInventoryItem, TSubclassOfATBLCharacter,
    TSubclassOfULoadoutSelection,
};
use crate::chivalry2::tbl::{ATBLPlayerController, ATBLPlayerState, FOwnershipResponse};
use std::os::raw::c_void;

#[unchained_macros::bind_func_detour(
    ATBLPlayerController::get_ownership_from_player_controller_and_state
)]
pub(crate) unsafe fn get_ownership_from_player_controller_and_state_detour(
    result: *mut FOwnershipResponse,
    PlayerController: *mut ATBLPlayerController,
    PlayerState: *mut ATBLPlayerState,
    AssetIdToCheck: *mut FPrimaryAssetId,
    BaseOnly: bool,
) -> *mut FOwnershipResponse {
    // Caller-provided result storage precedes the controller in the native ABI.
    let response = unsafe {
        ATBLPlayerController::get_ownership_from_player_controller_and_state(
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

#[unchained_macros::bind_func_detour(ATBLPlayerController::can_use_loadout_item)]
pub(crate) unsafe fn can_use_loadout_item_detour(
    this: *mut ATBLPlayerController,
    result: *mut FOwnershipResponse,
    InLoadOutSelection: *mut TSubclassOfULoadoutSelection,
    InItem: *mut TSubclassOfAInventoryItem,
) -> *mut FOwnershipResponse {
    let response = unsafe { this.can_use_loadout_item(result, InLoadOutSelection, InItem) };
    if let Some(resp) = unsafe { response.as_mut() } {
        resp.level = 0;
        resp.owned = true;
    }
    if let Some(res) = unsafe { result.as_mut() } {
        res.owned = true;
    }
    response
}

#[unchained_macros::bind_func_detour(ATBLPlayerController::can_use_character)]
pub(crate) unsafe fn can_use_character_detour(
    this: *mut ATBLPlayerController,
    result: *mut FOwnershipResponse,
    CharacterSubclass: *mut TSubclassOfATBLCharacter,
) -> *mut FOwnershipResponse {
    let response = unsafe { this.can_use_character(result, CharacterSubclass) };
    if let Some(resp) = unsafe { response.as_mut() } {
        resp.level = 0;
        resp.owned = true;
    }
    response
}

#[unchained_macros::bind_func_detour(
    ATBLPlayerController::conditional_initialize_customization_on_server
)]
pub(crate) unsafe fn conditional_initialize_customization_on_server_detour(
    this: *mut ATBLPlayerController,
    player_state: *mut c_void,
) -> c_void {
    let this = this;
    if let Some(pc) = unsafe { this.as_mut() } {
        pc.bOnlineInventoryInitialized = true;
        pc.bPlayerCustomizationReceived = true;
    }
    unsafe { this.conditional_initialize_customization_on_server(player_state) }
}
