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
extern std::map<LogLevel, std::string> log_level_to_string;
extern std::map<std::string, LogLevel> string_to_log_level;

// Check if log level is enabled
bool isLogLevelEnabled(LogLevel level);

// Type alias for string variants
using StringVariant = std::variant<std::string, std::wstring>;

// Utility class for string conversion
class StringConverter {
public:
    static std::wstring toWideString(const StringVariant& str);
    static std::wstring toWideString(const std::string& str);
    static std::wstring toWideString(const std::wstring& str);
    static std::string toNarrowString(const std::wstring& str);
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
    
    // Helper functions for argument conversion
    std::wstring convertArg(const StringVariant& arg);
    std::wstring convertArg(const std::string& arg);
    const std::wstring& convertArg(const std::wstring& arg);
    
    template<typename T>
    T&& convertArg(T&& arg) {
        return std::forward<T>(arg);
    }
    
    // Implementation of formatting using swprintf
    template<typename... Args>
    std::wstring formatMessageImpl(const std::wstring& format, Args&&... args) {
        // Calculate required buffer size
        int size = std::swprintf(nullptr, 0, format.c_str(), args...);
        if (size <= 0) {
            return format; // Return original format if formatting fails
        }
        
        // Create buffer and format the string
        std::vector<wchar_t> buffer(size + 1);
        std::swprintf(buffer.data(), buffer.size(), format.c_str(), args...);
        
        return std::wstring(buffer.data());
    }
    
    // Helper function to format messages using swprintf
    template<typename... Args>
    std::wstring formatMessage(const std::wstring& format, Args&&... args) {
        // Convert string variants to wide strings for formatting
        return formatMessageImpl(format, convertArg(args)...);
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