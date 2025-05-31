#pragma once

#include <Sig.hpp>
#include "../logging/Logger.hpp"
// TODO: this include gives HMODULE and stuff, for when this file eventually gets
// deleted and its contents moved elsewhere
//#include <windows.h>
#include "sigs.h"
#include "../logging/global_logger.hpp"

// Helper functions

long long FindSignature(HMODULE baseAddr, DWORD size, const char* title, const char* signature)
{
    const void* found = nullptr;
    found = Sig::find(baseAddr, size, signature);
    long long diff = 0;
    if (found != nullptr)
    {
        diff = (long long)found - (long long)baseAddr;
#ifdef _DEBUG_CONSOLE
        GLOG_INFO("?? -> {} : 0x{:X}", title, diff);
#endif
    }
#ifdef _DEBUG_CONSOLE
    else
        GLOG_WARNING("!! -> {} : nullptr", title);
#endif

    return diff;
}

HMODULE baseAddr;
MODULEINFO moduleInfo;



#define DECL_HOOK(retType, funcType, args) \
typedef retType (*funcType##_t) args;	   \
funcType##_t o_##funcType;				   \
retType hk_##funcType args

#define HOOK_ATTACH(moduleBase, funcType) \
auto offset_##funcType = g_state->GetBuildMetadata().GetOffset(strFunc[F_##funcType]); \
if (offset_##funcType.has_value()) { \
    auto offset = offset_##funcType.value(); \
    MH_CreateHook(moduleBase + offset, hk_##funcType, reinterpret_cast<void**>(&o_##funcType)); \
    MH_EnableHook(moduleBase + offset); \
}