#pragma once

#include "../hooking/hook_macros.hpp"

CREATE_HOOK(
	FindFileInPakFiles_1,
	UNIVERSAL_SIGNATURE("48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 41 54 41 55 41 56 41 57 48 83 EC 30 33 FF"),
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
	UNIVERSAL_SIGNATURE("48 8B C4 4C 89 48 ?? 4C 89 40 ?? 48 89 48 ?? 55 53 48 8B EC"),
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
	UNIVERSAL_SIGNATURE("48 89 5C 24 ?? 48 89 6C 24 ?? 56 57 41 56 48 83 EC 30 48 8B F1 45 33 C0"),
	ATTACH_ALWAYS,
	long long, (void* this_ptr, void* InFilename)
) {
	return 1;
}
AUTO_HOOK(IsNonPakFilenameAllowed)