#pragma once

#include <functional>
#include <optional>
#include <string>


class SignatureHeuristic {
private:
    const std::string name;
    const std::function<uint8_t(const std::string&)> signature_matcher;
    const std::function<uint64_t(const std::string&, uint64_t)> address_calculator;

public:
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