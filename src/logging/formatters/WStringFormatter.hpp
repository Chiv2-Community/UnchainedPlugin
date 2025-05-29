#pragma once
#include <string>
#include <format>

namespace std {
    namespace detail {
        inline std::string convert_wstring_to_string(const wchar_t* wstr, size_t len = std::wstring::npos) {
            std::string narrowStr;

            if (wstr == nullptr) {
                return "(null)";
            }

            if (len == std::wstring::npos) {
                while (*wstr) {
                    wchar_t wc = *wstr++;
                    if (wc >= 32 && wc < 127) {
                        narrowStr.push_back(static_cast<char>(wc));
                    }
                }
            } else {
                for (size_t i = 0; i < len && wstr[i]; ++i) {
                    wchar_t wc = wstr[i];
                    if (wc >= 32 && wc < 127) {
                        narrowStr.push_back(static_cast<char>(wc));
                    }
                }
            }

            return narrowStr;
        }
    }

    template <typename CharT>
    struct formatter<wstring, CharT> {
        constexpr auto parse(format_parse_context& ctx) {
            return ctx.begin();
        }

        auto format(const wstring& p, std::format_context& ctx) const {
            std::string narrowStr = detail::convert_wstring_to_string(p.c_str(), p.size());
            return std::copy(narrowStr.begin(), narrowStr.end(), ctx.out());
        }
    };

    template <typename CharT>
    struct formatter<const wchar_t*, CharT> {
        constexpr auto parse(format_parse_context& ctx) {
            return ctx.begin();
        }

        auto format(const wchar_t* p, std::format_context& ctx) const {
            std::string narrowStr = detail::convert_wstring_to_string(p);
            return std::copy(narrowStr.begin(), narrowStr.end(), ctx.out());
        }
    };

    template <typename CharT, size_t N>
    struct formatter<wchar_t[N], CharT> {
        constexpr auto parse(format_parse_context& ctx) {
            return ctx.begin();
        }

        auto format(const wchar_t* p, std::format_context& ctx) const {
            std::string narrowStr = detail::convert_wstring_to_string(p, N);
            return std::copy(narrowStr.begin(), narrowStr.end(), ctx.out());
        }
    };
}