#pragma once

#include "../patching/patch_macros.hpp"

REGISTER_HOOK_PATCH(
	FindFileInPakFiles_1,
	APPLY_ALWAYS,
	long long, (void* this_ptr, const wchar_t* Filename, void** OutPakFile, void* OutEntry)
) {
	const auto attr{ GetFileAttributesW(Filename) };
	bool res = true;
	if (attr != INVALID_FILE_ATTRIBUTES && Filename && wcsstr(Filename, L"../../../")) {
		// if (Filename)
		// 	printf("FindFileInPakFiles_2: Checking file: %ls: %d\n", Filename, res);
		if (OutPakFile) OutPakFile = nullptr;
		res = 0;
	}
	else
		res = o_FindFileInPakFiles_1(this_ptr, Filename, OutPakFile, OutEntry);
	// true if contains Mods/ArgonSDK/Mods
	// if (wcsstr(Filename, L"Mods/ArgonSDK/Mods")) {
	// 	res = 1;
	// }
	// if (Filename && !res)
	// 	printf("FindFileInPakFiles_1: Checking file: %ls: %d\n", Filename, res);
	return res;
}

REGISTER_HOOK_PATCH(
	FindFileInPakFiles_2,
	APPLY_ALWAYS,
	long long, (void* this_ptr, const wchar_t* Filename, void** OutPakFile, void* OutEntry)
) {
	bool res = true;

	const auto attr{ GetFileAttributesW(Filename) };
	if (attr != INVALID_FILE_ATTRIBUTES && Filename && wcsstr(Filename, L"../../../")) {
		// if (Filename)
		// 	printf("FindFileInPakFiles_2: Checking file: %ls: %d\n", Filename, res);

		if (OutPakFile) OutPakFile = nullptr;
		res = 0;
	}
	else
		res =  o_FindFileInPakFiles_2(this_ptr, Filename, OutPakFile, OutEntry);
	
	// true if contains Mods/ArgonSDK/Mods
	// if (wcsstr(Filename, L"Mods/ArgonSDK/Mods")) {
	// 	res = 1;
	// }

	// if (Filename && !res)
	// 	printf("FindFileInPakFiles_2: Checking file: %ls: %d\n", Filename, res);

	return res;
}

REGISTER_HOOK_PATCH(
	IsNonPakFilenameAllowed,
	APPLY_ALWAYS,
	long long, (void* this_ptr, void* InFilename)
) {
	return 1;
}
