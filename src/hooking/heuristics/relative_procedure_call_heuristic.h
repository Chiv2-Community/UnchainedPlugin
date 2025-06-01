#pragma once

#include "../heuristic_macros.hpp"

/**
 * Procedure calls using the E8 (call rel32) opcode can be used to find the actual location of a function address.
 *
 * The 4 bytes following the E8 opcode represent a signed 32-bit relative address location.  This heuristic leverages
 * that relative address to determine where a function is defined.
 */
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
