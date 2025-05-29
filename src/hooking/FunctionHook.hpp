#pragma once

#include <memory>
#include <utility>
#include <vector>
#include <functional>
#include <string>
#include <map>
#include <optional>
#include "../Platform.hpp"
#include "../state/State.hpp"

template<typename RetType, typename... Args>
class FunctionHook {
private:
    typedef RetType (*OriginalFunction)(Args...);
    typedef std::function<RetType(OriginalFunction, Args...)> InputHookFunction;

    std::string name;
    std::function<std::optional<std::string>(Platform)> select_signature_for_platform;
    std::function<bool()> should_attach;

    InputHookFunction hook_function;
    OriginalFunction original_function;

    // Use std::function instead of a raw function pointer
    std::function<RetType(Args...)> hooked_function;

public:
    FunctionHook(std::string name,
                 std::function<std::optional<std::string>(Platform)> select_signature_for_platform,
                 std::function<bool()> should_attach,
                 InputHookFunction hook_function)
        : name(std::move(name)),
          select_signature_for_platform(std::move(select_signature_for_platform)),
          should_attach(should_attach),
          hook_function(hook_function) {
            original_function = nullptr;
            hooked_function = [this](Args... args) -> RetType {
                return this->hook_function(this->original_function, args...);
            };
    }
    
    std::string get_name() const {
        return name;
    }

    OriginalFunction get_original() const {
        return original_function;
    }

    std::function<bool()> get_should_attach() const {
        return should_attach;
    }

    std::optional<std::string> get_signature(Platform platform) const {
        return select_signature_for_platform(platform);
    }

    MH_STATUS enable(uint64_t original_function_address) {
        auto result = MH_CreateHook(
            reinterpret_cast<void*>(original_function_address),
            reinterpret_cast<void*>(hooked_function),
            reinterpret_cast<void**>(original_function)
        );
        if (result != MH_OK) {
            return result;
        }
        return MH_EnableHook(reinterpret_cast<void*>(original_function_address));
    }
};

#define UNIVERSAL_SIGNATURE(signature) \
    [](Platform platform) -> std::optional<std::string> { \
        return signature; \
    }

#define UNKNOWN_SIGNATURE() \
    [](Platform platform) -> std::optional<std::string> { \
        return std::nullopt; \
    }

#define PLATFORM_SIGNATURES(...) \
    [](Platform platform) -> std::optional<std::string> { \
        switch (platform) { \
            __VA_ARGS__ \
            default: \
                return std::nullopt; \
        } \
    }

#define PLATFORM_SIGNATURE(platform, signature) \
    case platform: \
        return signature;

#define ATTACH_ALWAYS [](){ return true; }
#define ATTACH_WHEN(condition) [](){ return condition; }