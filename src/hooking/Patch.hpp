#pragma once

#include "../Platform.hpp"
#include <functional>
#include <optional>
#include <string>

using OffsetOrString = std::variant<uint64_t, std::string>;

struct Patch {

    std::string name;
    uintptr_t address = 0;
    std::function<std::optional<OffsetOrString>(Platform)> select_signature_for_platform;
    std::function<bool()> should_apply_func;
    bool(*apply_func)(const Patch*);

public:
    Patch(
        std::string name,
        std::function<std::optional<OffsetOrString>(Platform)> select_signature_for_platform,
        std::function<bool()> should_apply_func,
        bool(*apply)(const Patch*)
    )
        : name(std::move(name)),
          select_signature_for_platform(std::move(select_signature_for_platform)),
          should_apply_func(std::move(should_apply_func)),
          apply_func(apply) {}

    [[nodiscard]] bool apply() const {
        if (!this->should_apply_func()) {
            GLOG_DEBUG("Hook '{}' not enabled.", this->name);
            return true;
        }
        if (this->address == 0) {
            GLOG_ERROR("Attempted to enable hook '{}' before its address was found.", this->name);
            return false;
        }

        return apply_func(this);
    }
};