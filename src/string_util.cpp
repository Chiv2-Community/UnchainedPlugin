#include <optional>
#include <vector>
#include <string>
#include <Windows.h>

std::vector<std::string> split(std::string_view str, std::string_view delimiter) {
    std::vector<std::string> result;
    
    if (str.empty()) {
        return result;
    }
    
    size_t start = 0;
    size_t end = str.find(delimiter);
    
    while (end != std::string::npos) {
        result.emplace_back(str.substr(start, end - start));
        start = end + delimiter.length();
        end = str.find(delimiter, start);
    }
    
    // Add the last part
    result.emplace_back(str.substr(start));
    
    return result;
}

std::string ws(const int indent) {
    return "\n" + std::string(indent * 2, ' ');
}

std::optional<std::string> get_last_windows_error_message_string() {
    DWORD error = GetLastError();
    LPSTR error_message = nullptr;
    const DWORD flags = FORMAT_MESSAGE_ALLOCATE_BUFFER
                        | FORMAT_MESSAGE_FROM_SYSTEM
                        | FORMAT_MESSAGE_IGNORE_INSERTS;
    FormatMessageA(
        flags,
        nullptr,
        error,
        0,
        reinterpret_cast<LPSTR>(&error_message),
        0,
        nullptr
    );
    if (error_message[strlen(error_message) - 2] == '\r') {
        error_message[strlen(error_message) - 2] = '\0';
    }

    if (error_message == nullptr) {
        return std::nullopt;
    }

    return std::string(error_message);
}