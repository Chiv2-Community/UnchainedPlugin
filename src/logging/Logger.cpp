#include <format>
#include <chrono>

#include "Logger.hpp"

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
        case LogLevel::WARNING: return "WARN";
        case LogLevel::ERR:     return "ERROR";
        default: return "UNKNOWN";
    }
}

int Logger::get_level_color(LogLevel level) {
    switch (level) {
        case LogLevel::TRACE:   return DARKGRAY;
        case LogLevel::DEBUG:   return GRAY;
        case LogLevel::INFO:    return WHITE;
        case LogLevel::WARNING: return YELLOW;
        case LogLevel::ERR:     return RED;
        default: return WHITE;
    }
}
