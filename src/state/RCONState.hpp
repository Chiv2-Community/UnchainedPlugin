#pragma once

#include <optional>
#include <string>
#include "../logging/global_logger.hpp"


class RCONState {
private:
    std::wstring command;
    bool has_pending_command;

public:
    RCONState(): has_pending_command(false) {};

    void set_command(const std::wstring command) {
        this->command = command;
        this->has_pending_command = true;
    }

    std::optional<std::wstring> get_command() {
        if (has_pending_command) {
            has_pending_command = false;
            return command;
        }

        return std::nullopt;
    }
};
