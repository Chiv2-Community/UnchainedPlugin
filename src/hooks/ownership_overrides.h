#pragma once

#include "../state/global_state.hpp"
#include "../stubs/Chivalry2.h"
#include "../hooking/patch_macros.hpp"

REGISTER_HOOK_PATCH(
    ATBLPlayerController__GetOwnershipFromPlayerControllerAndState,
    PLATFORM_SIGNATURES(
        PLATFORM_SIGNATURE(EGS, "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD")
        PLATFORM_SIGNATURE(STEAM, "40 55 56 41 54 41 55 41 56 41 57 48 8D AC 24 B8")
    ),
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
    PLATFORM_SIGNATURES(
        PLATFORM_SIGNATURE(EGS, "48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00")
        PLATFORM_SIGNATURE(STEAM, "48 89 5C 24 08 48 89 74 24 18 55 57 41 55 41 56 41 57 48 8B EC 48 83 EC")
    ),
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
    UNIVERSAL_SIGNATURE("48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 50 49 8B 18"),
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
    PLATFORM_SIGNATURES(
        PLATFORM_SIGNATURE(EGS, "48 89 54 24 10 53 56 57 41 54 48 83 EC 78 48 8B 99 60 02 00 00 48 8B F2 0F B6")
        PLATFORM_SIGNATURE(STEAM, "48 89 54 24 10 53 55 57 41 54 48 83 EC 78")
    ),
    ATTACH_WHEN(g_state->GetCLIArgs().is_headless),
    void, (ATBLPlayerController* _this, const void* player_state)
) {
    _this->bOnlineInventoryInitialized = true;
    _this->bPlayerCustomizationReceived = true;
    o_ATBLPlayerController__ConditionalInitializeCustomizationOnServer(_this, player_state);
}
