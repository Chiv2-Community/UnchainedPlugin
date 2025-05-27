#pragma once

#include <memory>
#include <vector>
#include <functional>
#include <string>
#include <map>
#include <optional>

enum Platform {
    STEAM,
    EGS,
    GAMEPASS
};

std::map<std::wstring, Platform> string_to_platform = {
    {L"STEAM", STEAM},
    {L"EGS", EGS},
    {L"GAMEPASS", GAMEPASS}
};

std::map<Platform, std::wstring> platform_to_string = {
    {STEAM, L"STEAM"},
    {EGS, L"EGS"},
    {GAMEPASS, L"GAMEPASS"}
};



template<typename RetType, typename... Args>
class FunctionHook {
public:
    using OriginalFunctionType = RetType(*)(Args...);
    using HookFunctionType = RetType(*)(OriginalFunctionType, Args...);
private:
    std::string name;
    std::function<std::optional<std::string>(Platform)> select_signature_for_platform;
    std::function<RetType(OriginalFunctionType, Args...)> hook_function;
    OriginalFunctionType original_function;
    
public:

    // Constructor - note the parameter types match your usage
    FunctionHook(const std::string& name,
                 std::function<std::optional<std::string>(Platform)> select_signature_for_platform, 
                 HookFunctionType hook_function)
        : name(name), 
          select_signature_for_platform(select_signature_for_platform), 
          hook_function(hook_function),
          original_function(nullptr) {}

    inline std::function<RetType(Args...)> get_hook_function() const {
        auto original_func = this->original_function;
        auto hook_func = this->hook_function;
        return [original_func, hook_func](Args... args) -> RetType { 
            return hook_func(original_func, args...); 
        };
    }
    
    
    inline OriginalFunctionType get_original() const {
        return original_function;
    }
    
    inline std::string get_name() const {
        return name;
    }

    inline std::optional<std::string> get_signature(Platform platform) const {
        return select_signature_for_platform(platform);
    }
};

template<typename RetType, typename... Args>
FunctionHook<RetType, Args...> CreateFunctionHook(const std::string& name, 
                 std::function<std::optional<std::string>(Platform)> select_signature_for_platform, 
                 std::function<RetType(std::function<RetType(Args...)>, Args...)> hook_function) {
    return FunctionHook<RetType, Args...>(name, select_signature_for_platform, hook_function);
}

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