#pragma once

#include <mutex>
#include <fstream>

#include "LogSink.hpp"


class FileSink : public LogSink {
private:
    std::ofstream file_;
    std::mutex mutex_;

public:
    explicit FileSink(const std::string& filename);
    ~FileSink() override;
    void write(const std::string& message);
    void close();
};
