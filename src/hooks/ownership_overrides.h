#pragma once

#include "../state/global_state.hpp"
#include "../stubs/Chivalry2.h"
#include "../patching/patch_macros.hpp"

REGISTER_HOOK_PATCH(
    ATBLPlayerController__GetOwnershipFromPlayerControllerAndState,
    ATTACH_WHEN(g_state->GetCLIArgs().is_headless),
    FOwnershipResponse*, (FOwnershipResponse* result, void* PlayerController, void* PlayerState, void* AssetIdToCheck, bool BaseOnly)
) {
    FOwnershipResponse* response = o_ATBLPlayerController__GetOwnershipFromPlayerControllerAndState(result, PlayerController, PlayerState, AssetIdToCheck, BaseOnly);
    response->owned = true;
    response->level = 0;
    return response;
}

REGISTER_HOOK_PATCH(
    ATBLPlayerController__CanUseLoadoutItem,
    ATTACH_ALWAYS,
    FOwnershipResponse*, (ATBLPlayerController* _this, FOwnershipResponse* result, const void* InLoadOutSelection, const void* InItem)
) {
    auto response = o_ATBLPlayerController__CanUseLoadoutItem(_this, result, InLoadOutSelection, InItem);
    response->owned = true;
    response->level = 0;
    result->owned = true;
    return response;
}

REGISTER_HOOK_PATCH(
    ATBLPlayerController__CanUseCharacter,
    ATTACH_ALWAYS,
    FOwnershipResponse*, (ATBLPlayerController* _this, FOwnershipResponse* result, const void* CharacterSubclass)
) {
    auto response = o_ATBLPlayerController__CanUseCharacter(_this, result, CharacterSubclass);
    response->level = 0;
    response->owned = true;
    return response;
}

REGISTER_HOOK_PATCH(
    ATBLPlayerController__ConditionalInitializeCustomizationOnServer,
    ATTACH_WHEN(g_state->GetCLIArgs().is_headless),
    void, (ATBLPlayerController* _this, const void* player_state)
) {
    _this->bOnlineInventoryInitialized = true;
    _this->bPlayerCustomizationReceived = true;
    o_ATBLPlayerController__ConditionalInitializeCustomizationOnServer(_this, player_state);
}
