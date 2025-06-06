#pragma once

#include <cstdint>
#include <string>
#include <map>

#include "state/BuildMetadata.hpp"

uint32_t calculateCRC32(const std::string& filename);

// Loads builds from a JSON config file
std::map<std::string, BuildMetadata> LoadBuildMetadata();