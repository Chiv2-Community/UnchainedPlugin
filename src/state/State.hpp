#pragma once

#include <utility>
#include <map>
#include <string>

#include "CLIArgs.hpp"
#include "RCONState.hpp"

class State {
    CLIArgs args;
    void* uworld = nullptr;
    void* CurGameMode = nullptr;
    RCONState rcon_state;


public:
    State(CLIArgs args)
        : args(std::move(args)){
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

    inline CLIArgs& GetCLIArgs() {
        return this->args;
    }

    inline RCONState& GetRCONState() {
        return this->rcon_state;
    }
};
