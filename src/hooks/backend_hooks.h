#pragma once

#include "../stubs/UE4.h"
#include "../stubs/Chivalry2.h"
#include "../nettools.hpp"
#include "../patching/patch_macros.hpp"
#include "../logging/global_logger.hpp"

#ifdef CPP_HOOKS_IMPL
REGISTER_HOOK_PATCH(
	FString_AppendChars,
	APPLY_ALWAYS,
	void, (FString* this_ptr, const wchar_t* Str, uint32_t Count)
) {
	return o_FString_AppendChars(this_ptr, Str, Count);
}
#endif

#ifdef CPP_HOOKS_IMPL
// Distributed bans
REGISTER_HOOK_PATCH(
	PreLogin,
	APPLY_WHEN(g_state->GetCLIArgs().use_backend_banlist),
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
#endif

#ifdef CPP_HOOKS_IMPL
REGISTER_HOOK_PATCH(
	GetMotd,
	APPLY_ALWAYS,
	void*,(GCGObj* this_ptr, void* a2, GetMotdRequest* request, void* a4)
) {
	GLOG_DEBUG("GetMotd Called");

	auto old_base = this_ptr->url_base;

	auto originalToken = request->token;
	auto emptyToken = FString(L"");

	try {
		auto url = GetServerBrowserBackendApiUrl(L"/api/tbio");
		this_ptr->url_base = FString(url.c_str());
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

REGISTER_HOOK_PATCH(
	GetCurrentGames,
	APPLY_ALWAYS,
	void*, (GCGObj* this_ptr, void* a2, GetCurrentGamesRequest* request, void* a4)
) {
	GLOG_DEBUG("GetCurrentGames called");

	auto old_base = this_ptr->url_base;

	auto originalToken = request->token;
	auto emptyToken = FString(L"");

	try {
		auto url = GetServerBrowserBackendApiUrl(L"/api/tbio");
		this_ptr->url_base = FString(url.c_str());
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
#endif

#ifdef CPP_HOOKS_IMPL
REGISTER_HOOK_PATCH(
	SendRequest,
	APPLY_ALWAYS,
	void*, (GCGObj* this_ptr, FString* fullUrlInputPtr, FString* bodyContentPtr, FString* authKeyHeaderPtr, FString* authKeyValuePtr)
) {
	if (fullUrlInputPtr->letter_count > 0 &&
		wcscmp(L"https://EBF8D.playfabapi.com/Client/Matchmake?sdk=Chiv2_Version", fullUrlInputPtr->str) == 0)
	{
		FString original = *fullUrlInputPtr; //save original string and buffer information
		auto url = GetServerBrowserBackendApiUrl(L"/api/playfab/Client/Matchmake");
		*fullUrlInputPtr = FString(url.c_str()); //overwrite with new string
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
#endif