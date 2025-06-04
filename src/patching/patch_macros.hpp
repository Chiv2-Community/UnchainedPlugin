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

#define REGISTER_HOOK_PATCH(name, apply_predicate, return_type, arguments) \
    static auto name##_predicate = apply_predicate; \
    /* A little bit of a hack so we can use a forward declaration here */ \
    struct name##_hook_struct { static return_type hook_fn arguments; }; \
    return_type(*o_##name)arguments = nullptr; \
    return_type (*hk_##name)arguments = name##_hook_struct::hook_fn; \
    static auto name##_Patch = register_patch(std::make_unique<HookPatch>( \
        HookPatch( \
            #name, \
            name##_predicate, \
            reinterpret_cast<void**>(&o_##name), \
            reinterpret_cast<void*>(&name##_hook_struct::hook_fn) \
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
#define REGISTER_BYTE_PATCH(name, attach_predicate, replacement_byte) \
    static auto name##_predicate = attach_predicate; \
    static auto name##_patch = register_patch(std::make_unique<ByteReplacementPatch>(\
        ByteReplacementPatch(#name, name##_predicate, replacement_byte) \
    ));


#define ATTACH_ALWAYS [](){ return true; }
#define ATTACH_WHEN(condition) [](){ return condition; }