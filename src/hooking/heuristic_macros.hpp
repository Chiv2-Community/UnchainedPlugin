#pragma once
#include <vector>

#include "SignatureHeuristic.hpp"

static std::vector<std::unique_ptr<SignatureHeuristic>> all_heuristics = {};

static SignatureHeuristic* register_heuristic(SignatureHeuristic heuristic) {
    all_heuristics.push_back(std::make_unique<SignatureHeuristic>(heuristic));
    return all_heuristics.back().get();
}

#define CREATE_HEURISTIC(name, match_func, address_calculation) \
    static std::function<uint8_t(const std::string&)> name##_heuristic_matcher = match_func; \
    static std::function<uint64_t(const std::string&, const uint64_t)> name##_heuristic_address_calculator = address_calculation; \
    static SignatureHeuristic* name##_signature_heuristic = register_heuristic(SignatureHeuristic(#name, name##_heuristic_matcher, name##_heuristic_address_calculator));
