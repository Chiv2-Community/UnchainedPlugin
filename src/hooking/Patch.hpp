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
        const std::function<bool()> &should_apply_patch_func
    )
        : name(std::move(name)),
          should_apply_patch_func(should_apply_patch_func) {}

private:
    [[nodiscard]] virtual bool apply_impl(const uintptr_t address) = 0;  // Abstract method

public:
    [[nodiscard]] virtual std::optional<uintptr_t> get_address(uintptr_t base_addr, uint64_t image_size, Platform platform) = 0;

    [[nodiscard]] bool apply(const uintptr_t base_addr, const uint64_t image_size, const Platform platform) {
        if (!this->should_apply()) {
            GLOG_DEBUG("Patch '{}' not enabled.", this->name);
            return false;
        }

        auto address = get_address(base_addr, image_size, platform);
        if (!address.has_value()) {
            GLOG_ERROR("Patch '{}' cannot be applied for platform '{}'.  No address found.", get_name(), platform);
            return false;
        }

        return this->apply_impl(address.value());
    }

    [[nodiscard]] bool should_apply() const {
        if (!this->should_apply_patch_func()) {
            GLOG_DEBUG("Patch '{}' not enabled.", this->name);
            return false;
        }
        return true;
    }

    [[nodiscard]] const std::string& get_name() const {
        return name;
    }
};

class SignaturePatch : public Patch {
private:
    const std::function<std::optional<OffsetOrString>(Platform)>& select_signature_for_platform;
    const std::function<uintptr_t(uintptr_t)> handle_signature_address;

    uintptr_t address = 0;

public:
    SignaturePatch(
        const std::string name,
        const std::function<std::optional<OffsetOrString>(Platform)>& select_signature_for_platform,
        const std::function<uintptr_t(uintptr_t)>& handle_signature_address,
        const std::function<bool()>& should_apply_patch_func
    ): Patch(name, should_apply_patch_func), select_signature_for_platform(select_signature_for_platform), handle_signature_address(handle_signature_address) {
    }

    [[nodiscard]] std::optional<std::string> get_signature(const Platform platform) const {
        auto maybe_signature_or_offset = select_signature_for_platform(platform);
        if (!maybe_signature_or_offset.has_value()) {
            return std::nullopt;
        }

        auto signature_or_offset = maybe_signature_or_offset.value();
        if (std::holds_alternative<std::string>(signature_or_offset)) {
            return std::get<std::string>(signature_or_offset);
        }

        return std::nullopt;
    }

    [[nodiscard]] std::optional<uintptr_t> get_offset(const Platform platform) const {
        auto maybe_signature_or_offset = select_signature_for_platform(platform);
        if (!maybe_signature_or_offset.has_value()) {
            return std::nullopt;
        }

        auto signature_or_offset = maybe_signature_or_offset.value();
        if (std::holds_alternative<uintptr_t>(signature_or_offset)) {
            return std::get<uintptr_t>(signature_or_offset);
        }

        return std::nullopt;
    }

    [[nodiscard]] std::optional<uintptr_t> get_address(const uintptr_t base_addr, const uint64_t image_size, const Platform platform) override {
        if (address != 0) { return address;}

        const auto& platform_string = platform_to_string.at(platform);

        auto maybe_offset = get_offset(platform);
        if (maybe_offset.has_value()) {
            const auto offset = maybe_offset.value();
            GLOG_TRACE("Patch '{}' on platform '{}' has hardcoded offset: 0x{:X}", get_name(), platform_string, offset);
            address = base_addr + offset;
            return address;
        }

        auto maybe_signature = get_signature(platform);
        if (maybe_signature.has_value()) {
            const auto signature = maybe_signature.value();
            GLOG_TRACE("Patch '{}' on platform '{}' has signature... Searching.", get_name(), platform_string);
            address = reinterpret_cast<uintptr_t>(
                Sig::find(reinterpret_cast<void *>(base_addr), image_size, signature.c_str())
            );
            GLOG_TRACE("Patch '{}' on platform '{}' has offset: 0x{:X}", get_name(), platform_string, address - base_addr);
            return handle_signature_address(address);
        }

        GLOG_ERROR("Patch '{}' is unsupported for platform '{}'. Failed to find offset.", get_name(), platform_string);
        return std::nullopt;
    }
};

class HookSignaturePatch final : public SignaturePatch {
private:
    void** trampoline;
    void* hook_function;
public:
    HookSignaturePatch(
        const std::string name,
        const std::function<std::optional<OffsetOrString>(Platform)>& select_signature_for_platform,
        const std::function<uintptr_t(uintptr_t)>& handle_signature_address,
        const std::function<bool()>& should_apply_patch_func,
        void** trampoline,
        void* hook_function
    ): SignaturePatch(name, select_signature_for_platform, handle_signature_address, should_apply_patch_func), trampoline(trampoline), hook_function(hook_function) {}

    bool apply_impl(const uintptr_t address) override {
        const auto address_ptr = reinterpret_cast<void*>(address);

        auto result = MH_CreateHook(address_ptr, this->hook_function, this->trampoline);
        if (result != MH_OK) {
            log_and_validate_mh_status(get_name(), result);
            return false;
        }

        result = MH_EnableHook(address_ptr);
        return !log_and_validate_mh_status(get_name(), result);
    }

private:
    static bool log_and_validate_mh_status(const std::string &hook_name, const MH_STATUS status) {
        if (status == MH_OK) {
            GLOG_DEBUG("Successfully hooked '{}'", hook_name);
            return true;
        }

        GLOG_ERROR("Minhook error while hooking '{}': {}", hook_name, MH_StatusToString(status));
        return false;
    }
};

class ByteReplacementSignaturePatch final : public SignaturePatch {
    const uint8_t replacement_byte;
public:

    ByteReplacementSignaturePatch(
        const std::string name,
        const std::function<std::optional<OffsetOrString>(Platform)>& select_signature_for_platform,
        const std::function<uintptr_t(uintptr_t)>& handle_signature_address,
        const std::function<bool()>& should_apply_patch_func,
        const uint8_t replacement_byte
    ): SignaturePatch(name, select_signature_for_platform, handle_signature_address, should_apply_patch_func), replacement_byte(replacement_byte) {}

    bool apply_impl(const uintptr_t address) override {
        return Ptch_Repl(reinterpret_cast<void*>(address), replacement_byte);
    }
};