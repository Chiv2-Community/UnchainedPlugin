#pragma once

DECL_HOOK(FString*, FViewport, (FViewport_C* this_ptr, void* viewportClient))
{
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
			printf("Build String found!%s\n\t%s\n", (curBuild.buildId == 0) ? "" : " (loaded)", curBuild.nameStr.c_str());

			if (offsetsLoaded && needsSerialization)
				serializeBuilds();
		}

#ifdef _DEBUG_CONSOLE
		//std::wcout << this_ptr->AppVersionString.str << ": " << buildNr << " " << buildId << std::endl;
#endif
	}
	return val;
}

// FRONTEND_MAP_FMT L"Frontend%ls?mods=%ls?nextmap=%ls?nextmods=%ls?defmods=%ls"
DECL_HOOK(bool, LoadFrontEndMap, (void* this_ptr, FString* param_1))
{
	static wchar_t szBuffer[512];

	static bool init = false;
	if (true) {
		auto pwdStr = CmdParseParam(L"ServerPassword", L"?Password=");

		log("Frontend Map params: ");
		wsprintfW(szBuffer, L"Frontend%ls%ls%ls", (CmdGetParam(L"-rcon") == -1) ? L"" : L"?rcon", pwdStr.c_str(), init ? L"" : L"?startup");
		logWideString(szBuffer);
		std::wstring ws(param_1->str);
		std::string nameStr = std::string(ws.begin(), ws.end());
		//printf("LoadFrontEndMap: %s %d\n", nameStr.c_str(), param_1->max_letters);
		init = true;
		return o_LoadFrontEndMap(this_ptr, new FString(szBuffer));
	}
	else
		return o_LoadFrontEndMap(this_ptr, param_1);
}

void* UWORLD = nullptr;
DECL_HOOK(ENetMode, InternalGetNetMode, (void* world))
{
	UWORLD = world;
	return o_InternalGetNetMode(world);
}

DECL_HOOK(ENetMode, UNetDriver__GetNetMode, (void* this_ptr)) {
	ENetMode mode = o_UNetDriver__GetNetMode(this_ptr);
	ENetMode result = mode == LISTEN_SERVER ? DEDICATED_SERVER : mode;
	return result;
}


bool playableListen = CmdGetParam(L"--playable-listen") != -1;
DECL_HOOK(bool, UGameplay__IsDedicatedServer, (long long param_1))
{
	if (UWORLD != nullptr && !playableListen) {
		ENetMode mode = o_InternalGetNetMode(UWORLD);
		bool isHosting = mode == DEDICATED_SERVER || mode == LISTEN_SERVER;
		return isHosting;
	}
	else return o_UGameplay__IsDedicatedServer(param_1);
}