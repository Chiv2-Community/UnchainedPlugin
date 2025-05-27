#pragma once

#include "FunctionHook.hpp"
#include "Logging.hpp"
#include <optional>

auto FViewport_Hook = FunctionHook<FString*, FViewport_C*, void*>(
    "FViewport",
    PLATFORM_SIGNATURES(
		PLATFORM_SIGNATURE(STEAM, "48 89 5C 24 08 48 89 74 24 10 48 89 7C 24 18 41 56 48 83 EC 30 33 F6")
		PLATFORM_SIGNATURE(EGS, "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 30 33 ED C7")
	),
    [](auto o_FViewport, auto this_ptr, auto viewportClient) -> FString* {
        auto val = o_FViewport(this_ptr, viewportClient);
        wchar_t* buildNr = wcschr(this_ptr->AppVersionString.str, L'+') + 1;
        if (buildNr != nullptr)
        {
            uint32_t buildId = _wtoi(buildNr);
            if (curBuild.buildId == 0 || curBuild.nameStr.length() == 0)
            {
                needsSerialization = true;
                curBuild.SetName(this_ptr->AppVersionString.str + 7);

                curBuild.buildId = buildId;
                curBuild.fileHash = calculateCRC32("Chivalry2-Win64-Shipping.exe");
            }
            if (curBuild.nameStr.length() > 0)
            {
                LOG_INFO(g_logger, "Build String found!{}\n\t{}\n", (curBuild.buildId == 0) ? L"" : L" (loaded)", curBuild.nameStr.c_str());

                if (offsetsLoaded && needsSerialization)
                    serializeBuilds();
            }
        }
        return val;
    }
);

auto LoadFrontEndMapHook = FunctionHook<bool, void*, FString*>(
	"LoadFrontEndMap",
	UNIVERSAL_SIGNATURE("48 8B C4 48 89 50 10 48 89 48 08 55 41 55 48 8D 68 98 48 81 EC 58 01 00 00 83 7A 08 00"),
	[](auto o_LoadFrontEndMap, auto this_ptr, auto param_1) -> bool {
		static wchar_t szBuffer[512];

		static bool init = false;
		if (true) {
			auto pwdStr = CmdParseParam(L"ServerPassword", L"?Password=");

			LOG_INFO(g_logger, "Frontend Map params: ");
			wsprintfW(szBuffer, L"Frontend%ls%ls%ls", (CmdGetParam(L"-rcon") == -1) ? L"" : L"?rcon", pwdStr.c_str(), init ? L"" : L"?startup");
			LOG_INFO(g_logger, "{}", std::wstring(szBuffer));
			std::wstring ws(param_1->str);
			std::string nameStr = std::string(ws.begin(), ws.end());
			//printf("LoadFrontEndMap: %s %d\n", nameStr.c_str(), param_1->max_letters);
			init = true;
			return o_LoadFrontEndMap(this_ptr, new FString(szBuffer));
		}
		else {
			return o_LoadFrontEndMap(this_ptr, param_1);
		}
	}
);

void* UWORLD = nullptr;

void SetWorld(void* world) {
	if (world != nullptr) {
		UWORLD = world;
	}
}

auto InternalGetNetMode_Hook = FunctionHook<ENetMode, void*>( // FunctionHook<ENetMode, void*>
	"InternalGetNetMode",
	UNIVERSAL_SIGNATURE("40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9"),
	[](auto o_InternalGetNetMode, auto world) -> ENetMode {
		SetWorld(world);
		return o_InternalGetNetMode(world);
	}
);

auto UNetDriver__GetNetMode_Hook = FunctionHook<ENetMode, void*>(
	"UNetDriver::GetNetMode",
	UNIVERSAL_SIGNATURE("48 83 EC 28 48 8B 01 ?? ?? ?? ?? ?? ?? 84 C0 ?? ?? 33 C0 38 ?? ?? ?? ?? 02 0F 95 C0 FF C0 48 83 C4"),
	[](auto o_UNetDriver__GetNetMode, auto this_ptr) -> ENetMode {
		ENetMode mode = o_UNetDriver__GetNetMode(this_ptr);
		ENetMode result = mode == LISTEN_SERVER ? DEDICATED_SERVER : mode;
		return result;
	}
);



const bool playableListen = CmdGetParam(L"--playable-listen") != -1;

auto UGameplay__IsDedicatedServer_Hook = FunctionHook<bool, long long>(
	"UGameplay::IsDedicatedServer",
	UNIVERSAL_SIGNATURE("48 83 EC 28 48 85 C9 ? ? BA 01 00 00 00 ? ? ? ? ? 48 85 C0 ? ? 48 8B C8 ? ? ? ? ? 83 F8 01 0F 94 C0 48"),
	[](auto o_UGameplay__IsDedicatedServer, auto param_1) -> bool {
		if (UWORLD != nullptr && !playableListen) {
			ENetMode mode = InternalGetNetMode_Hook.get_original()(UWORLD);
			bool isHosting = mode == DEDICATED_SERVER || mode == LISTEN_SERVER;
			return isHosting;
		}
		else return o_UGameplay__IsDedicatedServer(param_1);
	}
);