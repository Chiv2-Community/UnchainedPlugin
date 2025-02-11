#pragma once

// https://stackoverflow.com/a/8349281

#define FUNCTYPES \
	etype(FViewport) \
	etype(SendRequest) \
	etype(GetMotd) \
	etype(GetCurrentGames) \
	etype(IsNonPakFilenameAllowed) \
	etype(FindFileInPakFiles_1) \
	etype(FindFileInPakFiles_2) \
	etype(UTBLLocalPlayer_Exec) \
	etype(GetGameInfo) \
	etype(ConsoleCommand) \
	etype(LoadFrontEndMap) \
	etype(CanUseLoadoutItem) \
	etype(CanUseCharacter) \
	etype(ApproveLogin) \
	etype(UGameplay__IsDedicatedServer) \
	etype(InternalGetNetMode) \
	etype(ClientMessage) \
	etype(PreLogin) \
	etype(FString_AppendChars) \
	etype(GetOwnershipFromPlayerControllerAndState) \
	etype(ExecuteConsoleCommand) \
	etype(GetTBLGameMode) \
	etype(FText_AsCultureInvariant) \
	etype(BroadcastLocalizedChat) \
	etype(ConditionalInitializeCustomizationOnServer) \
    etype(UNetDriver__GetNetMode) \
	etype(MaxFuncType) //this always needs to be last

#define etype(x) uint32_t x = 0x0;


#define etype(x) F_##x,
typedef enum { FUNCTYPES }  FuncType;
#undef etype
#define etype(x) #x,
static const char* strFunc[F_MaxFuncType + 1] = { FUNCTYPES };

