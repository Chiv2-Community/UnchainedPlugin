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

extern "C" uint8_t generate_json();


//black magic for the linker to get winsock2 to work
//TODO: properly add this to the linker settings
#pragma comment(lib, "Ws2_32.lib")

#include <string_view>

#include "constants.h"
#include "builds.hpp"
#include "string_util.hpp"

#include "logging/global_logger.hpp"
#include "stubs/UE4.h"
#include "state/global_state.hpp"
#include "patching/PatchManager.hpp"

#include "hooks/all_hooks.h"


#include <mutex>
#include <condition_variable>

std::mutex mtx;
std::condition_variable cv;
bool ready = false;

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



bool Patch(BYTE *address) {
  // Patch the target function to always return true.
  // mov al, 1; ret
//   const BYTE patchBytes[] = {0xB0, 0x01, 0xC3};
//   const BYTE patchBytes[] = {0x90, 0x90 };
  const BYTE patchBytes[] = {0xEB};
  DWORD oldProtect;
  if (!VirtualProtect(address, sizeof(patchBytes), PAGE_EXECUTE_READWRITE,
                      &oldProtect)) {
    // LogMessage(LogLevel::INFO, "UniversalPatch", __LINE__,
    // (std::ostringstream() << "Failed to change protection at " << std::hex <<
    // address).str());

    // GLOG_INFO("Failed to change protection\n");
    return false;
  }

  std::memcpy(address, patchBytes, sizeof(patchBytes));
  FlushInstructionCache(GetCurrentProcess(), address, sizeof(patchBytes));
  VirtualProtect(address, sizeof(patchBytes), oldProtect, &oldProtect);
    // GLOG_INFO("Patched Sig\n");
  return true;
}

void * FPakPlatformFile = nullptr;

typedef bool (*FPakPlatformFile_Mount_t)(void /*FPakPlatformFile*/  *this_ptr, wchar_t *param_1,int param_2,wchar_t *param_3,bool param_4);
FPakPlatformFile_Mount_t o_FPakPlatformFile_Mount;

// bool Mount(const TCHAR* InPakFilename, uint32 PakOrder, const TCHAR* InPath = NULL, bool bLoadIndex = true);
bool hk_FPakPlatformFile_Mount(void /*FPakPlatformFile*/  *this_ptr, wchar_t *param_1,int param_2,wchar_t *param_3,bool param_4) {
    // *(bool*)((uint8_t*)this_ptr + 0x30) = false;
    FPakPlatformFile = this_ptr;
    // if (param_3 != nullptr) {
    //     printf("   mount path: %ls\n", param_3);
    // }   
    bool res = o_FPakPlatformFile_Mount(this_ptr, param_1, param_2, param_3, param_4);
    // GLOG_INFO("FPakPlatformFile__Mount: {}  : {}", std::wstring(param_1), res);
    printf(">>>>>>>>>>>> FPakPlatformFile__Mount: %ls  loaded: %d index: %d\n", param_1, res, param_2);
    return res;
}

// // FUN_141026be0
// typedef void * (*SCOPED_BOOT_TIMING_t)(void * this_ptr,wchar_t *param_1);
// SCOPED_BOOT_TIMING_t o_SCOPED_BOOT_TIMING;

// // bool Mount(const TCHAR* InPakFilename, uint32 PakOrder, const TCHAR* InPath = NULL, bool bLoadIndex = true);
// void * hk_SCOPED_BOOT_TIMING(void * this_ptr, wchar_t *param_1) {
//     if (param_1 != nullptr && wstrlen(param_1) > 0) {
//         printf("   hk_SCOPED_BOOT_TIMING: %ls\n", param_1);
//     }   
//     return o_SCOPED_BOOT_TIMING(this_ptr, param_1);
// }
// FUN_141026be0
typedef uint32_t (*GetPakOrderFromFilePath_t)(void * this_ptr, FString * param_1);
GetPakOrderFromFilePath_t o_GetPakOrderFromFilePath;

// bool Mount(const TCHAR* InPakFilename, uint32 PakOrder, const TCHAR* InPath = NULL, bool bLoadIndex = true);
uint32_t hk_GetPakOrderFromFilePath(void * this_ptr, FString * param_1) {
    
    // std::unique_lock<std::mutex> lock(mtx);

    // cv.wait(lock, []{ return ready; });
    auto rval = o_GetPakOrderFromFilePath(this_ptr, param_1);
    if (param_1 != nullptr) {
        // printf("   hk_GetPakOrderFromFilePath: %ls %d\n", std::wstring(param_1->str).c_str(), rval);
        printf("   hk_GetPakOrderFromFilePath: %d\n", rval);
    }   
    return 1;
}


