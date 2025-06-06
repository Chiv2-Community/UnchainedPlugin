#pragma once

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "tiny-json.h"
#include "../Platform.hpp"

/**
 * Build metadata is loaded from a json file at start, and then any unknown
 * signatures are added as they are found.
 */
class BuildMetadata {
    uint32_t fileHash = 0;
    std::map<std::string, uintptr_t> offsets = {};
    Platform platform;
public:
    BuildMetadata(uint32_t fileHash, std::map<std::string, uint64_t> offsets, Platform platform);
    ~BuildMetadata();

    static std::optional<BuildMetadata> Parse(const json_t* json);

    void SetOffset(std::string name, uintptr_t offset);
    std::optional<uintptr_t> GetOffset(const std::string& name) const;
    std::vector<std::pair<std::string, uintptr_t>> GetOffsets() const;

    void SetFileHash(uint32_t hash);
    uint32_t GetFileHash() const;

    std::string GetBuildKey() const;

    Platform GetPlatform() const;
};
