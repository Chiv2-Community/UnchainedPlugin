﻿#pragma once

#include <utility>

#include "BuildMetadata.hpp"
#include "CLIArgs.hpp"
#include "RCONState.hpp"

class State {
    CLIArgs args;                                     // Changed from reference to object
    BuildMetadata& current_build_metadata;                      // Changed from reference to object
    void* uworld = nullptr;
    void* CurGameMode = nullptr;
    RCONState rcon_state;

    std::map<std::string, BuildMetadata> build_metadata;

public:
    // Updated constructor to take objects by value (copy) or by rvalue reference (move)
    State(CLIArgs args, std::map<std::string, BuildMetadata> all_build_metadata, BuildMetadata& current_build_metadata)
        : args(std::move(args)), current_build_metadata(current_build_metadata), build_metadata(all_build_metadata){
        this->uworld = nullptr;
        this->rcon_state = RCONState();
    }

    inline void SetUWorld(void* uworld) {
        this->uworld = uworld;
    }
    
    inline void* GetUWorld() const {
        return this->uworld;
    }

    inline void SetCurGameMode(void* CurGameMode) {
        this->CurGameMode = CurGameMode;
    }

    inline void* GetCurGameMode() const {
        return this->CurGameMode;
    }

    // Updated getter to return const reference
    inline CLIArgs& GetCLIArgs() {
        return this->args;
    }
    
    // Updated getter to return const reference
    inline BuildMetadata& GetBuildMetadata() const {
        return this->current_build_metadata;
    }

    inline std::map<std::string, BuildMetadata> GetSavedBuildMetadata() const {
        return this->build_metadata;
    }

    inline RCONState& GetRCONState() {
        return this->rcon_state;
    }
};
