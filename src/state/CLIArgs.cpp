#include <string>
#include <sstream>
#include <vector>
#include <algorithm>
#include "CLIArgs.hpp"

#include "../logging/global_logger.hpp"

CLIArgs::CLIArgs(std::optional<uint16_t> rcon_port, bool apply_desync_patch, bool use_backend_banlist,
                 bool is_headless, bool is_server, bool playable_listen, std::optional<std::wstring> next_map,
                 std::wstring server_browser_backend, std::optional<std::wstring> server_password,
                 Platform platform)
    : rcon_port(rcon_port)
    , apply_desync_patch(apply_desync_patch)
    , use_backend_banlist(use_backend_banlist)
    , is_headless(is_headless)
    , is_server(is_server)
    , playable_listen(playable_listen)
    , server_browser_backend(server_browser_backend)
    , server_password(server_password)
    , platform(platform)
    , next_map(next_map)
{
}

CLIArgs CLIArgs::Parse(std::wstring cli_param_string)
{
    std::optional<uint16_t> rcon_port = std::nullopt;
    bool apply_desync_patch = false;
    bool use_backend_banlist = false;
    bool is_headless = false;
    bool is_server = false;
    bool playable_listen = false;
    std::wstring server_browser_backend = L"https://servers.polehammer.net";
    std::optional<std::wstring> server_password = std::nullopt;
    std::optional<std::wstring> next_map = std::nullopt;
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
            try {
                auto port_str = tokens.at(++i);
                rcon_port = std::stoi(port_str);
            } catch (const std::exception&) {
                GLOG_ERROR("Invalid port.  Expected an integer. RCON will not be enabled.");
            }
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
            try {
                next_map = tokens.at(++i);
            } catch (const std::exception&) {
                GLOG_ERROR("Expected a map name following arg '--next-map-name'. Got nothing.");
            }
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
            const auto& platform_wstr = tokens[++i];
            const auto platform_str = std::format("{}", platform_wstr);
            if (string_to_platform.contains(platform_str))
                platform = string_to_platform.at(platform_str);
            else {
                GLOG_ERROR("Invalid platform '{}'.  Expected 'STEAM', 'EGS', or 'XBOX'", platform_str);
                GLOG_WARNING("Defaulting to STEAM");
            }
        }
        else if (arg == L"-epicapp=Peppermint") {
            platform = Platform::EGS;
        }
    }

    return CLIArgs(rcon_port, apply_desync_patch, use_backend_banlist,
                  is_headless, is_server, playable_listen, next_map,
                  server_browser_backend, server_password, platform);
}