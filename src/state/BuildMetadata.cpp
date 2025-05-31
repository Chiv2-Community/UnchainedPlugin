//
// Created by Fam on 5/27/2025.
//

#include "BuildMetadata.hpp"

#include <optional>
#include <sstream>

#include <tiny-json.h>

#include "../logging/global_logger.hpp"
#include "../string_util.hpp"

BuildMetadata::BuildMetadata(uint32_t fileHash, uint32_t buildId, std::map<std::string, uint64_t> offsets,
    std::string nameStr) {
    this->fileHash = fileHash;
    this->buildId = buildId;
    this->offsets = std::move(offsets);
    this->nameStr = std::move(nameStr);
}

BuildMetadata::~BuildMetadata() {}

void BuildMetadata::SetOffset(std::string name, uint64_t offset) {
    if (offset == 0) return;
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

void BuildMetadata::SetName(const std::string &newName) {
    if (!newName.empty()) {
        nameStr = newName;
    }
}

void BuildMetadata::SetName(std::wstring newName) {
    if (!newName.empty()) {
        nameStr = std::format("{}", newName);
    }
}

std::string BuildMetadata::GetName() const {
    return nameStr;
}

std::string BuildMetadata::GetBuildKey() const {
    return std::format("{}", this->GetFileHash());
}


std::optional<std::string> BuildMetadata::Serialize(int indent) const {
    std::stringstream ss;

    if (this->GetFileHash() == 0) {
        GLOG_WARNING("No file hash set. Cannot serialize build");
        return std::nullopt;
    }

    ss << ws(indent  ) << quot << this->GetBuildKey() << quot << ": {"
       << ws(indent+1) << quot << "Build"             << quot << ": " << this->GetBuildId() << ","
       << ws(indent+1) << quot << "FileHash"          << quot << ": " << this->GetFileHash() << ","
       << ws(indent+1) << quot << "Name"              << quot << ": " << quot << this->GetName() << quot << ","
       << ws(indent+1) << quot << "Offsets"           << quot << ": {";

    auto offsets = this->GetOffsets();
    const auto offsets_length = offsets.size();
    for (size_t i = 0; i < offsets_length; i++) {
        auto [name, offset] = offsets[i];
        ss << ws(indent + 2) << quot << name << quot << ": " << offset;
        if (i != offsets_length - 1) {
            ss << ",";
        }
    }

    ss << ws(indent+1) << "}"
       << ws(indent) << "}";

    return ss.str();
}

std::optional<BuildMetadata> BuildMetadata::Parse(const json_t* json) {
    if (!json) {
        GLOG_ERROR("Invalid JSON object or build name");
        return std::nullopt;
    }
    
    const json_t* buildNameJson = json_getProperty(json, "Name");
    if (!buildNameJson || JSON_TEXT != json_getType(buildNameJson)) {
        GLOG_WARNING("Error, the 'Name' property is not found or not a string. Serializing build without it.");
    }

    std::string buildName(json_getValue(buildNameJson));

    // Get Build ID
    const json_t* buildIdJson = json_getProperty(json, "Build");
    if (!buildIdJson || JSON_INTEGER != json_getType(buildIdJson)) {
        GLOG_WARNING("Error, the 'Build' property is not found or not an integer. Serializing build without it.");
    }
    auto buildId = static_cast<uint32_t>(json_getInteger(buildIdJson));

    // Get File Hash
    const json_t* fileHashJson = json_getProperty(json, "FileHash");
    if (!fileHashJson || JSON_INTEGER != json_getType(fileHashJson)) {
        GLOG_ERROR("Error, the 'FileHash' property is not found or not an integer");
        return std::nullopt;
    }
    auto file_hash = static_cast<uint32_t>(json_getInteger(fileHashJson));

    const json_t* offsetsJson = json_getProperty(json, "Offsets");
    if (!offsetsJson || JSON_OBJ != json_getType(offsetsJson)) {
        GLOG_ERROR("Error, the 'Offsets' property is not found or not an object");
        return std::nullopt;
    }

    std::map<std::string, uint64_t> offsets;
    for (const json_t* offsetEntry = json_getChild(offsetsJson); offsetEntry != nullptr; offsetEntry = json_getSibling(offsetEntry)) {
        if (JSON_INTEGER == json_getType(offsetEntry)) {
            const char* offsetName = json_getName(offsetEntry);
            if (offsetName && strlen(offsetName) > 0) {
                uint64_t offsetValue = static_cast<uint64_t>(json_getInteger(offsetEntry));
                offsets.emplace(offsetName, offsetValue);
            }
        }
    }

    BuildMetadata metadata(file_hash, buildId, std::move(offsets), std::move(buildName));

    return metadata;
}