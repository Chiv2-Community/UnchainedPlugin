#pragma once

#include "../logging/Logger.hpp"
#include "../state/global_state.hpp"
#include "../stubs/UE4.h"
#include "../hooking/hook_macros.hpp"
#include <optional>

CREATE_HOOK(
	FViewport,
	ATTACH_ALWAYS,
	FString*, (FViewport_C* this_ptr, void* viewportClient)
) {
		FString* val = o_FViewport(this_ptr, viewportClient);
		wchar_t* buildNr = wcschr(this_ptr->AppVersionString.str, L'+') + 1;
		if (buildNr != nullptr)
		{
			bool needsSerialization = false;

			uint32_t buildId = _wtoi(buildNr);
			if (g_state->GetBuildMetadata().GetBuildId() == 0)
			{
				needsSerialization = true;
				const wchar_t* build_name = this_ptr->AppVersionString.str + 7;
				const std::wstring build_name_str(build_name);
				g_state->GetBuildMetadata().SetName(build_name_str);
				g_state->GetBuildMetadata().SetBuildId(buildId);

				GLOG_INFO("Build metadata set - Name: {} BuildId: {} Hash: 0x{:X}",
						  g_state->GetBuildMetadata().GetName(),
						  g_state->GetBuildMetadata().GetBuildId(),
						  g_state->GetBuildMetadata().GetFileHash());

				SaveBuildMetadata(g_state->GetSavedBuildMetadata());
			}

			if (!g_state->GetBuildMetadata().GetName().empty())
			{
				GLOG_INFO("Build String found!{} {}", (g_state->GetBuildMetadata().GetBuildId() == 0) ? L"" : L" (loaded)", g_state->GetBuildMetadata().GetName());
			}
		}
		return val;
}
AUTO_HOOK(FViewport)

CREATE_HOOK(
	LoadFrontEndMap,
	ATTACH_ALWAYS,
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
AUTO_HOOK(LoadFrontEndMap);

CREATE_HOOK(
	InternalGetNetMode,
	ATTACH_ALWAYS,
	ENetMode, (void* world)
) {
	g_state->SetUWorld(world);
	return o_InternalGetNetMode(world);
}
AUTO_HOOK(InternalGetNetMode);

CREATE_HOOK(
	UNetDriver_GetNetMode,
	ATTACH_WHEN(g_state->GetCLIArgs().apply_desync_patch),
	ENetMode, (void* this_ptr)
) {
	const ENetMode mode = o_UNetDriver_GetNetMode(this_ptr);
	const ENetMode result = mode == LISTEN_SERVER ? DEDICATED_SERVER : mode;
	return result;
}
AUTO_HOOK(UNetDriver_GetNetMode);



CREATE_HOOK(
	UGameplay_IsDedicatedServer,
	ATTACH_ALWAYS,
	bool, (long long param_1)
) {
	if (g_state->GetUWorld() != nullptr && !g_state->GetCLIArgs().playable_listen) {
		ENetMode mode = o_InternalGetNetMode(g_state->GetUWorld());
		bool isHosting = mode == DEDICATED_SERVER || mode == LISTEN_SERVER;
		return isHosting;
	}

	return o_UGameplay_IsDedicatedServer(param_1);
}
