#pragma once

// https://stackoverflow.com/a/8349281

#define FUNCTYPES \
	etype(UTBLLocalPlayer_Exec) \
	etype(GetGameInfo) \
	etype(MaxFuncType) //this always needs to be last

#define etype(x) uint32_t x = 0x0;


#define etype(x) F_##x,
typedef enum { FUNCTYPES }  FuncType;
#undef etype
#define etype(x) #x,
static const char* strFunc[F_MaxFuncType + 1] = { FUNCTYPES };

static const char* signatures[F_MaxFuncType + 1] =
{
	/*"UTBLLocalPlayer::Exec"*/
	"75 18 ?? ?? ?? ?? 75 12 4d 85 f6 74 0d 41 38 be ?? ?? ?? ?? 74 04 32 db eb 9b 48 8b 5d 7f 49 8b d5 4c 8b 45 77 4c 8b cb 49 8b cf",
	/*"GetGameInfo"*/
	"48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00",
	/*MaxFuncType*/
	""
};