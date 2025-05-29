#pragma once
#include <string>

class LogSink {
public:
    virtual ~LogSink() = default;
    virtual void write(const std::string& message) {};
};
