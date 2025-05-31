#pragma once

#include <regex>
#include "../logging/Logger.hpp"
#include "legacy_hooks.h"
#include "../state/global_state.hpp"

DECL_HOOK(FString, ConsoleCommand, (void* this_ptr, FString const& str, bool b)) {
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

DECL_HOOK(void, ExecuteConsoleCommand, (FString* param)) {
	GLOG_INFO("EXECUTECONSOLECMD: {}", std::wstring(param->str));
	o_ExecuteConsoleCommand(param);
}

//FText* __cdecl FText::AsCultureInvariant(FText* __return_storage_ptr__, FString* param_1)
DECL_HOOK(void*, FText_AsCultureInvariant, (void* ret_ptr, FString* input)) {
	// This is extremely loud in the console
	//if (input->str != NULL) {
	//	printf("FText_AsCultureInvariant: ");
	//	wprintf(input->str);
	//  printf("\n");
	//}
	return o_FText_AsCultureInvariant(ret_ptr, input);
}

//void __thiscall ATBLGameMode::BroadcastLocalizedChat(ATBLGameMode *this,FText *param_1,Type param_2)
DECL_HOOK(void, BroadcastLocalizedChat, (void* game_mode, FText* text, uint8_t chat_type)) {
	GLOG_DEBUG("BroadcastLocalizedChat");
	return o_BroadcastLocalizedChat(game_mode, text, chat_type);
}

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

// TODO: nasty global. Make this a static local
// and use a getter-pattern call to this function to receive it
// wherever its value is needed. Make sure this pattern will actually
// work, too
void* CurGameMode = NULL;
// ATBLGameMode * __cdecl UTBLSystemLibrary::GetTBLGameMode(UObject *param_1)
DECL_HOOK(void*, GetTBLGameMode, (void* uobj)) {
	//LOG_DEBUG("GetTBLGameMode");
	CurGameMode = o_GetTBLGameMode(uobj);
	return CurGameMode;
}

/*
void __thiscall
APlayerController::ClientMessage
		  (APlayerController *this,FString *param_1,FName param_2,float param_3)
*/
DECL_HOOK(void, ClientMessage, (void* this_ptr, FString* param_1, void* param_2, float param_3))
{
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
		if (res != NULL && CurGameMode != NULL)
		{
			GLOG_DEBUG("[ChatCommands] Could print server text");
			o_BroadcastLocalizedChat(CurGameMode, (FText*)res, 3);
		}

		GLOG_INFO("[ChatCommands] Executing command {}", *command);

		auto empty = FString(command->c_str());

		o_ExecuteConsoleCommand(&empty);
	}
	o_ClientMessage(this_ptr, param_1, param_2, param_3);
}


#ifdef PRINT_CLIENT_MSG
DECL_HOOK(void, ClientMessage, (void* this_ptr, FString* param_1, void* param_2, float param_3))
{
	std::wstring commandLine = GetCommandLineW();
	bool egs = CmdGetParam(L"-epicapp=Peppermint") != -1;
	static uint64_t init = false;
	LOG_DEBUG("ClientMessage");

	wprintf(L"CLIENT_MESSAGE: %ls %.2f\n", param_1->str, param_3);

	std::wstring ws(param_1->str);
	std::string msg_str(ws.begin(), ws.end());
	o_ClientMessage(this_ptr, param_1, param_2, param_3);
}
#endif // PRINT_CLIENT_MSG