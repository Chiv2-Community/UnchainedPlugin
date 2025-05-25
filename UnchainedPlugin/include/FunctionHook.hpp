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

std::map<std::string, Platform> string_to_platform = {
    {"STEAM", STEAM},
    {"EGS", EGS},
    {"GAMEPASS", GAMEPASS}
};

std::map<Platform, std::string> platform_to_string = {
    {STEAM, "STEAM"},
    {EGS, "EGS"},
    {GAMEPASS, "GAMEPASS"}
};



template<typename RetType, typename... Args>
class FunctionHook {
private:
    using FunctionType = RetType(*)(Args...);
    std::string name;
    std::function<std::optional<std::string>(Platform)> select_signature_for_platform;
    std::function<RetType(FunctionType, Args...)> hook_function;
    uint64_t offset;
    FunctionType original_function;
    bool hook_enabled;

public:
    // Constructor - note the parameter types match your usage
    FunctionHook(const std::string& name, 
                 std::function<std::optional<std::string>(Platform)> select_signature_for_platform, 
                 std::function<RetType(FunctionType, Args...)> hook_function)
        : name(name), 
          select_signature_for_platform(select_signature_for_platform), 
          hook_function(hook_function),
          offset(0), 
          original_function(nullptr),
          hook_enabled(false) {}

    inline std::function<RetType(Args...)> get_hook_function() const {
        auto original_func = this->original_function;
        auto hook_func = this->hook_function;
        return [original_func, hook_func](Args... args) -> RetType { 
            return hook_func(original_func, args...); 
        };
    }
    
    inline FunctionType get_original() const {
        return original_function;
    }
    
    inline std::string get_name() const {
        return name;
    }

    inline void set_hook_enabled(uint64_t offset, FunctionType original, bool enabled = true) {
        this->offset = offset;
        this->original_function = original;
        this->hook_enabled = enabled;
    }

    inline bool is_hook_enabled() const {
        return hook_enabled;
    }

    inline std::string get_signature(std::string platform) const {
        return select_signature_for_platform(platform);
    }

    inline void set_offset(uint64_t offset) {
        this->offset = offset;
    }

    inline uint64_t get_offset() const {
        return offset;
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