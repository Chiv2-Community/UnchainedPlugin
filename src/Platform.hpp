#pragma once

#include <map>
#include <string>

enum Platform {
    STEAM,
    EGS,
    XBOX
};

inline std::map<std::wstring, Platform> string_to_platform = {
    {L"STEAM", STEAM},
    {L"EGS", EGS},
    {L"XBOX", XBOX}
};

inline std::map<Platform, std::wstring> platform_to_string = {
    {STEAM, L"STEAM"},
    {EGS, L"EGS"},
    {XBOX, L"XBOX"}
};
