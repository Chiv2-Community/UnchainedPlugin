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
extern "C" const RustBuildInfo* load_current_build_info(bool scan_missing);
extern "C" uint8_t build_info_save(const void* bi);
extern "C" uint32_t build_info_get_file_hash(const void* bi);
extern "C" uint64_t build_info_get_offset(const RustBuildInfo* info, const char* name);

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


// Becomes true if all prelim patches cannot be applied due to missing offsets.
static bool restart_required = false;

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

static CLIArgs cliArgs = CLIArgs::Parse(GetCommandLineW());

DWORD WINAPI main_thread(LPVOID lpParameter) {
	try {
		auto logo_parts = split(std::string(UNCHAINED_LOGO), "\n");
		for (const auto& part : logo_parts) {
			GLOG_ERROR("{}", part);
		}

		GLOG_ERROR("Chivalry 2 Unchained Plugin");

		GLOG_INFO("Command line args:");
		GLOG_INFO("{}", std::wstring(GetCommandLineW()));
		GLOG_INFO("");

		auto rust_build_info = load_current_build_info(true);
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

		if (restart_required)
		{
			GLOG_ERROR("A restart is required to apply all patches. Exiting in 5 seconds. Please re-launch.");
			GLOG_INFO("");
			GLOG_WARNING("The error above is normal on the first launch of a new update.");
			Sleep(5000);
			TerminateProcess(GetCurrentProcess(), -67);
			return -67;
		}

		return 0;
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


bool initialize_minhook()
{
	GLOG_DEBUG("Initializing MinHook");
	auto mh_result = MH_Initialize();
	if (!mh_result == MH_OK) {
		GLOG_ERROR("MinHook initialization failed: {}", MH_StatusToString(mh_result));
		return false;
	}
	GLOG_DEBUG("MinHook initialized");
	return true;
}

/**
 * Some patches must be quickly applied before UE performs certain operations.
 * Any patch with a priority > 100 will be applied in this stage.
 * These patches block the main thread and are applied before all others.
 * This function cannot do much or else it will cause the function to crash.
 */
bool apply_preliminary_patches()
{
	bool success = true;

	const RustBuildInfo* build_info = load_current_build_info(false);

	if (build_info != nullptr)
	{
		auto hmodule = GetModuleHandleA("Chivalry2-Win64-Shipping.exe");
		auto base_addr = reinterpret_cast<uintptr_t>(hmodule);

		for (auto& patch : ALL_REGISTERED_PATCHES)
		{
			if (patch->get_priority() < 100)
				continue;

			auto address = base_addr + build_info_get_offset(build_info, patch->get_name().c_str());
			auto res = patch->apply(address);
			if (res == APPLY_FAILED) {
				success = false;
			} else
			{
				GLOG_INFO("Successfully applied preliminary patch '{}'", patch->get_name());
			}
		}
	} else {
		restart_required = true;
		success = false;
	}

	if (!success)
	{
		GLOG_ERROR("Failed to enable preliminary patches.");
	}
	return success;
}

int __stdcall DllMain(HMODULE hModule, DWORD ul_reason_for_call, LPVOID lpReserved) {
	CreateDebugConsole();
	switch (ul_reason_for_call) {
		case DLL_PROCESS_ATTACH: {
			OutputDebugStringA("[DLL] DLL PROCESS ATTACH");

			initialize_global_logger(LogLevel::TRACE);
			GLOG_INFO("Logger initialized.");

			if (!initialize_minhook())
			{
				GLOG_ERROR("Failed to initialize MinHook. Unchained cannot apply patches.");
				GLOG_ERROR("Sleeping for 5 seconds, then exiting.");
				Sleep(5000);
				TerminateProcess(GetCurrentProcess(), 1);
				return 0;
			}

			if (!apply_preliminary_patches())
			{
				GLOG_ERROR("Failed to apply preliminary patches. Some functionality may be broken.");
			}

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
