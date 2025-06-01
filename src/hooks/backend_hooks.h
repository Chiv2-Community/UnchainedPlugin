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

CREATE_HOOK(
	GetCurrentGames,
	UNIVERSAL_SIGNATURE("E8 ?? ?? ?? ?? 4C 39 38 74 34"),
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