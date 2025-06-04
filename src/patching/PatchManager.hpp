#pragma once

#include <vector>
#include <string>
#include <Sig.hpp>
#include <variant>

#include "Patch.hpp"
#include "../logging/global_logger.hpp"


/**
 * The PatchManager keeps track of patches to be enabled
 */
class PatchManager {
private:
    std::vector<std::string> failed_patches;
    std::vector<std::reference_wrapper<Patch>> pending_patches;
    BuildMetadata& current_build_metadata;
    uintptr_t base_addr;
    MODULEINFO module_info;


public:
    PatchManager(const HMODULE base_addr, const MODULEINFO &module_info, BuildMetadata& current_build_metadata)
        :current_build_metadata(current_build_metadata) {
        this->base_addr = reinterpret_cast<uintptr_t>(base_addr);
        this->module_info = module_info;
        this->failed_patches = {};
        this->pending_patches = {};
    } ;

    inline bool register_patch(Patch& patch) {
        GLOG_DEBUG("{} : Registered patch", patch.get_name());
        pending_patches.push_back(std::ref(patch));
        return true;
    }

    /**
     * Enables all patches that have been registered using register_patch().
     *
     * @return true if all patches were successfully enabled, false otherwise.
     */
    inline bool apply_patches() {
        for (auto& patch : pending_patches) {
            apply_patch(&patch.get(), true);
        }

        pending_patches.clear();

        if (!failed_patches.empty()) {
            GLOG_ERROR("Failed to enable the following hooks:");
            for (const auto& hook_name : failed_patches) {
                GLOG_ERROR(" - {}", hook_name);
            }

            failed_patches.clear();

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
                if (batched) failed_patches.push_back(patch_name);
                return false;
            }

            const auto result = patch->apply(base_addr + maybe_offset.value());

            if (result) {
                GLOG_INFO("{} : Successfully enabled patch at offset 0x{:X}", patch_name, maybe_offset.value());
            } else {
                GLOG_ERROR("{} : Failed to enable patch", patch_name);
                if (batched) failed_patches.push_back(patch_name);
            }

            return result;
        } catch (const std::exception& e) {
            GLOG_ERROR("{}: Failed to enable patch ({})", patch_name, e.what());
            if (batched) failed_patches.push_back(patch_name);
            return false;
        }
    }

};