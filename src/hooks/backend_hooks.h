#pragma once

#include "../stubs/UE4.h"
#include "../stubs/Chivalry2.h"
#include "../nettools.hpp"
#include "../hooking/hook_macros.hpp"
#include "../logging/global_logger.hpp"

CREATE_HOOK(
	FString_AppendChars,
	UNIVERSAL_SIGNATURE("45 85 C0 0F 84 89 00 00 00 48 89 5C 24 18 48 89 6C 24 20 56 48 83 EC 20 48 89 7C 24 30 48 8B EA 48 63 79 08 48 8B D9 4C 89 74 24 38 45 33 F6 85 FF 49 63 F0 41 8B C6 0F 94 C0 03 C7 03 C6 89 41 08 3B 41 0C 7E 07 8B D7 E8 ?? ?? ?? ?? 85 FF 49 8B C6 48 8B CF 48 8B D5 0F 95 C0 48 2B C8 48 8B 03 48 8D 1C 36 4C 8B C3 48 8D 3C 48 48 8B CF E8 ?? ?? ?? ?? 48 8B 6C 24 48 66 44 89 34 3B 4C 8B 74 24 38 48 8B 7C 24 30 48 8B 5C 24 40 48 83 C4 20 5E C3"),
	ATTACH_ALWAYS,
	void, (FString* this_ptr, const wchar_t* Str, uint32_t Count)
) {
	return o_FString_AppendChars(this_ptr, Str, Count);
}
AUTO_HOOK(FString_AppendChars)

// Distributed bans
CREATE_HOOK(
	PreLogin,
	UNIVERSAL_SIGNATURE("4C 89 4C 24 20 48 89 54 24 10 48 89 4C 24 08 55 53 57 41 55 41 57 48 8D 6C"),
	ATTACH_WHEN(g_state->GetCLIArgs().use_backend_banlist),
	void, (ATBLGameMode* this_ptr, const FString& Options, const FString& Address, const FUniqueNetIdRepl& UniqueId, FString& ErrorMessage)
) {
	std::wstring addressString(Address.str);
	GLOG_INFO("{} is attempting to connect.", addressString);

	o_PreLogin(this_ptr, Options, Address, UniqueId, ErrorMessage);

	// An error is already present
	if (ErrorMessage.letter_count != 0)
		return;

	GLOG_INFO("Checking Unchained ban status.");

	std::wstring path = L"/api/v1/check-banned/";
	path.append(addressString);
	std::wstring apiUrl = GetServerBrowserBackendApiUrl(path.c_str());
	std::wstring result = HTTPGet(&apiUrl);

	if (result.empty()) {
		GLOG_INFO("Failed to get ban status");
		return;
	}

	bool banned = result.find(L"true") != std::wstring::npos;

	if (banned) {
		std::wstring message = L"You are banned from this server.";
		hk_FString_AppendChars(&ErrorMessage, message.c_str(), static_cast<uint32_t>(message.length()));
	}


	std::wstring suffix = banned ?
		L" is banned" : L" is not banned";

	GLOG_INFO("{}{}",addressString, suffix);
}
AUTO_HOOK(PreLogin)

SCAN_HOOK(ApproveLogin, UNIVERSAL_SIGNATURE("48 89 5C 24 18 48 89 74 24 20 55 57 41 54 41 55 41 56 48 8D 6C 24 C9 48 81 EC A0 00 00 00 8B"))

// Browser plugin

#define EGS_GET_MOTD_SIGNATURE R"(
4C 89 4C 24 ?? 4C 89 44 24 ?? 48 89 4C 24 ?? 55 56 57 41 54 48 8D 6C 24 ?? 48 81 EC D8 00 00 00 83 79 ?? 01 4C 8B E2 48 8B \
F9 7F ?? 33 F6 48 8B C2 48 89 32 48 89 72 ?? 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 ?? ?? ?? ?? 48 8D 55 ?? 4C \
89 AC 24 ?? ?? ?? ?? 4C 89 B4 24 ?? ?? ?? ?? 4C 89 BC 24 ?? ?? ?? ?? E8 ?? ?? ?? ?? 4C 8B 6D ?? 48 8D 4D ?? 33 F6 48 89 75 \
?? 48 89 75 ?? 49 8B 45 00 8D 56 ?? 48 8B 40 ?? 48 89 45 ?? E8 ?? ?? ?? ?? 8B 55 ?? 8D 5A ?? 89 5D ?? 3B 5D ?? 7E ?? 48 8D \
4D ?? E8 ?? ?? ?? ?? 8B 5D ?? 4C 8B 75 ?? 48 8D 15 ?? ?? ?? ?? 49 8B CE 41 B8 12 00 00 00
)"

