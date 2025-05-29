#include "FileSink.hpp"

#include <mutex>
#include <ostream>
#include <stdexcept>
#include <string>

FileSink::FileSink(const std::string& filename) {
    file_.open(filename, std::ios::out | std::ios::app);
    if (!file_.is_open()) {
        throw std::runtime_error("Failed to open log file: " + filename);
    }
}

FileSink::~FileSink() {
    if (file_.is_open()) {
        close();
    }
}

void FileSink::write(const std::string& message) {
    std::lock_guard<std::mutex> lock(mutex_);
    file_ << message << std::endl;
}

void FileSink::close() {
    file_.close();
}