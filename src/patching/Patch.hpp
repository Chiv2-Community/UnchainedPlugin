﻿#pragma once

#include "../Platform.hpp"
#include <functional>
#include <optional>
#include <string>
#include <vector>
#include "../string_util.hpp"

using OffsetOrString = std::variant<uintptr_t, std::string>;

enum ApplyResult {
    APPLY_SUCCESS,
    APPLY_FAILED,
    APPLY_DISABLED
};

class Patch {
private:
    const std::string name;
    const std::function<bool()> should_apply_patch_func;
public:
    virtual ~Patch() = default;

    Patch(
        const std::string name,
        const std::function<bool()> should_apply_patch_func
    )
        : name(name),
          should_apply_patch_func(should_apply_patch_func) {}

private:
    [[nodiscard]] virtual bool apply_impl(const uintptr_t address) = 0;

public:
    [[nodiscard]] ApplyResult apply(uintptr_t address) {
        if (!should_apply_patch_func()) {
            return APPLY_DISABLED;
        }

        GLOG_TRACE("{} : Patch should be enabled.  Attempting to enable it...", name);
        return this->apply_impl(address) ? APPLY_SUCCESS : APPLY_FAILED;
    }

    [[nodiscard]] const std::string& get_name() const {
        return name;
    }
};

class HookPatch final : public Patch {
private:
    void** trampoline;
    void* hook_function;
public:
    HookPatch(
        const std::string name,
        const std::function<bool()> should_apply_patch_func,
        void** trampoline,
        void* hook_function
    ): Patch(name, should_apply_patch_func), trampoline(trampoline), hook_function(hook_function) {}

    bool apply_impl(const uintptr_t address) override {
        const auto address_ptr = reinterpret_cast<void*>(address);

        GLOG_TRACE("{} : Hooking address {}", get_name(), address_ptr);
        auto result = MH_CreateHook(address_ptr, this->hook_function, this->trampoline);
        if (result != MH_OK) {
            log_and_validate_mh_status(get_name(), result);
            return false;
        }
        result = MH_EnableHook(address_ptr);

        GLOG_TRACE("{} : Trampoline address set to {}", get_name(), *this->trampoline);
        return log_and_validate_mh_status(get_name(), result);
    }

private:
    static bool log_and_validate_mh_status(const std::string &hook_name, const MH_STATUS status) {
        if (status == MH_OK) {
            GLOG_DEBUG("{} : Successfully hooked", hook_name);
            return true;
        }

        GLOG_ERROR("{} : Minhook error while hooking ({})", hook_name, MH_StatusToString(status));
        return false;
    }
};

class ByteReplacementPatch : public Patch {
    const std::vector<uint8_t> replacement_bytes;
    const std::function<uint64_t()> additional_offset;
public:

    ByteReplacementPatch(
        const std::string name,
        const std::function<bool()> should_apply_patch_func,
        const std::function<uint64_t()> additional_offset,
        const std::vector<uint8_t> replacement_bytes
    ): Patch(name, should_apply_patch_func), replacement_bytes(replacement_bytes), additional_offset(additional_offset) {}

    bool apply_impl(const uintptr_t address) override {
        unsigned long old_protect, temp_protect;

        const auto bytes = replacement_bytes.data();
        const auto size = replacement_bytes.size();

        auto address_ptr = reinterpret_cast<void*>(address + additional_offset());

        auto res = VirtualProtect(address_ptr, size, PAGE_EXECUTE_READWRITE, &old_protect);
        if (!res) {
            log_windows_error(address_ptr);
            return false;
        }

        memcpy(address_ptr, bytes, size);

        FlushInstructionCache(GetCurrentProcess(), address_ptr, size);

        res = VirtualProtect(address_ptr, size, old_protect, &temp_protect);
        if (!res) {
            log_windows_error(address_ptr);
            return false;
        }

        GLOG_DEBUG("{} : Successfully patched {} bytes at address {}", get_name(), size, address_ptr);
        return true;
    }

private:
    static void log_windows_error(void *address) {
        std::optional<std::string> error_message = get_last_windows_error_message_string();
        GLOG_ERROR("Failed to patch {}. Error {}", address,
                   error_message.has_value() ? error_message.value() : "Unknown error");
    }
};

class NopPatch : public ByteReplacementPatch {
public:
    NopPatch(
        const std::string name,
        const std::function<bool()> should_apply_patch_func,
        const std::function<uint64_t()> additional_offset,
        const uint64_t size
    ): ByteReplacementPatch(name, should_apply_patch_func, additional_offset, std::vector<uint8_t>(size, 0x90)) {}
};