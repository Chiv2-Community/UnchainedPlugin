#pragma once

#include "../logging/Logger.hpp"
#include "../state/global_state.hpp"
#include "../stubs/UE4.h"
#include "../patching/patch_macros.hpp"
#include <optional>

REGISTER_HOOK_PATCH(
	LoadFrontEndMap,
	APPLY_ALWAYS,
	bool, (void* this_ptr, FString* param_1)
) {
	static wchar_t szBuffer[512];
	static bool init = false;
	if (true) {
		auto pwd_str = g_state->GetCLIArgs().server_password.has_value()
			? L"?Password=" + g_state->GetCLIArgs().server_password.value()
			: L"";

		wsprintfW(szBuffer, L"Frontend%ls%ls%ls",
			(g_state->GetCLIArgs().rcon_port.has_value()) ? L"?rcon" : L"",
			pwd_str.c_str(),
			init ? L"" : L"?startup");

		GLOG_INFO("{}", szBuffer);
		std::wstring ws(param_1->str);
		std::string nameStr = std::convert_wstring_to_string(ws.c_str(), ws.size());
		//printf("LoadFrontEndMap: %s %d\n", nameStr.c_str(), param_1->max_letters);
		init = true;
		return o_LoadFrontEndMap(this_ptr, new FString(szBuffer));
	}
	else
		return o_LoadFrontEndMap(this_ptr, param_1);
}

REGISTER_HOOK_PATCH(
	InternalGetNetMode,
	APPLY_ALWAYS,
	ENetMode, (void* world)
) {
	g_state->SetUWorld(world);
	return o_InternalGetNetMode(world);
}

REGISTER_HOOK_PATCH(
	UNetDriver_GetNetMode,
	APPLY_WHEN(g_state->GetCLIArgs().apply_desync_patch),
	ENetMode, (void* this_ptr)
) {
	const ENetMode mode = o_UNetDriver_GetNetMode(this_ptr);
	const ENetMode result = mode == LISTEN_SERVER ? DEDICATED_SERVER : mode;
	return result;
}

REGISTER_HOOK_PATCH(
	UGameplay_IsDedicatedServer,
	APPLY_ALWAYS,
	bool, (long long param_1)
) {
	if (g_state->GetUWorld() != nullptr && !g_state->GetCLIArgs().playable_listen) {
		ENetMode mode = o_InternalGetNetMode(g_state->GetUWorld());
		bool isHosting = mode == DEDICATED_SERVER || mode == LISTEN_SERVER;
		return isHosting;
	}

	return o_UGameplay_IsDedicatedServer(param_1);
}
