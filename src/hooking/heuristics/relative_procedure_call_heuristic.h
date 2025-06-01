#pragma once

#include "../heuristic_macros.hpp"

CREATE_HEURISTIC(
    RelativeProcedureCall,
    [](const std::string &signature) {
        if (signature.starts_with("E8 ?? ?? ?? ??") || signature.starts_with("E8 ? ? ? ?"))
            return 100;

        if (signature.starts_with("E8"))
            return 50;

        return 0;
    },
    [](const std::string& s, const uint64_t procedure_call_opcode_address) {
        std::int32_t relative_call_address;
        memcpy(&relative_call_address, reinterpret_cast<void*>(procedure_call_opcode_address + 1), sizeof(std::int32_t));

        const std::uint32_t procedure_call_size = 5; // 1 byte for the op code, 4 bytes for the relative address
        return procedure_call_opcode_address + procedure_call_size + relative_call_address;
    }
)
