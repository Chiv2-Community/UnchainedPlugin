#pragma once

#include <functional>
#include "FunctionHookManager.hpp"
inline std::vector<HookData> g_auto_hooks;

inline bool register_auto_hooks(FunctionHookManager& hook_manager) {
	auto any_failed = false;
	for (const auto& pending_hook : g_auto_hooks) {
		if (!hook_manager.register_hook(pending_hook))
			any_failed = true;
	}

	return !any_failed;
}

template<typename RetType, typename... Args>
inline HookData register_hook(std::string name,
		 const std::function<std::optional<std::string>(Platform)> select_signature_for_platform,
		 const std::function<bool()> should_attach,
		 RetType(*&trampoline)(Args...),  // Note the & here - we need the address of the function pointer
		 RetType(*hook_function)(Args...)
) {
	auto data = HookData(name, select_signature_for_platform, should_attach, reinterpret_cast<void**>(&trampoline), hook_function);
	g_auto_hooks.push_back(data);
	return data;
}

#define CREATE_HOOK(name, signatures_func, attach_predicate, return_type, arguments) \
    static const auto name##_signature = signatures_func; \
    static const auto name##_predicate = attach_predicate; \
    return_type(*o_##name)arguments = nullptr; \
    return_type hk_##name arguments

#define AUTO_HOOK(name) \
    static auto name##_Hook = register_hook( \
        #name, \
        name##_signature, \
        name##_predicate, \
        o_##name, \
        hk_##name \
    );

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