#pragma once

#include <functional>
#include "PatchManager.hpp"
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
 */
#define REGISTER_HOOK_PATCH(name, apply_predicate, return_type, parameters) \
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
            reinterpret_cast<void*>(&name##_hook_struct::hook_fn) \
        ) \
    )); \
    return_type name##_hook_struct::hook_fn parameters

/**
 * Creates a patch that replaces bytes starting at a given address
 *
 * @param name
 * @param apply_predicate Predicate returning true when the patch should apply
 * @param replacement_bytes A single byte, or vector of bytes.
 */
#define REGISTER_BYTE_PATCH(name, apply_predicate, replacement_bytes) \
    static auto name##_predicate = apply_predicate; \
    static auto name##_patch = register_patch(std::make_unique<ByteReplacementPatch>(\
        ByteReplacementPatch(#name, name##_predicate, replacement_bytes) \
    ));

/**
 * Creates a patch that replaces bytes starting at a given address
 *
 * @param name
 * @param apply_predicate Predicate returning true when the patch should apply
 * @param size The number of bytes to replace with Nop (0x90)
 */
#define REGISTER_NOP_PATCH(name, apply_predicate, size) \
    static auto name##_predicate = apply_predicate; \
    static auto name##_patch = register_patch(std::make_unique<NopPatch>(\
        NopPatch(#name, name##_predicate, size) \
    ));

#define APPLY_ALWAYS [](){ return true; }
#define APPLY_NEVER [](){ return false; }
#define APPLY_WHEN(condition) [](){ return condition; }