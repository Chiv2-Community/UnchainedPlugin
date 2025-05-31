#pragma once

#include "BuildMetadata.hpp"
#include "CLIArgs.hpp"

class State {
    CLIArgs args;                                     // Changed from reference to object
    BuildMetadata current_build;                      // Changed from reference to object
    void* uworld = nullptr;

    std::map<std::string, BuildMetadata> saved_build_metadata;

public:
    // Updated constructor to take objects by value (copy) or by rvalue reference (move)
    State(CLIArgs args, BuildMetadata build) 
        : args(std::move(args)), current_build(std::move(build)) {
        this->uworld = nullptr;
    }

    inline void SetUWorld(void* uworld) {
        this->uworld = uworld;
    }
    
    inline void* GetUWorld() const {
        return this->uworld;
    }

    // Updated getter to return const reference
    inline CLIArgs& GetCLIArgs() {
        return this->args;
    }
    
    // Updated getter to return const reference
    inline BuildMetadata& GetBuildMetadata() {
        return this->current_build;
    }

    // Updated setter to take object by reference
    inline void SetBuildMetadata(const BuildMetadata& build) {
        this->current_build = build;
    }

    inline void SetSavedBuildMetadata(std::map<std::string, BuildMetadata> build) {
        this->saved_build_metadata = std::move(build);
    }

    inline std::map<std::string, BuildMetadata> GetSavedBuildMetadata() const {
        return this->saved_build_metadata;
    }
};