static const char* signatures[F_MaxFuncType + 1] =
{
	/*"FViewport"*/
	"48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 48 83 EC 30 33 ED C7 41 ?? FF FF FF FF",
	/*"SendRequest"*/ 
	"48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9",
	/*"GetMotd"*/ 
	"4C 89 4C 24 ?? 4C 89 44 24 ?? 48 89 4C 24 ?? 55 56 57 41 54 48 8D 6C 24 ?? 48 81 EC D8 00 00 00 83 79 ?? 01 4C 8B E2 48 8B \
	F9 7F ?? 33 F6 48 8B C2 48 89 32 48 89 72 ?? 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 ?? ?? ?? ?? 48 8D 55 ?? 4C \
	89 AC 24 ?? ?? ?? ?? 4C 89 B4 24 ?? ?? ?? ?? 4C 89 BC 24 ?? ?? ?? ?? E8 ?? ?? ?? ?? 4C 8B 6D ?? 48 8D 4D ?? 33 F6 48 89 75 \
	?? 48 89 75 ?? 49 8B 45 00 8D 56 ?? 48 8B 40 ?? 48 89 45 ?? E8 ?? ?? ?? ?? 8B 55 ?? 8D 5A ?? 89 5D ?? 3B 5D ?? 7E ?? 48 8D \
	4D ?? E8 ?? ?? ?? ?? 8B 5D ?? 4C 8B 75 ?? 48 8D 15 ?? ?? ?? ?? 49 8B CE 41 B8 12 00 00 00",
	/*"GetCurrentGames"*/ 
	"4C 89 4C 24 ?? 4C 89 44 24 ?? 48 89 4C 24 ?? 55 56 57 41 54 48 8D 6C 24 ?? 48 81 EC D8 00 00 00 83 79 ?? 01 4C 8B E2 48 8B \
	F9 7F ?? 33 F6 48 8B C2 48 89 32 48 89 72 ?? 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 ?? ?? ?? ?? 48 8D 55 ?? 4C \
	89 AC 24 ?? ?? ?? ?? 4C 89 B4 24 ?? ?? ?? ?? 4C 89 BC 24 ?? ?? ?? ?? E8 ?? ?? ?? ?? 4C 8B 6D ?? 48 8D 4D ?? 33 F6 48 89 75 \
	?? 48 89 75 ?? 49 8B 45 00 8D 56 ?? 48 8B 40 ?? 48 89 45 ?? E8 ?? ?? ?? ?? 8B 55 ?? 8D 5A ?? 89 5D ?? 3B 5D ?? 7E ?? 48 8D \
	4D ?? E8 ?? ?? ?? ?? 8B 5D ?? 4C 8B 75 ?? 48 8D 15 ?? ?? ?? ?? 49 8B CE 41 B8 22 00 00 00 E8 ?? ?? ?? ?? 8B 4F ?? 83 F9 01 \
	7F ?? 4D 8B FE 4C 8B F6 EB ?? 85 DB 74 ?? 44 8D 7B ?? 41 8B C7 EB ?? 44 8B FE 8B C6 48 8B 5D ?? 8D 14 ?? 48 8B F9 48 89 75 \
	?? 45 33 C0 89 7D ?? 48 8D 4D ?? 48 8B 1B E8 ?? ?? ?? ?? 48 8B 4D ?? 4C 8D 04 ?? 48 8B D3 E8 ?? ?? ?? ?? 45 8B C7 48 8D 4D \
	?? 49 8B D6 E8 ?? ?? ?? ?? 4C 8B 7D ?? 8B 5D ?? 48 89 75 ?? 48 89 75 ?? 48 89 75 ?? 8B D6 89 55 ?? 8B CE 89 4D ?? 85 DB 74 \
	?? 49 8B FF 4D 85 FF 74 ?? EB ?? 48 8D 3D ?? ?? ?? ?? 66 39 37 74 ?? 48 C7 C3 FF FF FF FF 48 FF C3 66 39 34 ?? 75 ?? FF C3 \
	85 DB 7E ?? 8B D3 48 8D 4D ?? E8 ?? ?? ?? ?? 8B 4D ?? 8B 55 ?? 8D 04 ?? 89 45 ?? 3B C1 7E ?? 48 8D 4D ?? E8 ?? ?? ?? ?? 48 \
	8B 4D ?? 48 8B D7 4C 63 C3 4D 03 C0 E8 ?? ?? ?? ?? 48 8D 55 ?? 49 8B CD FF 55 ?? 48 8B 4D ?? 48 85 C9 74 ?? E8 ?? ?? ?? ?? \
	4D 85 FF 74 ?? 49 8B CF E8 ?? ?? ?? ?? 4D 85 F6 74 ?? 49 8B CE E8 ?? ?? ?? ?? 4C 8B 75 ?? 8B CE 49 83 C6 10 89 4D ?? 45 8B \
	56 ?? 49 8D 7E ?? C7 45 ?? 01 00 00 00 44 8B C6 48 89 7D ?? 41 BB 1F 00 00 00 C7 45 ?? FF FF FF FF 48 89 75 ?? 45 85 D2 74 \
	?? 48 8B 47 ?? 4C 8B CF 48 85 C0 4C 0F 45 C8 41 8D 42 ?? 99 41 23 D3 8D 1C ?? 41 8B 11 C1 FB 05 85 D2 75 ?? 0F 1F 80 00 00 \
	00 00 FF C1 41 83 C0 20 89 4D ?? 44 89 45 ?? 3B CB 7F ?? 48 63 C1 C7 45 ?? FF FF FF FF 41 8B 14 ?? 85 D2 74 ?? 8B C2 F7 D8 \
	23 C2 0F BD C8 89 45 ?? 74 ?? 41 8B C3 2B C1 EB ?? B8 20 00 00 00 44 2B C0 41 8D 40 ?? 89 45 ?? 41 3B C2 7E ?? 44 89 55 ?? \
	41 8B 56 ?? 41 BD FF FF FF FF 0F 10 55 ?? 8B CA 4C 89 75 ?? 0F 10 45 ?? 41 23 CB 44 8B C2 0F 11 55 ?? 41 D3 E5 44 8B CA 0F \
	11 45 ?? 41 C1 F8 05 41 83 E1 E0 66 0F 15 D2 45 8B FA F2 0F 11 55 ?? 44 89 6D ?? 89 55 ?? 0F 10 45 ?? 0F 10 4D ?? 0F 11 45 \
	?? 0F 11 4D ?? 41 3B D2 74 ?? 48 8B 47 ?? 4C 8B D7 48 85 C0 49 63 C8 4C 0F 45 D0 41 8D 47 ?? 99 41 23 D3 8D 1C ?? 41 8B 14 \
	?? C1 FB 05 41 23 D5 75 ?? 41 FF C0 41 83 C1 20 44 3B C3 7F ?? 49 63 C0 C7 45 ?? FF FF FF FF 41 8B 14 ?? 85 D2 74 ?? 8B C2 \
	F7 D8 23 C2 0F BD C8 74 ?? 44 2B D9 EB ?? 41 BB 20 00 00 00 45 2B CB 41 8D 41 ?? 89 45 ?? 41 3B C7 7E ?? 44 89 7D ?? 48 8B \
	5D ?? 4C 8B BC 24 ?? ?? ?? ?? 4C 8B AC 24 ?? ?? ?? ?? 48 C1 EB 20 48 63 45 ?? 48 8B 55 ?? 3B C3 75 ?? 48 39 7D ?? 75 ?? 49 \
	3B D6 74 ?? 48 8D 0C ?? 48 8B 02 48 8D 14 ?? 48 8B 4D ?? 4C 8D 42 ?? 48 8B 01 FF 50 ?? 8B 45 ?? 48 8D 4D ?? F7 D0 21 45 ?? \
	E8 ?? ?? ?? ?? EB ?? 48 8B 4D ?? 48 8D 55 ?? E8 ?? ?? ?? ?? 48 8B 4D ?? 48 8B 01 FF 90 ?? ?? ?? ?? 4C 8B B4 24 ?? ?? ?? ?? \
	4C 8D 45 ?? 48 8B D8 48 8B CE 48 8B 45 ?? 8B D6 48 89 4D ?? 89 55 ?? 49 3B C0 74 ?? 39 48 ?? 74 ?? 4C 8B 00 4D 85 C0 74 ?? \
	49 8B 00 48 8D 55 ?? 49 8B C8 FF 50 ?? 8B 55 ?? 48 8B 4D ?? 48 89 75 ?? 89 75 ?? 85 D2 74 ?? 48 85 C9 74 ?? 48 8B 01 48 8D \
	55 ?? FF 50 ?? 48 8B 55 ?? 4C 8D 4D ?? 4C 8D 05 ?? ?? ?? ?? 48 8D 4D ?? E8 ?? ?? ?? ?? 48 8B D0 48 8B CB E8 ?? ?? ?? ?? 39 \
	75 ?? 74 ?? 48 8B 4D ?? 48 85 C9 74 ?? 48 8B 01 33 D2 FF 50 ?? 48 8B 45 ?? 48 85 C0 74 ?? 45 33 C0 33 D2 48 8B C8 E8 ?? ?? \
	?? ?? 48 89 45 ?? 89 75 ?? EB ?? 48 8B 45 ?? 48 85 C0 74 ?? 48 8B C8 E8 ?? ?? ?? ?? 8B 45 ?? 48 8B 4D ?? 85 C0 74 ?? 48 85 \
	C9 75 ?? 85 C0 74 ?? 48 85 C9 74 ?? 48 8B 01 33 D2 FF 50 ?? 48 8B 4D ?? 48 85 C9 74 ?? 45 33 C0 33 D2 E8 ?? ?? ?? ?? 48 8B \
	C8 48 89 45 ?? 89 75 ?? 48 85 C9 74 ?? E8 ?? ?? ?? ?? 48 8B 4D ?? 48 8B 01 FF 90 ?? ?? ?? ?? 48 8B 45 ?? 49 89 04 24 48 8B \
	45 ?? 49 89 44 24 ?? 48 85 C0 74 ?? FF 40 ?? 48 8B 5D ?? 48 85 DB 74 ?? 83 6B ?? 01 75 ?? 48 8B 03 48 8B CB FF 10 83 6B ?? \
	01 75 ?? 48 8B 03 BA 01 00 00 00 48 8B CB FF 50 ?? 48 8B 9C 24 ?? ?? ?? ?? 49 8B C4 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3",
	/*"IsNonPakFilenameAllowed"*/ 
	"48 89 5C 24 ?? 48 89 6C 24 ?? 56 57 41 56 48 83 EC 30 48 8B F1 45 33 C0",
	/*"FindFileInPakFiles_1"*/ 
	"48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 41 54 41 55 41 56 41 57 48 83 EC 30 33 FF",
	/*"FindFileInPakFiles_2"*/ 
	"48 8B C4 4C 89 48 ?? 4C 89 40 ?? 48 89 48 ?? 55 53 48 8B EC",
	/*"UTBLLocalPlayer::Exec"*/
	"75 18 ?? ?? ?? ?? 75 12 4d 85 f6 74 0d 41 38 be ?? ?? ?? ?? 74 04 32 db eb 9b 48 8b 5d 7f 49 8b d5 4c 8b 45 77 4c 8b cb 49 8b cf",
	/*"GetGameInfo"*/
	"48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00",
	/*ConsoleCommand*/
	"40 53 48 83 EC 20 48 8B 89 D0 02 00 00 48 8B DA 48 85 C9 74 0E E8 ?? ?? ?? ?? 48 8B C3 48 83 C4 20 5B C3 33 C0 48 89 02 48 \
	89 42 08 48 8B C3 48 83 C4 20 5B C3",
	/*LoadFrontEndMap*/
	"48 8B C4 48 89 50 10 48 89 48 08 55 41 55 48 8D 68 98 48 81 EC 58 01 00 00 83 7A 08 00",
	/*ATBLPlayerController::CanUseLoadoutItem*/
	"48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00",
	/*ATBLPlayerController::CanUseCharacter*/
	"48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 50 49 8B 18",
	/*ApproveLogin*/
	"48 89 5C 24 18 48 89 74 24 20 55 57 41 54 41 55 41 56 48 8D 6C 24 C9 48 81 EC A0 00 00 00 8B",
	/*UGameplay__IsDedicatedServer*/
	"48 83 EC 28 48 85 C9 ? ? BA 01 00 00 00 ? ? ? ? ? 48 85 C0 ? ? 48 8B C8 ? ? ? ? ? 83 F8 01 0F 94 C0 48",
	/*InternalGetNetMode*/
	"40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9 ? ? 48 81 C4 90 00 00 00 5B ? ? ? ? ? 48 8B 8B F0 00 00 00 48",
	/*ClientMessage*/
	"4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43",
	/*PreLogin*/
	"4C 89 4C 24 20 48 89 54 24 10 48 89 4C 24 08 55 53 57 41 55 41 57 48 8D 6C",
	/*FString::AppendChars*/
	"45 85 C0 0F 84 89 00 00 00 48 89 5C 24 18 48 89 6C 24 20 56 48 83 EC 20 48 89 7C 24 30 48 8B EA 48 63 79 08 48 8B D9 4C 89 74 24 38 45 33 F6 85 FF 49 63 F0 41 8B C6 0F 94 C0 03 C7 03 C6 89 41 08 3B 41 0C 7E 07 8B D7 E8 ?? ?? ?? ?? 85 FF 49 8B C6 48 8B CF 48 8B D5 0F 95 C0 48 2B C8 48 8B 03 48 8D 1C 36 4C 8B C3 48 8D 3C 48 48 8B CF E8 ?? ?? ?? ?? 48 8B 6C 24 48 66 44 89 34 3B 4C 8B 74 24 38 48 8B 7C 24 30 48 8B 5C 24 40 48 83 C4 20 5E C3",
	/*ATBLPlayerController::GetOwnershipFromPlayerControllerAndState*/
	"40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD",
	/*ExecuteConsoleCommand*/
	"40 53 48 83 EC 30 48 8B 05 ? ? ? ? 48 8B D9 48 8B 90 58 0C 00 00",
	/*GetTBLGameMode*/
	"40 53 48 83 EC 20 48 8B D9 48 85 C9 ? ? 48 8B 01 ? ? ? ? ? ? 48 85 C0 ? ? 0F 1F 40 00 48 8B 5B 20 48 85 DB ? ? 48 8B 03 48 8B CB ? ? ? ? ? ? 48 85 C0 ? ? 48 8B 98 28 01 00 00 48 85 DB ? ? ? ? ? ? ? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51",
	/*FText_AsCultureInvariant*/
	"48 89 5C 24 18 48 89 74 24 20 41 56 48 83 EC 60 33 C0 48 89 7C 24 78 48 63",
	/*BroadcastLocalizedChat*/
	"48 89 74 24 10 57 48 83 EC 30 48 8B 01 41 8B F8 48 8B F2 ? ? ? ? ? ? 48 8B C8 48 8D",
	/*ATBLPlayerController::ConditionalInitializeCustomizationOnServer*/
	"48 89 54 24 10 53 56 57 41 54 48 83 EC 78 48 8B 99 60 02 00 00 48 8B F2 0F B6",
	/*UNetDriver::GetNetMode*/
	"48 83 EC 28 48 8B 01 ?? ?? ?? ?? ?? ?? 84 C0 ?? ?? 33 C0 38 ?? ?? ?? ?? 02 0F 95 C0 FF C0 48 83 C4",
	/*MaxFuncType*/
	""
};