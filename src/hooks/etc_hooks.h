#pragma once

#include "../stubs/UE4.h"
#include "../patching/patch_macros.hpp"


#ifdef CPP_HOOKS_IMPL

REGISTER_HOOK_PATCH(
	GetGameInfo,
	APPLY_ALWAYS,
	FString*, (FString* ret_ptr, void* uWorld)
) {
	auto val = o_GetGameInfo(ret_ptr, uWorld);
	std::wcout << "GetGameInfo: " << *val->str << std::endl;
	return val;
}

#endif