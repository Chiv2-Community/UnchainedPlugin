#include "builds.hpp"

#include <cstring>
#include <stdint.h>
#include <iostream>
#include <fstream>
#include <string>
#include <filesystem>
#include <tiny-json.h>
#include <nmmintrin.h> // SSE4.2 intrinsics
#include <functional>
#include <Windows.h>
#include <mutex>


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

std::filesystem::path getBuildMetadataPath() {
	char localAppData[MAX_PATH];
	DWORD result = GetEnvironmentVariableA("LOCALAPPDATA", localAppData, MAX_PATH);

	if (result == 0 || result > MAX_PATH) {
		// Handle error - environment variable not found or buffer too small
		GLOG_ERROR("Failed to get LOCALAPPDATA environment variable");
		return {};
	}

	return std::filesystem::path(localAppData) /
		   "Chivalry 2" / "Saved" / "Config" / "c2uc.builds.json";
}

static std::mutex g_saveBuildMutex;
bool SaveBuildMetadata(const std::map<std::string, BuildMetadata>& builds)
{
	std::lock_guard lock(g_saveBuildMutex);

	auto buildMetadataPath = getBuildMetadataPath();
	if (!std::filesystem::exists(buildMetadataPath.parent_path())) {
		std::filesystem::create_directories(buildMetadataPath.parent_path());
	}

	GLOG_DEBUG("Saving build metadata to: {}", buildMetadataPath.string());

	std::stringstream out("");
	out << "{";

	// Add builds from the provided map
	if (!builds.empty()) {
		bool isFirst = true;
		for (const auto& [buildName, buildData] : builds) {
			auto buildSerialized = buildData.Serialize(1);
			if (buildSerialized.has_value()) {
				if (!isFirst)
					out << ",";
				else
					isFirst = false;
				out << buildSerialized.value();
			} else {
				GLOG_WARNING("Failed to serialize build metadata: {}", buildName);
			}
		}
	}

	out << "\n}";

	std::ofstream file(buildMetadataPath);
	if (!file.is_open()) {
		GLOG_ERROR("Error opening build metadata: {}", buildMetadataPath);
		return false;
	}

	file << out.str();
	GLOG_INFO("Successfully saved build metadata to: {}", buildMetadataPath.string());
	return true;
}

std::map<std::string, BuildMetadata> LoadBuildMetadata()
{
	std::lock_guard lock(g_saveBuildMutex);

    auto configPath = getBuildMetadataPath();
    std::map<std::string, BuildMetadata> buildMap;
    
	GLOG_DEBUG("Loading build metadata from: {}", configPath.string());

    if (!std::filesystem::exists(configPath)) {
        GLOG_WARNING("Build metadata file ({}) does not exist. This is normal on first start.", configPath);
        return buildMap;
    }
    
    std::ifstream file(configPath);
    if (!file.is_open()) {
        GLOG_ERROR("Error opening build metadata: {}", configPath);
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
	json_t const* buildEntry;

	buildEntry = json_getChild(json);
	while (buildEntry != nullptr) {
		if (JSON_OBJ == json_getType(buildEntry)) {
			const auto buildKey = std::string(json_getName(buildEntry));
			GLOG_DEBUG("Parsing build metadata: {}", buildKey);

            // Parse the build metadata
            auto parsedBuild = BuildMetadata::Parse(buildEntry);
            if (!parsedBuild.has_value()) {
                GLOG_ERROR("Failed to parse build metadata for {}", buildKey);
                buildEntry = json_getSibling(buildEntry);
                continue;
            }

            BuildMetadata bd = parsedBuild.value();
			buildMap.emplace(buildKey, bd);
		}
		buildEntry = json_getSibling(buildEntry);
	}

	return buildMap;
}