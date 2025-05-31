//
// Created by Fam on 5/27/2025.
//

#pragma once

#include <format>
#include <filesystem>

// This should replace the current formatter specialization (lines 17-25)
namespace std {
    template <typename CharT>
    struct formatter<std::filesystem::path, CharT> {
        constexpr auto parse(std::format_parse_context& ctx) {
            return ctx.begin();
        }

        auto format(const std::filesystem::path& p, std::format_context& ctx) const {
            return std::format_to(ctx.out(), "{}", p.string());
        }
    };
}