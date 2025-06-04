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
    std::unique_ptr<Patch> patch
) {
    g_auto_hooks.push_back(std::move(patch));
    return g_auto_hooks.back().get();
}

static auto IDENTITY = [](uintptr_t ptr) -> uintptr_t { return ptr; };
static auto FOLLOW_32BIT_RELATIVE_PROCEDURE_CALL = [](const uint64_t procedure_call_opcode_address) -> uintptr_t {
    std::int32_t relative_call_address;
    memcpy(&relative_call_address, reinterpret_cast<void*>(procedure_call_opcode_address + 1), sizeof(std::int32_t));

    constexpr std::uint32_t procedure_call_size = 5; // 1 byte for the op code, 4 bytes for the relative address
    return procedure_call_opcode_address + procedure_call_size + relative_call_address;
};

#define REGISTER_HOOK_PATCH(name, signatures_func, apply_predicate, return_type, arguments) \
    static auto name##_signature = signatures_func; \
    static auto name##_predicate = apply_predicate; \
    /* A little bit of a hack so we can use a forward declaration here */ \
    struct name##_hook_struct { static return_type hook_fn arguments; }; \
    return_type(*o_##name)arguments = nullptr; \
    return_type (*hk_##name)arguments = name##_hook_struct::hook_fn; \
    static auto name##_Patch = register_patch(std::make_unique<HookSignaturePatch>( \
        HookSignaturePatch( \
            #name, \
            name##_signature, \
            IDENTITY, \
            name##_predicate, \
            reinterpret_cast<void**>(o_##name), \
            reinterpret_cast<void*>(name##_hook_struct::hook_fn) \
        ) \
    )); \
    return_type name##_hook_struct::hook_fn arguments

#define REGISTER_HOOK_PATCH_WITH_INDIRECT_OFFSET(name, signatures_func, handle_signature_address, apply_predicate, return_type, arguments) \
    static auto name##_signature = signatures_func; \
    static auto name##_predicate = apply_predicate; \
    /* A little bit of a hack so we can use a forward declaration here */ \
    struct name##_hook_struct { static return_type hook_fn arguments; }; \
    return_type(*o_##name)arguments = nullptr; \
    return_type (*hk_##name)arguments = name##_hook_struct::hook_fn; \
    static auto name##_Patch = register_patch(std::make_unique<HookSignaturePatch>( \
        HookSignaturePatch( \
            #name, \
            name##_signature, \
            handle_signature_address, \
            name##_predicate, \
            reinterpret_cast<void**>(o_##name), \
            reinterpret_cast<void*>(name##_hook_struct::hook_fn) \
        ) \
    )); \
    return_type name##_hook_struct::hook_fn arguments


/**
 * Creates a patch that replaces a byte at a given address
 *
 * @param name
 * @param signatures_func
 * @param apply_predicate Predicate returning true when the patch should apply
 */
#define REGISTER_BYTE_PATCH(name, signatures_func, attach_predicate, replacement_byte) \
    static auto name##_signature = signatures_func; \
    static auto name##_predicate = attach_predicate; \
    static auto name##_patch = register_patch(std::make_unique<ByteReplacementSignaturePatch>(\
        ByteReplacementSignaturePatch(#name, name##_signature, IDENTITY, name##_predicate, replacement_byte) \
    ));


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