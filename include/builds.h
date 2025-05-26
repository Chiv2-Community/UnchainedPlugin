#pragma once

#include <cstdint>
#include <deque>
#include <string>
#include <map>

struct BuildType {

	void SetName(const char* newName) {
		nameStr = std::string(newName);
	}

	void SetName(const wchar_t* newName) {
		std::wstring ws(newName);
		nameStr = std::string(ws.begin(), ws.end());
	}

	~BuildType() {
		delete[] name;
	}

	uint32_t fileHash = 0;
	uint32_t buildId = 0;
	std::map<std::wstring, uint64_t> offsets = {};
	std::string nameStr = "";
private:
	char* name = nullptr;
};

uint32_t calculateCRC32(const std::string& filename);

// TODO: put these globals where they belong (in the below functions)
// some hooks rely on accessing them directly. This needs to be cleaned up
// btw, they are defined in this class's corresponding cpp file
extern std::deque<BuildType*> configBuilds;
//BuildInfo* curBuildInfo = nullptr;
extern BuildType curBuild;
extern bool jsonDone;
extern bool offsetsLoaded;
extern bool needsSerialization;

bool serializeBuilds();
bool LoadBuildConfig();