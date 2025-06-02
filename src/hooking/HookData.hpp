#pragma once

#include "../Platform.hpp"
#include <functional>
#include <optional>
#include <string>

using OffsetOrString = std::variant<uint64_t, std::string>;

struct HookData {
    std::string name;
    std::function<std::optional<OffsetOrString>(Platform)> select_signature_for_platform;
    std::function<bool()> should_attach;
    uint64_t address = 0;
    void** trampoline = nullptr;
    void* hook;
    bool scan_only;

    HookData(std::string name, std::function<std::optional<OffsetOrString>(Platform)> select_signature_for_platform, std::function<bool()> should_attach, void** trampoline, void* hook)
        : name(std::move(name)),
          select_signature_for_platform(std::move(select_signature_for_platform)),
          should_attach(should_attach),
          trampoline(trampoline),
          hook(hook),
          scan_only(false){}

    HookData(std::string name, std::function<std::optional<OffsetOrString>(Platform)> select_signature_for_platform, bool scan_only)
        : name(std::move(name)),
          select_signature_for_platform(std::move(select_signature_for_platform)),
          should_attach([]() { return false; }),
          trampoline(nullptr),
          hook(nullptr),
          scan_only(scan_only){}
};
