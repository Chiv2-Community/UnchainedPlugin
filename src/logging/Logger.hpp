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
#include <map>

#include "sinks/LogSink.hpp"

enum class LogLevel {
    TRACE,
    DEBUG,
    INFO,
    WARNING,
    ERR
};

static std::map<std::string, LogLevel> log_level_to_string = {
    {"TRACE", LogLevel::TRACE},
    {"DEBUG", LogLevel::DEBUG},
    {"INFO", LogLevel::INFO},
    {"WARNING", LogLevel::WARNING},
    {"ERROR", LogLevel::ERR}
};

class Logger {
private:
    LogLevel level_;
    std::vector<std::shared_ptr<LogSink>> sinks_;
    std::mutex mutex_;

    static std::string level_to_string(LogLevel level);

    // Windows console color codes
    static constexpr int DARKGRAY = 241;
    static constexpr int GRAY = 245;
    static constexpr int WHITE = 15;
    static constexpr int YELLOW = 226;
    static constexpr int RED = 196;
    
    // Get color for log level
    static int get_level_color(LogLevel level);
    
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
            formatted_message = std::vformat(fmt, std::make_format_args(args...));
        } catch (const std::format_error& e) {
            formatted_message = fmt + " [Formatting error: " + e.what() + "]";
        }

        std::string timestamp = std::format("{:%Y-%m-%d %H:%M:%S}", std::chrono::time_point_cast<std::chrono::seconds>(std::chrono::system_clock::now()));
        std::string level_str = level_to_string(level);
        
        ColoredMessage colored_message;
        colored_message.push_back({std::format("[{}] ", timestamp), DARKGRAY});
        colored_message.push_back({std::format("[{:<5}] ", level_str), get_level_color(level)});
        
#ifdef _SHOW_SOURCE_LOCATION_IN_LOG
        std::string file = location.file_name();
        size_t pos = file.find_last_of("/\\");
        if (pos != std::string::npos) {
            file = file.substr(pos + 1);
        }

        std::string location_str = std::format("{}:{}", file, location.line());
        colored_message.push_back({std::format("[{:<27.27}] ", location_str), GRAY});
#endif
        colored_message.push_back({formatted_message, WHITE});
        
        std::string full_message;
        for (const auto& segment : colored_message) {
            full_message += segment.text;
        }
        
        std::lock_guard<std::mutex> lock(mutex_);
        for (const auto& sink : sinks_) {
            sink->write_colored(colored_message);
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