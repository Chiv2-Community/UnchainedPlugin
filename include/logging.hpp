#pragma once

#include <iostream>
#include <fstream>
#include <string>
#include <map>
#include <variant>
#include <vector>
#include <cstdio>
#include <memory>
#include <cwchar>
#include <locale>
#include <codecvt>
#include <functional>

enum LogLevel {
    DEBUG,
    INFO,
    WARNING,
    ERR
};

// Global log level configuration
extern LogLevel LOG_LEVEL;

// Log level mappings
extern std::map<LogLevel, std::wstring> log_level_to_string;
extern std::map<std::wstring, LogLevel> string_to_log_level;

// Check if log level is enabled
bool isLogLevelEnabled(LogLevel level);

// Type alias for string variants
using StringVariant = std::variant<std::string, std::wstring>;

// Utility class for string conversion
class StringConverter {
public:
    static std::wstring toWideString(const StringVariant& str);
};

// Abstract base class for log outputs
class LogOutput {
public:
    virtual ~LogOutput() = default;
    virtual void write(LogLevel level, const std::wstring& timestamp, const std::wstring& message) = 0;
    virtual bool isEnabled() const { return true; }
};

// Console output implementation
class ConsoleOutput : public LogOutput {
public:
    void write(LogLevel level, const std::wstring& timestamp, const std::wstring& message) override;
};

// File output implementation
class FileOutput : public LogOutput {
private:
    std::wofstream file;
    std::wstring filename;
    bool isOpen;
    
    bool open();

public:
    explicit FileOutput(const std::wstring& filename);
    explicit FileOutput(const std::string& filename);
    ~FileOutput();
    
    bool isEnabled() const override;
    void write(LogLevel level, const std::wstring& timestamp, const std::wstring& message) override;
    bool reopen();
};

// Custom output using function callback
class CallbackOutput : public LogOutput {
private:
    std::function<void(LogLevel, const std::wstring&, const std::wstring&)> callback;

public:
    explicit CallbackOutput(std::function<void(LogLevel, const std::wstring&, const std::wstring&)> cb);
    void write(LogLevel level, const std::wstring& timestamp, const std::wstring& message) override;
};

// Main logger class with output registration
class Logger {
private:
    std::vector<std::unique_ptr<LogOutput>> outputs;
    
    std::wstring getCurrentTimestamp();
    void logMessage(LogLevel level, const std::wstring& message);
    
    template<typename... Args>
    std::wstring formatMessageImpl(const std::wstring& format, Args&&... args) {
        // Start with a reasonable buffer size
        std::vector<wchar_t> buffer(1024);

        int size = std::swprintf(buffer.data(), buffer.size(), format.c_str(), args...);

        if (size < 0) {
            return format; // Return original format if formatting fails
        }

        if (size >= static_cast<int>(buffer.size())) {
            // Buffer was too small, resize and try again
            buffer.resize(size + 1);
            std::swprintf(buffer.data(), buffer.size(), format.c_str(), args...);
        }

        return std::wstring(buffer.data());
    }

    // Helper function to convert arguments to wide strings
    template<typename T>
    std::wstring convertArg(T&& arg) {
        if constexpr (std::is_convertible_v<T, StringVariant>) {
            // If it can be converted to StringVariant, use toWideString
            return StringConverter::toWideString(static_cast<StringVariant>(std::forward<T>(arg)));
        } else if constexpr (std::is_same_v<std::decay_t<T>, std::wstring>) {
            // Already a wide string, return as is
            return std::forward<T>(arg);
        }

        return L"";
    }
    
    template<typename... Args>
    std::wstring formatMessage(const std::wstring& format, Args&&... args) {
        return formatMessageImpl(format, convertArg(std::forward<Args>(args))...);
    }

public:
    // Output registration methods
    void addOutput(std::unique_ptr<LogOutput> output);
    void addConsoleOutput();
    void addFileOutput(const std::wstring& filename);
    void addFileOutput(const std::string& filename);
    void addCallbackOutput(std::function<void(LogLevel, const std::wstring&, const std::wstring&)> callback);
    void clearOutputs();
    size_t getOutputCount() const;

    // Template function for printf-style logging with variadic arguments
    template<typename... Args>
    void log(LogLevel level, const StringVariant& format, Args&&... args) {
        if (!isLogLevelEnabled(level)) {
            return;
        }
        
        std::wstring wideFormat = StringConverter::toWideString(format);
        std::wstring message = formatMessage(wideFormat, std::forward<Args>(args)...);
        logMessage(level, message);
    }
    
    // Debug logging functions
    template<typename... Args>
    void debug(const StringVariant& format, Args&&... args) {
        log(DEBUG, format, std::forward<Args>(args)...);
    }
    
    // Info logging functions
    template<typename... Args>
    void info(const StringVariant& format, Args&&... args) {
        log(INFO, format, std::forward<Args>(args)...);
    }
    
    // Warning logging functions
    template<typename... Args>
    void warning(const StringVariant& format, Args&&... args) {
        log(WARNING, format, std::forward<Args>(args)...);
    }
    
    // Error logging functions
    template<typename... Args>
    void error(const StringVariant& format, Args&&... args) {
        log(ERR, format, std::forward<Args>(args)...);
    }
};

// Global logger instance
extern Logger g_logger;

// Convenience functions that use the global logger
template<typename... Args>
void log_debug(const StringVariant& format, Args&&... args) {
    g_logger.debug(format, std::forward<Args>(args)...);
}

template<typename... Args>
void log_info(const StringVariant& format, Args&&... args) {
    g_logger.info(format, std::forward<Args>(args)...);
}

template<typename... Args>
void log_warning(const StringVariant& format, Args&&... args) {
    g_logger.warning(format, std::forward<Args>(args)...);
}

template<typename... Args>
void log_error(const StringVariant& format, Args&&... args) {
    g_logger.error(format, std::forward<Args>(args)...);
}

// Convenience macros for easier usage (using global logger)
#define LOG_DEBUG(format, ...) g_logger.debug(format, ##__VA_ARGS__)
#define LOG_INFO(format, ...) g_logger.info(format, ##__VA_ARGS__)
#define LOG_WARNING(format, ...) g_logger.warning(format, ##__VA_ARGS__)
#define LOG_ERROR(format, ...) g_logger.error(format, ##__VA_ARGS__)

Logger& getLogger();