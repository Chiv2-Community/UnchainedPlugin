#pragma once

#include <cstdint>
#include <string>
#include <map>

#include "state/BuildMetadata.hpp"

uint32_t calculateCRC32(const std::string& filename);

extern bool needsSerialization;

// Serializes builds to a JSON config file
bool SaveBuildMetadata(const std::map<std::string, BuildMetadata>& builds);
// Loads builds from a JSON config file
std::map<std::string, BuildMetadata> LoadBuildMetadata();