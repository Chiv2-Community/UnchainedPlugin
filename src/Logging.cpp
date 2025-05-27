
#include "Logging.hpp"
#include "quill/Backend.h"
#include "quill/Frontend.h"
#include "quill/sinks/ConsoleSink.h"
#include "quill/sinks/RotatingFileSink.h"


bool initialized = false;

std::vector<std::shared_ptr<quill::Sink>> sinks;
std::map<std::string, std::shared_ptr<quill::Logger>> loggers;

auto default_log_format = quill::PatternFormatterOptions(
    "%(time) [%(thread_id)] %(short_source_location:<28) %(log_level:<9): %(message)"
);

auto default_clock_source = quill::ClockSourceType::Tsc;
quill::UserClockSource* default_user_clock = nullptr;

inline quill::PatternFormatterOptions get_default_log_format() { return default_log_format; }
inline quill::ClockSourceType get_default_clock_source() { return default_clock_source; }
inline quill::UserClockSource* get_default_user_clock() { return default_user_clock; }


void initialize_quill() {
    if (initialized) return;

    sinks = std::vector<std::shared_ptr<quill::Sink>>();
    loggers = std::map<std::string, std::shared_ptr<quill::Logger>>();

    quill::Backend::start();

    const auto console_sink = quill::Frontend::create_or_get_sink<quill::ConsoleSink>("default_console_logger");
    const auto file_sink = quill::Frontend::create_or_get_sink<quill::RotatingFileSink>(
        "UnchainedPlugin.log",
        []()
        {
          // See RotatingFileSinkConfig for more options
          quill::RotatingFileSinkConfig cfg;
          cfg.set_open_mode('w');
          cfg.set_filename_append_option(quill::FilenameAppendOption::StartDateTime);
          cfg.set_rotation_time_daily("00:00");
          cfg.set_rotation_max_file_size(1024 * 1024 * 32);
          return cfg;
        }());

    register_sink(console_sink);
    register_sink(file_sink);

    auto signal_handler = +[](int signal) {
        for (auto& logger_pair : loggers) {
            logger_pair.second->flush_log();
        }
    };

    signal(SIGINT,  signal_handler);
    signal(SIGABRT, signal_handler);
    signal(SIGTERM, signal_handler);
}

std::shared_ptr<quill::Logger> initialize_global_logger() {
    initialize_quill();
    return create_or_get_logger("root");
}

std::shared_ptr<quill::Logger> create_or_get_logger(
    const std::string& name,
    const quill::PatternFormatterOptions& pattern_formatter_options,
    quill::ClockSourceType const clock_source,
    quill::UserClockSource* user_clock) {
    initialize_quill();

    if (loggers.find(name) != loggers.end()) {
        return loggers[name];
    }

    auto logger = std::shared_ptr<quill::Logger>(quill::Frontend::create_or_get_logger(name, sinks, pattern_formatter_options, clock_source, user_clock));
    loggers[name] = logger;

    return logger;
}

void register_sink(const std::shared_ptr<quill::Sink>& sink) {
    sinks.push_back(sink);
}
