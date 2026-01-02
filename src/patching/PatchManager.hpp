#pragma once

#include <vector>
#include <string>
#include <variant>
#include <algorithm>

#include "Patch.hpp"
#include "../logging/global_logger.hpp"

struct RustBuildInfo;

extern "C" uint64_t build_info_get_offset(const RustBuildInfo* info, const char* name);

/**
 * The PatchManager keeps track of patches to be enabled
 */
class PatchManager {
private:
    std::vector<std::string> failed_patches;
    std::vector<std::reference_wrapper<Patch>> pending_patches;
    uintptr_t base_addr;
    MODULEINFO module_info;
    const RustBuildInfo* build_info;


public:
    PatchManager(const HMODULE base_addr, const MODULEINFO &module_info, const RustBuildInfo* build_info) {
        this->base_addr = reinterpret_cast<uintptr_t>(base_addr);
        this->module_info = module_info;
        this->build_info = build_info;
        this->failed_patches = {};
        this->pending_patches = {};
    } ;

    inline bool register_patch(Patch& patch) {
        GLOG_DEBUG("Registered patch '{}'", patch.get_name());
        pending_patches.push_back(std::ref(patch));
        return true;
    }

    /**
     * Enables all patches that have been registered using register_patch().
     *
     * @return true if all patches were successfully enabled, false otherwise.
     */
    inline bool apply_patches() {
        std::sort(pending_patches.begin(), pending_patches.end(), [](const Patch& a, const Patch& b) {
            return a.get_priority() > b.get_priority();
        });

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
        GLOG_TRACE("Applying patch '{}'", patch_name);

        try {
            const uint64_t offset = build_info_get_offset(build_info, patch_name.c_str());
            if (offset == 0) {
                GLOG_ERROR("No offset found for patch '{}'", patch_name);
                if (batched) failed_patches.push_back(patch_name);
                return false;
            }

            if (patch->is_applied(base_addr + offset))
            {
                GLOG_INFO("0x{:X} : Already enabled patch '{}'", offset, patch_name);
                return true;
            }

            const auto result = patch->apply(base_addr + offset);

            switch (result) {
                case APPLY_SUCCESS:
                    GLOG_INFO("0x{:X} : Successfully enabled patch '{}'",  offset, patch_name);
                    break;
                case APPLY_FAILED:
                    GLOG_ERROR("Failed to enable patch '{}'", patch_name);
                    if (batched) failed_patches.push_back(patch_name);
                    break;
                case APPLY_DISABLED:
                    GLOG_DEBUG("Patch '{}' not enabled", patch_name);
                    break;
            }

            return result;
        } catch (const std::exception& e) {
            GLOG_ERROR("Failed to enable patch '{}'. {}", patch_name, e.what());
            if (batched) failed_patches.push_back(patch_name);
            return false;
        }
    }

};