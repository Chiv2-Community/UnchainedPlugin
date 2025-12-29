#include "builds.hpp"

#include <cstring>
#include <stdint.h>
#include <map>
#include <string>
#include <Windows.h>

#include "logging/global_logger.hpp"
#include "state/global_state.hpp"

extern "C" void load_known_builds();
extern "C" size_t get_known_builds_count();
extern "C" uint32_t get_known_build_hash(size_t index);
extern "C" char* get_known_build_platform(size_t index);
extern "C" size_t get_known_build_offset_count(size_t index);
extern "C" char* get_known_build_offset_name(size_t build_index, size_t offset_index);
extern "C" uint64_t get_known_build_offset_value(size_t build_index, size_t offset_index);
extern "C" void free_string(char* s);

std::map<std::string, BuildMetadata> LoadBuildMetadata()
{
	load_known_builds();
	std::map<std::string, BuildMetadata> buildMap;
    
	size_t build_count = get_known_builds_count();
	for (size_t i = 0; i < build_count; ++i) {
		uint32_t hash = get_known_build_hash(i);
		char* platform_str = get_known_build_platform(i);
		
		Platform platform = STEAM;
		if (string_to_platform.contains(platform_str)) {
			platform = string_to_platform.at(platform_str);
		}
		free_string(platform_str);

		size_t offset_count = get_known_build_offset_count(i);
		std::map<std::string, uint64_t> offsets;
		for (size_t j = 0; j < offset_count; ++j) {
			char* name = get_known_build_offset_name(i, j);
			uint64_t value = get_known_build_offset_value(i, j);
			if (name) {
				offsets[name] = value;
				free_string(name);
			}
		}

		BuildMetadata metadata(hash, std::move(offsets), platform);
		buildMap.emplace(std::to_string(hash), std::move(metadata));
	}

	return buildMap;
}