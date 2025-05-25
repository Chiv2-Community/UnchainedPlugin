#include "logging.hpp"
#include <windows.h>
#include <chrono>
#include <ctime>
#include <cstdarg>

// Global variable definitions
LogLevel LOG_LEVEL = LogLevel::ERR;

std::map<LogLevel, std::string> log_level_to_string = {
    {INFO, "INFO"},
    {WARNING, "WARNING"},
    {ERR, "ERROR"},
    {DEBUG, "DEBUG"}
};

std::map<std::string, LogLevel> string_to_log_level = {
    {"INFO", INFO},
    {"WARNING", WARNING},
    {"ERROR", ERR},
    {"DEBUG", DEBUG}
};

Logger g_logger;

// Global functions
bool isLogLevelEnabled(LogLevel level) {
    return level >= LOG_LEVEL;
}

// StringConverter implementations
std::wstring StringConverter::toWideString(const StringVariant& str) {
    if (std::holds_alternative<std::wstring>(str)) {
        return std::get<std::wstring>(str);
    } else {
        return toWideString(std::get<std::string>(str));
    }
}

std::wstring StringConverter::toWideString(const std::string& str) {
    if (str.empty()) return std::wstring();
    
    int size_needed = MultiByteToWideChar(CP_UTF8, 0, str.c_str(), static_cast<int>(str.size()), nullptr, 0);
    if (size_needed == 0) {
        // Handle error - could throw or return empty string
        return {};
    }
    
    std::wstring result(size_needed, 0);
    MultiByteToWideChar(CP_UTF8, 0, str.c_str(), (int)str.size(), &result[0], size_needed);
    return result;
}

std::wstring StringConverter::toWideString(const std::wstring& str) {
    return str;
}

std::string StringConverter::toNarrowString(const std::wstring& str) {
    if (str.empty()) return std::string();
    
    int size_needed = WideCharToMultiByte(CP_UTF8, 0, str.c_str(), (int)str.size(), NULL, 0, NULL, NULL);
    if (size_needed == 0) {
        // Handle error - could throw or return empty string
        return std::string();
    }
    
    std::string result(size_needed, 0);
    WideCharToMultiByte(CP_UTF8, 0, str.c_str(), (int)str.size(), &result[0], size_needed, NULL, NULL);
    return result;
}

// ConsoleOutput implementation
void ConsoleOutput::write(LogLevel level, const std::wstring& timestamp, const std::wstring& message) {
    std::wstring levelStr = StringConverter::toWideString(log_level_to_string[level]);
    std::wcout << L"[" << timestamp << L"] [" << levelStr << L"] " << message << std::endl;
}

bool FileOutput::open() {
    file.open(filename.c_str(), std::ios::app | std::ios::binary);
    if (file.is_open()) {
        // Check if file is empty and write BOM for UTF-16LE
        file.seekp(0, std::ios::end);
        if (file.tellp() == 0) {
            // Write UTF-16LE BOM
            file.put(0xFF);
            file.put(0xFE);
        }

        // Set UTF-16 locale
        file.imbue(std::locale(file.getloc(),
            new std::codecvt_utf16<wchar_t, 0x10ffff, std::little_endian>));

        isOpen = true;
        return true;
    }
    isOpen = false;
    return false;
}
CallbackOutput::CallbackOutput(std::function<void(LogLevel, const std::wstring&, const std::wstring&)> cb)
    : callback(std::move(cb)) {}

void CallbackOutput::write(LogLevel level, const std::wstring& timestamp, const std::wstring& message) {
    if (callback) {
        callback(level, timestamp, message);
    }
}

// Logger implementation
std::wstring Logger::getCurrentTimestamp() {
	auto now = std::chrono::system_clock::to_time_t(std::chrono::system_clock::now());
    std::tm tm{};
	localtime_s(&tm, &now);
    wchar_t buffer[100];
    std::wcsftime(buffer, sizeof(buffer) / sizeof(wchar_t), L"%Y-%m-%d %H:%M:%S", &tm);
    return std::wstring(buffer);
}

void Logger::logMessage(LogLevel level, const std::wstring& message) {
    if (!isLogLevelEnabled(level)) {
        return;
    }
    
    std::wstring timestamp = getCurrentTimestamp();
    
    // Write to all registered outputs
    for (auto& output : outputs) {
        if (output && output->isEnabled()) {
            output->write(level, timestamp, message);
        }
    }
}

std::wstring Logger::convertArg(const StringVariant& arg) {
    return StringConverter::toWideString(arg);
}

std::wstring Logger::convertArg(const std::string& arg) {
    return StringConverter::toWideString(arg);
}

const std::wstring& Logger::convertArg(const std::wstring& arg) {
    return arg;
}

void Logger::addOutput(std::unique_ptr<LogOutput> output) {
    outputs.push_back(std::move(output));
}

void Logger::addConsoleOutput() {
    addOutput(std::make_unique<ConsoleOutput>());
}

void Logger::addFileOutput(const std::wstring& filename) {
    addOutput(std::make_unique<FileOutput>(filename));
}

void Logger::addFileOutput(const std::string& filename) {
    addOutput(std::make_unique<FileOutput>(filename));
}

void Logger::addCallbackOutput(std::function<void(LogLevel, const std::wstring&, const std::wstring&)> callback) {
    addOutput(std::make_unique<CallbackOutput>(std::move(callback)));
}

void Logger::clearOutputs() {
    outputs.clear();
}

size_t Logger::getOutputCount() const {
    return outputs.size();
}

Logger& getLogger() {
    return g_logger;
}