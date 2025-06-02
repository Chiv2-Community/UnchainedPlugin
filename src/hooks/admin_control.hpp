#pragma once

#include <regex>
#include "../logging/Logger.hpp"
#include "../hooking/hook_macros.hpp"
#include "../state/global_state.hpp"
#include "../stubs/UE4.h"

SCAN_HOOK(UTBLLocalPlayer_Exec, PLATFORM_SIGNATURES(
	PLATFORM_SIGNATURE(EGS, "75 18 ?? ?? ?? ?? 75 12 4d 85 f6 74 0d 41 38 be ?? ?? ?? ?? 74 04 32 db eb 9b 48 8b 5d 7f 49 8b d5 4c 8b 45 77 4c 8b cb 49 8b cf")
	PLATFORM_SIGNATURE(STEAM, "4C 89 4C 24 20 4C 89 44 24 18 48 89 54 24 10 55 53 56 57 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 68")
))

// Commenting this out because we don't really need it, and the functions have different inputs on steam and EGS.
// I do not feel like dealing with it right now.
/*
CREATE_HOOK(
	ConsoleCommand,
	UNIVERSAL_SIGNATURE(
		"40 53 48 83 EC 20 48 8B 89 D0 02 00 00 48 8B DA 48 85 C9 74 0E E8 ?? ?? ?? ?? 48 8B C3 48 83 C4 20 5B C3 33 \
		    C0 48 89 02 48 89 42 08 48 8B C3 48 83 C4 20 5B C3"),
	ATTACH_ALWAYS,
	FString, (void* this_ptr, FString const& str, bool b)
) {
#ifdef _DEBUG_
	static void* cached_this;
	if (this_ptr == NULL) {
		this_ptr = cached_this;
	}
	else {
		if (cached_this != this_ptr) {
			cached_this = this_ptr;
			//std::cout << "0x" << std::hex << this_ptr << std::endl;
		}
	}

	GLOG_DEBUG("[RCON]: PlayerController Exec called with: {}", str);

	const wchar_t* interceptPrefix = L"RCON_INTERCEPT";
	//if the command starts with the intercept prefix
	//TODO: clean up mutex stuff here. Way too sloppy to be final
	if (wcslen(str.str) >= 14 && memcmp(str.str, interceptPrefix, lstrlenW(interceptPrefix) * sizeof(wchar_t)) == 0) {
		GLOG_DEBUG("[RCON]: Intercept command detected");
	}
#endif
	return o_ConsoleCommand(this_ptr, str, b);
}
AUTO_HOOK(ConsoleCommand);
*/

CREATE_HOOK(
	ExecuteConsoleCommand,
	UNIVERSAL_SIGNATURE("40 53 48 83 EC 30 48 8B 05 ? ? ? ? 48 8B D9 48 8B 90 58 0C 00 00"),
	ATTACH_ALWAYS,
	void, (FString* param)
) {
	GLOG_INFO("EXECUTECONSOLECMD: {}", std::wstring(param->str));
	o_ExecuteConsoleCommand(param);
}
AUTO_HOOK(ExecuteConsoleCommand);

//FText* __cdecl FText::AsCultureInvariant(FText* __return_storage_ptr__, FString* param_1)
CREATE_HOOK(
	FText_AsCultureInvariant,
	PLATFORM_SIGNATURES(
		PLATFORM_SIGNATURE(EGS, "48 89 5C 24 18 48 89 74 24 20 41 56 48 83 EC 60 33 C0 48 89 7C 24 78 48 63")
		PLATFORM_SIGNATURE(STEAM, "40 53 55 57 48 83 EC 50 83 7A 08 01 48 8B F9 4C 89 B4 24 80 00 00 00 C7 44 24 70 00 00 00 00 7F 33 E8 ?? ?? ?? ?? 48 8B 58 08 48 8B 08 48 89 4C 24 20 48 89 5C 24 28 48 85 DB 74 04 F0 FF 43 08 8B 40 10 41 BE 01 00 00 00 89 44 24 30 48 8D 44 24 20 EB 18 48 8D 4C 24 38 E8 ?? ?? ?? ?? 48 8B 5C 24 28 41 BE 02 00 00 00 48 8B 08 48 89 0F 48 8B 48 08 48 89 4F 08 48 85 C9 74 04 F0 FF 41 08 8B 40 10 BD FF FF FF FF 89 47 10 41 F6 C6 02 74 46 48 89 74 24 78 41 83 E6 FD 48 8B 74 24 40 48 85 F6 74 2E 8B C5 F0 0F C1 46 08 83 F8 01 75 22 48 8B 06 48 8B CE FF 10 8B C5 F0 0F C1 46 0C 83 F8 01 75 0E 48 8B 06 BA 01 00 00 00 48 8B CE FF 50 ?? 48 8B 74 24 78 41 F6 C6 01 4C 8B B4 24 80 00 00 00 74 2E 48 85 DB 74 29 8B C5 F0 0F C1 43 08 83 F8 01 75 1D 48 8B 03 48 8B CB FF 10 F0 0F C1 6B 0C 83 FD 01 75 0B 48 8B 03 8B D5 48 8B CB FF 50 ?? 83 4F 10 02")
	),
	ATTACH_ALWAYS,
	void*, (void* ret_ptr, FString* input)
) {
	// This is extremely loud in the console
	//if (input->str != NULL) {
	//	printf("FText_AsCultureInvariant: ");
	//	wprintf(input->str);
	//  printf("\n");
	//}
	return o_FText_AsCultureInvariant(ret_ptr, input);
}
AUTO_HOOK(FText_AsCultureInvariant);

