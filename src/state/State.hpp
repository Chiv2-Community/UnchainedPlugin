#pragma once

#include "BuildMetadata.hpp"
#include "CLIArgs.hpp"

class State {
    CLIArgs& args;
    BuildMetadata& current_build;
    void* uworld = nullptr;

    std::map<std::string, BuildMetadata> saved_build_metadata;

public:
    State(CLIArgs& args, BuildMetadata& build) : args(args), current_build(build) {
        this->uworld = nullptr;
    }

    inline void SetUWorld(void* uworld) {
        this->uworld = uworld;
    }
    
    inline void* GetUWorld() const {
        return this->uworld;
    }

    inline CLIArgs& GetCLIArgs() const {
        return this->args;
    }
    
    inline BuildMetadata& GetBuildMetadata() const {
        return this->current_build;
    }

    inline void SetBuildMetadata(BuildMetadata& build) {
        this->current_build = build;
    }

    inline void SetSavedBuildMetadata(std::map<std::string, BuildMetadata> build) {
        this->saved_build_metadata = build;
    }

    inline std::map<std::string, BuildMetadata> GetSavedBuildMetadata() const {
        return this->saved_build_metadata;
    }
};