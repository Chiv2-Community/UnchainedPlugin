#include "logging.hpp"
#include <windows.h>
#include <chrono>
#include <ctime>
#include <cstdarg>

// Global variable definitions
LogLevel LOG_LEVEL = LogLevel::ERR;

std::map<LogLevel, std::wstring> log_level_to_string = {
    {INFO, L"INFO"},
    {WARNING, L"WARNING"},
    {ERR, L"ERROR"},
    {DEBUG, L"DEBUG"}
};

std::map<std::wstring, LogLevel> string_to_log_level = {
    {L"INFO", INFO},
    {L"WARNING", WARNING},
    {L"ERROR", ERR},
    {L"DEBUG", DEBUG}
};

Logger g_logger;

// Global functions
bool isLogLevelEnabled(LogLevel level) {
    return level >= LOG_LEVEL;
}

// StringConverter implementations
std::wstring StringConverter::toWideString(const StringVariant& in_str) {
    if (std::holds_alternative<std::wstring>(in_str)) {
        return std::get<std::wstring>(in_str);
    } else {
        auto str = std::get<std::string>(in_str);
        int count = MultiByteToWideChar(CP_UTF8, 0, str.c_str(), str.length(), NULL, 0);
        std::wstring wstr(count, 0);
        MultiByteToWideChar(CP_UTF8, 0, str.c_str(), str.length(), &wstr[0], count);
        return wstr;
    }
}

// ConsoleOutput implementation
void ConsoleOutput::write(LogLevel level, const std::wstring& timestamp, const std::wstring& message) {
    wprintf(L"[%s] [%s] %s\n", timestamp.c_str(), log_level_to_string[level].c_str(), message.c_str());
}

FileOutput::FileOutput(const std::wstring& filename)
    : filename(filename), isOpen(false) {
    open();
}

FileOutput::FileOutput(const std::string& filename)
    : filename(StringConverter::toWideString(filename)), isOpen(false) {
    open();
}

FileOutput::~FileOutput() {
    if (file.is_open()) {
        file.close();
    }
}

bool FileOutput::reopen() {
    if (file.is_open()) {
        file.close();
    }
    return open();
}

bool FileOutput::isEnabled() const {
    return isOpen;
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

        isOpen = true;
        return true;
    }
    isOpen = false;
    return false;
}

void FileOutput::write(LogLevel level, const std::wstring& timestamp, const std::wstring& message) {
    if (!isOpen && !open()) {
        return;
    }

    std::wstring msg = L"[" + timestamp + L"] [" + log_level_to_string[level] + L"] " + message + L"\n";
    file.write(msg.c_str(), msg.length());
    file.flush();
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