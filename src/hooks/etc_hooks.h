#pragma once

#include "../stubs/UE4.h"
#include "../hooking/hook_macros.hpp"



CREATE_HOOK(GetGameInfo,
	ATTACH_ALWAYS,
	FString*, (FString* ret_ptr, void* uWorld)
) {
	auto val = o_GetGameInfo(ret_ptr, uWorld);
#ifdef _DEBUG_CONSOLE
	std::wcout << "GetGameInfo: " << *val->str << std::endl;
#endif
	return val;
}