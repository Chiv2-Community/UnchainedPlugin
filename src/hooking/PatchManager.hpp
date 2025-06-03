#pragma once

#include <vector>
#include <string>
#include <Sig.hpp>
#include <variant>

#include "Patch.hpp"
#include "SignatureHeuristic.hpp"
#include "../logging/global_logger.hpp"


/**
 * The FunctionHookManager keeps track of hooks that will need their signatures to be scanned and
 * enabled via minhook.
 */
class PatchManager {
private:
    const std::vector<std::unique_ptr<SignatureHeuristic>>& heuristics;
    std::vector<std::string> failed_hooks;
    std::vector<Patch> pending_patches;
    std::map<std::string, uint64_t> hook_offsets;
    BuildMetadata& current_build_metadata;
    HMODULE base_addr;
    MODULEINFO module_info;


    uint64_t apply_heuristics(const std::string& hook_name, const std::string& signature, uint64_t address) const {
        if (heuristics.empty()) {return address;}

        GLOG_TRACE("Running Heuristics checks for {}", hook_name);

        uint8_t best_match = 0;
        const SignatureHeuristic* current_best = nullptr;
        for (const auto& heuristic : heuristics) {
            auto match_confidence = heuristic->matches_signature(signature);
            if (match_confidence > 0) {
                GLOG_TRACE("  - '{}' matched signature with confidence {}", heuristic->get_name(), match_confidence);
            }

            if (match_confidence > best_match) {
                best_match = match_confidence;
                current_best = heuristic.get();
            }
        }


        if (best_match > 0 && current_best != nullptr) {
            GLOG_TRACE("Using heuristic '{}' with confidence {}", current_best->get_name(), best_match);
            return current_best->calculate_address(signature, address);
        }

        GLOG_TRACE("No heuristics matched signature. Using signature address.");
        return address;
    }

public:
    PatchManager(const HMODULE base_addr, const MODULEINFO &module_info, BuildMetadata& current_build_metadata, const std::vector<std::unique_ptr<SignatureHeuristic>> &heuristics)
        :heuristics(heuristics),
        current_build_metadata(current_build_metadata) {
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
        GLOG_TRACE("Registering hook '{}' for platform '{}'", patch.name, platform_to_string.at(current_build_metadata.GetPlatform()));

        const auto maybe_signature_or_offset = patch.select_signature_for_platform(current_build_metadata.GetPlatform());
        if (!maybe_signature_or_offset.has_value()) {
            GLOG_WARNING("{}: no signature for platform '{}'", patch.name, platform_to_string.at(current_build_metadata.GetPlatform()));
            return false;
        }

        auto signature_or_offset = maybe_signature_or_offset.value();
        uint64_t address = 0;
        uint64_t offset = current_build_metadata.GetOffset(patch.name).value_or(0);

        if (offset == 0) {

            if (std::holds_alternative<std::string>(signature_or_offset)) {
                const auto signature = std::get<std::string>(signature_or_offset);
                GLOG_TRACE("{} : searching for signature", patch.name);
                address = (uint64_t)Sig::find(base_addr, module_info.SizeOfImage, signature.c_str());

                if (address == 0) {
                    GLOG_WARNING("{} : nullptr. Signature requires updating", patch.name);
                    failed_hooks.push_back(patch.name);
                    return false;
                }

                address = apply_heuristics(patch.name, signature, address);

                offset = address - (uint64_t)(base_addr);
                current_build_metadata.SetOffset(patch.name, offset);
            } else if (std::holds_alternative<uint64_t>(signature_or_offset)) {
                offset = std::get<uint64_t>(signature_or_offset);
                GLOG_TRACE("{} : using hardcoded offset 0x{:X}", patch.name, offset);
                address = (uint64_t)(base_addr) + offset;
            }

            if (address == 0) {
                GLOG_WARNING("{} : nullptr. Signature requires updating", patch.name);
                failed_hooks.push_back(patch.name);
                return false;
            }

        } else {
            address = (uint64_t)(base_addr) + offset;
        }

        GLOG_INFO("{} : 0x{:X}", patch.name, offset);

        patch.address = address;
        pending_patches.push_back(patch);

        return true;
    }

    /**
     * Enables a specific hook that has been registered using register_hook().
     *
     * @return true if all hooks were successfully enabled, false otherwise.
     */
    inline bool apply_patch(const Patch* hook) {
        auto hook_name = hook->name;
        auto it = std::find_if(pending_patches.begin(), pending_patches.end(),
            [&hook_name](const auto& hookData) {
                return hookData.name == hook_name;
            });

        if (it == pending_patches.end()) {
            GLOG_WARNING("Hook '{}' not registered, but enable_hook was called with it.", hook_name);
            return false;
        }
        try {
            auto result = it->apply();

            pending_patches.erase(it);
            return result;
        } catch (const std::exception& e) {
            GLOG_ERROR("Failed to enable hook: '{}': {}", hook_name, e.what());
            pending_patches.erase(it);
            return false;
        }
    }

    /**
     * Enables all hooks that have been registered using register_hook().
     * It iterates through the list of hook enablers, attempts to enable each hook,
     * and logs any failures.
     *
     * @return true if all hooks were successfully enabled, false otherwise.
     */
    inline bool apply_patches() {
        for (const auto& patch : pending_patches) {
            try {
                auto result = patch.apply();
                if (!result) {
                    failed_hooks.push_back(patch.name);
                }

            } catch (const std::exception& e) {
                GLOG_ERROR("Failed to enable hook '{}': {}", patch.name, e.what());
                failed_hooks.push_back(patch.name);
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