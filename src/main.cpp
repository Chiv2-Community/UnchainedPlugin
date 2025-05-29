#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include <psapi.h>
#include <MinHook.h>
#include <iostream>
#include <fcntl.h>
#include <io.h>
#include <fstream>
#include <string>
#include <vector>

#include <direct.h>

//always open output window
// #define _DEBUG
#include "constants.h"
#include "Chivalry2.h"
#include "UE4.h"
#include "logging/Logger.hpp"
#include "nettools.h"
#include "commandline.h"
#include "builds.h"

//black magic for the linker to get winsock2 to work
//TODO: properly add this to the linker settings
#pragma comment(lib, "Ws2_32.lib")

// hooks
// TODO? figure out a better/cleaner way to do this
#include <csignal>

#include "hooks/all_hooks.h"
#include "hooking/FunctionHookManager.hpp"
#include "StringUtil.h"
#include <share.h>
#include "logging/global_logger.hpp"
#include "state/global_state.hpp"
#include <string_view>

inline void AppendToFile(const std::string& message) {
	OutputDebugStringA(message.c_str());
}

// parse the command line for the rcon flag, and return the port specified
// if not port was specified, or the string that was supposed to be a port number 
// was invalid, then -1 is returned
// TODO: swap this out for more generalized commandline parsing introduced in commandline.h
int parsePortParams(std::wstring commandLine, size_t flagLoc) {
	size_t portStart = commandLine.find(L" ", flagLoc); //next space
	if (portStart == std::wstring::npos) {
		return -1;
	}
	size_t portEnd = commandLine.find(L" ", portStart + 1); //space after that

	std::wstring port = portEnd != std::wstring::npos
		? commandLine.substr(portStart, portEnd - portStart)
		: commandLine.substr(portStart);

	GLOG_DEBUG("found port: {}", port);

	try {
		return std::stoi(port);
	}
	catch (std::exception e) {
		return -1;
	}
}

void handleRCON() {
	std::wstring commandLine = GetCommandLineW();
	size_t flagLoc = commandLine.find(L"-rcon");
	if (!g_state->GetCLIArgs().enable_rcon) {
		ExitThread(0);
		return;
	}

	GLOG_INFO("[RCON]: Found -rcon flag. RCON will be enabled.");

	int port = parsePortParams(commandLine, flagLoc);
	if (port == -1) {
		port = 9001; //default port
	}

	WSADATA wsaData;
	if (WSAStartup(MAKEWORD(2, 2), &wsaData) != 0) {
		GLOG_ERROR("[RCON]: Failed to initialize Winsock!");
		ExitThread(0);
		return;
	}

	GLOG_INFO("[RCON]: Opening RCON server socket on TCP/{}", port);

	SOCKET listenSock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);

	sockaddr_in addr;
	addr.sin_family = AF_INET;
	addr.sin_port = htons(port);
	inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr);

	bind(listenSock, (sockaddr*)&addr, sizeof(addr));
	listen(listenSock, SOMAXCONN);


	while (true) {
		//set up a new command string
		auto command = std::make_unique<std::wstring>();
		GLOG_DEBUG("[RCON]: Waiting for command");
		//get a command from a socket
		int addrLen = sizeof(addr);
		SOCKET remote = accept(listenSock, (sockaddr*)&addr, &addrLen);
		GLOG_DEBUG("[RCON]: Accepted connection");
		if (remote == INVALID_SOCKET) {
			GLOG_ERROR("[RCON]: invalid socket error");
			return;
		}
		const int BUFFER_SIZE = 256;
		//create one-filled buffer
		char buffer[BUFFER_SIZE + 1];
		for (int i = 0; i < BUFFER_SIZE + 1; i++) {
			buffer[i] = 1;
		}
		int count; //holds number of received bytes 
		do {
			count = recv(remote, (char*)&buffer, BUFFER_SIZE, 0); //receive a chunk (may not be the whole command)
			buffer[count] = 0; //null-terminate it implicitly
			//convert to wide string
			std::string chunkString(buffer, count);
			std::wstring wideChunkString(chunkString.begin(), chunkString.end() - 1);
			*command += wideChunkString; //append this chunk to the command
		} while (buffer[count - 1] != '\n');
		//we now have the whole command as a wide string
		closesocket(remote);

		if (command->size() == 0) {
			continue;
		}

		//add into command queue
		FString commandString(command->c_str());
		o_ExecuteConsoleCommand(&commandString);
	}

	return;
}


