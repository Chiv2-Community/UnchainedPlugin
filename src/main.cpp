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
#include <thread>

#include <direct.h>

struct RustBuildInfo;
extern "C" const RustBuildInfo* load_current_build_info();
extern "C" uint8_t build_info_save(const void* bi);
extern "C" uint32_t build_info_get_file_hash(const void* bi);

//black magic for the linker to get winsock2 to work
//TODO: properly add this to the linker settings
#pragma comment(lib, "Ws2_32.lib")

#include <string_view>

#include "constants.h"
#include "string_util.hpp"

#include "logging/global_logger.hpp"
#include "stubs/UE4.h"
#include "state/global_state.hpp"
#include "patching/PatchManager.hpp"

#include "hooks/all_hooks.h"

void handleRCON(RCONState& rcon_state, int port) {
    WSADATA wsaData;
    if (WSAStartup(MAKEWORD(2, 2), &wsaData) != 0) {
        GLOG_ERROR("[RCON]: Failed to initialize Winsock!");
        return;
    }

    GLOG_DEBUG("[RCON]: Opening RCON server socket on TCP/{}", port);

    SOCKET listenSock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (listenSock == INVALID_SOCKET) {
        GLOG_ERROR("[RCON]: Failed to create socket, error: {}", WSAGetLastError());
        WSACleanup();
        return;
    }

    sockaddr_in addr;
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr);

    if (bind(listenSock, (sockaddr*)&addr, sizeof(addr)) == SOCKET_ERROR) {
        GLOG_ERROR("[RCON]: Bind failed with error: {}", WSAGetLastError());
        closesocket(listenSock);
        WSACleanup();
        return;
    }

    if (listen(listenSock, SOMAXCONN) == SOCKET_ERROR) {
        GLOG_ERROR("[RCON]: Listen failed with error: {}", WSAGetLastError());
        closesocket(listenSock);
        WSACleanup();
        return;
    }

    u_long mode = 1;
    ioctlsocket(listenSock, FIONBIO, &mode);

    GLOG_INFO("[RCON]: Listening for RCON commands on TCP/{}", port);

    constexpr size_t BUFFER_SIZE = 256;
    constexpr size_t CMD_BUFFER_SIZE = 1024;
    wchar_t cmd_buffer[CMD_BUFFER_SIZE];

	// TODO: listen for some signal to exit this loop
    while (true) {
        std::wstring command;

        int addrLen = sizeof(addr);
        SOCKET remote = accept(listenSock, (sockaddr*)&addr, &addrLen);

        if (remote == INVALID_SOCKET) {
            int error = WSAGetLastError();
            if (error == WSAEWOULDBLOCK) {
                // No connection available, sleep a bit and continue
                Sleep(100);
            } else {
	            GLOG_ERROR("[RCON]: Accept failed with error: {}", error);
            }
            continue;
        }

        GLOG_DEBUG("[RCON]: Accepted connection");

        char buffer[BUFFER_SIZE + 1] = {};
        int bytes_read = 0;

        do {
            bytes_read = recv(remote, buffer, BUFFER_SIZE, 0);

            if (bytes_read <= 0) {
                if (bytes_read < 0) {
                    GLOG_ERROR("[RCON]: Receive error: {}", WSAGetLastError());
                }
                break;
            }

            // Null-terminate received data
            buffer[bytes_read] = '\0';

            // Convert to wide string
            int required_size = MultiByteToWideChar(CP_UTF8, 0, buffer, bytes_read, nullptr, 0);
            if (required_size > 0) {
                size_t current_size = command.size();
                command.resize(current_size + required_size);
                MultiByteToWideChar(CP_UTF8, 0, buffer, bytes_read,
                                   &command[current_size], required_size);
            }
        } while (bytes_read > 0 && buffer[bytes_read - 1] != '\n');

        // Always close the client socket when done
        closesocket(remote);

        if (command.empty()) {
            continue;
        }

        // Copy command to static buffer with bounds checking
        if (command.size() < CMD_BUFFER_SIZE) {
            wcsncpy_s(cmd_buffer, command.c_str(), CMD_BUFFER_SIZE - 1);
            cmd_buffer[CMD_BUFFER_SIZE - 1] = 0; // ensure null-termination
            rcon_state.set_command(cmd_buffer);
        } else {
            GLOG_ERROR("[RCON]: Command too long, truncating");
            wcsncpy_s(cmd_buffer, command.c_str(), CMD_BUFFER_SIZE - 1);
            cmd_buffer[CMD_BUFFER_SIZE - 1] = 0;
            rcon_state.set_command(cmd_buffer);
        }
    }

    // Clean up resources
    closesocket(listenSock);
    WSACleanup();
    GLOG_INFO("[RCON]: RCON server stopped");
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
		initialize_global_logger(LogLevel::TRACE);
		GLOG_INFO("Logger initialized.");

		auto cliArgs = CLIArgs::Parse(GetCommandLineW());
		g_logger->set_level(cliArgs.log_level);

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

		auto rust_build_info = load_current_build_info();
		if (rust_build_info == nullptr) {
			GLOG_ERROR("Failed to get build info from Sleuth");
			return 1;
		}

		uint32_t build_hash = build_info_get_file_hash(rust_build_info);
		GLOG_INFO("Current build hash: {}", build_hash);

		auto state = new State(cliArgs);
		initialize_global_state(state);

		auto baseAddr = GetModuleHandleA("Chivalry2-Win64-Shipping.exe");
		MODULEINFO moduleInfo;

		GLOG_DEBUG("Base address: 0x{:X}", reinterpret_cast<uintptr_t>(baseAddr));

		int file_descript;
		auto err = _sopen_s(&file_descript, "Chivalry2-Win64-Shipping.exe", O_RDONLY, _SH_DENYNO, 0);
		if (err) GLOG_ERROR("Error {}", err);

		GetModuleInformation(GetCurrentProcess(), baseAddr, &moduleInfo, sizeof(moduleInfo));

		PatchManager patch_manager(baseAddr, moduleInfo, rust_build_info);

		for (auto& patch: ALL_REGISTERED_PATCHES) {
			patch_manager.register_patch(*patch);
		}

		auto all_patches_successful = patch_manager.apply_patches();
		if (!all_patches_successful) {
			GLOG_ERROR("Failed to apply all patches. Unchained may not function as expected.");
		} else {
			GLOG_INFO("All patches applied successfully");
		}

		if (state->GetCLIArgs().rcon_port.has_value()) {
			handleRCON(state->GetRCONState(), state->GetCLIArgs().rcon_port.value()); //this has an infinite loop for commands! Keep this at the end!
		}

		build_info_save(rust_build_info);
		ExitThread(0);
	} catch (const std::exception& e) {
		std::string error = "std::exception: " + std::string(e.what());
		GLOG_ERROR("std::exception: {}", e.what());
		GLOG_ERROR("Patching failed. Things are probably broken.");
		return 1;
	} catch (...) {
		GLOG_ERROR("Unknown C++ exception caught");
		GLOG_ERROR("Patching failed. Things are probably broken.");
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