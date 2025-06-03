#pragma once

#include <functional>
#include "PatchManager.hpp"
inline std::vector<std::unique_ptr<Patch>> g_auto_hooks;

inline bool register_auto_patches(PatchManager& hook_manager) {
	auto any_failed = false;
	for (const auto& pending_hook : g_auto_hooks) {
		if (!hook_manager.register_patch(*pending_hook))
			any_failed = true;
	}

	return !any_failed;
}

inline Patch* register_patch(
    std::string name,
	std::function<std::optional<OffsetOrString>(Platform)> select_signature_for_platform,
    std::function<bool()> apply_predicate,
    bool(*apply_patch)(const Patch*)
) {
    g_auto_hooks.push_back(std::make_unique<Patch>(Patch(name, select_signature_for_platform, apply_predicate, apply_patch)));
    return g_auto_hooks.back().get();
}

static bool log_and_validate_mh_status(const std::string &hook_name, const MH_STATUS status) {
    if (status == MH_OK) {
        GLOG_DEBUG("Successfully hooked '{}'", hook_name);
        return true;
    }

    GLOG_ERROR("Minhook error while hooking '{}': {}", hook_name, MH_StatusToString(status));
    return false;
}

#define CREATE_HOOK(name, signatures_func, apply_predicate, return_type, arguments) \
    static auto name##_signature = signatures_func; \
    static auto name##_predicate = apply_predicate; \
    return_type(*o_##name)arguments = nullptr; \
    return_type hk_##name arguments

#define AUTO_HOOK(hook_name) \
    static bool apply_hook_##hook_name(const Patch* patch) { \
        auto result = MH_CreateHook( \
            reinterpret_cast<void*>(patch->address), \
            reinterpret_cast<void*>(hk_##hook_name), \
            reinterpret_cast<void**>(o_##hook_name) \
        ); \
        if (result != MH_OK) { \
            log_and_validate_mh_status(patch->name, result); \
            return false; \
        } \
        result = MH_EnableHook(reinterpret_cast<void*>(patch->address)); \
        return !log_and_validate_mh_status(patch->name, result); \
    } \
    static auto hook_name##_Patch = register_patch( \
        #hook_name, \
        hook_name##_signature, \
        hook_name##_predicate, \
        apply_hook_##hook_name \
    );

/**
 * Creates a patch with the given functionality. Patches should modify memory in some way
 * .
 * @param name
 * @param signatures_func
 * @param apply_predicate Predicate returning true when the patch should apply
 */
#define CREATE_PATCH(name, signatures_func, attach_predicate) \
    static auto name##_signature = signatures_func; \
    static auto name##_predicate = attach_predicate; \
    bool patch_##name(const Patch* patch)

#define AUTO_PATCH(name) \
    static auto name##_Patch = register_patch( \
        #name, \
        name##_signature, \
        name##_predicate, \
        patch_##name \
    );


/**
 * Registers a "patch" that does nothing. All this really does is scan for the signature.
 *
 * @param name
 * @param signatures_func
 */
#define AUTO_SCAN(name, signatures_func) \
    CREATE_PATCH(name, signatures_func, [](){ return false; }) { return true; } \
    AUTO_PATCH(name)



#define UNIVERSAL_SIGNATURE(signature) \
    [](Platform platform) -> std::optional<OffsetOrString> { \
        return signature; \
    }

#define UNKNOWN_SIGNATURE() \
    [](Platform platform) -> std::optional<OffsetOrString> { \
        return std::nullopt; \
    }

#define PLATFORM_SIGNATURES(...) \
    [](Platform platform) -> std::optional<OffsetOrString> { \
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