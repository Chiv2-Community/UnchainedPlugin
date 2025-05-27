#pragma once

#include <quill/Frontend.h>
#include <quill/Logger.h>
#include <quill/LogMacros.h>
#include <quill/std/WideString.h>
#include <quill/std/FilesystemPath.h>
#include <quill/std/Vector.h>
#include <quill/std/Array.h>


inline quill::PatternFormatterOptions get_default_log_format();
inline quill::ClockSourceType get_default_clock_source();
inline quill::UserClockSource* get_default_user_clock();

std::shared_ptr<quill::Logger> initialize_global_logger();

std::shared_ptr<quill::Logger> create_or_get_logger(
    const std::string& name,
    const quill::PatternFormatterOptions& pattern_formatter_options = get_default_log_format(),
    quill::ClockSourceType const clock_source = get_default_clock_source(),
    quill::UserClockSource* user_clock = get_default_user_clock());

void register_sink(const std::shared_ptr<quill::Sink>& sink);

inline std::shared_ptr<quill::Logger> g_logger = initialize_global_logger();

