#pragma once

#include "LogSink.hpp"

class ConsoleSink : public LogSink {
public:
    void write_colored(const ColoredMessage& message) override;
    void write(const std::string& message) override;
};