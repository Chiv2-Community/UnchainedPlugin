#include <iostream>
#include <format>
#include <chrono>

#include "Logger.hpp"

// Logger implementation
Logger::Logger(LogLevel level) : level_(level) {}

void Logger::add_sink(std::shared_ptr<LogSink> sink) {
    std::lock_guard<std::mutex> lock(mutex_);
    sinks_.push_back(std::move(sink));
}

void Logger::set_level(LogLevel level) {
    std::lock_guard<std::mutex> lock(mutex_);
    level_ = level;
}

LogLevel Logger::get_level() const {
    return level_;
}

std::string Logger::level_to_string(LogLevel level) {
    switch (level) {
        case LogLevel::TRACE:   return "TRACE";
        case LogLevel::DEBUG:   return "DEBUG";
        case LogLevel::INFO:    return "INFO";
        case LogLevel::WARNING: return "WARNING";
        case LogLevel::ERR:     return "ERROR";
        default: return "UNKNOWN";
    }
}

std::string Logger::wstring_to_string(const std::wstring& wstr) {
    std::string result;
    result.reserve(wstr.size());
    for (const auto& wc : wstr) {
        // Simple conversion - note this doesn't handle all Unicode characters properly
        // For production, you might want to use a proper conversion function
        if (wc <= 127) {
            result.push_back(static_cast<char>(wc));
        } else {
            result.push_back('?');
        }
    }
    return result;
}