void CreateDebugConsole() {
	if (GetConsoleWindow()) {
		return;
	}

	if (!AllocConsole()) {
		OutputDebugStringA("[DLL] Failed to allocate console\n");
		return;
	}

	FILE* pCout;
	FILE* pCerr;

	freopen_s(&pCout, "CONOUT$", "w", stdout);
	freopen_s(&pCerr, "CONOUT$", "w", stderr);

	std::ios::sync_with_stdio(true);

	SetConsoleTitleA("Chivalry 2 Unchained Debug");

	GLOG_INFO("Debug console created successfully\n");
}

DWORD WINAPI  main_thread(LPVOID lpParameter) {


	try {
		initialize_global_logger(LogLevel::DEBUG);
		GLOG_INFO("Logger initialized");

		auto cliArgs = CLIArgs::Parse(GetCommandLineW());
		auto buildMetadata = BuildMetadata();
		auto state = State(cliArgs, buildMetadata);
		initialize_global_state(&state);

		HMODULE hModule = static_cast<HMODULE>(lpParameter);
		auto logo_parts = split(std::string(UNCHAINED_LOGO), "\n");
		for (const auto& part : logo_parts) {
			GLOG_ERROR("{}", part);
		}

		GLOG_ERROR("Chivalry 2 Unchained Plugin");

		GLOG_INFO("Command line args:");
		GLOG_INFO("{}", std::wstring(GetCommandLineW()));
		GLOG_INFO("");

		GLOG_DEBUG("Initializing MinHook");
		auto mh_result = MH_Initialize();
		if (!mh_result == MH_OK) {
			GLOG_ERROR("MinHook initialization failed: {}", MH_StatusToString(mh_result));
			return 1;
		}
		GLOG_DEBUG("MinHook initialized");

		// https://github.com/HoShiMin/Sig
		const void* found = nullptr;
		bool loaded = LoadBuildConfig();

		if(!loaded) {
			GLOG_INFO("Continuing with empty build config.");
		}


		baseAddr = GetModuleHandleA("Chivalry2-Win64-Shipping.exe");

		GLOG_DEBUG("Base address: 0x{:X}", reinterpret_cast<uintptr_t>(baseAddr));


		int file_descript;

		auto err = _sopen_s(&file_descript, "Chivalry2-Win64-Shipping.exe", O_RDONLY, _SH_DENYNO, 0);
		if (err) GLOG_ERROR("Error {}", err);

		off_t file_size = _filelength(file_descript);

		//MODULEINFO moduleInfo;
		GetModuleInformation(GetCurrentProcess(), baseAddr, &moduleInfo, sizeof(moduleInfo));

		auto module_base{ reinterpret_cast<unsigned char*>(baseAddr) };

		FunctionHookManager hook_manager(baseAddr, moduleInfo, STEAM);
		register_auto_hooks(hook_manager);

		hook_manager.enable_hook(&FViewport_Hook);
		hook_manager.enable_hooks();

		for (uint8_t i = 0; i < F_MaxFuncType; ++i)
		{
			auto maybeOffset = state.GetBuildMetadata().GetOffset(strFunc[i]);
			if (!maybeOffset.has_value())
				state.GetBuildMetadata().SetOffset(
					strFunc[i],
					FindSignature(baseAddr, moduleInfo.SizeOfImage, strFunc[i], signatures[i])
				);
			else GLOG_INFO("ok -> {} : (conf)", strFunc[i]);
		}

		char buff[512];
		char* dest = buff;

		GLOG_INFO("Serializing builds");
		offsetsLoaded = true;
		serializeBuilds();

		HOOK_ATTACH(module_base, GetMotd);
		HOOK_ATTACH(module_base, GetCurrentGames);
		HOOK_ATTACH(module_base, SendRequest);
		HOOK_ATTACH(module_base, IsNonPakFilenameAllowed);
		HOOK_ATTACH(module_base, FindFileInPakFiles_1);
		HOOK_ATTACH(module_base, FindFileInPakFiles_2);
		HOOK_ATTACH(module_base, GetGameInfo);
		HOOK_ATTACH(module_base, ConsoleCommand);
		HOOK_ATTACH(module_base, CanUseLoadoutItem);
		HOOK_ATTACH(module_base, CanUseCharacter);

		bool useBackendBanList = CmdGetParam(L"--use-backend-banlist") != -1;
		if (useBackendBanList) {
			HOOK_ATTACH(module_base, FString_AppendChars);
			HOOK_ATTACH(module_base, PreLogin);

		}

		bool IsHeadless = CmdGetParam(L"-nullrhi") != -1;
		if (IsHeadless) {
			HOOK_ATTACH(module_base, GetOwnershipFromPlayerControllerAndState);
			HOOK_ATTACH(module_base, ConditionalInitializeCustomizationOnServer);
		}

#ifdef PRINT_CLIENT_MSG
		HOOK_ATTACH(module_base, ClientMessage);
#endif

		HOOK_ATTACH(module_base, ClientMessage);
		HOOK_ATTACH(module_base, ExecuteConsoleCommand);
		HOOK_ATTACH(module_base, GetTBLGameMode);
		HOOK_ATTACH(module_base, FText_AsCultureInvariant);
		HOOK_ATTACH(module_base, BroadcastLocalizedChat);

		auto localPlayerOffset = g_state->GetBuildMetadata().GetOffset(strFunc[F_UTBLLocalPlayer_Exec]);
		if (localPlayerOffset.has_value()) {
			// Patch for command permission when executing commands (UTBLLocalPlayer::Exec)

			auto cmd_permission{ module_base + localPlayerOffset.value() };
			Ptch_Repl(module_base + localPlayerOffset.value(), 0xEB);
		}
		else
			GLOG_ERROR("F_UTBLLocalPlayer_Exec missing");

		/*printf("offset dedicated: 0x%08X", curBuild.offsets[strFunc[F_UGameplay__IsDedicatedServer]] + 0x22);
		Ptch_Repl(module_base + curBuild.offsets[strFunc[F_UGameplay__IsDedicatedServer]] + 0x22, 0x2);*/
		// Dedicated server hook in ApproveLogin
		//Nop(module_base + curBuild.offsets[strFunc[F_ApproveLogin]] + 0x46, 6);

		GLOG_INFO("Functions hooked. Continuing to RCON");
		handleRCON(); //this has an infinite loop for commands! Keep this at the end!

		ExitThread(0);
		return 0;
	} catch (const std::exception& e) {
		std::string error = "std::exception: " + std::string(e.what());
		GLOG_ERROR("std::exception: {}", e.what());
		GLOG_ERROR("Function hooking failed. Things are probably broken.");
		return 1;
	} catch (...) {
		GLOG_ERROR("Unknown C++ exception caught");
		GLOG_ERROR("Function hooking failed. Things are probably broken.");
		return 1;
	}
}


int __stdcall DllMain(HMODULE hModule, DWORD ul_reason_for_call, LPVOID lpReserved) {
	CreateDebugConsole();
	switch (ul_reason_for_call) {
		case DLL_PROCESS_ATTACH: {
			OutputDebugStringA("[DLL] DLL PROCESS ATTACH");
			DisableThreadLibraryCalls(hModule);
			HANDLE thread_handle = CreateThread(NULL, 0, main_thread, hModule, 0, NULL);
			if (thread_handle) {
				CloseHandle(thread_handle);
			} else {
				OutputDebugStringA("[DLL] Failed to create main thread\n");
				return FALSE;
			}
			break;
		}
		case DLL_THREAD_ATTACH:
		case DLL_THREAD_DETACH:
		case DLL_PROCESS_DETACH:
			break;
	}
	return 1;
}
