#pragma once

#include <functional>
#include <memory>
#include "PatchManager.hpp"

#define DEFAULT_PATCH_PRIORITY 0

#define COMMA ,

inline std::vector<std::unique_ptr<Patch>> ALL_REGISTERED_PATCHES;

inline Patch* register_patch(
    std::unique_ptr<Patch> patch
) {
    ALL_REGISTERED_PATCHES.push_back(std::move(patch));
    return ALL_REGISTERED_PATCHES.back().get();
}

/**
 *
 * @param name
 * @param apply_predicate Predicate returning true when the patch should apply
 * @param return_type The return type of the hooked function
 * @param parameters The parameters (wrapped in parens) of the hooked function
 * @param priority The priority of the patch
 */
#define REGISTER_PRIORITIZED_HOOK_PATCH(name, apply_predicate, priority, return_type, parameters) \
    static auto name##_predicate = apply_predicate; \
    /* A little bit of a hack so we can use a forward declaration here */ \
    struct name##_hook_struct { static return_type hook_fn parameters; }; \
    return_type(*o_##name)parameters = nullptr; \
    return_type (*hk_##name)parameters = name##_hook_struct::hook_fn; \
    static auto name##_Patch = register_patch(std::make_unique<HookPatch>( \
        HookPatch( \
            #name, \
            name##_predicate, \
            reinterpret_cast<void**>(&o_##name), \
            reinterpret_cast<void*>(&name##_hook_struct::hook_fn), \
            priority \
        ) \
    )); \
    return_type name##_hook_struct::hook_fn parameters

#define REGISTER_HOOK_PATCH(name, apply_predicate, return_type, parameters) \
    REGISTER_PRIORITIZED_HOOK_PATCH(name, apply_predicate, DEFAULT_PATCH_PRIORITY, return_type, parameters)

/**
 * Creates a patch that replaces bytes starting at a given address
 *
 * @param name
 * @param apply_predicate Predicate returning true when the patch should apply
 * @param additional_offset Function returning some additional offset
 * @param replacement_bytes A single byte, or vector of bytes.
 * @param priority The priority of the patch. If 100 or higher, will be applied in the prelim stages.
 */
#define REGISTER_PRIORITIZED_BYTE_PATCH(name, apply_predicate, priority, additional_offset, replacement_bytes) \
    static auto name##_predicate = apply_predicate; \
    static auto name##_patch = register_patch(std::make_unique<ByteReplacementPatch>(\
        ByteReplacementPatch(#name, name##_predicate, additional_offset, replacement_bytes, priority) \
    ));

#define REGISTER_BYTE_PATCH(name, apply_predicate, additional_offset, replacement_bytes) \
    REGISTER_PRIORITIZED_BYTE_PATCH(name, apply_predicate, DEFAULT_PATCH_PRIORITY, additional_offset, replacement_bytes)

/**
 * Creates a patch that replaces bytes starting at a given address
 *
 * @param name
 * @param apply_predicate Predicate returning true when the patch should apply
 * @param additional_offset Function returning some additional offset
 * @param size The number of bytes to replace with Nop (0x90)
 * @param priority The priority of the patch
 */
#define REGISTER_PRIORITIZED_NOP_PATCH(name, apply_predicate, priority, additional_offset, size) \
    static auto name##_predicate = apply_predicate; \
    static auto name##_patch = register_patch(std::make_unique<NopPatch>(\
        NopPatch(#name, name##_predicate, additional_offset, size, priority) \
    ));

#define REGISTER_NOP_PATCH(name, apply_predicate, additional_offset, size) \
    REGISTER_PRIORITIZED_NOP_PATCH(name, apply_predicate, DEFAULT_PATCH_PRIORITY, additional_offset, size)

#define APPLY_ALWAYS [](Patch* p){ return true; }
#define APPLY_NEVER [](Patch* p){ return false; }
#define APPLY_WHEN(condition) [](Patch* p){ return condition; }

#define EGS_OFFSET(x) case EGS: return x;
#define STEAM_OFFSET(x) case STEAM: return x;
#define ADDITIONAL_PLATFORM_OFFSETS(...) \
    []() { switch (g_state->GetCLIArgs().platform) { \
        __VA_ARGS__ \
        default: return 0; \
    }}

#define NO_ADDITIONAL_OFFSET \
    []() { return 0; }
