#pragma once

#include "../hooking/hook_macros.hpp"

CREATE_HOOK(
	FindFileInPakFiles_1,
	ATTACH_ALWAYS,
	long long, (void* this_ptr, const wchar_t* Filename, void** OutPakFile, void* OutEntry)
) {
	const auto attr{ GetFileAttributesW(Filename) };
	if (attr != INVALID_FILE_ATTRIBUTES && Filename && wcsstr(Filename, L"../../../")) {
		if (OutPakFile) OutPakFile = nullptr;
		return 0;
	}

	return o_FindFileInPakFiles_1(this_ptr, Filename, OutPakFile, OutEntry);
}
AUTO_HOOK(FindFileInPakFiles_1)

CREATE_HOOK(
	FindFileInPakFiles_2,
	ATTACH_ALWAYS,
	long long, (void* this_ptr, const wchar_t* Filename, void** OutPakFile, void* OutEntry)
) {
	const auto attr{ GetFileAttributesW(Filename) };
	if (attr != INVALID_FILE_ATTRIBUTES && Filename && wcsstr(Filename, L"../../../")) {
		if (OutPakFile) OutPakFile = nullptr;
		return 0;
	}

	return o_FindFileInPakFiles_2(this_ptr, Filename, OutPakFile, OutEntry);
}
AUTO_HOOK(FindFileInPakFiles_2)

CREATE_HOOK(
	IsNonPakFilenameAllowed,
	ATTACH_ALWAYS,
	long long, (void* this_ptr, void* InFilename)
) {
	return 1;
}
AUTO_HOOK(IsNonPakFilenameAllowed)