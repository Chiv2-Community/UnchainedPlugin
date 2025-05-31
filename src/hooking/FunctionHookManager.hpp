#pragma once

#include <vector>
#include <string>
#include <Sig.hpp>

#include "HookData.hpp"
#include "../logging/global_logger.hpp"

/**
 * The FunctionHookManager keeps track of hooks that will need their signatures to be scanned and
 * enabled via minhook.
 */
class FunctionHookManager {
private:
    std::vector<std::string> failed_hooks;
    std::vector<HookData> pending_hooks;
    std::map<std::string, uint64_t> hook_offsets;
    BuildMetadata& current_build_metadata;
    HMODULE base_addr;
    MODULEINFO module_info;
    Platform platform;


    static bool log_and_validate_mh_status(std::string hook_name, MH_STATUS status) {
        if (status == MH_OK) {
            GLOG_DEBUG("Successfully hooked '{}'", hook_name);
            return true;
        }

        GLOG_ERROR("Minhook error while hooking '{}': {}", hook_name, MH_StatusToString(status));
        return false;
    }

public:
    FunctionHookManager(const HMODULE base_addr, const MODULEINFO &module_info, const Platform platform, BuildMetadata& current_build_metadata): current_build_metadata(current_build_metadata) {
        this->base_addr = base_addr;
        this->module_info = module_info;
        this->failed_hooks = {};
        this->pending_hooks = {};
        this->platform = platform;
    } ;

    /**
     * Registers a hook using a HookData structure.
     * This function finds the signature for the hook, calculates the address, and sets up the hook.
     * If the signature is not found or the address is null, it logs an error and returns false.
     * If the hook is successfully prepared, it adds the hook to a list of hooks pending to be enabled.
     * Call enable_hooks() to enable all prepared hooks.
     *
     * The should_enable function will pass in the current global state at the time of hook enablement.
     * If it returns false, the hook will not be enabled.
     *
     * @param hookData The HookData object containing all hook details.
     * @return true if the hook was successfully registered, false otherwise.
     */
    inline bool register_hook(const HookData& hookData) {
        GLOG_TRACE("Registering hook '{}'", hookData.name);

        const auto signature = hookData.select_signature_for_platform(platform);
        if (!signature.has_value()) {
            GLOG_WARNING("!! -> {} : no signature for platform '{}'", hookData.name, platform_to_string.at(platform));
            return false;
        }

        uint64_t address = 0;
        uint64_t offset = current_build_metadata.GetOffset(hookData.name).value_or(0);

        if (offset == 0) {
            address = (uint64_t)Sig::find(base_addr, module_info.SizeOfImage, signature.value().c_str());

            if (address == 0) {
                GLOG_WARNING("!! -> {} : nullptr. Signature requires updating", hookData.name);
                failed_hooks.push_back(hookData.name);
                return false;
            }

            offset = address - (uint64_t)(base_addr);
            current_build_metadata.SetOffset(hookData.name, offset);
        } else {
            address = (uint64_t)(base_addr) + offset;
        }

        GLOG_INFO("?? -> {} : 0x{:X}", hookData.name, offset);

        // Create a copy of hookData that includes the calculated address
        HookData registeredHook = hookData;
        registeredHook.address = address;

        if (!registeredHook.scan_only)
            pending_hooks.push_back(registeredHook);

        return true;
    }

    /**
     * Enables a specific hook that has been registered using register_hook().
     *
     * @return true if all hooks were successfully enabled, false otherwise.
     */
    inline bool enable_hook(const HookData* hook) {
        auto hook_name = hook->name;
        auto it = std::find_if(pending_hooks.begin(), pending_hooks.end(),
            [&hook_name](const auto& hookData) {
                return hookData.name == hook_name;
            });

        if (it == pending_hooks.end()) {
            GLOG_WARNING("Hook '{}' not registered, but enable_hook was called with it.", hook_name);
            return false;
        }
        try {
            if (!it->should_attach()) {
                GLOG_DEBUG("Skipping enablement of hook '{}'.  State predicate returned false.", hook_name);
                return true;
            }

            auto result = MH_CreateHook(
                reinterpret_cast<void*>(it->address),
                it->hook,
                it->trampoline
            );

            if (result != MH_OK) {
                return log_and_validate_mh_status(hook_name, result);
            }

            result = MH_EnableHook(reinterpret_cast<void*>(it->address));
            auto success = log_and_validate_mh_status(hook_name, result);

            pending_hooks.erase(it);
            return success;
        } catch (const std::exception& e) {
            GLOG_ERROR("Failed to enable hook: '{}': {}", hook_name, e.what());
            pending_hooks.erase(it);
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
    inline bool enable_hooks() {
        for (const auto& hookData : pending_hooks) {
            try {
                if (!hookData.should_attach()) {
                    GLOG_DEBUG("Skipping enablement of hook '{}'.  State predicate returned false.", hookData.name);
                    continue;
                }

                auto result = MH_CreateHook(
                    reinterpret_cast<void*>(hookData.address),
                    hookData.hook,
                    hookData.trampoline
                );

                if (result != MH_OK) {
                    log_and_validate_mh_status(hookData.name, result);
                    failed_hooks.push_back(hookData.name);
                    continue;
                }

                result = MH_EnableHook(reinterpret_cast<void*>(hookData.address));
                if (!log_and_validate_mh_status(hookData.name, result)) {
                    failed_hooks.push_back(hookData.name);
                }
            } catch (const std::exception& e) {
                GLOG_ERROR("Failed to enable hook '{}': {}", hookData.name, e.what());
                failed_hooks.push_back(hookData.name);
            }
        }

        pending_hooks.clear();

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