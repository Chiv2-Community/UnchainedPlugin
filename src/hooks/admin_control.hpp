#pragma once

#include <regex>
#include "../logging/Logger.hpp"
#include "../patching/patch_macros.hpp"
#include "../state/global_state.hpp"
#include "../stubs/UE4.h"

REGISTER_BYTE_PATCH(UTBLLocalPlayer_Exec, APPLY_ALWAYS, { 0xEB })

REGISTER_HOOK_PATCH(
	ExecuteConsoleCommand,
	APPLY_ALWAYS,
	void, (FString* param)
) {
	GLOG_INFO("Executing console command: {}", std::wstring(param->str));
	o_ExecuteConsoleCommand(param);
}

//FText* __cdecl FText::AsCultureInvariant(FText* __return_storage_ptr__, FString* param_1)
REGISTER_HOOK_PATCH(
	FText_AsCultureInvariant,
	APPLY_ALWAYS,
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

//void __thiscall ATBLGameMode::BroadcastLocalizedChat(ATBLGameMode *this,FText *param_1,Type param_2)
REGISTER_HOOK_PATCH(
	BroadcastLocalizedChat,
	APPLY_ALWAYS,
	void, (void* game_mode, FText* text, uint8_t chat_type)
) {
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

// ATBLGameMode * __cdecl UTBLSystemLibrary::GetTBLGameMode(UObject *param_1)
REGISTER_HOOK_PATCH(
	GetTBLGameMode,
	APPLY_ALWAYS,
	void*, (void* uobj)
) {
	//LOG_DEBUG("GetTBLGameMode");
	const auto curGameMode = o_GetTBLGameMode(uobj);
	g_state->SetCurGameMode(curGameMode);
	return curGameMode;
}

/*
void __thiscall
APlayerController::ClientMessage
		  (APlayerController *this,FString *param_1,FName param_2,float param_3)
*/
REGISTER_HOOK_PATCH(
	ClientMessage,
	APPLY_ALWAYS,
	void, (void* this_ptr, FString* param_1, void* param_2, float param_3)
) {
	GLOG_TRACE("ClientMessage");

	bool egs = g_state->GetCLIArgs().platform == EGS;
	static uint64_t init = false;

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
		GLOG_DEBUG("Writing Client Logs to: ", ladBuff);

	std::wofstream  out(ladBuff, init++ ? std::ios_base::app : std::ios_base::trunc);
	if (out.is_open())
		out << init << L":: " << param_1->str << std::endl;
		GLOG_DEBUG("ClientMessage: {}", param_1->str);
	else
		GLOG_ERROR("Can't open ClientMessage log for writing.");

#ifdef CHAT_COMMANDS
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

		hk_ExecuteConsoleCommand(&empty);
	}
#endif
	o_ClientMessage(this_ptr, param_1, param_2, param_3);
}
