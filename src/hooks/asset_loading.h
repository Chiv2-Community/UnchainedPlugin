#pragma once

#include "../patching/patch_macros.hpp"

#ifdef CPP_HOOKS_IMPL

REGISTER_HOOK_PATCH(
	FindFileInPakFiles_1,
	APPLY_ALWAYS,
	long long, (void* this_ptr, const wchar_t* Filename, void** OutPakFile, void* OutEntry)
) {
	const auto attr{ GetFileAttributesW(Filename) };
	if (attr != INVALID_FILE_ATTRIBUTES && Filename && wcsstr(Filename, L"../../../")) {
		if (OutPakFile) OutPakFile = nullptr;
		return 0;
	}

	return o_FindFileInPakFiles_1(this_ptr, Filename, OutPakFile, OutEntry);
}

REGISTER_HOOK_PATCH(
	FindFileInPakFiles_2,
	APPLY_ALWAYS,
	long long, (void* this_ptr, const wchar_t* Filename, void** OutPakFile, void* OutEntry)
) {
	const auto attr{ GetFileAttributesW(Filename) };
	if (attr != INVALID_FILE_ATTRIBUTES && Filename && wcsstr(Filename, L"../../../")) {
		if (OutPakFile) OutPakFile = nullptr;
		return 0;
	}

	return o_FindFileInPakFiles_2(this_ptr, Filename, OutPakFile, OutEntry);
}

REGISTER_HOOK_PATCH(
	IsNonPakFilenameAllowed,
	APPLY_ALWAYS,
	long long, (void* this_ptr, void* InFilename)
) {
	return 1;
}
#endif
