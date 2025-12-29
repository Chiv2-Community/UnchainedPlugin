//
// Created by Fam on 5/27/2025.
//

#include "BuildMetadata.hpp"

#include <optional>
#include <sstream>

#include "../logging/global_logger.hpp"
#include "../string_util.hpp"

BuildMetadata::BuildMetadata(uint32_t fileHash, std::map<std::string, uint64_t> offsets, Platform platform) {
    this->fileHash = fileHash;
    this->offsets = std::move(offsets);
    this->platform = platform;
}

BuildMetadata::~BuildMetadata() {}

void BuildMetadata::SetOffset(std::string name, uint64_t offset) {
    if (offset == 0) return;
    offsets[std::move(name)] = offset;
}

std::optional<uintptr_t> BuildMetadata::GetOffset(const std::string &name) const {
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

std::string BuildMetadata::GetBuildKey() const {
    return std::format("{}", this->GetFileHash());
}

Platform BuildMetadata::GetPlatform() const {
    return platform;
}

extern "C" uint32_t get_file_hash();
extern "C" char* get_platform();
extern "C" size_t get_offset_count();
extern "C" char* get_offset_name(size_t index);
extern "C" uint64_t get_offset_value(size_t index);
extern "C" void free_string(char* s);

std::optional<BuildMetadata> BuildMetadata::FromSleuth() {
    uint32_t file_hash = get_file_hash();
    if (file_hash == 0) return std::nullopt;

    char* platform_str = get_platform();
    Platform platform = STEAM;
    if (string_to_platform.contains(platform_str)) {
        platform = string_to_platform.at(platform_str);
    }
    free_string(platform_str);

    size_t offset_count = get_offset_count();
    std::map<std::string, uint64_t> offsets;
    for (size_t i = 0; i < offset_count; ++i) {
        char* name = get_offset_name(i);
        uint64_t value = get_offset_value(i);
        if (name) {
            offsets[name] = value;
            free_string(name);
        }
    }

    return BuildMetadata(file_hash, std::move(offsets), platform);
}