
#pragma once

#include "FunctionHook.hpp"
#include <sigs.h>
#include <vector>
#include <string>


class FunctionHookManager {
private: 
    std::vector<std::wstring> failed_hooks;
    std::vector<std::tuple<std::wstring, std::function<void()>>> pending_hooks;
    std::map<std::string, uint64_t> hook_offsets;
    BuildType build;
    HMODULE base_addr;
    MODULEINFO module_info;
    Platform platform;

public:
    FunctionHookManager(HMODULE base_addr, MODULEINFO module_info, BuildType build, Platform platform) {
        this->base_addr = base_addr;
        this->module_info = module_info;
        this->failed_hooks = std::vector<std::wstring>();
        this->pending_hooks = std::vector<std::tuple<std::wstring, std::function<void()>>>();
        this->platform = platform;
        this->build = build;
    };

    /**
     * Registers a hook for the specified platform. 
     * This function finds the signature for the hook, calculates the address, and sets up the hook.
     * If the signature is not found or the address is null, it logs an error and returns false.
     * If the hook is successfully prepared, it adds the hook to a list of hooks pending to be enabled.
     * Call enable_hooks() to enable all prepared hooks.
     * 
     * @param hook The FunctionHook object containing the hook details.
     */
    template<typename RetType, typename... Args>
    inline bool register_hook(FunctionHook<RetType, Args...>& hook) {
        LOG_DEBUG(L"Registering hook %s", hook.get_name());

        const auto signature = hook.get_signature(platform);
        if (!signature.has_value()) {
            LOG_WARNING(L"!! -> %s : no signature for platform '%s'", hook.get_name().c_str(), platform_to_string.at(platform).c_str());
            return false;
        }

        uint64_t address = 0;
        uint64_t offset = build.offsets.at(hook.get_name());
        if (offset == 0) {
            address = (uint64_t)Sig::find(baseAddr, moduleInfo.SizeOfImage, signature.value().c_str());

            if (address == 0) {
                LOG_WARNING(L"!! -> %s : nullptr. Signature requires updating", hook.get_name().c_str());
                failed_hooks.push_back(hook.get_name());
                return false;
            }

            offset = address - (uint64_t)(baseAddr);
            build.offsets[hook.get_name()] = offset;
        } else {
            address = (uint64_t)(baseAddr) + offset;
        }

        LOG_INFO(L"?? -> %s : 0x%llx", hook.get_name().c_str(), offset);

        auto hook_function = hook.get_hook_function();
        auto original_function = hook.get_original();

        pending_hooks.push_back(std::make_tuple(hook.get_name(), [&hook_function, &original_function, &address]() {
            MH_CreateHook(reinterpret_cast<void*>(address), &hook_function, reinterpret_cast<void**>(&original_function));
            MH_EnableHook(reinterpret_cast<void*>(address));
        }));
    
        return true;
    }

    /**
     * Enables a specific hook that has been registered using register_hook().
     * 
     * @return true if all hooks were successfully enabled, false otherwise.
     */
    template<typename RetType, typename... Args>
    inline bool enable_hook(const FunctionHook<RetType, Args...>& hook) {
        auto hook_name = hook.get_name();
        auto it = std::find_if(pending_hooks.begin(), pending_hooks.end(),
            [&hook_name](const auto& enabler) {
                return std::get<0>(enabler) == hook_name;
            });

        if (it != pending_hooks.end()) {
            auto hook_enabler = std::get<1>(*it);
            try {
                hook_enabler();
                LOG_INFO("Successfully hooked '%s'\n", hook_name.c_str());
                pending_hooks.erase(it);
                return true;
            } catch (const std::exception& e) {
                LOG_ERROR("Failed to enable hook '%s': %s\n", hook_name.c_str(), e.what());
                pending_hooks.erase(it);
                return false;
            }
        } else {
            LOG_WARNING("Hook '%s' not found in enablers.\n", hook_name.c_str());
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
        for (const auto& [name, enabler] : pending_hooks) {
            try {
                enabler();
                LOG_INFO("Successfully hooked '%s'", name.c_str());
            } catch (const std::exception& e) {
                LOG_ERROR("Failed to enable hook '%s': %s\n", name.c_str(), e.what());
                failed_hooks.push_back(name);
            }
        }

        pending_hooks.clear();

        if (!failed_hooks.empty()) {
            LOG_ERROR("Failed to enable the following hooks:\n");
            for (const auto& hook_name : failed_hooks) {
                LOG_ERROR(" - %s\n", hook_name.c_str());
            }

            failed_hooks.clear();

            return false;
        }

        return true;
    }
};