CREATE_HOOK(
	GetMotd,
	UNIVERSAL_SIGNATURE(EGS_GET_MOTD_SIGNATURE),
	ATTACH_ALWAYS,
	void*,(GCGObj* this_ptr, void* a2, GetMotdRequest* request, void* a4)
) {
	GLOG_DEBUG("GetMotd Called");

	auto old_base = this_ptr->url_base;

	auto originalToken = request->token;
	auto emptyToken = FString(L"");

	try {
		this_ptr->url_base = FString(GetServerBrowserBackendApiUrl(L"/api/tbio").c_str());
		request->token = emptyToken;
		void* res = o_GetMotd(this_ptr, a2, request, a4);
		this_ptr->url_base = old_base;
		request->token = originalToken;
		GLOG_DEBUG("GetMotd returned");
		return res;
	}
	catch (...) {
		this_ptr->url_base = old_base;
		request->token = originalToken;
		throw;
	}
}
AUTO_HOOK(GetMotd)

#define EGS_GET_CURRENT_GAMES_SIGNATURE R"(
"4C 89 4C 24 ?? 4C 89 44 24 ?? 48 89 4C 24 ?? 55 56 57 41 54 48 8D 6C 24 ?? 48 81 EC D8 00 00 00 83 79 ?? 01 4C 8B E2 48 8B \
F9 7F ?? 33 F6 48 8B C2 48 89 32 48 89 72 ?? 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 ?? ?? ?? ?? 48 8D 55 ?? 4C \
89 AC 24 ?? ?? ?? ?? 4C 89 B4 24 ?? ?? ?? ?? 4C 89 BC 24 ?? ?? ?? ?? E8 ?? ?? ?? ?? 4C 8B 6D ?? 48 8D 4D ?? 33 F6 48 89 75 \
?? 48 89 75 ?? 49 8B 45 00 8D 56 ?? 48 8B 40 ?? 48 89 45 ?? E8 ?? ?? ?? ?? 8B 55 ?? 8D 5A ?? 89 5D ?? 3B 5D ?? 7E ?? 48 8D \
4D ?? E8 ?? ?? ?? ?? 8B 5D ?? 4C 8B 75 ?? 48 8D 15 ?? ?? ?? ?? 49 8B CE 41 B8 22 00 00 00 E8 ?? ?? ?? ?? 8B 4F ?? 83 F9 01 \
7F ?? 4D 8B FE 4C 8B F6 EB ?? 85 DB 74 ?? 44 8D 7B ?? 41 8B C7 EB ?? 44 8B FE 8B C6 48 8B 5D ?? 8D 14 ?? 48 8B F9 48 89 75 \
?? 45 33 C0 89 7D ?? 48 8D 4D ?? 48 8B 1B E8 ?? ?? ?? ?? 48 8B 4D ?? 4C 8D 04 ?? 48 8B D3 E8 ?? ?? ?? ?? 45 8B C7 48 8D 4D \
?? 49 8B D6 E8 ?? ?? ?? ?? 4C 8B 7D ?? 8B 5D ?? 48 89 75 ?? 48 89 75 ?? 48 89 75 ?? 8B D6 89 55 ?? 8B CE 89 4D ?? 85 DB 74 \
?? 49 8B FF 4D 85 FF 74 ?? EB ?? 48 8D 3D ?? ?? ?? ?? 66 39 37 74 ?? 48 C7 C3 FF FF FF FF 48 FF C3 66 39 34 ?? 75 ?? FF C3 \
85 DB 7E ?? 8B D3 48 8D 4D ?? E8 ?? ?? ?? ?? 8B 4D ?? 8B 55 ?? 8D 04 ?? 89 45 ?? 3B C1 7E ?? 48 8D 4D ?? E8 ?? ?? ?? ?? 48 \
8B 4D ?? 48 8B D7 4C 63 C3 4D 03 C0 E8 ?? ?? ?? ?? 48 8D 55 ?? 49 8B CD FF 55 ?? 48 8B 4D ?? 48 85 C9 74 ?? E8 ?? ?? ?? ?? \
4D 85 FF 74 ?? 49 8B CF E8 ?? ?? ?? ?? 4D 85 F6 74 ?? 49 8B CE E8 ?? ?? ?? ?? 4C 8B 75 ?? 8B CE 49 83 C6 10 89 4D ?? 45 8B \
56 ?? 49 8D 7E ?? C7 45 ?? 01 00 00 00 44 8B C6 48 89 7D ?? 41 BB 1F 00 00 00 C7 45 ?? FF FF FF FF 48 89 75 ?? 45 85 D2 74 \
?? 48 8B 47 ?? 4C 8B CF 48 85 C0 4C 0F 45 C8 41 8D 42 ?? 99 41 23 D3 8D 1C ?? 41 8B 11 C1 FB 05 85 D2 75 ?? 0F 1F 80 00 00 \
00 00 FF C1 41 83 C0 20 89 4D ?? 44 89 45 ?? 3B CB 7F ?? 48 63 C1 C7 45 ?? FF FF FF FF 41 8B 14 ?? 85 D2 74 ?? 8B C2 F7 D8 \
23 C2 0F BD C8 89 45 ?? 74 ?? 41 8B C3 2B C1 EB ?? B8 20 00 00 00 44 2B C0 41 8D 40 ?? 89 45 ?? 41 3B C2 7E ?? 44 89 55 ?? \
41 8B 56 ?? 41 BD FF FF FF FF 0F 10 55 ?? 8B CA 4C 89 75 ?? 0F 10 45 ?? 41 23 CB 44 8B C2 0F 11 55 ?? 41 D3 E5 44 8B CA 0F \
11 45 ?? 41 C1 F8 05 41 83 E1 E0 66 0F 15 D2 45 8B FA F2 0F 11 55 ?? 44 89 6D ?? 89 55 ?? 0F 10 45 ?? 0F 10 4D ?? 0F 11 45 \
?? 0F 11 4D ?? 41 3B D2 74 ?? 48 8B 47 ?? 4C 8B D7 48 85 C0 49 63 C8 4C 0F 45 D0 41 8D 47 ?? 99 41 23 D3 8D 1C ?? 41 8B 14 \
?? C1 FB 05 41 23 D5 75 ?? 41 FF C0 41 83 C1 20 44 3B C3 7F ?? 49 63 C0 C7 45 ?? FF FF FF FF 41 8B 14 ?? 85 D2 74 ?? 8B C2 \
F7 D8 23 C2 0F BD C8 74 ?? 44 2B D9 EB ?? 41 BB 20 00 00 00 45 2B CB 41 8D 41 ?? 89 45 ?? 41 3B C7 7E ?? 44 89 7D ?? 48 8B \
5D ?? 4C 8B BC 24 ?? ?? ?? ?? 4C 8B AC 24 ?? ?? ?? ?? 48 C1 EB 20 48 63 45 ?? 48 8B 55 ?? 3B C3 75 ?? 48 39 7D ?? 75 ?? 49 \
3B D6 74 ?? 48 8D 0C ?? 48 8B 02 48 8D 14 ?? 48 8B 4D ?? 4C 8D 42 ?? 48 8B 01 FF 50 ?? 8B 45 ?? 48 8D 4D ?? F7 D0 21 45 ?? \
E8 ?? ?? ?? ?? EB ?? 48 8B 4D ?? 48 8D 55 ?? E8 ?? ?? ?? ?? 48 8B 4D ?? 48 8B 01 FF 90 ?? ?? ?? ?? 4C 8B B4 24 ?? ?? ?? ?? \
4C 8D 45 ?? 48 8B D8 48 8B CE 48 8B 45 ?? 8B D6 48 89 4D ?? 89 55 ?? 49 3B C0 74 ?? 39 48 ?? 74 ?? 4C 8B 00 4D 85 C0 74 ?? \
49 8B 00 48 8D 55 ?? 49 8B C8 FF 50 ?? 8B 55 ?? 48 8B 4D ?? 48 89 75 ?? 89 75 ?? 85 D2 74 ?? 48 85 C9 74 ?? 48 8B 01 48 8D \
55 ?? FF 50 ?? 48 8B 55 ?? 4C 8D 4D ?? 4C 8D 05 ?? ?? ?? ?? 48 8D 4D ?? E8 ?? ?? ?? ?? 48 8B D0 48 8B CB E8 ?? ?? ?? ?? 39 \
75 ?? 74 ?? 48 8B 4D ?? 48 85 C9 74 ?? 48 8B 01 33 D2 FF 50 ?? 48 8B 45 ?? 48 85 C0 74 ?? 45 33 C0 33 D2 48 8B C8 E8 ?? ?? \
?? ?? 48 89 45 ?? 89 75 ?? EB ?? 48 8B 45 ?? 48 85 C0 74 ?? 48 8B C8 E8 ?? ?? ?? ?? 8B 45 ?? 48 8B 4D ?? 85 C0 74 ?? 48 85 \
C9 75 ?? 85 C0 74 ?? 48 85 C9 74 ?? 48 8B 01 33 D2 FF 50 ?? 48 8B 4D ?? 48 85 C9 74 ?? 45 33 C0 33 D2 E8 ?? ?? ?? ?? 48 8B \
C8 48 89 45 ?? 89 75 ?? 48 85 C9 74 ?? E8 ?? ?? ?? ?? 48 8B 4D ?? 48 8B 01 FF 90 ?? ?? ?? ?? 48 8B 45 ?? 49 89 04 24 48 8B \
45 ?? 49 89 44 24 ?? 48 85 C0 74 ?? FF 40 ?? 48 8B 5D ?? 48 85 DB 74 ?? 83 6B ?? 01 75 ?? 48 8B 03 48 8B CB FF 10 83 6B ?? \
01 75 ?? 48 8B 03 BA 01 00 00 00 48 8B CB FF 50 ?? 48 8B 9C 24 ?? ?? ?? ?? 49 8B C4 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3
)"

