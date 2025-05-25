
#pragma once

#include "FunctionHook.hpp"
#include <Sigs.h>
#include <vector>
#include <string>


class FunctionHookEnabler {
private: 
    std::vector<std::string&> failed_hooks;
    HMODULE base_addr;
    MODULEINFO module_info;

public:
    FunctionHookEnabler(HMODULE base_addr, MODULEINFO module_info) {
        this->base_addr = base_addr;
        this->module_info = module_info;
        this->failed_hooks = std::vector<std::string&>();
    };

    template<typename RetType, typename... Args>
    static inline bool enable_hook(std::string platform, FunctionHook<RetType, Args...>& hook) {
        auto address = Sig::find(baseAddr, moduleInfo.SizeOfImage, hook.get_signature(platform));
        if (address == nullptr) {
		    printf("!! -> %s : nullptr\n", hook.get_name().c_str());
            failed_hooks.push_back(hook.get_name());
            return false;
        }

        auto offset = reinterpret_cast<uint64_t>(address) - reinterpret_cast<uint64_t>(baseAddr);
        auto hook_function = hook.call_original;
        hook.set_hook_enabled(offset, reinterpret_cast<typename FunctionHook<RetType, Args...>::FunctionType>(address), true);
        printf("?? -> %s : 0x%llx\n", hook.get_name().c_str(), offset);
        MH_CreateHook(reinterpret_cast<void*>(address), &FunctionHook<RetType, Args...>::call, reinterpret_cast<void**>(&hook.call_original));
        MH_EnableHook(reinterpret_cast<void*>(address));
        return true;
    }
}