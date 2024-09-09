#pragma once

#include "UE4.h"

struct GetMotdRequest {
	uint8_t ph[0xD8];
	FString token;
};

// Technically the same as GetMotdRequest for now. Just figure they should be decoupled.
struct GetCurrentGamesRequest {
	uint8_t ph[0xD8];
	FString token;
};

struct FOwnershipResponse {
	bool owned;
	int32_t crowns;
	int32_t gold;
	int32_t usdCents;
	uint8_t levelType;
	int32_t level;
};

struct UTBLCharacterMovement {};

struct ATBLPlayerController { 
	uint8_t  ph[0x1328];
	bool bOnlineInventoryInitialized;
	bool bPlayerCustomizationReceived;
};

struct ATBLGameMode { };

enum EOnlineStat
{
	OS_Invalid = 0,
	OS_Default = 1,
	OS_Store = 2,
	OS_Unknown = 3,
	OS_Campaign = 4,
	OS_Playtime = 5,
	OS_GlobalXp = 6,
	OS_ExperienceFootman = 7,
	OS_ExperienceArcher = 8,
	OS_ExperienceKnight = 9,
	OS_ExperienceVanguard = 10,
	OS_ExperienceWeaponAxe = 11,
	OS_ExperienceWeaponBastardSword = 12,
	OS_ExperienceWeaponBattleAxe = 13,
	OS_ExperienceWeaponCudgel = 14,
	OS_ExperienceWeaponDagger = 15,
	OS_ExperienceWeaponDaneAxe = 16,
	OS_ExperienceWeaponGreatsword = 17,
	OS_ExperienceWeaponHalberd = 18,
	OS_ExperienceWeaponHeavyMace = 19,
	OS_ExperienceWeaponLance = 20,
	OS_ExperienceWeaponMace = 21,
	OS_ExperienceWeaponMaul = 22,
	OS_ExperienceWeaponMesser = 23,
	OS_ExperienceWeaponPoleAxe = 24,
	OS_ExperienceWeaponPoleHammer = 25,
	OS_ExperienceWeaponShortSword = 26,
	OS_ExperienceWeaponSpear = 27,
	OS_ExperienceWeaponSword = 28,
	OS_ExperienceWeaponWarHammer = 29,
	OS_ExperienceWeaponBow = 30,
	OS_ExperienceWeaponCrossbow = 31,
	OS_ExperienceWeaponThrowingAxe = 32,
	OS_ExperienceWeaponThrowingKnife = 33,
	OS_ExperienceWeaponHeavyShield = 34,
	OS_ExperienceWeaponLightShield = 35,
	OS_ExperienceWeaponMediumShield = 36,
	OS_ExperienceWeaponJavelin = 37,
	OS_ExperienceWeaponFalchion = 38,
	OS_ExperienceWeaponShovel = 39,
	OS_ExperienceWeaponSledgeHammer = 40,
	OS_ExperienceWeaponPickAxe = 41,
	OS_ExperienceWeaponTwoHandedHammer = 42,
	OS_ExperienceWeaponWarClub = 43,
	OS_ExperienceWeaponExecutionersAxe = 44,
	OS_ExperienceWeaponGlaive = 45,
	OS_ExperienceWeaponKnife = 46,
	OS_ExperienceWeaponWarAxe = 47,
	OS_ExperienceWeaponMorningStar = 48,
	OS_ExperienceWeaponHatchet = 49,
	OS_ExperienceWeaponOneHandedSpear = 50,
	OS_ExperienceWeaponThrowingMallet = 51,
	OS_ExperienceWeaponRapier = 52,
	OS_ExperienceWeaponHighlandSword = 53,
	OS_ExperienceWeaponWarBow = 54,
	OS_ExperienceWeaponHeavyCrossbow = 55,
	OS_ExperienceWeaponKatars = 56,
	OS_ExperienceWeaponArmouredFists = 57,
	OS_DailyPlaytime = 58,
	OS_PROGRESSION_MAX = 59,
	OS_Kills = 60,
	OS_Deaths = 61,
	OS_Suicides = 62,
	OS_WinsAgatha = 63,
	OS_LossesAgatha = 64,
	OS_WinsMason = 65,
	OS_LossesMason = 66,
	OS_MatchesCompleted = 67,
	OS_MatchesCompletedTeamObjective = 68,
	OS_MatchesCompletedTeamDeathmatch = 69,
	OS_MatchesCompletedFreeForAll = 70,
	OS_MatchesCompletedArena = 71,
	OS_MatchesCompletedLastTeamStanding = 72,
	OS_MatchesCompletedBrawl = 73,
	OS_ItemsConstructed = 74,
	OS_Revives = 75,
	OS_DISUSED_A = 76,
	OS_DISUSED_B = 77,
	OS_DISUSED_C = 78,
	OS_DamageInflicted = 79,
	OS_Blocks = 80,
	OS_FFAFirst = 81,
	OS_FFASecond = 82,
	OS_FFAThird = 83,
	OS_MultiKills = 84,
	OS_KillingSprees = 85,
	OS_VoteKickSeverity = 86,
	OS_VoteKickTime = 87,
	OS_WinsTenosia = 88,
	OS_LossesTenosia = 89,
	OS_FLAVOR_MAX = 90,
};


