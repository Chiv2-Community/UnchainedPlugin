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
#include "main.h"
#include "Chivalry2.h"
#include "UE4.h"
#include "logging.hpp"
#include "nettools.h"
#include "commandline.h"
#include "builds.h" 

//black magic for the linker to get winsock2 to work
//TODO: properly add this to the linker settings
#pragma comment(lib, "Ws2_32.lib")

// hooks
// TODO? figure out a better/cleaner way to do this
#include <csignal>

#include "backendHooks.h"
#include "ownershipOverrides.h"
#include "assetLoading.h"
#include "unchainedIntegration.h"
#include "adminControl.h"
#include "etcHooks.h"
#include "FunctionHookEnabler.hpp"
#include "StringUtil.h"
#include <share.h>
#include "Logging.hpp"
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

	LOG_DEBUG(g_logger, "found port: {}", port);

	try {
		return std::stoi(port);
	}
	catch (std::exception e) {
		return -1;
	}
}

void handleRCON() {
	MH_Initialize();

	std::wstring commandLine = GetCommandLineW();
	size_t flagLoc = commandLine.find(L"-rcon");
	if (flagLoc == std::wstring::npos) {
		ExitThread(0);
		return;
	}

	LOG_INFO(g_logger, "[RCON]: Found -rcon flag. RCON will be enabled.");

	int port = parsePortParams(commandLine, flagLoc);
	if (port == -1) {
		port = 9001; //default port
	}

	WSADATA wsaData;
	if (WSAStartup(MAKEWORD(2, 2), &wsaData) != 0) {
		LOG_ERROR(g_logger, "[RCON]: Failed to initialize Winsock!");
		ExitThread(0);
		return;
	}

	LOG_INFO(g_logger, "[RCON]: Opening RCON server socket on TCP/{}", port);

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
		LOG_DEBUG(g_logger, "[RCON]: Waiting for command");
		//get a command from a socket
		int addrLen = sizeof(addr);
		SOCKET remote = accept(listenSock, (sockaddr*)&addr, &addrLen);
		LOG_DEBUG(g_logger, "[RCON]: Accepted connection");
		if (remote == INVALID_SOCKET) {
			LOG_ERROR(g_logger, "[RCON]: invalid socket error");
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
	// Check if console already exists
	if (GetConsoleWindow()) {
		return; // Console already exists
	}

	// Allocate console
	if (!AllocConsole()) {
		AppendToFile("[DLL] Failed to allocate console\n");
		return;
	}

	// Redirect stdout, stderr, and stdin
	FILE* pCout;
	FILE* pCerr;
	FILE* pCin;

	freopen_s(&pCout, "CONOUT$", "w", stdout);
	freopen_s(&pCerr, "CONOUT$", "w", stderr);
	freopen_s(&pCin, "CONIN$", "r", stdin);

	// Make cout, wcout, cin, wcin, wcerr, cerr, wclog and clog
	// point to console as well
	std::ios::sync_with_stdio(true);

	// Set console title to identify it
	SetConsoleTitleA("Chivalry 2 Unchained Debug");

	// Test output
	AppendToFile("Debug console created successfully\n");
	std::cout << "C++ streams also work\n";
}

DWORD WINAPI  main_thread(LPVOID lpParameter) {
	try {
		initialize_global_logger();
		HMODULE hModule = static_cast<HMODULE>(lpParameter);
		auto logo_parts = split(std::string(UNCHAINED_LOGO), "\n");
		for (const auto& part : logo_parts) {
			LOG_ERROR(g_logger, "{}", part);
		}

		LOG_ERROR(g_logger, "Chivalry 2 Unchained Plugin");

		LOG_INFO(g_logger, "Command line args:");
		LOG_INFO(g_logger, "{}", std::wstring(GetCommandLineW()));
		LOG_INFO(g_logger, "");

		MH_Initialize();

		// https://github.com/HoShiMin/Sig
		const void* found = nullptr;
		bool loaded = LoadBuildConfig();

		if(!loaded) {
			LOG_INFO(g_logger, "Continuing with empty build config.");
		}


		baseAddr = GetModuleHandleA("Chivalry2-Win64-Shipping.exe");

		LOG_DEBUG(g_logger, "Base address: 0x{:X}", reinterpret_cast<uintptr_t>(baseAddr));


		int file_descript;
		//unsigned long file_size;
		auto err = _sopen_s(&file_descript, "Chivalry2-Win64-Shipping.exe", O_RDONLY, _SH_DENYNO, 0);
		if (err) LOG_ERROR(g_logger, "Error {}", err);

		// Get the size of the file
		off_t file_size = _filelength(file_descript);

		//MODULEINFO moduleInfo;
		GetModuleInformation(GetCurrentProcess(), baseAddr, &moduleInfo, sizeof(moduleInfo));

		auto module_base{ reinterpret_cast<unsigned char*>(baseAddr) };

		FunctionHookManager hookEnabler(baseAddr, moduleInfo, curBuild, STEAM);

		hookEnabler.register_hook(FViewport_Hook);
		hookEnabler.register_hook(LoadFrontEndMapHook);
		hookEnabler.register_hook(InternalGetNetMode_Hook);
		hookEnabler.register_hook(UGameplay__IsDedicatedServer_Hook);

		bool applyDesyncPatch = CmdGetParam(L"--desync-patch") != -1;
		if (applyDesyncPatch) {
			hookEnabler.register_hook(UNetDriver__GetNetMode_Hook);
		}


		hookEnabler.enable_hooks();

		for (uint8_t i = 0; i < F_MaxFuncType; ++i)
		{
			if (curBuild.offsets.at(strFunc[i]) == 0)
				curBuild.offsets.emplace(strFunc[i], FindSignature(baseAddr, moduleInfo.SizeOfImage, strFunc[i], signatures[i]));
			else LOG_INFO(g_logger, "ok -> %s : (conf)", strFunc[i]);
		}

		// Explicitly enable the FViewport hook before the others, because it is used in some other places.
		hookEnabler.enable_hook(FViewport_Hook);

		char buff[512];
		char* dest = buff;

		LOG_INFO(g_logger, "Serializing builds");
		offsetsLoaded = true;
		serializeBuilds();

		//HOOK_ATTACH(module_base, FViewport);
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

		if (curBuild.offsets[strFunc[F_UTBLLocalPlayer_Exec]])
		{
			// ServerPlugin
			auto cmd_permission{ module_base + curBuild.offsets[strFunc[F_UTBLLocalPlayer_Exec]] }; // Patch for command permission when executing commands (UTBLLocalPlayer::Exec)

			// From ServerPlugin
			// Patch for command permission when executing commands (UTBLLocalPlayer::Exec)
			Ptch_Repl(module_base + curBuild.offsets[strFunc[F_UTBLLocalPlayer_Exec]], 0xEB);
		}
		else
			LOG_ERROR(g_logger, "F_UTBLLocalPlayer_Exec missing");

		/*printf("offset dedicated: 0x%08X", curBuild.offsets[strFunc[F_UGameplay__IsDedicatedServer]] + 0x22);
		Ptch_Repl(module_base + curBuild.offsets[strFunc[F_UGameplay__IsDedicatedServer]] + 0x22, 0x2);*/
		// Dedicated server hook in ApproveLogin
		//Nop(module_base + curBuild.offsets[strFunc[F_ApproveLogin]] + 0x46, 6);

		LOG_INFO(g_logger, "Functions hooked. Continuing to RCON");
		handleRCON(); //this has an infinite loop for commands! Keep this at the end!

		ExitThread(0);
		return 0;
	} catch (const std::exception& e) {
		std::string error = "[DLL] std::exception: " + std::string(e.what());
		AppendToFile(error.c_str());
		return 1;
	} catch (...) {
		AppendToFile("[DLL] Unknown C++ exception caught\n");
		return 1;
	}
}


int __stdcall DllMain(HMODULE hModule, DWORD ul_reason_for_call, LPVOID lpReserved) {
	AppendToFile("[DLL] DllMain called\n");
	CreateDebugConsole();
	switch (ul_reason_for_call) {
		case DLL_PROCESS_ATTACH: {
			AppendToFile("[DLL] DLL PROCESS ATTACH");
			DisableThreadLibraryCalls(hModule);
			HANDLE thread_handle = CreateThread(NULL, 0, main_thread, hModule, 0, NULL);
			if (thread_handle) {
				CloseHandle(thread_handle);
			} else {
				AppendToFile("[DLL] Failed to create main thread\n");
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