CREATE_HOOK(
	GetCurrentGames,
	UNIVERSAL_SIGNATURE(EGS_GET_CURRENT_GAMES_SIGNATURE),
	ATTACH_ALWAYS,
	void*, (GCGObj* this_ptr, void* a2, GetCurrentGamesRequest* request, void* a4)
) {
	GLOG_DEBUG("GetCurrentGames called");

	auto old_base = this_ptr->url_base;

	auto originalToken = request->token;
	auto emptyToken = FString(L"");

	try {
		this_ptr->url_base = FString(GetServerBrowserBackendApiUrl(L"/api/tbio").c_str());
		request->token = emptyToken;
		void* res = o_GetCurrentGames(this_ptr, a2, request, a4);
		this_ptr->url_base = old_base;
		request->token = originalToken;
		GLOG_DEBUG("GetCurrentGames returned");
		return res;
	}
	catch (...) {
		this_ptr->url_base = old_base;
		request->token = originalToken;
		throw;
	}
}
AUTO_HOOK(GetCurrentGames)

CREATE_HOOK(
	SendRequest,
	UNIVERSAL_SIGNATURE("48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"),
	ATTACH_ALWAYS,
	void*, (GCGObj* this_ptr, FString* fullUrlInputPtr, FString* bodyContentPtr, FString* authKeyHeaderPtr, FString* authKeyValuePtr)
) {
	if (fullUrlInputPtr->letter_count > 0 &&
		wcscmp(L"https://EBF8D.playfabapi.com/Client/Matchmake?sdk=Chiv2_Version", fullUrlInputPtr->str) == 0)
	{
		FString original = *fullUrlInputPtr; //save original string and buffer information
		*fullUrlInputPtr = FString(GetServerBrowserBackendApiUrl(L"/api/playfab/Client/Matchmake").c_str()); //overwrite with new string
		GLOG_DEBUG("hk_SendRequest Client/Matchmake");

		auto empty = FString(L""); // Send empty string for auth, so that our backend isn't getting user tokens.
		try {
			auto res = o_SendRequest(this_ptr, fullUrlInputPtr, bodyContentPtr, authKeyHeaderPtr, &empty); //run the request as normal with new string
			*fullUrlInputPtr = original; //set everything back to normal and pretend nothing happened
			return res;
		}
		catch (...) {
			*fullUrlInputPtr = original; //set everything back to normal and pretend nothing happened
			throw;
		}
		;
	}
	return o_SendRequest(this_ptr, fullUrlInputPtr, bodyContentPtr, authKeyHeaderPtr, authKeyValuePtr);
}
AUTO_HOOK(SendRequest)