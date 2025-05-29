#pragma once
#include <Windows.h>
#include <cstdint>
#include <string>

// UE Types

struct FString {
	FString(const wchar_t* str) {
		this->letter_count = lstrlenW(str) + 1;
		this->max_letters = this->letter_count;
		this->str = const_cast<wchar_t*>(str);
	}

	wchar_t* str;
	int letter_count;
	int max_letters;
};

struct FText
{
	uint8_t text_data[0x10];
	uint32_t flags;

};

enum ENetMode: uint8_t {
	STANDALONE = 0,
	DEDICATED_SERVER = 1,
	LISTEN_SERVER = 2,
	CLIENT = 3,
	MAX = 4
};

//FViewport* __thiscall FViewport::FViewport(FViewport* this, FViewportClient* param_1)
struct FViewport_C
{
	uint8_t ph[0x20];
	FString AppVersionString;
};

struct GCGObj {
	FString url_base;
};

struct FUniqueNetIdRepl {};

struct UAbilitySpec {};

struct ECharacterControlEvent {};