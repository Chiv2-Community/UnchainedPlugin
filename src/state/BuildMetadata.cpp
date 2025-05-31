//
// Created by Fam on 5/27/2025.
//

#include "BuildMetadata.hpp"

#include <optional>
#include <sstream>

#include <tiny-json.h>

#include "../logging/global_logger.hpp"
#include "../string_util.hpp"

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

void BuildMetadata::SetName(std::string* newName) {
    if (newName) {
        nameStr = *newName;
    }
}

void BuildMetadata::SetName(std::wstring* newName) {
    if (newName) {
        nameStr = std::format("{}", newName->c_str());
    }
}

std::string BuildMetadata::GetName() const {
    return nameStr;
}

std::optional<std::string> BuildMetadata::Serialize(int indent) const {
    std::stringstream ss;

    if (this->GetBuildId() == 0) {
        GLOG_WARNING("No build ID set, cannot serialize build");
        return std::nullopt;
    }

    if (this->GetName().empty()) {
        GLOG_WARNING("No build name set, cannot serialize build");
        return std::nullopt;
    }

    ss << ws(indent  ) << quot << this->GetName() << quot << ": {"
       << ws(indent+1) << quot << "Build"         << quot << ": " << this->GetBuildId() << ","
       << ws(indent+1) << quot << "FileHash"      << quot << ": " << this->GetFileHash() << ","
       << ws(indent+1) << quot << "Offsets"       << quot << ": {";

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
       << ws(indent  ) << "}";

    return ss.str();
}

std::optional<BuildMetadata> BuildMetadata::Parse(const json_t* json, const char* buildName) {
    // Create a new BuildMetadata instance
    BuildMetadata metadata;
    
    if (!json || !buildName) {
        GLOG_ERROR("Invalid JSON object or build name");
        return std::nullopt;
    }
    
    // Set the build name
    std::string name(buildName);
    metadata.SetName(&name);
    
    // Get Build ID
    const json_t* buildIdJson = json_getProperty(json, "Build");
    if (!buildIdJson || JSON_INTEGER != json_getType(buildIdJson)) {
        GLOG_ERROR("Error, the 'Build' property is not found or not an integer");
        return std::nullopt;
    }
    metadata.SetBuildId(static_cast<uint32_t>(json_getInteger(buildIdJson)));
    
    // Get File Hash
    const json_t* fileHashJson = json_getProperty(json, "FileHash");
    if (!fileHashJson || JSON_INTEGER != json_getType(fileHashJson)) {
        GLOG_ERROR("Error, the 'FileHash' property is not found or not an integer");
        return std::nullopt;
    }
    metadata.SetFileHash(static_cast<uint32_t>(json_getInteger(fileHashJson)));
    
    // Get Offsets
    const json_t* offsetsJson = json_getProperty(json, "Offsets");
    if (!offsetsJson || JSON_OBJ != json_getType(offsetsJson)) {
        GLOG_ERROR("Error, the 'Offsets' property is not found or not an object");
        return std::nullopt;
    }
    
    // Parse all offsets
    for (const json_t* offsetEntry = json_getChild(offsetsJson); offsetEntry != nullptr; offsetEntry = json_getSibling(offsetEntry)) {
        if (JSON_INTEGER == json_getType(offsetEntry)) {
            const char* offsetName = json_getName(offsetEntry);
            if (offsetName && strlen(offsetName) > 0) {
                uint64_t offsetValue = static_cast<uint64_t>(json_getInteger(offsetEntry));
                metadata.SetOffset(offsetName, offsetValue);
            }
        }
    }
    
    return metadata;
}