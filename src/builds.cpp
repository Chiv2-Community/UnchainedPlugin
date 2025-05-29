#include "builds.h"

#include <cstring>
#include <stdint.h>
#include <iostream>
#include <fstream>
#include <string>
#include <filesystem>
#include <tiny-json.h>
#include <nmmintrin.h> // SSE4.2 intrinsics
#include <functional>

#include "logging/global_logger.hpp"
#include "state/global_state.hpp"

uint32_t calculateCRC32(const std::string& filename) {
	std::ifstream file(filename, std::ios::binary);
	if (!file.is_open()) {
		std::cerr << "Error opening file: " << filename << std::endl;
		return 0;
	}

	uint32_t crc = 0; // Initial value for CRC-32

	char buffer[4096];
	while (file) {
		file.read(buffer, sizeof(buffer));
		std::streamsize bytesRead = file.gcount();

		for (std::streamsize i = 0; i < bytesRead; ++i) {
			crc = _mm_crc32_u8(crc, buffer[i]);
		}
	}

	file.close();
	return crc ^ 0xFFFFFFFF; // Final XOR value for CRC-32
}

bool needsSerialization = true;

std::filesystem::path getConfigPath() {
    const char* localAppData = std::getenv("LOCALAPPDATA");
    return std::filesystem::path(localAppData) / 
           "Chivalry 2" / "Saved" / "Config" / "c2uc.builds.json";
}

bool SaveBuildMetadata(const std::map<std::string, BuildMetadata>& builds)
{
	auto configPath = getConfigPath();
	if (!std::filesystem::exists(configPath.parent_path())) {
		std::filesystem::create_directories(configPath.parent_path());
	}

	std::stringstream out("");
	out << "{";

	// First, serialize the current build from global state
	auto currentBuild = g_state->GetBuildMetadata();
	auto serialized = currentBuild.Serialize(1);
	if (!serialized.has_value()) {
		GLOG_ERROR("Failed to serialize current build");
		return false;
	}
	out << serialized.value();

	// Add builds from the provided map
	if (!builds.empty()) {
		bool firstEntry = true;
		for (const auto& [buildName, buildData] : builds) {
			// Skip the current build if it's already in the map
			if (buildData.GetFileHash() == currentBuild.GetFileHash() && 
				buildData.GetBuildId() == currentBuild.GetBuildId()) {
				continue;
			}
			
			if (firstEntry) {
				out << ",";
				firstEntry = false;
			} else {
				out << ",";
			}
			
			auto buildSerialized = buildData.Serialize(1);
			if (buildSerialized.has_value()) {
				out << buildSerialized.value();
			} else {
				GLOG_WARNING("Failed to serialize build: {}", buildName);
			}
		}
	}

	out << "}";

	std::ofstream file(configPath);
	if (!file.is_open()) {
		GLOG_ERROR("Error opening build config: {}", configPath);
		return false;
	}

	file << out.str();
	GLOG_INFO("Successfully saved build config to: {}", configPath.string());
	return true;
}

std::map<std::string, BuildMetadata> LoadBuildMetadata()
{
    auto configPath = getConfigPath();
    std::map<std::string, BuildMetadata> buildMap;
    
	GLOG_DEBUG("Loading build config from: {}", configPath.string());

    if (!std::filesystem::exists(configPath)) {
        GLOG_WARNING("Config file ({}) does not exist. This is normal on first start.", configPath);
        return buildMap;
    }
    
    std::ifstream file(configPath);
    if (!file.is_open()) {
        GLOG_ERROR("Error opening build config: {}", configPath);
        return buildMap;
    }
    
    std::string file_content{
        std::istreambuf_iterator<char>(file), 
        std::istreambuf_iterator<char>()
    };
    
    GLOG_DEBUG("File content preview: {}", file_content);
    
    json_t mem[128];
    const json_t* json = json_create(const_cast<char *>(file_content.c_str()), mem, 128);

	if (!json) {
		GLOG_ERROR("Failed to create json parser");
		return buildMap;
	}
	uint32_t curFileHash = calculateCRC32("Chivalry2-Win64-Shipping.exe");

	json_t const* buildEntry;
	needsSerialization = true;
	buildEntry = json_getChild(json);
	while (buildEntry != nullptr) {
		if (JSON_OBJ == json_getType(buildEntry)) {
			const char* buildName = json_getName(buildEntry);
			GLOG_DEBUG("Parsing build: {}", buildName);

            // Parse the build metadata
            auto parsedBuild = BuildMetadata::Parse(buildEntry, buildName);
            if (!parsedBuild.has_value()) {
                GLOG_ERROR("Failed to parse build metadata for {}", buildName);
                buildEntry = json_getSibling(buildEntry);
                continue;
            }

            BuildMetadata bd = parsedBuild.value();
            
            // Add the parsed build to the map
            buildMap[buildName] = bd;

            // Check if this build matches the current executable
            bool hashMatch = bd.GetFileHash() == curFileHash;

            if (hashMatch) {
                g_state->SetBuildMetadata(bd);
                needsSerialization = false;
                GLOG_INFO("Found matching build: {}", buildName);
            }
		}
		buildEntry = json_getSibling(buildEntry);
	}

	return buildMap;
}