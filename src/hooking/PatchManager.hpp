#pragma once

#include <vector>
#include <string>
#include <Sig.hpp>
#include <variant>

#include "Patch.hpp"
#include "../logging/global_logger.hpp"


/**
 * The FunctionHookManager keeps track of hooks that will need their signatures to be scanned and
 * enabled via minhook.
 */
class PatchManager {
private:
    std::vector<std::string> failed_hooks;
    std::vector<Patch> pending_patches;
    std::map<std::string, uint64_t> hook_offsets;
    BuildMetadata& current_build_metadata;
    HMODULE base_addr;
    MODULEINFO module_info;


public:
    PatchManager(const HMODULE base_addr, const MODULEINFO &module_info, BuildMetadata& current_build_metadata)
        :current_build_metadata(current_build_metadata) {
        this->base_addr = base_addr;
        this->module_info = module_info;
        this->failed_hooks = {};
        this->pending_patches = {};
    } ;

    /**
     * Registers a patch using a Patch structure.
     * This function finds the signature for the patch and calculates the address.
     * If the signature is not found or the address is null, it logs an error and returns false.
     * If the patch is successfully prepared, it adds the patch to a list of hooks pending to be enabled.
     * Call enable_patches() to enable all prepared patches.
     *
     * The should_enable function will pass in the current global state at the time of patch application.
     * If it returns false, the patch will not be applied.
     *
     * @param patch The Patch object containing all patch details.
     * @return true if the patch was successfully registered, false otherwise.
     */
    inline bool register_patch(Patch& patch) {
        GLOG_TRACE("Registering hook '{}' for platform '{}'", patch.get_name(), platform_to_string.at(current_build_metadata.GetPlatform()));

        uintptr_t address = 0;
        uintptr_t offset = current_build_metadata.GetOffset(patch.get_name()).value_or(0);
        if (offset == 0) {
            auto maybe_address = patch.get_address(reinterpret_cast<uintptr_t>(base_addr), module_info.SizeOfImage, current_build_metadata.GetPlatform());
            if (!maybe_address.has_value()) { return false; }

            address = maybe_address.value();
            offset = address - reinterpret_cast<uint64_t>(base_addr);
            current_build_metadata.SetOffset(patch.get_name(), offset);
        } else {
            GLOG_TRACE("Loaded offset from config file: 0x{:X}", offset);
            address = reinterpret_cast<uint64_t>(base_addr) + offset;
        }

        GLOG_INFO("{} : 0x{:X}", patch.get_name(), offset);

        pending_patches.push_back(patch);
        return true;
    }

    /**
     * Enables a specific hook that has been registered using register_hook().
     *
     * @return true if all hooks were successfully enabled, false otherwise.
     */
    inline bool apply_patch(const Patch* patch) {
        auto patch_name = patch->get_name();
        auto it = std::find_if(pending_patches.begin(), pending_patches.end(),
            [&patch_name](const auto& other_patch) {
                return other_patch.get_name() == patch_name;
            });

        if (it == pending_patches.end()) {
            GLOG_WARNING("Patch '{}' not registered, but apply_patch was called with it.", patch_name);
            return false;
        }
        try {
            const auto result = it->apply(reinterpret_cast<uintptr_t>(base_addr), module_info.SizeOfImage, current_build_metadata.GetPlatform() );
            pending_patches.erase(it);
            return result;
        } catch (const std::exception& e) {
            GLOG_ERROR("Failed to enable patch: '{}': {}", patch_name, e.what());
            pending_patches.erase(it);
            return false;
        }
    }

    /**
     * Enables all patches that have been registered using register_hook().
     *
     * @return true if all patches were successfully enabled, false otherwise.
     */
    inline bool apply_patches() {
        for (auto& patch : pending_patches) {
            try {
                auto result = patch.apply(reinterpret_cast<uintptr_t>(base_addr), module_info.SizeOfImage, current_build_metadata.GetPlatform());
                if (!result) {
                    failed_hooks.push_back(patch.get_name());
                }

            } catch (const std::exception& e) {
                GLOG_ERROR("Failed to enable hook '{}': {}", patch.get_name(), e.what());
                failed_hooks.push_back(patch.get_name());
            }
        }

        pending_patches.clear();

        if (!failed_hooks.empty()) {
            GLOG_ERROR("Failed to enable the following hooks:");
            for (const auto& hook_name : failed_hooks) {
                GLOG_ERROR(" - {}", hook_name);
            }

            failed_hooks.clear();

            return false;
        }

        return true;
    }
};