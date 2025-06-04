#pragma once

#include "../Platform.hpp"
#include <functional>
#include <optional>
#include <string>

using OffsetOrString = std::variant<uintptr_t, std::string>;

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
    [[nodiscard]] bool apply(uintptr_t address) {
        if (!should_apply_patch_func()) {
            GLOG_DEBUG("{} : Patch not enabled.", this->name);
            return true;
        }

        GLOG_TRACE("{} : Patch should be enabled.  Attempting to enable it...", name);
        return this->apply_impl(address);
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

class ByteReplacementPatch final : public Patch {
    const uint8_t replacement_byte;
public:

    ByteReplacementPatch(
        const std::string name,
        const std::function<bool()> should_apply_patch_func,
        const uint8_t replacement_byte
    ): Patch(name, should_apply_patch_func), replacement_byte(replacement_byte) {}

    bool apply_impl(const uintptr_t address) override {
        return Ptch_Repl(reinterpret_cast<void*>(address), replacement_byte);
    }
};