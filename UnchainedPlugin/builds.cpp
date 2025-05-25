#include "builds.h"
#include <stdint.h>
#include <iostream>
#include <fstream>
#include <string>
#include <filesystem>
#include "logging.hpp"

#include "tiny-json.h"

#include <nmmintrin.h> // SSE4.2 intrinsics
#include <functional>
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

std::deque<BuildType*> configBuilds;
//BuildInfo* curBuildInfo = nullptr;
BuildType curBuild;
bool jsonDone = false;
bool offsetsLoaded = false;
bool needsSerialization = true;

const char* quot = "\"";

std::string ws(int indent) {
	return "\n" + std::string(indent * 2, ' ');
}

std::filesystem::path getConfigPath() {
    const char* localAppData = std::getenv("LOCALAPPDATA");
    return std::filesystem::path(localAppData) / 
           "Chivalry 2" / "Saved" / "Config" / "c2uc.builds.json";
}

bool serializeBuilds()
{
	std::function<bool(BuildType&, std::ofstream&, int)> SerializeBuild = [](BuildType& build, std::ofstream& out, int indent)
	{
		if (build.buildId == 0) {
			LOG_WARNING("No build ID set, cannot serialize build.\n");
			return false;
		}

		if (build.nameStr.length() == 0) {
			LOG_WARNING("No build name set, cannot serialize build.\n");
			return false;
		}

		const char* buildKey = build.nameStr.c_str();

		out	<< ws(indent  ) << buildKey << "\": {"
			<< ws(indent+1) << quot << "Build"    << quot << ": " << curBuild.buildId << ","
			<< ws(indent+1) << quot << "FileHash" << quot << ": " << curBuild.fileHash << ","
			<< ws(indent+1) << quot << "Offsets"  << quot << ": {";

		for (auto it = build.offsets.begin(); it != build.offsets.end(); ++it) {
			out << ws(indent + 2) << quot << it->first << quot << ": " << it->second;

			if (std::next(it) != build.offsets.end()) {
				out << ",";
			}
		}

		out << ws(indent+1) << "}" 
			<< ws(indent  ) << "}";

		return true;
	};

	auto configPath = getConfigPath();
	if (!std::filesystem::exists(configPath.parent_path())) {
		std::filesystem::create_directories(configPath.parent_path());
	}

	std::ofstream out(configPath);

	out << "{";

	SerializeBuild(curBuild, out, 1);

	if(configBuilds.size() != 0) {
		out << ",";
	}

	for (auto build : configBuilds)
	{
		SerializeBuild(*build, out, 1);
	}

	out << "}";

	return true;
}


bool LoadBuildConfig()
{
    auto configPath = getConfigPath();
    
	LOG_DEBUG("Loading build config from: %s", configPath.string());

    if (!std::filesystem::exists(configPath)) {
        LOG_WARNING("Config file (%s) does not exist. This is normal on first start.", configPath);
        return false;
    }
    
    std::ifstream file(configPath);  // Open in binary mode
    if (!file.is_open()) {
        LOG_ERROR("Error opening build config: %s", configPath);
        return false;
    }
    
    std::string file_content{
        std::istreambuf_iterator<char>(file), 
        std::istreambuf_iterator<char>()
    };
    
    // Check for and remove UTF-8 BOM
    if (file_content.size() >= 3 && 
        static_cast<unsigned char>(file_content[0]) == 0xEF &&
        static_cast<unsigned char>(file_content[1]) == 0xBB &&
        static_cast<unsigned char>(file_content[2]) == 0xBF) {
        file_content.erase(0, 3);
        LOG_DEBUG("Removed UTF-8 BOM from config file");
    }
    
    // Check for UTF-16 BOM
    if (file_content.size() >= 2 && 
        ((static_cast<unsigned char>(file_content[0]) == 0xFF && 
          static_cast<unsigned char>(file_content[1]) == 0xFE) ||
         (static_cast<unsigned char>(file_content[0]) == 0xFE && 
          static_cast<unsigned char>(file_content[1]) == 0xFF))) {
        LOG_ERROR("Config file appears to be UTF-16 encoded. Please save as UTF-8.");
        return false;
    }
    
    LOG_DEBUG("File content preview: %.50s", file_content.c_str());
    
    json_t mem[128];
    const json_t* json = json_create(const_cast<char *>(file_content.c_str()), mem, 128);

	if (!json) {
		LOG_ERROR("Failed to create json parser");
		return false;
	}
	uint32_t curFileHash = calculateCRC32("Chivalry2-Win64-Shipping.exe");

	json_t const* buildEntry;
	needsSerialization = true;
	buildEntry = json_getChild(json);
	while (buildEntry != 0) {
		if (JSON_OBJ == json_getType(buildEntry)) {

			char const* fileSize = json_getPropertyValue(buildEntry, "FileSize");
			json_t const* build = json_getProperty(buildEntry, "Build");
			char const* buildName = json_getName(buildEntry);
			LOG_DEBUG("parsing %s\n", buildName);

			json_t const* fileHash = json_getProperty(buildEntry, "FileHash");
			if (!fileHash || JSON_INTEGER != json_getType(fileHash)) {
				LOG_ERROR("Error, the 'FileHash' property is not found.");
				return EXIT_FAILURE;
			}
			if (!build || JSON_INTEGER != json_getType(build)) {
				LOG_ERROR("Error, the 'Build' property is not found.");
				return EXIT_FAILURE;
			}
			// compare hash
			uint64_t fileHashVal = json_getInteger(fileHash);
			bool hashMatch = fileHashVal == curFileHash;

			// Create Build Config entry
			BuildType bd_;
			BuildType& bd = bd_;

			if (hashMatch)
			{
				bd = curBuild;
				needsSerialization = false;
				LOG_INFO("Found matching Build: %s\n", buildName);
			}

			LOG_INFO("%s : %u\n", buildName, strlen(buildName));

			if (strlen(buildName) > 0)
			{
				bd.SetName(buildName);
			}

			bd.buildId = (uint32_t)json_getInteger(build);
			bd.fileHash = (uint32_t)fileHashVal;

			auto offsetsProperty = json_getProperty(buildEntry, "Offsets");

			if (!offsetsProperty || JSON_OBJ != json_getType(offsetsProperty)) {
				LOG_ERROR("Error, the 'Offsets' property is not found.");
				return false;
			}

			for (json_t const* offsetEntry = json_getChild(offsetsProperty); offsetEntry != 0; offsetEntry = json_getSibling(offsetEntry))
			{
				if (JSON_INTEGER == json_getType(offsetEntry)) {
					const char* offsetName = json_getName(offsetEntry);
					if (offsetName && strlen(offsetName) > 0) {
						uint64_t offsetVal = (uint64_t)json_getInteger(offsetEntry);
						bd.offsets.emplace(std::string(offsetName), offsetVal);
					}
				}
			}

			if (hashMatch)
				curBuild = bd;
			else
				configBuilds.push_back(new BuildType(bd));
		}
		buildEntry = json_getSibling(buildEntry);
	}

	return true;
}