#include "builds.h"

#include <cstring>
#include <stdint.h>
#include <iostream>
#include <fstream>
#include <string>
#include <filesystem>
#include "logging.hpp"
#include <tiny-json.h>

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
	auto SerializeBuild = [](BuildType& build, std::ofstream& out, int indent)
	{
		if (build.buildId == 0) {
			LOG_WARNING(g_logger, "No build ID set, cannot serialize build.");
			return false;
		}

		if (build.nameStr.length() == 0) {
			LOG_WARNING(g_logger, "No build name set, cannot serialize build.");
			return false;
		}

		const char* buildKey = build.nameStr.c_str();

		out	<< ws(indent  ) << quot << buildKey   << quot << ": {"
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

	out << L"{";

	SerializeBuild(curBuild, out, 1);

	if(configBuilds.size() != 0) {
		out << L",";
	}

	for (auto build : configBuilds)
	{
		SerializeBuild(*build, out, 1);
	}

	out << L"}";

	return true;
}


bool LoadBuildConfig()
{
    auto configPath = getConfigPath();
    
	LOG_DEBUG(g_logger, "Loading build config from: {}", configPath.string());

    if (!std::filesystem::exists(configPath)) {
        LOG_WARNING(g_logger, "Config file ({}) does not exist. This is normal on first start.", configPath);
        return false;
    }
    
    std::ifstream file(configPath);  // Open in binary mode
    if (!file.is_open()) {
        LOG_ERROR(g_logger, "Error opening build config: {}", configPath);
        return false;
    }
    
    std::string file_content{
        std::istreambuf_iterator<char>(file), 
        std::istreambuf_iterator<char>()
    };
    
    if (file_content.size() >= 2 &&
        ((static_cast<unsigned char>(file_content[0]) == 0xFF &&
          static_cast<unsigned char>(file_content[1]) == 0xFE) ||
         (static_cast<unsigned char>(file_content[0]) == 0xFE && 
          static_cast<unsigned char>(file_content[1]) == 0xFF))) {
        LOG_ERROR(g_logger, "Config file appears to be UTF-16 encoded. Please save as UTF-8.");
        return false;
    }
    
    LOG_DEBUG(g_logger, "File content preview: {}", file_content);
    
    json_t mem[128];
    const json_t* json = json_create(const_cast<char *>(file_content.c_str()), mem, 128);

	if (!json) {
		LOG_ERROR(g_logger, "Failed to create json parser");
		return false;
	}
	uint32_t curFileHash = calculateCRC32("Chivalry2-Win64-Shipping.exe");

	json_t const* buildEntry;
	needsSerialization = true;
	buildEntry = json_getChild(json);
	while (buildEntry != nullptr) {
		if (JSON_OBJ == json_getType(buildEntry)) {
			json_t const* build = json_getProperty(buildEntry, "Build");
			char const* buildName = json_getName(buildEntry);
			LOG_DEBUG(g_logger, "parsing {}", buildName);

			json_t const* fileHash = json_getProperty(buildEntry, "FileHash");
			if (!fileHash || JSON_INTEGER != json_getType(fileHash)) {
				LOG_ERROR(g_logger, "Error, the 'FileHash' property is not found.");
				return EXIT_FAILURE;
			}
			if (!build || JSON_INTEGER != json_getType(build)) {
				LOG_ERROR(g_logger, "Error, the 'Build' property is not found.");
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
				LOG_INFO(g_logger, "Found matching Build: {}", buildName);
			}

			LOG_INFO(g_logger, "{} : {}", buildName, strlen(buildName));

			if (strlen(buildName) > 0)
			{
				bd.SetName(buildName);
			}

			bd.buildId = static_cast<uint32_t>(json_getInteger(build));
			bd.fileHash = static_cast<uint32_t>(fileHashVal);

			auto offsetsProperty = json_getProperty(buildEntry, "Offsets");

			if (!offsetsProperty || JSON_OBJ != json_getType(offsetsProperty)) {
				LOG_ERROR(g_logger, "Error, the 'Offsets' property is not found.");
				return false;
			}

			for (json_t const* offsetEntry = json_getChild(offsetsProperty); offsetEntry != 0; offsetEntry = json_getSibling(offsetEntry))
			{
				if (JSON_INTEGER == json_getType(offsetEntry)) {
					if (const char* offsetName = json_getName(offsetEntry); offsetName && strlen(offsetName) > 0) {
						const auto offsetVal = static_cast<uint64_t>(json_getInteger(offsetEntry));
						bd.offsets[offsetName] = offsetVal;
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