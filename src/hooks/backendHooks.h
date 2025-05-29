#pragma once

DECL_HOOK(void, FString_AppendChars, (FString* this_ptr, const wchar_t* Str, int Count)) {
	o_FString_AppendChars(this_ptr, Str, Count);
}

// Distributed bans
DECL_HOOK(void, PreLogin, (ATBLGameMode* this_ptr, const FString& Options, const FString& Address, const FUniqueNetIdRepl& UniqueId, FString& ErrorMessage)) {
	std::wstring addressString(Address.str);
	GLOG_INFO("{} is attempting to connect.", addressString);

	o_PreLogin(this_ptr, Options, Address, UniqueId, ErrorMessage);

	// An error is already present
	if (ErrorMessage.letter_count != 0)
		return;

	GLOG_INFO("Checking Unchained ban status.");

	std::wstring path = L"/api/v1/check-banned/";
	path.append(addressString);
	std::wstring apiUrl = GetApiUrl(path.c_str());
	std::wstring result = HTTPGet(&apiUrl);

	if (result.empty()) {
		GLOG_INFO("Failed to get ban status");
		return;
	}

	bool banned = result.find(L"true") != std::wstring::npos;

	if (banned) {
		std::wstring message = L"You are banned from this server.";
		hk_FString_AppendChars(&ErrorMessage, message.c_str(), message.length());
	}


	std::wstring suffix = banned ?
		L" is banned" : L" is not banned";

	GLOG_INFO("{}{}",addressString, suffix);
}

// Browser plugin
DECL_HOOK(void*, GetMotd, (GCGObj* this_ptr, void* a2, GetMotdRequest* request, void* a4)) {
	GLOG_DEBUG("GetMotd Called");

	auto old_base = this_ptr->url_base;

	auto originalToken = request->token;
	auto emptyToken = FString(L"");

	try {
		this_ptr->url_base = FString(GetApiUrl(L"/api/tbio").c_str());
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

DECL_HOOK(void*, GetCurrentGames, (GCGObj* this_ptr, void* a2, GetCurrentGamesRequest* request, void* a4)) {
	GLOG_DEBUG("GetCurrentGames called");

	auto old_base = this_ptr->url_base;

	auto originalToken = request->token;
	auto emptyToken = FString(L"");

	try {
		this_ptr->url_base = FString(GetApiUrl(L"/api/tbio").c_str());
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

DECL_HOOK(void*, SendRequest, (GCGObj* this_ptr, FString* fullUrlInputPtr, FString* bodyContentPtr, FString* authKeyHeaderPtr, FString* authKeyValuePtr)) {
	if (fullUrlInputPtr->letter_count > 0 &&
		wcscmp(L"https://EBF8D.playfabapi.com/Client/Matchmake?sdk=Chiv2_Version", fullUrlInputPtr->str) == 0)
	{
		FString original = *fullUrlInputPtr; //save original string and buffer information
		*fullUrlInputPtr = FString(GetApiUrl(L"/api/playfab/Client/Matchmake").c_str()); //overwrite with new string
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