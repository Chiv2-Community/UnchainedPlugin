#pragma once

#include <cstdint>
#include <string>
#include <map>

#include "state/BuildMetadata.hpp"

// Loads builds via Sleuth memory
std::map<std::string, BuildMetadata> LoadBuildMetadata();