// void FPakFile::GetFilenames(longlong *param_1,longlong *param_2)
typedef void (*FPakFile_GetFilenames_t)(void * this_ptr, void * param_1);
FPakFile_GetFilenames_t o_FPakFile_GetFilenames;

// bool Mount(const TCHAR* InPakFilename, uint32 PakOrder, const TCHAR* InPath = NULL, bool bLoadIndex = true);
void hk_FPakFile_GetFilenames(void * this_ptr, void * param_1) {
    
    // std::unique_lock<std::mutex> lock(mtx);

    printf(">>>>>>>>>>>>>>>>>>>>>>>   hk_GetPakOrderFromFilePath\n");
    // cv.wait(lock, []{ return ready; });
    return o_FPakFile_GetFilenames(this_ptr, param_1);
    // if (param_1 != nullptr) {
    //     // printf("   hk_FPakFile_GetFilenames: %ls %d\n", std::wstring(param_1->str).c_str(), rval);
    //     printf("   hk_GetPakOrderFromFilePath: %d\n", rval);
    // }   
}

typedef void (*LinkerLoad_OnNewFileAdded_t)(void * param_1);
LinkerLoad_OnNewFileAdded_t o_LinkerLoad_OnNewFileAdded;

// bool Mount(const TCHAR* InPakFilename, uint32 PakOrder, const TCHAR* InPath = NULL, bool bLoadIndex = true);
void hk_LinkerLoad_OnNewFileAdded(void * param_1) {
    
    // std::unique_lock<std::mutex> lock(mtx);

    printf(">>>>>>>>>>>>>>>>>>>>>>>   hk_GetPakOrderFromFilePath\n");
    // cv.wait(lock, []{ return ready; });
    return o_LinkerLoad_OnNewFileAdded(param_1);
    // if (param_1 != nullptr) {
    //     // printf("   hk_FPakFile_GetFilenames: %ls %d\n", std::wstring(param_1->str).c_str(), rval);
    //     printf("   hk_GetPakOrderFromFilePath: %d\n", rval);
    // }   
}

bool Patch2(BYTE *address) {
  // Patch the target function to always return true.
  // mov al, 1; ret
//   const BYTE patchBytes[] = {0xB0, 0x01, 0xC3};
//   const BYTE patchBytes[] = {0x90, 0x90 };
  const BYTE patchBytes[] = {0xEB, 0x7c+2};
  DWORD oldProtect;
  if (!VirtualProtect(address, sizeof(patchBytes), PAGE_EXECUTE_READWRITE,
                      &oldProtect)) {
    // LogMessage(LogLevel::INFO, "UniversalPatch", __LINE__,
    // (std::ostringstream() << "Failed to change protection at " << std::hex <<
    // address).str());

    GLOG_INFO("Failed to change protection\n");
    return false;
  }

  std::memcpy(address, patchBytes, sizeof(patchBytes));
  FlushInstructionCache(GetCurrentProcess(), address, sizeof(patchBytes));
  VirtualProtect(address, sizeof(patchBytes), oldProtect, &oldProtect);
    // GLOG_INFO("Patched Sig\n");
  return true;
}

// bool Patch_bytes(BYTE *address, const BYTE patchBytes[], size_t size) {
//   DWORD oldProtect;
//   if (!VirtualProtect(address, size, PAGE_EXECUTE_READWRITE,
//                       &oldProtect)) {
//     GLOG_INFO("Failed to change protection\n");
//     return false;
//   }

//   std::memcpy(address, patchBytes, size);
//   FlushInstructionCache(GetCurrentProcess(), address, size);
//   VirtualProtect(address, size, oldProtect, &oldProtect);
//   return true;
// }
#include <initializer_list> // Necessary header

