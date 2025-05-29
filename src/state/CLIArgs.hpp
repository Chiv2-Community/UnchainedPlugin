#pragma once
#include <optional>

#include "../Platform.hpp"

class CLIArgs {
public:
    std::optional<uint8_t> rcon_port;
    bool apply_desync_patch;
    bool use_backend_banlist;
    bool is_headless;
    bool is_server;
    bool playable_listen;
    std::wstring server_browser_backend;
    std::optional<std::wstring> server_password;
    std::optional<std::wstring> next_map;
    Platform platform;

    CLIArgs(std::optional<uint8_t> rcon_port, bool apply_desync_patch, bool use_backend_banlist, bool is_headless, bool is_server, bool playable_listen, std::optional<std::wstring> next_map, std::wstring server_browser_backend, std::optional<std::wstring> server_password, Platform platform);
    static CLIArgs Parse(std::wstring cli_param_string);
};
