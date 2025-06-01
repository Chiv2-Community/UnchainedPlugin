#pragma once

#include <functional>
#include <optional>
#include <string>

/**
 * For signatures who's head byte does not represent the first byte of the target function, a SignatureHeuristic may be
 * defined to calculate the function address using some other method.
 *
 * See heuristics/relative_procedure_call_heuristic.h for an example.
 */
class SignatureHeuristic {
private:
    const std::string name;
    const std::function<uint8_t(const std::string&)> signature_matcher;
    const std::function<uint64_t(const std::string&, uint64_t)> address_calculator;

public:
    /**
     * @param name The name of the heuristic. Used for logging.
     * @param signature_matcher A method returning 0-255, representing the confidence with which this heuristic matches the given signature.
     * @param address_calculator A method that takes the signature pattern and signature address, then returns the actual address of the target function.
     */
    SignatureHeuristic(const std::string name, const std::function<uint8_t(const std::string&)> signature_matcher, const std::function<uint64_t(const std::string&, uint64_t)> address_calculator):
        name(name), signature_matcher(signature_matcher), address_calculator(address_calculator) {}

    inline uint8_t matches_signature(std::string signature) const {
        return signature_matcher(signature);
    }

    inline uint64_t calculate_address(const std::string& signature, const uint64_t signature_address) const {
        return address_calculator(signature, signature_address);
    }

    inline std::string get_name() const {
        return name;
    }
};