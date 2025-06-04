#pragma once

#include "../stubs/UE4.h"
#include "../hooking/patch_macros.hpp"



REGISTER_HOOK_PATCH(
	GetGameInfo,
	UNIVERSAL_SIGNATURE("48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00"),
	ATTACH_ALWAYS,
	FString*, (FString* ret_ptr, void* uWorld)
) {
	auto val = o_GetGameInfo(ret_ptr, uWorld);
#ifdef _DEBUG_CONSOLE
	std::wcout << "GetGameInfo: " << *val->str << std::endl;
#endif
	return val;
}