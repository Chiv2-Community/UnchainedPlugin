//
// Created by Fam on 5/27/2025.
//

#include "BuildMetadata.hpp"

#include <optional>
#include <sstream>

#include "../logging/global_logger.hpp"

BuildMetadata::~BuildMetadata() {}

void BuildMetadata::SetOffset(std::string name, uint64_t offset) {
    offsets[std::move(name)] = offset;
}

std::optional<uint64_t> BuildMetadata::GetOffset(const std::string &name) const {
    if (auto it = offsets.find(name); it != offsets.end()) {
        return it->second;
    }
    return std::nullopt;
}

std::vector<std::pair<std::string, uint64_t>> BuildMetadata::GetOffsets() const {
    std::vector<std::pair<std::string, uint64_t>> result;
    result.reserve(offsets.size());

    for (const auto& [name, offset] : offsets) {
        result.emplace_back(name, offset);
    }

    return result;
}

void BuildMetadata::SetFileHash(uint32_t hash) {
    fileHash = hash;
}

uint32_t BuildMetadata::GetFileHash() const {
    return fileHash;
}

void BuildMetadata::SetBuildId(uint32_t id) {
    buildId = id;
}

uint32_t BuildMetadata::GetBuildId() const {
    return buildId;
}

void BuildMetadata::SetName(std::string* newName) {
    if (newName) {
        nameStr = *newName;
    }
}

void BuildMetadata::SetName(std::wstring* newName) {
    if (newName) {
        nameStr = std::string(newName->begin(), newName->end());
    }
}

std::string BuildMetadata::GetName() const {
    return nameStr;
}

std::string BuildMetadata::Serialize() const {
    std::stringstream ss;

    // Format: FileHash|BuildId|Name|OffsetCount|OffsetName1:OffsetValue1|OffsetName2:OffsetValue2|...
    ss << fileHash << "|" << buildId << "|" << nameStr << "|" << offsets.size();

    for (const auto& [name, offset] : offsets) {
        ss << "|" << name << ":" << offset;
    }

    return ss.str();
}
