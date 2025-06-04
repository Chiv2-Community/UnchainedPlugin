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
    std::vector<std::reference_wrapper<Patch>> pending_patches;
    std::map<std::string, uint64_t> hook_offsets;
    BuildMetadata& current_build_metadata;
    uintptr_t base_addr;
    MODULEINFO module_info;


public:
    PatchManager(const HMODULE base_addr, const MODULEINFO &module_info, BuildMetadata& current_build_metadata)
        :current_build_metadata(current_build_metadata) {
        this->base_addr = reinterpret_cast<uintptr_t>(base_addr);
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
        GLOG_DEBUG("{} : Registered patch", patch.get_name());
        pending_patches.push_back(std::ref(patch));
        return true;
    }

    /**
     * Enables all patches that have been registered using register_hook().
     *
     * @return true if all patches were successfully enabled, false otherwise.
     */
    inline bool apply_patches() {
        for (auto& patch : pending_patches) {
            apply_patch(&patch.get(), true);
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

    bool apply_patch(Patch* patch, bool batched = false) {
        auto patch_name = patch->get_name();

        try {
            const auto maybe_offset = this->current_build_metadata.GetOffset(patch_name);
            if (!maybe_offset.has_value()) {
                GLOG_ERROR("{} : No offset found for patch.", patch_name);
                if (batched) failed_hooks.push_back(patch_name);
                return false;
            }

            const auto result = patch->apply(base_addr + maybe_offset.value());

            if (result) {
                GLOG_INFO("{} : Successfully enabled patch at offset 0x{:X}", patch_name, maybe_offset.value());
            } else {
                GLOG_ERROR("{} : Failed to enable patch", patch_name);
                if (batched) failed_hooks.push_back(patch_name);
            }

            return result;
        } catch (const std::exception& e) {
            GLOG_ERROR("{}: Failed to enable patch ({})", patch_name, e.what());
            if (batched) failed_hooks.push_back(patch_name);
            return false;
        }
    }

};