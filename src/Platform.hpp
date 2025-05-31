#pragma once

#include <map>
#include <string>

enum Platform {
    STEAM,
    EGS,
    XBOX
};

inline std::map<std::string, Platform> string_to_platform = {
    {"STEAM", STEAM},
    {"EGS", EGS},
    {"XBOX", XBOX}
};

inline std::map<Platform, std::string> platform_to_string = {
    {STEAM, "STEAM"},
    {EGS, "EGS"},
    {XBOX, "XBOX"}
};
