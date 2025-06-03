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



//black magic for the linker to get winsock2 to work
//TODO: properly add this to the linker settings
#pragma comment(lib, "Ws2_32.lib")

#include <string_view>

#include "constants.h"
#include "builds.hpp"
#include "patch.hpp"
#include "string_util.hpp"

#include "logging/global_logger.hpp"
#include "stubs/UE4.h"
#include "state/global_state.hpp"
#include "hooking/PatchManager.hpp"

#include "hooks/all_hooks.h"
#include "hooking/heuristics/all_heuristics.h"

void handleRCON() {
	std::wstring commandLine = GetCommandLineW();
	size_t flagLoc = commandLine.find(L"-rcon");
	if (!g_state->GetCLIArgs().rcon_port.has_value()) {
		ExitThread(0);
		return;
	}

	GLOG_INFO("[RCON]: Found -rcon flag. RCON will be enabled.");

	int port = g_state->GetCLIArgs().rcon_port.value();

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

	HANDLE hConsole = GetStdHandle(STD_OUTPUT_HANDLE);
	DWORD consoleMode;
	GetConsoleMode(hConsole, &consoleMode);
	SetConsoleMode(hConsole, consoleMode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);


	GLOG_INFO("Debug console created successfully\n");
}

DWORD WINAPI  main_thread(LPVOID lpParameter) {
	try {
		initialize_global_logger(LogLevel::INFO);
		GLOG_INFO("Logger initialized");

		auto cliArgs = CLIArgs::Parse(GetCommandLineW());

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

		std::map<std::string, BuildMetadata> loaded = LoadBuildMetadata();

		uint32_t build_hash = calculateCRC32("Chivalry2-Win64-Shipping.exe");
		std::string build_hash_string = std::to_string(build_hash);
		bool needsSerialization = false;

		if (!loaded.contains(build_hash_string)) {
			loaded.emplace(build_hash_string, BuildMetadata(build_hash, 0, {}, "", cliArgs.platform));
			needsSerialization = true;
		}

		BuildMetadata& current_build_metadata = loaded.at(build_hash_string);

		auto state = new State(cliArgs, loaded, current_build_metadata);
		initialize_global_state(state);

		auto baseAddr = GetModuleHandleA("Chivalry2-Win64-Shipping.exe");
		MODULEINFO moduleInfo;

		GLOG_DEBUG("Base address: 0x{:X}", reinterpret_cast<uintptr_t>(baseAddr));

		int file_descript;
		auto err = _sopen_s(&file_descript, "Chivalry2-Win64-Shipping.exe", O_RDONLY, _SH_DENYNO, 0);
		if (err) GLOG_ERROR("Error {}", err);

		GetModuleInformation(GetCurrentProcess(), baseAddr, &moduleInfo, sizeof(moduleInfo));

		auto module_base{ reinterpret_cast<unsigned char*>(baseAddr) };

		PatchManager hook_manager(baseAddr, moduleInfo, current_build_metadata, all_heuristics);
		register_auto_patches(hook_manager);
		auto all_patchess_successful = hook_manager.apply_patches();

		if (needsSerialization)
			SaveBuildMetadata(loaded);

		if (!all_patchess_successful) {
			GLOG_ERROR("Failed to hook all functions. Unchained may not function as expected.");
		}

		GLOG_INFO("Continuing to RCON");
		handleRCON(); //this has an infinite loop for commands! Keep this at the end!

		ExitThread(0);
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
