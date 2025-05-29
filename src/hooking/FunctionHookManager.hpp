#pragma once

#include "FunctionHook.hpp"
#include "../hooks/sigs.h"
#include <vector>
#include <string>

#include "HookData.hpp"
#include "../state/global_state.hpp"

/**
 * The FunctionHookManager keeps track of hooks that will need their signatures to be scanned and
 * enabled via minhook.
 *
 * g_state from global_state.hpp is used to track build metadata.
 */
class FunctionHookManager {
private:
    std::vector<std::string> failed_hooks;
    std::vector<std::tuple<std::string, std::function<bool()>, std::function<MH_STATUS()>>> pending_hooks;
    std::map<std::string, uint64_t> hook_offsets;
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
    FunctionHookManager(const HMODULE base_addr, const MODULEINFO &module_info, const Platform platform) {
        this->base_addr = base_addr;
        this->module_info = module_info;
        this->failed_hooks = {};
        this->pending_hooks = {};
        this->platform = platform;
    };

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
        uint64_t offset = g_state->GetBuildMetadata().GetOffset(hookData.name).value_or(0);

        if (offset == 0) {
            address = (uint64_t)Sig::find(base_addr, module_info.SizeOfImage, signature.value().c_str());

            if (address == 0) {
                GLOG_WARNING("!! -> {} : nullptr. Signature requires updating", hookData.name);
                failed_hooks.push_back(hookData.name);
                return false;
            }

            offset = address - (uint64_t)(base_addr);
            g_state->GetBuildMetadata().SetOffset(hookData.name, offset);
        } else {
            address = (uint64_t)(base_addr) + offset;
        }

        GLOG_INFO("?? -> {} : 0x{:X}", hookData.name, offset);

        pending_hooks.push_back(std::make_tuple(
            hookData.name,
            hookData.should_attach,
            [hook = hookData.hook, address, trampoline = hookData.trampoline]() {
                auto result = MH_CreateHook(
                    reinterpret_cast<void*>(address),
                    hook,
                    trampoline
                );
                if (result != MH_OK) return result;

                return MH_EnableHook(reinterpret_cast<void*>(address));
            }
        ));

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
            [&hook_name](const auto& enabler) {
                std::string name = std::get<0>(enabler);
                return name == hook_name;
            });

        if (it != pending_hooks.end()) {
            auto should_attach = std::get<1>(*it);
            auto hook_enabler = std::get<2>(*it);
            try {
                if (!should_attach()) {
                    GLOG_DEBUG("Skipping enablement of hook '{}'.  State predicate returned false.", hook_name);
                    return true;
                }
                auto result = log_and_validate_mh_status(hook_name, hook_enabler());
                pending_hooks.erase(it);
                return result;
            } catch (const std::exception& e) {
                GLOG_ERROR("Failed to enable hook: '{}': {}", hook_name, e.what());
                pending_hooks.erase(it);
                return false;
            }
        } else {
            GLOG_WARNING("Hook '{}' not registered, but enable_hook was called with it.", hook_name);
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
        for (const auto& [name, should_attach, enabler] : pending_hooks) {
            try {
                if (!should_attach()) {
                    GLOG_DEBUG("Skipping enablement of hook '{}'.  State predicate returned false.", name);
                    return true;
                }
                auto result = log_and_validate_mh_status(name, enabler());
                if (!result) {
                    failed_hooks.push_back(name);
                }
            } catch (const std::exception& e) {
                GLOG_ERROR("Failed to enable hook '{}': {}", name, e.what());
                failed_hooks.push_back(name);
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