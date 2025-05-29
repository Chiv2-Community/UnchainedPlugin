#pragma once

#include "../Platform.hpp"
#include <functional>
#include <optional>
#include <string>

struct HookData {
    std::string name;
    std::function<std::optional<std::string>(Platform)> select_signature_for_platform;
    std::function<bool()> should_attach;
    void** trampoline = nullptr;
    void* hook;

    HookData(std::string name, std::function<std::optional<std::string>(Platform)> select_signature_for_platform, std::function<bool()> should_attach, void** trampoline, void* hook)
        : name(std::move(name)),
          select_signature_for_platform(std::move(select_signature_for_platform)),
          should_attach(should_attach),
          trampoline(trampoline),
          hook(hook) {}
};