bool Patch_bytes(BYTE *address, std::initializer_list<BYTE> patchBytes, size_t size) {
    if (patchBytes.size() != size) {
        GLOG_ERROR("Patch_bytes size mismatch: expected {}, got {}", size, patchBytes.size());
        return false;
    }
    
    DWORD oldProtect;
    if (!VirtualProtect(address, size, PAGE_EXECUTE_READWRITE, &oldProtect)) {
        GLOG_ERROR("Failed to change protection\n");
        return false;
    }
    std::memcpy(address, patchBytes.begin(), size); 
    
    FlushInstructionCache(GetCurrentProcess(), address, size);
    VirtualProtect(address, size, oldProtect, &oldProtect);
    return true;
}

#include <chrono>
#include <thread> // For std::this_thread::sleep_for

bool attached = false;

DWORD WINAPI main_thread(LPVOID lpParameter) {
  try {

		initialize_global_logger(LogLevel::TRACE);
		GLOG_INFO("Logger initialized.");

        
        

		auto found_offsets = generate_json();
		GLOG_INFO("Sleuth found {} offsets", found_offsets);

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
		// auto mh_result = MH_Initialize();
		// if (!mh_result == MH_OK) {
		// 	GLOG_ERROR("MinHook initialization failed: {}", MH_StatusToString(mh_result));
		// 	return 1;
		// }
		GLOG_DEBUG("MinHook initialized");

		std::map<std::string, BuildMetadata> loaded = LoadBuildMetadata();

		uint32_t build_hash = calculateCRC32("Chivalry2-Win64-Shipping.exe");
		std::string build_hash_string = std::to_string(build_hash);

		if (!loaded.contains(build_hash_string)) {
			GLOG_ERROR("Failed to load build metadata for build hash: {}", build_hash_string);
			GLOG_ERROR("Something is probably wrong with the rust module invocation");
			return 1;
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

		PatchManager patch_manager(baseAddr, moduleInfo, current_build_metadata);

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

        // >>>>>>>>>>>> FPakPlatformFile__Mount: ../../../TBL/Content/Paks/DripSync.pak  loaded: 1 index: 3
        // >>>>>>>>>>>> FPakPlatformFile__Mount: ../../../TBL/Content/Paks/ChatSendTest.pak  loaded: 1 index: 3
        // >>>>>>>>>>>> FPakPlatformFile__Mount: ../../../TBL/Content/Paks/ChatHooks.pak  loaded: 1 index: 3
        // >>>>>>>>>>>> FPakPlatformFile__Mount: ../../../TBL/Content/Paks/ChatCommands.pak  loaded: 1 index: 3
        // >>>>>>>>>>>> FPakPlatformFile__Mount: ../../../TBL/Content/Paks/BearTest.pak  loaded: 1 index: 3
        // >>>>>>>>>>>> FPakPlatformFile__Mount: ../../../TBL/Content/Paks/asdf.pak  loaded: 1 index: 3
        // >>>>>>>>>>>> FPakPlatformFile__Mount: ../../../TBL/Content/Paks/Amos.pak  loaded: 1 index: 3
        // >>>>>>>>>>>> FPakPlatformFile__Mount: ../../../TBL/Content/Paks/AIArchers.pak  loaded: 1 index: 3
        // HMODULE baseAddr2 = GetModuleHandleA("Chivalry2-Win64-Shipping.exe");
        // BYTE *baseAddress = reinterpret_cast<BYTE *>(baseAddr2);
        // auto sig_bp = baseAddress + 0x1dc45e0; 
        // // 48 8D 0D ?? ?? ?? ?? E9 ?? ?? ?? ?? CC CC CC CC 48 83 EC 28 E8 ?? ?? ?? ?? 48 89 05 ?? ?? ?? ?? 48 83 C4 28 C3 CC CC CC CC CC CC CC CC CC CC CC 48 8D 0D ?? ?? ?? ?? E9 ?? ?? ?? ?? CC CC CC CC 48 8D 0D ?? ?? ?? ?? E9 ?? ?? ?? ?? CC CC CC CC
        // Patch_bytes(sig_bp, {0xC3}, 1);
// bool Mount(const TCHAR* InPakFilename, uint32 PakOrder, const TCHAR* InPath = NULL, bool bLoadIndex = true);
// bool hk_FPakPlatformFile_Mount(void /*FPakPlatformFile*/  *this_ptr, wchar_t *param_1,int param_2,wchar_t *param_3,bool param_4) {
    // bool res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/DripSync.pak"), 1, NULL, true);
    // res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/ChatSendTest.pak"), 1, NULL, true);
    // res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/ChatHooks.pak"), 1, NULL, true);
    // res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/ChatCommands.pak"), 1, NULL, true);
    // res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/BearTest.pak"), 1, NULL, true);
    // res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/asdf.pak"), 1, NULL, true);
    // res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/Amos.pak"), 1, NULL, true);
    // res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/AIArchers.pak"), 1, NULL, true);
    // res = hk_FPakPlatformFile_Mount(FPakPlatformFile, const_cast<wchar_t *>(L"../../../TBL/Content/Paks/Unchained-Mods.pak"), 1, NULL, true);

        {
            std::lock_guard<std::mutex> lock(mtx);
            ready = true;
        }
        cv.notify_one();

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

DWORD WINAPI BlockerThread(LPVOID) {
    // You can safely block here
    // Sleep(INFINITE); 
    std::unique_lock<std::mutex> lock(mtx);

    cv.wait(lock, []{ return ready; });
    return 0;
}



int __stdcall DllMain(HMODULE hModule, DWORD ul_reason_for_call, LPVOID lpReserved) {
    // HMODULE baseAddr2 = GetModuleHandleA("Chivalry2-Win64-Shipping.exe");
    // BYTE *baseAddress = reinterpret_cast<BYTE *>(baseAddr2);
    // Patch(baseAddress + 0x1dc45e0); // Patch the function at this address to
    //                                 // always return true          
    // CreateThread(nullptr, 0, BlockerThread, nullptr, 0, nullptr);

	// MH_CreateHook(baseAddress + 0x2fc3700, hk_GetPakOrderFromFilePath, reinterpret_cast<void**>(&o_GetPakOrderFromFilePath));
	// MH_EnableHook(baseAddress + 0x2fc3700);
	switch (ul_reason_for_call) {
		case DLL_PROCESS_ATTACH: {
            CreateDebugConsole();
            printf("Chivalry 2 Unchained DLL injected successfully.\n");
            HMODULE baseAddr2 = GetModuleHandleA("Chivalry2-Win64-Shipping.exe");
            BYTE *baseAddress = reinterpret_cast<BYTE *>(baseAddr2);

            GLOG_DEBUG("Initializing MinHook");
            auto mh_result = MH_Initialize();
            if (!mh_result == MH_OK) {
                GLOG_ERROR("MinHook initialization failed: {}", MH_StatusToString(mh_result));
                return 1;
            }

            // filename patch
            auto mount_stricmp_pakname = baseAddress + 0x2fc8d50 + 0xb7; 
            Patch_bytes(mount_stricmp_pakname, {0xEB}, 1); // jz->jmp
            auto sig_bp = baseAddress + 0x1dc45e0; 
            // 48 8D 0D ?? ?? ?? ?? E9 ?? ?? ?? ?? CC CC CC CC 48 83 EC 28 E8 ?? ?? ?? ?? 48 89 05 ?? ?? ?? ?? 48 83 C4 28 C3 CC CC CC CC CC CC CC CC CC CC CC 48 8D 0D ?? ?? ?? ?? E9 ?? ?? ?? ?? CC CC CC CC 48 8D 0D ?? ?? ?? ?? E9 ?? ?? ?? ?? CC CC CC CC
            // Patch_bytes(sig_bp, {0xB0, 0x01, 0xC3}, 3);
            // Patch_bytes(sig_bp, {0xC3}, 1);

            // Patch(baseAddress + 0x2fc8d50 + 0xB7);  // fpakplatformfile_mount
            // Patch2(baseAddress + 0x2fc8d50 + 0xdc3);  // fpakplatformfile_mount
            // Patch(baseAddress + 0x2fc8d50 + 0xdc5);  // fpakplatformfile_mount
            MH_CreateHook(baseAddress + 0x2fc8d50, hk_FPakPlatformFile_Mount, reinterpret_cast<void**>(&o_FPakPlatformFile_Mount));
            MH_EnableHook(baseAddress + 0x2fc8d50);
            
            MH_CreateHook(baseAddress + 0x1f85170, hk_LinkerLoad_OnNewFileAdded, reinterpret_cast<void**>(&o_LinkerLoad_OnNewFileAdded));
            MH_EnableHook(baseAddress + 0x1f85170);

            MH_CreateHook(baseAddress + 0x2fc2d50, hk_FPakFile_GetFilenames, reinterpret_cast<void**>(&o_FPakFile_GetFilenames));
            MH_EnableHook(baseAddress + 0x2fc2d50);

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