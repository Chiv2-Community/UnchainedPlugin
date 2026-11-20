//! Permit customization and loadouts on custom servers.

use crate::chivalry2::engine::{
    FPrimaryAssetId, TSubclassOfAInventoryItem, TSubclassOfATBLCharacter,
    TSubclassOfULoadoutSelection,
};
use crate::chivalry2::tbl::{ATBLPlayerController, ATBLPlayerState, FOwnershipResponse};
use std::os::raw::c_void;

#[unchained_macros::bind_func_detour(
    ATBLPlayerController::GetOwnershipFromPlayerControllerAndState
)]
pub(crate) unsafe fn get_ownership_from_player_controller_and_state_detour(
    result: *mut FOwnershipResponse,
    player_controller: *mut ATBLPlayerController,
    player_state: *mut ATBLPlayerState,
    asset_id_to_check: *mut FPrimaryAssetId,
    base_only: bool,
) -> *mut FOwnershipResponse {
    // Caller-provided result storage precedes the controller in the native ABI.
    let response = unsafe {
        ATBLPlayerController::GetOwnershipFromPlayerControllerAndState(
            result,
            player_controller,
            player_state,
            asset_id_to_check,
            base_only,
        )
    };
    if let Some(resp) = unsafe { response.as_mut() } {
        resp.level = 0;
        resp.owned = true;
    }
    response
}

#[unchained_macros::bind_func_detour(ATBLPlayerController::CanUseLoadoutItem)]
pub(crate) unsafe fn can_use_loadout_item_detour(
    this: *mut ATBLPlayerController,
    result: *mut FOwnershipResponse,
    in_loadout_selection: *mut TSubclassOfULoadoutSelection,
    in_item: *mut TSubclassOfAInventoryItem,
) -> *mut FOwnershipResponse {
    let response = unsafe { this.CanUseLoadoutItem(result, in_loadout_selection, in_item) };
    if let Some(resp) = unsafe { response.as_mut() } {
        resp.level = 0;
        resp.owned = true;
    }
    if let Some(res) = unsafe { result.as_mut() } {
        res.owned = true;
    }
    response
}

#[unchained_macros::bind_func_detour(ATBLPlayerController::CanUseCharacter)]
pub(crate) unsafe fn can_use_character_detour(
    this: *mut ATBLPlayerController,
    result: *mut FOwnershipResponse,
    character_subclass: *mut TSubclassOfATBLCharacter,
) -> *mut FOwnershipResponse {
    let response = unsafe { this.CanUseCharacter(result, character_subclass) };
    if let Some(resp) = unsafe { response.as_mut() } {
        resp.level = 0;
        resp.owned = true;
    }
    response
}

#[unchained_macros::bind_func_detour(
    ATBLPlayerController::ConditionalInitializeCustomizationOnServer
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
    unsafe { this.ConditionalInitializeCustomizationOnServer(player_state) }
}
