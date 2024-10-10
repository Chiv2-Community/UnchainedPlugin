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

/*
0x0	0x8	ITextData *	ITextData *	Object

0x8	0x8	FSharedReferencer<1>	/Chivalry2-Win64-Shipping.pdb/SharedPointerInternals/FSharedReferencer<1>
pack()
Structure FSharedReferencer<1> {
   0   FReferenceControllerBase *   8   ReferenceController   ""
}
Size = 8   Actual Alignment = 8
	SharedReferenceCount

0x10	0x4	uint	uint	Flags
*/
struct FText
{
	uint8_t text_data[0x10];
	uint32_t flags;

};

// TODO: it's probably ok to make all references to this FString,
// but I'm leaving it as-is right now.
// the difference is between a copy vs a reference
struct FString2 {
	/*FString(const wchar_t* str) {
		this->letter_count = lstrlenW(str) + 1;
		this->max_letters = this->letter_count;
		this->str = const_cast<wchar_t*>(str);
	}*/
	FString2(const wchar_t const* str) {
		this->letter_count = lstrlenW(str) + 1;
		this->max_letters = this->letter_count;
		this->str = new wchar_t[this->max_letters];
		wcscpy_s(this->str, this->max_letters, str);
	}
	~FString2() {
		delete[] this->str;
	}

	wchar_t* str;
	int letter_count;
	int max_letters;
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