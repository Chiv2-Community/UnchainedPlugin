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

std::deque<BuildMetadata*> configBuilds;
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
	auto SerializeBuild = [](BuildMetadata& build, std::ofstream& out, int indent)
	{
		if (build.GetBuildId() == 0) {
			GLOG_WARNING("No build ID set, cannot serialize build.");
			return false;
		}

		if (build.GetName().empty()) {
			GLOG_WARNING("No build name set, cannot serialize build.");
			return false;
		}

		out	<< ws(indent  ) << quot << build.GetName() << quot << ": {"
			<< ws(indent+1) << quot << "Build"         << quot << ": " << build.GetBuildId() << ","
			<< ws(indent+1) << quot << "FileHash"      << quot << ": " << build.GetFileHash() << ","
			<< ws(indent+1) << quot << "Offsets"       << quot << ": {";

		auto offsets = build.GetOffsets();
		const auto offsets_length = offsets.size();
		for (size_t i = 0; i < offsets_length; i++) {
			auto [name, offset] = offsets[i];
			out << ws(indent + 2) << quot << name << quot << ": " << offset;
			if (i != offsets_length - 1) {
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

	SerializeBuild(g_state->GetBuildMetadata(), out, 1);

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
    
	GLOG_DEBUG("Loading build config from: {}", configPath.string());

    if (!std::filesystem::exists(configPath)) {
        GLOG_WARNING("Config file ({}) does not exist. This is normal on first start.", configPath);
        return false;
    }
    
    std::ifstream file(configPath);  // Open in binary mode
    if (!file.is_open()) {
        GLOG_ERROR("Error opening build config: {}", configPath);
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
        GLOG_ERROR("Config file appears to be UTF-16 encoded. Please save as UTF-8.");
        return false;
    }
    
    GLOG_DEBUG("File content preview: {}", file_content);
    
    json_t mem[128];
    const json_t* json = json_create(const_cast<char *>(file_content.c_str()), mem, 128);

	if (!json) {
		GLOG_ERROR("Failed to create json parser");
		return false;
	}
	uint32_t curFileHash = calculateCRC32("Chivalry2-Win64-Shipping.exe");

	json_t const* buildEntry;
	needsSerialization = true;
	buildEntry = json_getChild(json);
	while (buildEntry != nullptr) {
		if (JSON_OBJ == json_getType(buildEntry)) {
			json_t const* build = json_getProperty(buildEntry, "Build");
			std::string buildName(json_getName(buildEntry));
			GLOG_DEBUG("parsing {}", buildName);

			json_t const* fileHash = json_getProperty(buildEntry, "FileHash");
			if (!fileHash || JSON_INTEGER != json_getType(fileHash)) {
				GLOG_ERROR("Error, the 'FileHash' property is not found.");
				return EXIT_FAILURE;
			}
			if (!build || JSON_INTEGER != json_getType(build)) {
				GLOG_ERROR("Error, the 'Build' property is not found.");
				return EXIT_FAILURE;
			}
			// compare hash
			uint64_t fileHashVal = json_getInteger(fileHash);
			bool hashMatch = fileHashVal == curFileHash;

			// Create Build Config entry
			BuildMetadata bd_;
			BuildMetadata& bd = bd_;

			if (hashMatch)
			{
				bd = g_state->GetBuildMetadata();
				needsSerialization = false;
				GLOG_INFO("Found matching Build: {}", buildName);
			}

			if (!buildName.empty())
			{
				bd.SetName(&buildName);
			}

			bd.SetBuildId(static_cast<uint32_t>(json_getInteger(build)));
			bd.SetFileHash(static_cast<uint32_t>(fileHashVal));

			auto offsetsProperty = json_getProperty(buildEntry, "Offsets");

			if (!offsetsProperty || JSON_OBJ != json_getType(offsetsProperty)) {
				GLOG_ERROR("Error, the 'Offsets' property is not found.");
				return false;
			}

			for (json_t const* offsetEntry = json_getChild(offsetsProperty); offsetEntry != 0; offsetEntry = json_getSibling(offsetEntry))
			{
				if (JSON_INTEGER == json_getType(offsetEntry)) {
					if (const char* offsetName = json_getName(offsetEntry); offsetName && strlen(offsetName) > 0) {
						const auto offsetVal = static_cast<uint64_t>(json_getInteger(offsetEntry));
						bd.SetOffset(offsetName, offsetVal);
					}
				}
			}

			if (hashMatch)
				g_state->SetBuildMetadata(bd);
			else
				configBuilds.push_back(new BuildMetadata(bd));
		}
		buildEntry = json_getSibling(buildEntry);
	}

	return true;
}