#include <iostream>
#include "ConsoleSink.hpp"
#include <Windows.h>

void ConsoleSink::write(const std::string& message) {
    OutputDebugStringA(message.c_str());
    std::cout << message << std::endl;
}

void ConsoleSink::write_colored(const ColoredMessage& message) {
    std::string fullMessage;
    for (const auto& segment : message) {
        fullMessage += segment.text;
    }
    OutputDebugStringA(fullMessage.c_str());

    for (const auto& segment : message) {
        if (segment.foregroundColor >= 0) {
            std::cout << "\033[38;5;" << segment.foregroundColor << "m";
        }

        std::cout << segment.text;

        if (segment.foregroundColor >= 0) {
            std::cout << "\033[0m";
        }
    }

    std::cout << std::endl;
}
