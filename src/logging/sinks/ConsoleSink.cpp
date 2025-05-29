//
// Created by Fam on 5/27/2025.
//

#include <iostream>
#include "ConsoleSink.hpp"
#include <Windows.h>

void ConsoleSink::write(const std::string& message) {
    OutputDebugStringA(message.c_str());
    std::cout << message << std::endl;
}
