#include <vector>
#include <string>

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

std::string ws(int indent) {
    return "\n" + std::string(indent * 2, ' ');
}