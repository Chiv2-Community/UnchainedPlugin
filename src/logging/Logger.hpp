#pragma once

#include <string>
#include <fstream>
#include <iostream>
#include <format>
#include <vector>
#include <memory>
#include <mutex>
#include <source_location>
#include <filesystem>
#include "sinks/LogSink.hpp"

enum class LogLevel {
    TRACE,
    DEBUG,
    INFO,
    WARNING,
    ERR
};

class Logger {
private:
    LogLevel level_;
    std::vector<std::shared_ptr<LogSink>> sinks_;
    std::mutex mutex_;

    static std::string level_to_string(LogLevel level);
    static std::string wstring_to_string(const std::wstring& wstr);
public:
    explicit Logger(LogLevel level = LogLevel::INFO);
    void add_sink(std::shared_ptr<LogSink> sink);
    void set_level(LogLevel level);
    LogLevel get_level() const;

    template<typename... Args>
    void log(LogLevel level, std::string fmt, const std::source_location& location, Args&&... args) {
        if (level < level_) return;

        std::string formatted_message;
        try {
            formatted_message = std::vformat(fmt, std::make_format_args(std::forward<Args>(args)...));
        } catch (const std::format_error& e) {
            formatted_message = fmt + " [FORMAT ERROR: " + e.what() + "]";
        }

        std::string timestamp = std::format("{:%Y-%m-%d %H:%M:%S}", std::chrono::time_point_cast<std::chrono::seconds>(std::chrono::system_clock::now()));
        std::string file = location.file_name();
        size_t pos = file.find_last_of("/\\");
        if (pos != std::string::npos) {
            file = file.substr(pos + 1);
        }

        std::string location_str = std::format("{}:{}", file, location.line());

        std::string full_message = std::format("[{}] [{:<7}] [{:<27.27}] {}",
            timestamp, level_to_string(level), location_str, formatted_message);

        std::lock_guard<std::mutex> lock(mutex_);
        for (const auto& sink : sinks_) {
            sink->write(full_message);
        }
    }

    // Helper methods for each log level
    template<typename... Args>
    void trace(const std::string fmt, const std::source_location& loc, Args&&... args) {
        log(LogLevel::TRACE, fmt, loc, std::forward<Args>(args)...);
    }

    template<typename... Args>
    void debug(const std::string fmt, const std::source_location& loc, Args&&... args) {
        log(LogLevel::DEBUG, fmt, loc, std::forward<Args>(args)...);
    }

    template<typename... Args>
    void info(const std::string fmt, const std::source_location& loc, Args&&... args) {
        log(LogLevel::INFO, fmt, loc, std::forward<Args>(args)...);
    }

    template<typename... Args>
    void warning(const std::string fmt, const std::source_location& loc, Args&&... args) {
        log(LogLevel::WARNING, fmt, loc, std::forward<Args>(args)...);
    }

    template<typename... Args>
    void error(const std::string fmt, const std::source_location& loc, Args&&... args) {
        log(LogLevel::ERR, fmt, loc, std::forward<Args>(args)...);
    }
};

// Convenience macros
#define LOG_TRACE(logger, fmt, ...) \
    if ((logger) && (logger)->get_level() <= LogLevel::TRACE) (logger)->trace(fmt, (std::source_location::current)(), ##__VA_ARGS__)

#define LOG_DEBUG(logger, fmt, ...) \
    if ((logger) && (logger)->get_level() <= LogLevel::DEBUG) (logger)->debug(fmt, (std::source_location::current)(), ##__VA_ARGS__)

#define LOG_INFO(logger, fmt, ...) \
    if ((logger) && (logger)->get_level() <= LogLevel::INFO) (logger)->info(fmt, (std::source_location::current)(), ##__VA_ARGS__)

#define LOG_WARNING(logger, fmt, ...) \
    if ((logger) && (logger)->get_level() <= LogLevel::WARNING) (logger)->warning(fmt, (std::source_location::current)(), ##__VA_ARGS__)

#define LOG_ERROR(logger, fmt, ...) \
    if ((logger) && (logger)->get_level() <= LogLevel::ERR) (logger)->error(fmt, (std::source_location::current)(), ##__VA_ARGS__)