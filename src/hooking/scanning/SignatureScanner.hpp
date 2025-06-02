#include <cassert>
#include <vector>
#include <unordered_map>
#include <unordered_set>
#include <string>
#include <memory>
#include <iostream>
#include <cstring>
#include <intrin.h>

#ifdef __AVX2__
#endif

// Structure to represent a signature pattern
struct Signature {
    std::vector<uint8_t> pattern;  // The actual byte pattern
    std::vector<bool> mask;        // Mask to indicate wildcards (true = exact match, false = wildcard)
    std::string name;              // Name or identifier for the signature
    std::string string_pattern;

    Signature(const std::string& patternStr, const std::string& sigName) : name(sigName), string_pattern(patternStr) {
        // Parse pattern string like "48 89 5C 24 ?? 48 89 74 24 ??" or "48 89 5C 24 ? 48  89 74 24 ?"
        std::string token;
        std::istringstream stream(patternStr);

        while (stream >> token) {
            if (token == "??" || token == "?") {
                // Handle both single and double question mark wildcards
                pattern.push_back(0);
                mask.push_back(false);
            } else {
                try {
                    pattern.push_back(static_cast<uint8_t>(std::stoi(token, nullptr, 16)));
                    mask.push_back(true);
                } catch (const std::exception&) {
                    GLOG_WARNING("'{}' contains invalid signature token: {}", sigName, token);
                    // Handle invalid tokens (optional)
                    // We could either skip them or throw an exception
                }
            }
        }
    }

};

// Trie node structure
class SignatureNode {
public:
    std::unordered_map<uint8_t, std::unique_ptr<SignatureNode>> children;
    std::unique_ptr<SignatureNode> wildcard;  // Special child for wildcards
    std::vector<const Signature*> matchedSignatures;  // Signatures that end at this node

    void addSignature(const Signature* signature, size_t index = 0) {
        if (index == signature->pattern.size()) {
            matchedSignatures.push_back(signature);
            return;
        }

        if (!signature->mask[index]) {
            // This is a wildcard position
            if (!wildcard) {
                wildcard = std::make_unique<SignatureNode>();
            }
            wildcard->addSignature(signature, index + 1);
        } else {
            // This is a specific byte
            uint8_t byte = signature->pattern[index];
            if (!children[byte]) {
                children[byte] = std::make_unique<SignatureNode>();
            }
            children[byte]->addSignature(signature, index + 1);
        }
    }
};

// Result structure for matches
struct SignatureMatch {
    const Signature* signature;
    size_t offset;

    SignatureMatch(const Signature* sig, size_t off) : signature(sig), offset(off) {}
};

// Main scanner class
class SignatureScanner {
private:
    HMODULE base_addr;
    uint64_t program_size;
    std::vector<Signature> signatures;
    
    // Prefiltering structure
    struct FirstByteInfo {
        std::vector<uint8_t> requiredFirstBytes;
        bool hasWildcard = false;
    };

public:
    SignatureScanner(HMODULE base_addr, uint64_t program_size) {
        this->base_addr = base_addr;
        this->program_size = program_size;
    }

    void addSignature(const std::string& pattern, const std::string& name) {
        signatures.emplace_back(pattern, name);
    }

    std::vector<SignatureMatch> scan() {
        std::vector<SignatureMatch> matches;

        // Early return if no signatures or empty memory
        if (signatures.empty() || program_size == 0) {
            return matches;
        }

        // Build the trie for multi-pattern matching
        SignatureNode root;
        for (const auto& sig : signatures) {
            root.addSignature(&sig);
        }

        // Create a map of first bytes for quick filtering
        std::unordered_set<uint8_t> firstBytes;
        bool hasWildcardStart = false;

        for (const auto& sig : signatures) {
            if (!sig.mask.empty()) {
                if (sig.mask[0]) {
                    // This signature starts with a specific byte
                    firstBytes.insert(sig.pattern[0]);
                } else {
                    // This signature starts with a wildcard
                    hasWildcardStart = true;
                }
            }
        }

        // Get memory regions
        const uint8_t* memory = reinterpret_cast<const uint8_t*>(base_addr);

        // Scan through memory
        for (size_t i = 0; i < program_size; ++i) {
            // Quick filter: skip positions where first byte doesn't match any signature
            if (!hasWildcardStart && firstBytes.find(memory[i]) == firstBytes.end()) {
                continue;
            }

            // Try to match from this position
            matchAtPosition(memory, i, program_size, root, matches);
        }

        return matches;
    }


    std::vector<SignatureMatch> scan_one(std::string& name) {
        std::vector<SignatureMatch> matches;

        // Find the signature with the given name
        const Signature* targetSig = nullptr;
        for (const auto& sig : signatures) {
            if (sig.name == name) {
                targetSig = &sig;
                break;
            }
        }

        if (!targetSig) {
            return matches; // No signature with this name
        }

        const uint8_t* memory = reinterpret_cast<const uint8_t*>(base_addr);

        // For a single signature, use a specialized algorithm
        const auto& pattern = targetSig->pattern;
        const auto& mask = targetSig->mask;

        if (pattern.empty()) {
            return matches;
        }

        // Check if this pattern has a specific first byte (not a wildcard)
        bool hasFixedFirstByte = !pattern.empty() && mask[0];
        uint8_t firstByte = hasFixedFirstByte ? pattern[0] : 0;

        // Scan memory for matches
        for (size_t i = 0; i <= program_size - pattern.size(); ++i) {
            // Quick first-byte check
            if (hasFixedFirstByte && memory[i] != firstByte) {
                continue;
            }

            bool found = true;

            // Check if the pattern matches at this position
            for (size_t j = 0; j < pattern.size(); ++j) {
                if (mask[j] && memory[i + j] != pattern[j]) {
                    found = false;
                    break;
                }
            }

            if (found) {
                matches.emplace_back(targetSig, i);
            }
        }

        return matches;
    }

private:
    void matchAtPosition(const uint8_t* memory, size_t startPos, size_t memSize,
                    const SignatureNode& root, std::vector<SignatureMatch>& matches) {

        std::vector<std::pair<const SignatureNode*, size_t>> nodeStack;
        nodeStack.emplace_back(&root, 0);

        while (!nodeStack.empty()) {
            auto [node, depth] = nodeStack.back();
            nodeStack.pop_back();

            // Check if we have matches at this node
            for (const Signature* sig : node->matchedSignatures) {
                GLOG_TRACE("Found match for signature '{}' at offset 0x{:x}", sig->name, startPos - depth);
                matches.emplace_back(sig, startPos - depth);
            }

            // Check if we can go deeper
            if (startPos + depth >= memSize) {
                continue;
            }

            uint8_t currentByte = memory[startPos + depth];

            // Try to follow wildcard path
            if (node->wildcard) {
                nodeStack.emplace_back(node->wildcard.get(), depth + 1);
            }

            // Try to follow exact match path
            auto it = node->children.find(currentByte);
            if (it != node->children.end()) {
                nodeStack.emplace_back(it->second.get(), depth + 1);
            }
        }
    }
};