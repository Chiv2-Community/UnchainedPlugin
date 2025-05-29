#include "global_logger.hpp"

#include <csignal>

#include "../stubs/Chivalry2.h"
#include "sinks/ConsoleSink.hpp"
#include "sinks/FileSink.hpp"

void initialize_global_logger(enum LogLevel level) {
    OutputDebugStringA("Initalizing logger");
    static Logger logger(level);
    static auto console_sink = std::make_shared<ConsoleSink>();

    logger.add_sink(console_sink);
    g_logger = &logger;
    OutputDebugStringA("Logger initialized");
}

void add_file_sink_to_global_logger(const std::string& filename) {
    if (g_logger) {
        g_logger->add_sink(std::make_shared<FileSink>(filename));
    }
}

void finalize_global_logger() {
    delete g_logger;
}
