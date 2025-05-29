#pragma once

#include "LogSink.hpp"

class ConsoleSink : public LogSink {
public:
    void write(const std::string& message) override;
};