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
 *
 * The FViewport hook finds and sets the fileHash and buildId, which are necessary
 * for loading the metadata from an existing config file.
 */
class BuildMetadata {
    uint32_t fileHash = 0;
    uint32_t buildId = 0;
    std::map<std::string, uintptr_t> offsets = {};
    std::string nameStr;
    Platform platform;
public:
    BuildMetadata(uint32_t fileHash, uint32_t buildId, std::map<std::string, uint64_t> offsets, std::string nameStr, Platform platform);
    ~BuildMetadata();

    std::optional<std::string> Serialize(int indent) const;
    static std::optional<BuildMetadata> Parse(const json_t* json);

    void SetOffset(std::string name, uintptr_t offset);
    std::optional<uintptr_t> GetOffset(const std::string& name) const;
    std::vector<std::pair<std::string, uintptr_t>> GetOffsets() const;

    void SetFileHash(uint32_t hash);
    uint32_t GetFileHash() const;

    void SetBuildId(uint32_t id);
    uint32_t GetBuildId() const;

    std::string GetBuildKey() const;

    void SetName(const std::string &newName);
    void SetName(std::wstring newName);
    std::string GetName() const;

    Platform GetPlatform() const;
};
