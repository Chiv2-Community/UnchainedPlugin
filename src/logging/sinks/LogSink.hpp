#pragma once
#include <string>
#include <vector>

struct ColoredText {
    std::string text;
    int foregroundColor = -1;
    int backgroundColor = -1;
};

using ColoredMessage = std::vector<ColoredText>;

class LogSink {
public:
    virtual ~LogSink() = default;
    virtual void write(const std::string& message) {}
    virtual void write_colored(const ColoredMessage& message) { 
        // Default implementation just concatenates all text and calls the plain write
        std::string plainText;
        for (const auto& segment : message) {
            plainText += segment.text;
        }
        write(plainText);
    }
};