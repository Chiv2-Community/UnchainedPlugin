#pragma once

#include "BuildMetadata.hpp"
#include "CLIArgs.hpp"

class State {
    CLIArgs& args;
    BuildMetadata& build;
    void* uworld = nullptr;

public:
    State(CLIArgs& args, BuildMetadata& build) : args(args), build(build) {
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
        return this->build;
    }

    inline void SetBuildMetadata(BuildMetadata& build) {
        this->build = build;
    }
};