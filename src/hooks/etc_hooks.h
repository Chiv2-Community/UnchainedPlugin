#pragma once

#include "../stubs/UE4.h"
#include "../patching/patch_macros.hpp"



REGISTER_HOOK_PATCH(
	GetGameInfo,
	APPLY_ALWAYS,
	FString*, (FString* ret_ptr, void* uWorld)
) {
	auto val = o_GetGameInfo(ret_ptr, uWorld);
#ifdef _DEBUG_CONSOLE
	std::wcout << "GetGameInfo: " << *val->str << std::endl;
#endif
	return val;
}