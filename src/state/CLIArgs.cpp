#include <string>
#include <sstream>
#include <vector>
#include <algorithm>
#include "CLIArgs.hpp"

#include "../logging/global_logger.hpp"

CLIArgs::CLIArgs(bool enable_rcon, bool apply_desync_patch, bool use_backend_banlist,
                 bool is_headless, bool is_server, bool playable_listen,
                 std::wstring server_browser_backend, std::optional<std::wstring> server_password,
                 Platform platform)
    : enable_rcon(enable_rcon)
    , apply_desync_patch(apply_desync_patch)
    , use_backend_banlist(use_backend_banlist)
    , is_headless(is_headless)
    , is_server(is_server)
    , playable_listen(playable_listen)
    , server_browser_backend(server_browser_backend)
    , server_password(server_password)
    , platform(platform)
{
}

CLIArgs CLIArgs::Parse(std::wstring cli_param_string)
{
    bool enable_rcon = false;
    bool apply_desync_patch = false;
    bool use_backend_banlist = false;
    bool is_headless = false;
    bool is_server = false;
    bool playable_listen = false;
    std::wstring server_browser_backend = L"";
    std::optional<std::wstring> server_password = std::nullopt;
    Platform platform = Platform::STEAM;

    std::wistringstream iss(cli_param_string);
    std::vector<std::wstring> tokens;
    std::wstring token;
    while (iss >> token) {
        tokens.push_back(token);
    }

    // Process each token
    for (size_t i = 0; i < tokens.size(); ++i) {
        const auto& arg = tokens[i];

        if (arg == L"-rcon") {
            enable_rcon = true;
        } 
        else if (arg == L"--desync-patch") {
            apply_desync_patch = true;
        } 
        else if (arg == L"--use-backend-banlist") {
            use_backend_banlist = true;
        } 
        else if (arg == L"-nullrhi") {
            is_headless = true;
            is_server = true;
        } 
        else if (arg == L"--next-map-name") {
            is_server = true;
        } 
        else if (arg == L"--playable-listen") {
            playable_listen = true;
        } 
        else if (arg == L"--server-browser-backend" && i + 1 < tokens.size()) {
            server_browser_backend = tokens[++i];
        } 
        else if (arg == L"--server-password" && i + 1 < tokens.size()) {
            server_password = tokens[++i];
        } 
        else if (arg == L"--platform" && i + 1 < tokens.size()) {
            const auto& platform_str = tokens[++i];
            if (string_to_platform.contains(platform_str))
                platform = string_to_platform.at(std::wstring(platform_str));
            else {
                GLOG_ERROR("Invalid platform '{}'.  Expected 'STEAM', 'EGS', or 'XBOX'", platform_str);
                GLOG_WARNING("Defaulting to STEAM");
            }
        }
        else if (arg == L"-epicapp=Peppermint") {
            platform = Platform::EGS;
        }
    }

    return CLIArgs(enable_rcon, apply_desync_patch, use_backend_banlist,
                  is_headless, is_server, playable_listen, 
                  server_browser_backend, server_password, platform);
}