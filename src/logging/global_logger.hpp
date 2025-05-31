#pragma once

#include <string>
#include "Logger.hpp"
#include "formatters/all_formatters.h"

// Global logger instance
inline Logger* g_logger;

void initialize_global_logger(LogLevel level = LogLevel::INFO);
void add_file_sink_to_global_logger(const std::string& filename);
void finalize_global_logger();

#define GLOG_TRACE(fmt, ...) \
LOG_TRACE(g_logger, fmt, ##__VA_ARGS__)

#define GLOG_DEBUG(fmt, ...) \
LOG_DEBUG(g_logger, fmt, ##__VA_ARGS__)

#define GLOG_INFO(fmt, ...) \
LOG_INFO(g_logger, fmt, ##__VA_ARGS__)

#define GLOG_WARNING(fmt, ...) \
LOG_WARNING(g_logger, fmt, ##__VA_ARGS__)

#define GLOG_ERROR(fmt, ...) \
LOG_ERROR(g_logger, fmt, ##__VA_ARGS__)