//void __thiscall ATBLGameMode::BroadcastLocalizedChat(ATBLGameMode *this,FText *param_1,Type param_2)
CREATE_HOOK(
	BroadcastLocalizedChat,
	UNIVERSAL_SIGNATURE("48 89 74 24 10 57 48 83 EC 30 48 8B 01 41 8B F8 48 8B F2 ?? ?? ?? ?? ?? ?? 48 8B C8 48 8D"),
	ATTACH_ALWAYS,
	void, (void* game_mode, FText* text, uint8_t chat_type)
) {
	GLOG_DEBUG("BroadcastLocalizedChat");
	return o_BroadcastLocalizedChat(game_mode, text, chat_type);
}
AUTO_HOOK(BroadcastLocalizedChat);

bool extractPlayerCommand(const wchar_t* input, std::wstring& playerName, std::wstring& command) {
	// Define the regular expression pattern
	std::wregex pattern(L"(.+) <0>: /cmd (.+)");

	// Convert the input to a wstring
	std::wstring inputString(input);

	// Define a wsmatch object to store the matched groups
	std::wsmatch matches;

	// Try to match the pattern in the input string
	if (std::regex_search(inputString, matches, pattern)) {
		if (matches.size() == 3) {
			playerName = matches[1].str();
			command = matches[2].str();
			return true; // Match found
		}
	}

	return false; // No match found
}

// TODO: make this a proper header-impl file, with other related things
bool IsServerStart()
{
	bool isHeadless = g_state->GetCLIArgs().is_headless;
	bool isSetToTravel = g_state->GetCLIArgs().next_map.has_value();
	return isHeadless || isSetToTravel;
}

// ATBLGameMode * __cdecl UTBLSystemLibrary::GetTBLGameMode(UObject *param_1)
CREATE_HOOK(
	GetTBLGameMode,
	PLATFORM_SIGNATURES(
		PLATFORM_SIGNATURE(EGS, "40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51")
		PLATFORM_SIGNATURE(STEAM, "40 53 48 83 EC 20 48 8B D9 48 85 C9 74 60 48 8B 01 FF 90 ?? ?? ?? ?? 48 85 C0 75 23 0F 1F 40 00 48 8B 5B 20 48 85 DB 74 11 48 8B 03 48 8B CB FF 90 ?? ?? ?? ?? 48 85 C0 74 E6 48 85 C0 74 2F 48 8B 98 28")
	),
	ATTACH_ALWAYS,
	void*, (void* uobj)
) {
	//LOG_DEBUG("GetTBLGameMode");
	const auto curGameMode = o_GetTBLGameMode(uobj);
	g_state->SetCurGameMode(curGameMode);
	return curGameMode;
}
AUTO_HOOK(GetTBLGameMode)

/*
void __thiscall
APlayerController::ClientMessage
		  (APlayerController *this,FString *param_1,FName param_2,float param_3)
*/
CREATE_HOOK(
	ClientMessage,
	UNIVERSAL_SIGNATURE("4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"),
	ATTACH_ALWAYS,
	void, (void* this_ptr, FString* param_1, void* param_2, float param_3)
) {
	bool egs = g_state->GetCLIArgs().platform == EGS;
	static uint64_t init = false;
	GLOG_DEBUG("ClientMessage");

	char* pValue;
	size_t len;
	char ladBuff[256];
	errno_t err = _dupenv_s(&pValue, &len, "LOCALAPPDATA");

	// TODO: make this nicer
	strncpy_s(ladBuff, 256, pValue, len);
	strncpy_s(ladBuff + len - 1, 256 - len, "\\Chivalry 2\\Saved\\Logs\\Unchained", 34);

	_mkdir(ladBuff);
	sprintf_s(ladBuff, 256, "%s\\Chivalry 2\\Saved\\Logs\\Unchained\\ClientMessage%s%s.log",
		pValue, (IsServerStart() ? "-server" : "-client"), (egs ? "-egs" : "-steam"));
	if (!init)
		GLOG_DEBUG("{}", ladBuff);

	std::wofstream  out(ladBuff, init++ ? std::ios_base::app : std::ios_base::trunc);
	if (out.is_open())
		out << init << L":: " << param_1->str << std::endl;
	else
		GLOG_ERROR("Can't open ClientMessage log for writing.");

	static std::wstring playerName;
	auto command = std::make_unique<std::wstring>();

	if (extractPlayerCommand(param_1->str, playerName, *command)) {
		GLOG_DEBUG("[ChatCommands] Extracted player name: {}", playerName);
		GLOG_DEBUG("[ChatCommands] Extracted command: {}", *command);

		FText txt;
		void* res = o_FText_AsCultureInvariant(&txt, new FString(L"Command detected"));
		if (res != nullptr && g_state->GetCurGameMode() != nullptr)
		{
			GLOG_DEBUG("[ChatCommands] Could print server text");
			o_BroadcastLocalizedChat(g_state->GetCurGameMode(), (FText*)res, 3);
		}

		GLOG_INFO("[ChatCommands] Executing command {}", *command);

		auto empty = FString(command->c_str());

		o_ExecuteConsoleCommand(&empty);
	}
	o_ClientMessage(this_ptr, param_1, param_2, param_3);
}
AUTO_HOOK(ClientMessage);