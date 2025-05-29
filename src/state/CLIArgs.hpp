#pragma once
#include <optional>

#include "../Platform.hpp"

class CLIArgs {
public:
    bool enable_rcon;
    bool apply_desync_patch;
    bool use_backend_banlist;
    bool is_headless;
    bool is_server;
    bool playable_listen;
    std::wstring server_browser_backend;
    std::optional<std::wstring> server_password;
    Platform platform;

    CLIArgs(bool enable_rcon, bool apply_desync_patch, bool use_backend_banlist, bool is_headless, bool is_server, bool playable_listen, std::wstring server_browser_backend, std::optional<std::wstring> server_password, Platform platform);
    static CLIArgs Parse(std::wstring cli_param_string);
};
